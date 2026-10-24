//! Whole-query predecessor, source custody, work cuts and physical lifetime.
use super::*;
use crate::bounded_output::{set_active_floor, with_limit_usage_refusal};
use crate::test_allocation_observer::{measure, Metrics};
use sha2::{Digest, Sha256};
use std::path::Path;

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
pub(super) fn restore(current: &str, path: &str) -> String {
    let encoded = include_str!("source_inverse.json");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        std::fs::read(
            root.join("src/cleanup_plan/replay/skeleton_work/query_proof/source_inverse.json")
        )
        .unwrap(),
        encoded.as_bytes()
    );
    let rows: serde_json::Value = serde_json::from_str(encoded).unwrap();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == path)
        .unwrap();
    let (old_length, old_hash) = match path {
        "src/cleanup_plan/replay.rs" => (327251, "6ac23252b3207a753b7f65940c17cd515de6cc7ebe4988d8a37fb775ae6034e4"),
        "src/cleanup_plan/replay/skeleton_work.rs" => (48238, "22f4de0b3fe49f2446588917440ad0beaa9473cfe0c638197149cdcf210d011f"),
        "src/cleanup_plan/replay/skeleton_work/oracle_entry.rs" => (4628, "9ccc81b3f79c93b7330f3cbe8013c05424e95b2808b51dc06bbb18f9bfa6f7b7"),
        "src/cleanup_plan/replay/ordinary_receiving/tests.rs" => (17167, "e377740ef20ff2c581038797439e6eaab13f7f3953b7719603c7977f59b70393"),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs" => (20133, "879722873041c2bdd8f390c91e39c14418d6d09550d5ffacd6cef9a7e6102d17"),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs" => (10194, "52299d69ae04653546366d2eb220e0acbfc590c823e0a7819d29a1b47e7be134"),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs" => (17889, "24506f3ae2705ba0274d20f43f261948f06cba5c70cd955e99907e82918a9899"),
        "src/cleanup_plan/replay/skeleton_work/query_proof/before.rs" => (10194, "52299d69ae04653546366d2eb220e0acbfc590c823e0a7819d29a1b47e7be134"),
        _ => panic!("unknown immutable predecessor {path}"),
    };
    assert_eq!(row["before_length"], old_length);
    assert_eq!(row["before_sha256"], old_hash);
    assert_eq!(std::fs::read(root.join(path)).unwrap(), current.as_bytes());
    assert_eq!(
        current.len(),
        row["current_length"].as_u64().unwrap() as usize
    );
    assert_eq!(digest(current.as_bytes()), row["current_sha256"]);
    let mut restored = current.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let at = edit[0].as_u64().unwrap() as usize;
        let after = edit[1].as_str().unwrap();
        assert_eq!(&restored[at..at + after.len()], after);
        restored.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
    }
    let target = std::fs::read(
        root.join("src/cleanup_plan/replay/skeleton_work/query_proof")
            .join(row["before_file"].as_str().unwrap()),
    )
    .unwrap();
    assert_eq!(restored.as_bytes(), target);
    assert_eq!(
        restored.len(),
        row["before_length"].as_u64().unwrap() as usize
    );
    assert_eq!(digest(restored.as_bytes()), row["before_sha256"]);
    restored
}
fn program() -> ResolvedProgram {
    let checked = crate::check(
        "module query_proof; @id(\"main\") fn main()->i64 {0}",
        "query-proof.spx",
    )
    .unwrap();
    crate::hir::resolve(&checked).unwrap()
}
fn tree(work: &mut SkeletonWork<'_, '_>, depth: usize) -> Result<Observations, Diagnostic> {
    let mut prefix = None;
    for _ in 0..depth {
        work.budget.reserve_bytes(
            work.function,
            super::super::enumerated_prefix::SHARE_CONTROLS,
            "structural query fixture sequence node receiving",
        )?;
        prefix = Some(super::super::enumerated_prefix::node(
            work,
            ObservationDagKind::Seq(prefix, Vec::new()),
        )?);
    }
    Ok(Observations {
        prefix,
        items: Vec::new(),
    })
}
fn call(
    work: &mut SkeletonWork<'_, '_>,
    observations: &Observations,
    old: bool,
) -> Result<bool, Diagnostic> {
    if old {
        work.budget.reserve_bytes(
            work.function,
            before::VALIDATE,
            "complete old structural query receiving",
        )?;
        before::sequence_only(observations, work)
    } else {
        sequence!(observations, work)
    }
}
fn fields(
    d: Diagnostic,
) -> (
    &'static str,
    crate::diagnostic::Severity,
    String,
    Option<String>,
    Option<crate::ast::Span>,
    Option<String>,
) {
    (
        d.code,
        d.severity,
        d.message.to_string(),
        d.path,
        d.span,
        d.help,
    )
}

#[test]
fn complete_sources_and_independent_old_query_bindings_are_exact() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("source_inverse.json")).unwrap();
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let current = match path {
            "src/cleanup_plan/replay.rs" => include_str!("../../../replay.rs"),
            "src/cleanup_plan/replay/skeleton_work.rs" => include_str!("../../skeleton_work.rs"),
            "src/cleanup_plan/replay/skeleton_work/oracle_entry.rs" => include_str!("../oracle_entry.rs"),
            "src/cleanup_plan/replay/ordinary_receiving/tests.rs" => include_str!("../../ordinary_receiving/tests.rs"),
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs" => include_str!("../../ordinary_plan_history/completed_fork/emission.rs"),
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs" => include_str!("../../ordinary_plan_history/completed_fork/emission/validation.rs"),
            "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs" => include_str!("../../ordinary_plan_history/completed_fork/emission/validation/tests.rs"),
            "src/cleanup_plan/replay/skeleton_work/query_proof/before.rs" => include_str!("before.rs"),
            _ => panic!("unknown complete source {path}"),
        };
        let before = restore(current, path);
        assert!(!before.is_empty());
    }
    let frozen_work = include_str!("work.before.txt");
    let first = frozen_work
        .find("pub(super) struct SkeletonWork<'a, 'b> {")
        .unwrap();
    let last = first + frozen_work[first..].find("\n}\n").unwrap() + 3;
    let layout = frozen_work[first..last].replacen(
        "pub(super) struct SkeletonWork<'a, 'b>",
        "struct BeforeWork<'a, 'b>",
        1,
    );
    assert_eq!(
        include_str!("../query_proof.rs").matches(&layout).count(),
        1
    );
    const {
        assert!(
            size_of::<SkeletonWork<'static, 'static>>()
                >= size_of::<BeforeWork<'static, 'static>>()
        );
        assert!(HEADER >= 3 * WORK_GROWTH + 6 * RESULT_GROWTH);
    }
    let classifier = include_str!("../../../../bounded_output/classifier_string.rs");
    assert_eq!(
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bounded_output/classifier_string.rs")
        )
        .unwrap(),
        classifier.as_bytes()
    );
    assert_eq!(classifier.len(), 23258);
    assert_eq!(
        digest(classifier.as_bytes()),
        "0fd752c50adec4ffe2269581cc7c7d2cc1e6d873d43a6bb47d35e6be9d826550"
    );
    let old = include_str!("before.rs");
    let validation = include_str!("validation.before.txt");
    let emission = include_str!("emission.before.txt");
    let fork = include_str!("fork.before.txt");
    assert_eq!(
        digest(include_bytes!("fork.before.txt")),
        "7e46966c5fd312a67b221adba57110447e20788af5c999133798a07d8003e761"
    );
    assert_eq!(
        digest(include_bytes!("enumerated.before.txt")),
        "2d982ef9483b56b96c736c5180c8556731e131b2c408aa0a59579ada3c0c7c2b"
    );
    let payment = &fork[fork.find("pub(super) const PAYMENT:").unwrap()..];
    let payment = &payment[..payment.find(';').unwrap() + 1];
    let validate = &emission[emission.find("const VALIDATE:").unwrap()..];
    let validate = &validate[..validate.find(';').unwrap() + 1];
    let start = emission
        .find("pub(in crate::cleanup_plan::replay) fn sequence_only(")
        .unwrap();
    let end = start
        + emission[start..]
            .find("\n#[allow(\n    unused_macros,")
            .unwrap();
    let rebound = validation
        .strip_suffix("#[cfg(test)]\nmod tests;\n")
        .unwrap()
        .replacen("SkeletonWork, PAYMENT,", "SkeletonWork,", 1);
    let (doc, rebound) = rebound.split_once('\n').unwrap();
    assert_eq!(
        old,
        format!(
            "{doc}\n{payment}\n{}\n{rebound}\n{}\n",
            validate.replacen("const VALIDATE:", "pub(super) const VALIDATE:", 1),
            &emission[start..end]
        )
    );
    assert_eq!(
        include_str!("../enumerated_prefix.rs"),
        include_str!("enumerated.before.txt")
    );
}

#[test]
fn root_reuse_subtree_extension_and_disjoint_roots_keep_old_logical_work() {
    let program = program();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let a = tree(&mut work, 32).unwrap();
        let b = tree(&mut work, 32).unwrap();
        for observations in [&a, &a, &b, &b, &a] {
            let left = work.budget.remaining;
            assert!(call(&mut work, observations, true).unwrap());
            let old = left - work.budget.remaining;
            let left = work.budget.remaining;
            assert!(call(&mut work, observations, false).unwrap());
            assert_eq!(left - work.budget.remaining, old);
            assert_eq!(old, 32);
        }
        work.budget
            .reserve_bytes(
                function,
                super::super::enumerated_prefix::SHARE_CONTROLS,
                "extended fixture node receivers",
            )
            .unwrap();
        let root = super::super::enumerated_prefix::node(
            &mut work,
            ObservationDagKind::Seq(a.prefix.clone(), Vec::new()),
        )
        .unwrap();
        let extended = Observations {
            prefix: Some(root),
            items: Vec::new(),
        };
        let left = work.budget.remaining;
        assert!(call(&mut work, &extended, false).unwrap());
        assert_eq!(left - work.budget.remaining, 33);
        assert_eq!(
            work.queries
                .owner
                .as_ref()
                .unwrap()
                .last
                .as_ref()
                .unwrap()
                .visits,
            33
        );
    });
}

#[test]
fn every_old_work_cut_keeps_all_six_diagnostic_fields_and_no_partial_publication() {
    let program = program();
    let function = &program.functions[0];
    for warm in [false, true] {
        for cut in 0..=9 {
            let mut outcomes = Vec::new();
            for old in [true, false] {
                let (outcome, _, _, _) = with_limit_usage_refusal(usize::MAX, || {
                    let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    let observations = tree(&mut work, 9).unwrap();
                    if warm {
                        assert!(call(&mut work, &observations, old).unwrap());
                    }
                    work.budget.remaining = cut;
                    let result = call(&mut work, &observations, old);
                    if !old && result.is_err() {
                        assert!(work.queries.owner.is_none());
                    }
                    result.map_err(fields)
                });
                outcomes.push(outcome);
            }
            assert_eq!(outcomes[0], outcomes[1]);
            assert_eq!(outcomes[0].is_ok(), cut == 9);
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct TextReceipt {
    bytes: [u8; 2048],
    length: usize,
}
impl TextReceipt {
    fn new(text: &str) -> Self {
        assert!(text.len() <= 2048);
        let mut bytes = [0; 2048];
        bytes[..text.len()].copy_from_slice(text.as_bytes());
        Self {
            bytes,
            length: text.len(),
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
struct DiagnosticReceipt {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: TextReceipt,
    path: Option<TextReceipt>,
    span: Option<crate::ast::Span>,
    help: Option<TextReceipt>,
}
impl From<Diagnostic> for DiagnosticReceipt {
    fn from(d: Diagnostic) -> Self {
        Self {
            code: d.code,
            severity: d.severity,
            message: TextReceipt::new(&d.message),
            path: d.path.as_deref().map(TextReceipt::new),
            span: d.span,
            help: d.help.as_deref().map(TextReceipt::new),
        }
    }
}

fn measured(
    program: &ResolvedProgram,
    old: bool,
    limit: usize,
    floor: bool,
) -> (
    (Result<bool, DiagnosticReceipt>, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    with_limit_usage_refusal(limit, || {
        measure(|| {
            let function = &program.functions[0];
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let result = (|| {
                let mut work = SkeletonWork::with_index(program, function, &mut budget, None)?;
                let observations = tree(&mut work, 24)?;
                for _ in 0..8 {
                    if floor {
                        set_active_floor(usize::MAX);
                    }
                    let result = call(&mut work, &observations, old);
                    if result.is_err() {
                        // Preserve the complete first error and prove sticky retry
                        // cannot consume/re-publish a removed invocation owner.
                        assert!(call(&mut work, &observations, old).is_err());
                        return result;
                    }
                }
                Ok(true)
            })();
            drop(budget);
            result.map_err(DiagnosticReceipt::from)
        })
    })
}
#[test]
fn cold_setup_warm_queries_final_drop_and_exact_byte_floor_controls_are_physical() {
    let program = program();
    for old in [true, false] {
        let ((result, heap), refused, paid, _) = measured(&program, old, usize::MAX, false);
        assert!(result.unwrap());
        assert!(!refused);
        assert_eq!(heap.live, 0);
        assert_eq!(heap.allocated, heap.freed);
        assert!(!heap.invalid);
        assert!(heap.allocated <= paid);
        for limit in [0, paid - 1, paid] {
            let ((result, heap), refused, _, _) = measured(&program, old, limit, false);
            assert_eq!(result.is_ok(), limit == paid);
            assert_eq!(refused, limit != paid);
            assert_eq!(heap.live, 0);
            assert_eq!(heap.allocated, heap.freed);
            assert!(!heap.invalid);
        }
        let ((result, heap), refused, _, _) = measured(&program, old, usize::MAX, true);
        assert!(result.is_err());
        assert!(refused);
        assert_eq!(heap.live, 0);
        assert_eq!(heap.allocated, heap.freed);
        assert!(!heap.invalid);
    }
}

#[test]
fn foreign_function_budget_and_nested_active_reject_with_complete_empty_h006() {
    let other = program();
    let program = program();
    let function = &program.functions[0];
    let expected = fields(Diagnostic::io("SPX-H006", String::new()));
    for warm in [false, true] {
        for arm in 0..3 {
            with_limit_usage_refusal(usize::MAX, || {
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut foreign = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                let observations = tree(&mut work, 3).unwrap();
                if warm {
                    assert!(call(&mut work, &observations, false).unwrap());
                }
                let error = if arm == 0 {
                    work.function = &other.functions[0];
                    call(&mut work, &observations, false).unwrap_err()
                } else if arm == 1 {
                    let before = foreign.remaining;
                    work.budget = &mut foreign;
                    let error = call(&mut work, &observations, false).unwrap_err();
                    assert_eq!(work.budget.remaining, before);
                    error
                } else {
                    let (error, refused, paid, _) = with_limit_usage_refusal(0, || {
                        call(&mut work, &observations, false).unwrap_err()
                    });
                    assert!(!refused);
                    assert_eq!(paid, 0);
                    error
                };
                assert_eq!(fields(error), expected);
                assert!(work.queries.owner.is_none());
            });
        }
    }
}

#[test]
fn actual_alt_query_does_not_publish_a_partial_sequence_proof() {
    let program = program();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        work.budget
            .reserve_bytes(
                function,
                4 * size_of::<ObservationDag>() + 4 * size_of::<ObservationDagKind>
                    > () + 4 * size_of::<Node>()
                        + 4 * size_of::<Vec<Node>>()
                        + 8 * size_of::<usize>()
                        + CALL,
                "complete hostile Alt fixture receivers",
            )
            .unwrap();
        let observations = Observations {
            prefix: Some(Rc::new(ObservationDag::legacy(ObservationDagKind::Alt(
                Vec::new(),
            )))),
            items: Vec::new(),
        };
        assert!(!call(&mut work, &observations, true).unwrap());
        assert!(!call(&mut work, &observations, false).unwrap());
        assert!(work.queries.owner.as_ref().unwrap().last.is_none());
        let valid = tree(&mut work, 3).unwrap();
        assert!(call(&mut work, &valid, false).unwrap());
        assert_eq!(
            work.queries
                .owner
                .as_ref()
                .unwrap()
                .last
                .as_ref()
                .unwrap()
                .visits,
            3
        );
    });
}

fn whole_inputs(
    program: &ResolvedProgram,
    old: bool,
) -> (
    (Result<usize, DiagnosticReceipt>, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let function = &program.functions[0];
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let result = (|| {
                let mut work = SkeletonWork::with_index(program, function, &mut budget, None)?;
                let observations = tree(&mut work, 64)?;
                work.budget.reserve_bytes(
                    function,
                    4 * size_of::<ExprSkeletonPath>() + CALL,
                    "whole caller fixture output receivers",
                )?;
                let mut path = empty_expr_path();
                path.observations = observations;
                let paths = [path];
                let before = work.budget.remaining;
                for _ in 0..24 {
                    let valid = if old {
                        work.budget.reserve_bytes(
                            function,
                            before::CALL,
                            "test complete sequence validation caller",
                        )?;
                        before::inputs(&paths, &[], &mut work)?
                    } else {
                        crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::shared_query_inputs_for_test(
                        &paths, &[], &mut work)?
                    };
                    assert!(valid);
                }
                Ok(before - work.budget.remaining)
            })();
            drop(budget);
            result.map_err(DiagnosticReceipt::from)
        })
    })
}
#[test]
fn query_only_old_inputs_and_actual_production_caller_keep_work_and_cover_complete_setup_tax() {
    let program = program();
    let ((old, old_heap), old_refused, old_paid, _) = whole_inputs(&program, true);
    let ((new, new_heap), new_refused, new_paid, _) = whole_inputs(&program, false);
    assert_eq!(old.unwrap(), new.unwrap());
    assert!(!old_refused && !new_refused);
    for heap in [old_heap, new_heap] {
        assert_eq!(heap.live, 0);
        assert_eq!(heap.allocated, heap.freed);
        assert!(!heap.invalid);
    }
    assert!(new_heap.allocations < old_heap.allocations);
    assert!(new_heap.allocated < old_heap.allocated);
    let original_invoice = old_paid
        .checked_sub(HEADER)
        .expect("one actual constructor tax");
    assert!(new_paid < original_invoice,
        "query-only common ABI old={old_paid} derived old without setup tax={original_invoice} new={new_paid} full tax={HEADER}");
    eprintln!("QUERY_ONLY shared Work ABI old_measured={old_paid} derived_original_invoice={original_invoice} complete_setup_tax={HEADER} new_paid={new_paid} old_heap={old_heap:?} new_heap={new_heap:?}; measured only when executed");
}

#[test]
fn unsupported_partitions_execute_complete_old_classifier_without_a_shared_owner() {
    let program = program();
    let function = &program.functions[0];
    for arm in 0..3 {
        with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let mut path = empty_expr_path();
            path.observations = tree(&mut work, 8).unwrap();
            let paths = [path];
            match arm {
                0 => {
                    work.budget.admission =
                        crate::cleanup_plan::replay::AdmissionMode::SelectedRetained
                }
                1 => work.budget.merge_paths = true,
                _ => work.budget.lent_skeleton = true,
            }
            work.budget
                .reserve_bytes(function, before::CALL, "old fallback full input caller")
                .unwrap();
            let remaining = work.budget.remaining;
            let old = before::inputs(&paths, &[], &mut work);
            let work_old = remaining - work.budget.remaining;
            let remaining = work.budget.remaining;
            let new = crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::shared_query_inputs_for_test(
                &paths, &[], &mut work);
            assert_eq!(old.map_err(fields), new.map_err(fields));
            assert_eq!(remaining - work.budget.remaining, work_old);
            assert!(work.queries.owner.is_none());
        });
    }
}

#[test]
fn actual_warm_byte_refusal_keeps_complete_first_diagnostic_and_sticky_receipt() {
    let program = program();
    let function = &program.functions[0];
    let ((success, heap), refused, paid, _) = measured(&program, false, usize::MAX, false);
    assert!(success.unwrap());
    assert!(!refused);
    assert_eq!(heap.live, 0);
    let ((actual, heap), refused, spent, first) = measured(&program, false, paid - 1, false);
    assert!(refused);
    assert!(spent <= paid - 1);
    assert!(first.is_some());
    assert_eq!(heap.live, 0);
    assert_eq!(heap.allocated, heap.freed);
    assert!(!heap.invalid);
    let (expected, _, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        budget.prepare_builder(function).unwrap();
        DiagnosticReceipt::from(budget.budget_refusal(
            function,
            "cleanup replay physical reservation exhausted during ",
            "shared sequence logical replay receivers",
        ))
    });
    assert_eq!(actual.unwrap_err(), expected);
}

#[test]
fn weak_one_entry_releases_disjoint_payloads_bounds_peak_and_final_drop() {
    let program = program();
    let function = &program.functions[0];
    let mut observations = Vec::new();
    for old in [true, false] {
        let ((result, heap), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let result = (|| {
                    let mut work = SkeletonWork::with_index(&program, function, &mut budget, None)?;
                    let first = tree(&mut work, 32)?;
                    work.budget.reserve_bytes(
                        function,
                        4 * size_of::<RootIdentity>()
                            + 4 * size_of::<&RootIdentity>()
                            + 4 * size_of::<&Node>()
                            + 4 * size_of::<Node>()
                            + 4 * size_of::<Option<Node>>()
                            + 4 * size_of::<bool>(),
                        "disjoint-root weak fixture receivers",
                    )?;
                    let weak = Rc::downgrade(first.prefix.as_ref().unwrap());
                    assert!(call(&mut work, &first, old)?);
                    let before_drop = crate::test_allocation_observer::current_metrics().live;
                    drop(first);
                    let after_drop = crate::test_allocation_observer::current_metrics().live;
                    assert!(
                        weak.upgrade().is_none(),
                        "neither query retains graph payload"
                    );
                    assert!(
                        before_drop - after_drop
                            >= 31 * physical_clone::rc_backing_bytes::<ObservationDag>(),
                        "all non-root allocations release before forming the disjoint root"
                    );
                    // The test Weak itself retains the root control block on
                    // both arms; the new proof adds no payload/children owner.
                    let second = tree(&mut work, 32)?;
                    assert!(call(&mut work, &second, old)?);
                    assert!(
                        weak.upgrade().is_none(),
                        "replacement releases the complete prior graph"
                    );
                    Ok::<_, Diagnostic>(true)
                })();
                drop(budget);
                result.map_err(DiagnosticReceipt::from)
            })
        });
        assert!(result.unwrap());
        assert!(!refused);
        assert_eq!(heap.live, 0);
        assert_eq!(heap.allocated, heap.freed);
        assert!(!heap.invalid);
        observations.push(heap);
    }
    assert!(
        observations[1].highwater
            <= observations[0].highwater
                + size_of::<Proof<'static>>()
                + physical_clone::rc_backing_bytes::<ObservationDag>(),
        "one proof Box and one root control block bound extra disjoint-root overlap"
    );
    eprintln!(
        "QUERY_ONLY weak cache disjoint-root old_common_ABI={:?} new={:?}; no forecast/app claim",
        observations[0], observations[1]
    );
}
