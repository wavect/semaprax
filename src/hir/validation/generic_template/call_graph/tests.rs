//! Full independently compiled original graph, exact ordering, and complete backing observations.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
const LIMIT: usize = 8 * 1024 * 1024;

fn checked() -> ResolvedProgram {
    // These exact source forms are the established generic collection corpus.
    let source = r#"module test.generic.graph;
@id("g.make") fn make<T>(value:T) -> Box<T> { box_new<T>(value) }
@id("g.relay") fn relay<T>(value:own Box<T>) -> Box<T> { value }
@id("g.take") fn take<T>(value:own Box<T>) -> T { box_into_inner<T>(value) }
@id("g.main") fn main()->i64 { take<i64>(relay<i64>(make<i64>(7))) }
"#;
    crate::hir::resolve(&crate::check(source, "generic-graph.spx").unwrap()).unwrap()
}

/// Graph-only hostile carriers retain the full typed expression payload from
/// a checked program; they do not claim ordinary generic-profile admission.
fn fixture(ids: &[&str]) -> ResolvedProgram {
    let mut program = checked();
    let template = program
        .function_templates
        .iter()
        .find(|t| t.id.as_str() == "g.make")
        .unwrap()
        .clone();
    program.function_templates = ids
        .iter()
        .map(|id| {
            let mut template = template.clone();
            template.id = DeclarationId::new(*id);
            template.requires.clear();
            template.ensures.clear();
            template.body.kind = ResolvedExprKind::Int(0);
            template
        })
        .collect();
    program
}
fn calls(like: &ResolvedExpr, targets: &[&str]) -> ResolvedExpr {
    let mut expression = like.clone();
    expression.kind = ResolvedExprKind::Call {
        callee: DeclarationId::new("unindexed.λ"),
        instance: None,
        type_arguments: vec![],
        args: targets
            .iter()
            .map(|id| {
                let mut child = like.clone();
                child.kind = ResolvedExprKind::FunctionReference {
                    target: DeclarationId::new(*id),
                };
                child
            })
            .collect(),
    };
    expression
}
fn connect(program: &mut ResolvedProgram, ordinal: usize, targets: &[&str]) {
    let template = &mut program.function_templates[ordinal];
    template.body = calls(&template.body, targets);
}
fn parity(program: &ResolvedProgram) {
    let old = original::validate_call_graph(program);
    let (((), physical), refused, paid, first) = with_limit_usage_refusal(LIMIT, || {
        measure(|| {
            let current = validate_call_graph_paid(program);
            assert_eq!(current, old);
            drop(current);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    assert!(physical.allocated <= paid && physical.highwater <= paid);
}

// ORIGINAL_TRACE_START
fn original_trace(program: &ResolvedProgram) -> Result<Vec<DeclarationId>, Diagnostic> {
    let ids = program
        .function_templates
        .iter()
        .map(|template| template.id.clone())
        .collect::<BTreeSet<_>>();
    let mut edges = BTreeMap::<DeclarationId, BTreeSet<DeclarationId>>::new();
    let mut incoming = ids
        .iter()
        .cloned()
        .map(|id| (id, 0usize))
        .collect::<BTreeMap<_, _>>();
    for template in &program.function_templates {
        for expression in template
            .requires
            .iter()
            .chain(std::iter::once(&template.body))
            .chain(&template.ensures)
        {
            visit_resolved_calls(expression, &mut |callee, _, _| {
                if ids.contains(callee)
                    && edges
                        .entry(template.id.clone())
                        .or_default()
                        .insert(callee.clone())
                {
                    *incoming.get_mut(callee).expect("template was indexed") += 1;
                }
            });
        }
    }
    let mut pending = incoming
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect::<std::collections::VecDeque<_>>();
    let mut visited = 0usize;
    let mut trace = Vec::new();
    while let Some(id) = pending.pop_front() {
        trace.push(id.clone());
        visited += 1;
        for callee in edges.get(&id).into_iter().flatten() {
            let count = incoming.get_mut(callee).expect("template was indexed");
            *count -= 1;
            if *count == 0 {
                pending.push_back(callee.clone());
            }
        }
    }
    (visited == ids.len())
        .then_some(trace)
        .ok_or_else(|| hir_error("generic function template call graph contains a direct cycle"))
}

// ORIGINAL_TRACE_END

#[test]
fn borrowed_graph_preserves_complete_original_outcomes_and_lexicographic_fifo_order() {
    let mut program = fixture(&["z", "a", "λ", "aa", "a"]);
    connect(&mut program, 0, &["λ", "λ", "foreign"]);
    connect(&mut program, 1, &["z", "aa"]);
    // Duplicate declaration IDs merge their edges but every authored body is visited.
    program.function_templates[4].requires =
        vec![calls(&program.function_templates[4].body, &["aa"])];
    program.function_templates[4].ensures =
        vec![calls(&program.function_templates[4].body, &["λ"])];
    let old = original_trace(&program).unwrap();
    assert_eq!(
        old.iter().map(DeclarationId::as_str).collect::<Vec<_>>(),
        ["a", "aa", "z", "λ"]
    );
    let (trace, refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let mut graph = Graph::new(&program).unwrap();
        graph.scan().unwrap();
        assert_eq!(graph.drain().unwrap(), graph.ids.len());
        graph
            .queue
            .iter()
            .map(|&ordinal| graph.ids[ordinal].as_str())
            .collect::<Vec<_>>()
    });
    assert!(!refused && first.is_none());
    assert_eq!(
        trace,
        old.iter().map(DeclarationId::as_str).collect::<Vec<_>>()
    );
    parity(&program);
    for (from, targets) in [(2, ["a"]), (3, ["aa"])] {
        let mut cyclic = program.clone();
        connect(&mut cyclic, from, &targets);
        parity(&cyclic);
        assert_eq!(
            original::validate_call_graph(&cyclic).unwrap_err().message,
            "generic function template call graph contains a direct cycle"
        );
    }
    // NUL/Unicode keys compare complete bytes in this graph-only receiver.
    let raw = fixture(&["prefix\0a", "prefix\0b", "préfix", "prefix"]);
    parity(&raw);
}

#[test]
fn every_contract_and_nested_call_is_visited_before_graph_verdict() {
    for boundary in 0..3 {
        let mut program = fixture(&["first", "late"]);
        connect(&mut program, 0, &["late"]);
        let mut nested = calls(&program.function_templates[1].body, &["first"]);
        for _ in 0..64 {
            let mut outer = program.function_templates[1].body.clone();
            outer.kind = ResolvedExprKind::Invoke {
                callable: Box::new(nested),
                args: vec![],
            };
            nested = outer;
        }
        match boundary {
            0 => program.function_templates[1].requires.push(nested),
            1 => program.function_templates[1].body = nested,
            2 => program.function_templates[1].ensures.push(nested),
            _ => unreachable!(),
        }
        parity(&program);
        assert!(original::validate_call_graph(&program).is_err());
    }
}

#[test]
fn complete_graph_backing_is_paid_through_drop_at_exact_short_zero_and_sticky_limits() {
    let ids: Vec<_> = (0..17).map(|i| format!("g.{i:02}")).collect();
    let borrowed: Vec<_> = ids.iter().map(String::as_str).collect();
    let mut program = fixture(&borrowed);
    for i in 0..ids.len() {
        connect(&mut program, i, &borrowed[i + 1..]);
    }
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = validate_call_graph_paid(&program);
                let ok = result.is_ok();
                drop(result);
                ok
            })
        })
    };
    let ((ok, physical), refused, paid, first) = observe(LIMIT);
    assert!(ok && !refused && first.is_none() && paid > 0);
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    assert!(physical.allocated <= paid && physical.highwater <= paid);
    let ((ok, physical), refused, used, first) = observe(paid);
    assert!(ok && !refused && used == paid && first.is_none());
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    for limit in [0, paid - 1] {
        let ((ok, physical), refused, used, first) = observe(limit);
        assert!(!ok && refused && used <= limit && first.is_some());
        assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    }
    let empty = fixture(&[]);
    let (((), physical), refused, used, first) = with_limit_usage_refusal(paid - 1, || {
        measure(|| {
            let first = validate_call_graph_paid(&program);
            assert!(first.is_err());
            drop(first);
            let retry = super::super::validate_call_graph(&empty);
            assert!(retry.is_err());
            drop(retry);
        })
    });
    assert!(refused && used <= paid - 1 && first.is_some());
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    let (old, old_physical) = measure(|| {
        let result = original::validate_call_graph(&program);
        let ok = result.is_ok();
        drop(result);
        ok
    });
    assert!(
        old && !old_physical.invalid
            && old_physical.live == 0
            && old_physical.allocated == old_physical.freed
    );
}

#[test]
fn ordered_ordinal_sets_keep_rotations_duplicates_and_all_actual_acquisitions() {
    for keys in [
        [1, 2, 3, 4, 5, 6, 7, 8],
        [8, 7, 6, 5, 4, 3, 2, 1],
        [5, 1, 8, 3, 2, 7, 4, 6],
    ] {
        let (((), physical), refused, paid, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                reserve(std::mem::size_of::<Outgoing>()).unwrap();
                let mut set = Outgoing::new();
                for &key in &keys {
                    assert!(set.insert(key).unwrap());
                }
                let capacity = set.capacity();
                for &key in &keys {
                    assert!(!set.insert(key).unwrap());
                }
                assert_eq!(set.capacity(), capacity);
                let mut output = [0usize; 8];
                let mut cursor = 0;
                set.visit(&mut |key| {
                    output[cursor] = key;
                    cursor += 1;
                    Ok(())
                })
                .unwrap();
                assert_eq!(output, [1, 2, 3, 4, 5, 6, 7, 8]);
                drop(set);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
        assert!(physical.allocated <= paid);
    }
}

#[test]
fn checked_generic_constructor_uses_independent_old_dispatch_and_complete_error_order() {
    let base = checked();
    for case in 0..4 {
        let mut program = base.clone();
        match case {
            1 => program.entrypoint = DeclarationId::new("first\0bad"),
            2 => {
                let ordinal = program
                    .function_templates
                    .iter()
                    .position(|t| t.id.as_str() == "g.make")
                    .unwrap();
                connect(&mut program, ordinal, &["g.make"]);
                program.functions[0].name.push_str(".later-forgery");
            }
            3 => {
                let duplicate = program.functions[0].clone();
                program.functions.push(duplicate);
            }
            _ => {}
        }
        let old =
            crate::hir::validation::constructor_seed::original::ConstructorSeed::authenticate(
                &program, None,
            )
            .err();
        let (((), physical), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let current =
                    crate::hir::validation::constructor_seed::ConstructorSeed::authenticate(
                        &program, None,
                    )
                    .err();
                assert_eq!(current, old);
                drop(current);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    }
}

#[test]
fn full_generic_graph_sources_and_independent_constructor_dispatch_preserve_original_bytes() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/paid-generic-constructor-graph-824/exact-source-inverses.json"
    )))
    .unwrap();
    let currents = [
        ci_str!("../../generic_template.rs"),
        ci_str!("../../constructor_seed/original.rs"),
        ci_str!("../../function_lookup/paid_custody.rs"),
    ];
    let originals = [
        ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-generic-constructor-graph-824/src__hir__validation__generic_template.rs.before.txt")),
        ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-generic-constructor-graph-824/src__hir__validation__constructor_seed__original.rs.before.txt")),
        ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-generic-constructor-graph-824/src__hir__validation__function_lookup__paid_custody.rs.before.txt")),
    ];
    let hashes = [
        "e10f391d931b3b9b9142618b54589eccb7317fabc1c08fda827f5564e9dffeeb",
        "b6ead6ed6c5bc33a27f3dafefe009e822f51e5d36934ceafd484c0441cbdb10e",
        "7ac2df081ab3f3eb32a30d92bf3e0e6c1e67f4da3d7a2b6b46b5f3afe9d481e6",
    ];
    assert_eq!(records.as_array().unwrap().len(), currents.len());
    for (((record, current), original), digest) in records
        .as_array()
        .unwrap()
        .iter()
        .zip(currents)
        .zip(originals)
        .zip(hashes)
    {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            digest
        );
        let mut current = current.to_owned();
        for edit in record["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(current.matches(after).count(), 1);
            current = current.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(current, original);
    }
    let old = ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/paid-generic-constructor-graph-824/complete-original-call-graph.before.txt"
    ));
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))
        ),
        "db3f404ea5c78a4d6135b080a01711e069e85ccaa99a7379f238e81ffc493594"
    );
    let complete_archive = originals[0];
    let start = complete_archive
        .find("pub(super) fn validate_call_graph(")
        .unwrap();
    let end = complete_archive[start..]
        .find("\npub(super) fn proof_return_type(")
        .unwrap()
        + start;
    assert_eq!(&complete_archive[start..end], old);
    let current = ci_str!("original.rs").strip_prefix("//! Complete original constructor graph receiver, independently compiled.\nuse super::super::*;\n\n").unwrap();
    let header = "pub(in crate::hir::validation) fn validate_call_graph(\n    program: &ResolvedProgram,\n) -> Result<(), Diagnostic> {";
    assert_eq!(current.matches(header).count(), 1);
    assert_eq!(current.replacen(header, "pub(super) fn validate_call_graph(program: &ResolvedProgram) -> Result<(), Diagnostic> {",1), old);
    let trace = ci_str!("tests.rs")
        .split_once("// ORIGINAL_TRACE_START\n")
        .unwrap()
        .1
        .split_once("\n// ORIGINAL_TRACE_END")
        .unwrap()
        .0;
    let trace = trace.replacen("fn original_trace(program: &ResolvedProgram) -> Result<Vec<DeclarationId>, Diagnostic> {", "pub(super) fn validate_call_graph(program: &ResolvedProgram) -> Result<(), Diagnostic> {", 1)
        .replacen("    let mut trace = Vec::new();\n", "", 1)
        .replacen("        trace.push(id.clone());\n", "", 1)
        .replacen(".then_some(trace)", ".then_some(())", 1);
    assert_eq!(trace, old);
}

#[test]
fn empty_compatibility_and_paid_routes_keep_zero_sticky_and_missing_owner_boundaries() {
    let empty = fixture(&[]);
    assert_eq!(
        super::super::validate_call_graph(&empty),
        original::validate_call_graph(&empty)
    );
    assert_eq!(
        validate_call_graph_paid(&empty).unwrap_err(),
        crate::hir::hir_error_static(NO_OWNER)
    );
    let ((ok, physical), refused, paid, first) =
        with_limit_usage_refusal(0, || measure(|| validate_call_graph_paid(&empty).is_ok()));
    assert!(ok && !refused && paid == 0 && first.is_none());
    assert!(!physical.invalid && physical.allocated == 0 && physical.live == 0);
    let (((), physical), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            assert!(!crate::bounded_output::reserve_active_required(1));
            let result = super::super::validate_call_graph(&empty);
            assert!(result.is_err());
            drop(result);
        })
    });
    assert!(refused && paid == 0 && first.is_some());
    assert!(!physical.invalid && physical.allocated == 0 && physical.live == 0);
}

#[test]
fn warmed_outgoing_hit_and_visit_authenticate_the_original_sticky_refusal_before_activity() {
    let (((), physical), was_refused, paid, first) = with_limit_usage_refusal(LIMIT, || {
        measure(|| {
            reserve(std::mem::size_of::<Outgoing>()).unwrap();
            let mut set = Outgoing::new();
            assert!(set.insert(1).unwrap());
            drop(set);
        })
    });
    assert!(!was_refused && first.is_none() && paid > 0);
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    assert!(physical.allocated <= paid && physical.highwater <= paid);
    let (((), physical), was_refused, used, first) = with_limit_usage_refusal(paid, || {
        measure(|| {
            reserve(std::mem::size_of::<Outgoing>()).unwrap();
            let mut set = Outgoing::new();
            assert!(set.insert(1).unwrap());
            let capacity = set.capacity();
            assert!(!crate::bounded_output::reserve_active_required(1));
            assert_eq!(set.insert(1).unwrap_err(), refused());
            assert_eq!(set.visit(&mut |_| Ok(())).unwrap_err(), refused());
            assert_eq!(set.capacity(), capacity);
            drop(set);
        })
    });
    assert!(was_refused && used == paid);
    let first = first.unwrap();
    assert_eq!(first.requested, 1);
    assert_eq!(first.remaining, 0);
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    assert!(physical.allocated <= used && physical.highwater <= used);
}
