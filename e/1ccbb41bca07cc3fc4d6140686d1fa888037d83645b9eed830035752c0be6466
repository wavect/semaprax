//! Whole primitive-signature constructor remains executable beside local records.
use super::{ordinary, Context, Failure, Invoice};
use crate::ast::{Program, Type, TypeDeclarationKind};
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use crate::workspace_graph::expected_projection::{call_carriers, shared_local_callees};
use sha2::{Digest, Sha256};

const SOURCE: &str = r#"module record.local;
use function @id("remote") from record.provider as remote;
@id("leaf") record Leaf { @id("leaf.n") n:i64, }
@id("outer") record Outer { @id("outer.leaf") leaf:Leaf, }
@id("read") fn read(v:borrow Leaf)->i64{v.n}
@id("caller") fn caller(v:borrow Outer)->i64{read(v.leaf)+read(v.leaf)}
@id("main") fn main()->i64{0}
"#;
fn programs() -> [Program; 2] {
    [
        crate::parse(SOURCE, "record.spx").unwrap(),
        crate::parse(
            "module record.provider; @id(\"remote\") fn remote()->i64{0}",
            "provider.spx",
        )
        .unwrap(),
    ]
}
fn context<'a>(p: &'a [Program], i: &'a shared_local_callees::Index<'a>) -> Context<'a, 'a> {
    Context {
        program: &p[0],
        authored: i,
        programs: p,
    }
}

#[test]
fn nested_local_record_project_calls_share_only_the_callee_identity_slot() {
    let p = programs();
    let sources = p
        .iter()
        .map(|program| crate::workspace_graph::WorkspaceSource {
            path: program.path.clone(),
            source: crate::format::canonical(program),
        })
        .collect();
    crate::workspace_graph::build_owned(sources).unwrap();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let c = context(&p, &i);
    for imports in [true, false] {
        let (data, slots) = call_carriers::count_modular(c, imports).unwrap();
        assert_eq!(slots.slots, 2);
        assert_eq!(
            data.roots,
            call_carriers::count(c.program, &i, imports).roots
        );
    }
    // Entire immediate-old constructor, not a restated eligibility predicate.
    let mut old = None;
    shared_local_callees::modular_session::record_before::Session::observe(
        &mut old,
        c,
        &p[0].functions[1],
        &p[0].functions[0],
        1,
    )
    .unwrap();
    assert_eq!(
        shared_local_callees::modular_session::record_before::Session::checkpoint(&old),
        0
    );
    shared_local_callees::modular_session::record_before::Session::rollback(&mut old, 0).unwrap();
    assert_eq!(
        shared_local_callees::modular_session::record_before::Session::finish(old, c)
            .unwrap()
            .slots,
        0
    );
}

#[test]
fn recursive_generic_unknown_nonrecord_and_import_alias_signatures_remain_unproved() {
    for attack in 0..7 {
        let mut p = programs();
        match attack {
            0 => {
                if let TypeDeclarationKind::Record { fields } = &mut p[0].types[0].kind {
                    fields[0].ty = Type::Named {
                        name: "Outer".into(),
                        arguments: vec![],
                    };
                }
            }
            1 => p[0].types[0].explicit_id = false,
            2 => p[0].types[0].invariants = Some(Box::new(vec![])),
            3 => p[0].types[0].extends = Some(Type::I64),
            4 => p[0].types[0].kind = TypeDeclarationKind::Resource { lifecycles: vec![] },
            5 => p[0].types[0].name = "Unknown".into(),
            _ => {
                let mut alias = p[0].module_uses[0].clone();
                alias.alias = "Leaf".into();
                p[0].module_uses.push(alias);
            }
        }
        let i = crate::workspace_graph::index_authored(&p).unwrap();
        for imports in [false, true] {
            assert_eq!(
                call_carriers::count_modular(context(&p, &i), imports)
                    .unwrap()
                    .1
                    .slots,
                0,
                "attack={attack}"
            );
        }
    }
    let mut p = programs();
    p[0] = crate::parse(
        &SOURCE.replace("record Leaf {", "record Leaf<T> {"),
        "record.spx",
    )
    .unwrap();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    assert_eq!(
        call_carriers::count_modular(context(&p, &i), false)
            .unwrap()
            .1
            .slots,
        0
    );
}

#[test]
fn copied_type_wrong_kind_provider_or_source_never_authenticate_a_record_loan() {
    let p = programs();
    let copied = p[0].types[0].clone();
    for attack in 0..4 {
        let mut i = crate::workspace_graph::index_authored(&p).unwrap();
        let target = i.get_mut("leaf").unwrap();
        match attack {
            0 => target.ty = Some(&copied),
            1 => target.kind = crate::workspace_graph::AuthoredKind::Function,
            2 => target.module = "record.provider",
            _ => target.path = "other.spx",
        }
        assert_eq!(
            call_carriers::count_modular(context(&p, &i), false)
                .unwrap()
                .1
                .slots,
            0
        );
    }
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let foreign = [p[0].clone(), p[1].clone()];
    let invoice = Invoice::acquire().unwrap();
    assert!(!ordinary(
        Context {
            program: &p[0],
            authored: &i,
            programs: &foreign
        },
        &p[0].functions[0],
        &invoice
    )
    .unwrap());
}

#[test]
fn record_constructor_exact_short_zero_foreign_sticky_and_full_drop() {
    let p = programs();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let c = context(&p, &i);
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = call_carriers::count_modular(c, false);
                let ok = result.is_ok();
                drop(result);
                if !ok {
                    assert!(!reserve_active_required(0));
                }
                ok
            })
        })
    };
    let ((ok, a), refused, paid, first) = run(usize::MAX);
    assert!(ok && !refused && first.is_none());
    assert_eq!(a.live, 0);
    assert_eq!(a.allocated, a.freed);
    for (limit, expected) in [(paid, true), (paid - 1, false), (0, false)] {
        let ((ok, a), refused, _, first) = run(limit);
        assert_eq!(ok, expected);
        assert_eq!(refused, !expected);
        assert_eq!(first.is_some(), !expected);
        assert!(!a.invalid && a.live == 0);
        assert_eq!(a.allocated, a.freed);
    }
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let invoice = Invoice::acquire().unwrap();
        let ((), inner_refused, paid, selected) = with_limit_usage_refusal(usize::MAX, || {
            assert_eq!(
                ordinary(c, &p[0].functions[0], &invoice),
                Err(Failure::Foreign)
            );
        });
        assert!(!inner_refused && selected.is_none());
        assert_eq!(paid, 0);
        assert_eq!(
            ordinary(c, &p[0].functions[0], &invoice),
            Err(Failure::Physical)
        );
        assert!(!reserve_active_required(0));
    });
    assert!(refused && first.is_some());
}

#[test]
fn optional_cycle_walk_exhaustion_cannot_publish_a_partial_record_certificate() {
    let p = programs();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let c = context(&p, &i);
    let invoice = Invoice::acquire().unwrap();
    assert_eq!(
        invoice.work(shared_local_callees::WORK + 1),
        Err(Failure::Work)
    );
    assert_eq!(
        ordinary(c, &p[0].functions[0], &invoice),
        Err(Failure::Work)
    );
    let mut held = None;
    shared_local_callees::modular_session::Session::observe(
        &mut held,
        c,
        &p[0].functions[1],
        &p[0].functions[0],
        1,
    )
    .unwrap();
    assert_eq!(
        held.as_ref()
            .unwrap()
            .invoice
            .work(shared_local_callees::WORK + 1),
        Err(Failure::Work)
    );
    shared_local_callees::modular_session::Session::observe(
        &mut held,
        c,
        &p[0].functions[1],
        &p[0].functions[2],
        0,
    )
    .unwrap();
    assert_eq!(
        shared_local_callees::modular_session::Session::checkpoint(&held),
        0
    );
    shared_local_callees::modular_session::Session::rollback(&mut held, 1).unwrap();
    assert_eq!(
        shared_local_callees::modular_session::Session::finish(held, c)
            .unwrap()
            .slots,
        0
    );
}

pub(in crate::workspace_graph::expected_projection) fn restore(
    path: &str,
    current: &str,
) -> String {
    let packet: serde_json::Value =
        serde_json::from_str(include_str!("record_signature_inverse.json")).unwrap();
    let canonical = if path.starts_with("src/") {
        path.to_owned()
    } else {
        format!("src/workspace_graph/expected_projection/{path}")
    };
    let current903 = crate::source_reflows::next_custody_before("constructor903", &canonical, current);
    let current = current903.as_str();
    let Some(entry) = packet["entries"].get(&canonical) else {
        return current.to_owned();
    };
    let hash = |s: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes()))
        )
    };
    assert_eq!(hash(current), entry["current_sha256"].as_str().unwrap());
    let mut raw = current.to_owned();
    for splice in entry["splices"].as_array().unwrap() {
        let after = splice["current"].as_str().unwrap();
        assert_eq!(raw.matches(after).count(), 1);
        raw = raw.replacen(after, splice["prior"].as_str().unwrap(), 1);
    }
    assert_eq!(hash(&raw), entry["prior_sha256"].as_str().unwrap());
    raw
}
#[test]
fn complete_immediate_old_session_census_and_executable_constructor_are_exact() {
    for (path, raw, prior) in [
        (
            "shared_local_callees/modular_session.rs",
            include_str!("modular_session.rs"),
            include_str!("record_session.before.txt"),
        ),
        (
            "shared_local_callees/modular_census.rs",
            include_str!("modular_census.rs"),
            include_str!("record_census.before.txt"),
        ),
    ] {
        assert_eq!(restore(path, raw), prior);
    }
    let tail="\n#[cfg(test)]\n#[path = \"modular_tests.rs\"]\npub(in crate::workspace_graph::expected_projection) mod tests;\n";
    assert_eq!(
        include_str!("record_before.rs"),
        include_str!("record_session.before.txt")
            .strip_suffix(tail)
            .unwrap()
    );
}

#[test]
fn full_old_forecasters_remain_independent_and_all_nonidentity_terms_match() {
    use crate::workspace_graph::expected_projection::{
        before_shared_local_projection, cached_exact, dependency_identity_max,
        HIR_IDENTITY_COPY_FACTOR,
    };
    let p = programs();
    let i = crate::workspace_graph::index_authored(&p).unwrap();
    let maximum = Some(dependency_identity_max(&p[0], &i, &p).unwrap());
    let old_pair =
        shared_local_callees::paired::estimates(&p[0], &i, &p, maximum, 6, true, None).unwrap();
    let current_pair =
        shared_local_callees::modular_pair::estimates(&p[0], &i, &p, maximum, 6, true, None)
            .unwrap();
    assert_eq!(HIR_IDENTITY_COPY_FACTOR, 16);
    for (imports, old, current) in [
        (true, old_pair.0, current_pair.0),
        (false, old_pair.1, current_pair.1),
    ] {
        let baseline = before_shared_local_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
            &p[0], &i, &p, maximum, imports, (6, cached_exact::Retention::Parsed), None,
        ).unwrap();
        for observed in [&old, &current] {
            assert_eq!(observed.synthetic_ast, baseline.synthetic_ast);
            assert_eq!(observed.runtime, baseline.runtime);
            assert_eq!(
                observed.transient_import_clone,
                baseline.transient_import_clone
            );
        }
        // Observe the whole old baseline and immediate predecessor; this
        // reports the actual fee tradeoff and makes no unmeasured gain claim.
        eprintln!(
            "local-record imports={imports} pre916={} immediateold={} current={}",
            baseline.retained_hir, old.retained_hir, current.retained_hir
        );
    }
}
