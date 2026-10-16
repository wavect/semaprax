//! Whole shipping receiver controls; the full predecessor keeps its old macros.
use super::super::super::*;
use super::{before, capacity, RECEIVING};
use crate::bounded_output::{
    active_remaining, set_active_floor, with_limit_usage_refusal, with_limit_usage_refusal_site,
};
use crate::test_allocation_observer::measure;
#[derive(Debug, Eq, PartialEq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 1024], usize),
    path: Option<([u8; 1024], usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8; 1024], usize)>,
}
fn text(value: &str) -> ([u8; 1024], usize) {
    let mut bytes = [0; 1024];
    assert!(value.len() <= bytes.len());
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    (bytes, value.len())
}
fn failure(value: Diagnostic) -> Failure {
    let result = Failure {
        code: value.code,
        severity: value.severity,
        message: text(&value.message),
        path: value.path.as_deref().map(text),
        span: value.span,
        help: value.help.as_deref().map(text),
    };
    drop(value);
    result
}

fn seed(
    count: usize,
    stopped: usize,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut paths = Vec::new();
    for index in 0..count {
        let mut path = empty_expr_path();
        path.failed = stopped == 1 && index % 3 == 0;
        path.residual = stopped == 2 || (stopped == 1 && index % 3 == 1);
        path.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
        for bit in 0..10 {
            let expression = work.clone_owned(&work.function.body.id, "vector fixture identity")?;
            work.push_observation(
                &mut path,
                SkeletonObservation::Status {
                    source: StatusSourceId {
                        expression,
                        lane: StatusLane::OperationFailure,
                    },
                    success: bit % 2 == 0,
                },
                "vector fixture observation",
            )?;
        }
        path.continuation =
            lexical_skeleton::Continuation::Advancing(path.observations.items[0].clone());
        work.push_expr_path(&mut paths, path, "vector fixture path")?;
    }
    Ok(paths)
}

fn assert_expr_paths_equal(actual: &[ExprSkeletonPath], expected: &[ExprSkeletonPath]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        let ExprSkeletonPath {
            observations,
            owned_source,
            failed,
            residual,
            continuation,
        } = actual;
        let ExprSkeletonPath {
            observations: old_observations,
            owned_source: old_owned_source,
            failed: old_failed,
            residual: old_residual,
            continuation: old_continuation,
        } = expected;
        assert_eq!(observations, old_observations);
        assert_eq!(owned_source, old_owned_source);
        assert_eq!(failed, old_failed);
        assert_eq!(residual, old_residual);
        assert_eq!(continuation, old_continuation);
    }
}

// Real same-ledger ordinary DAG producer, restored to the original caller
// mode before sequencing. Each resulting path retains a nonempty flat tail.
fn dag_seed(
    count: usize,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut members = seed(2, 0, work)?;
    for member in &mut members {
        member.owned_source = None;
        member.continuation = lexical_skeleton::Continuation::Normal;
    }
    let old_merge = work.budget.merge_paths;
    let old_region = work.active_copy_region.take();
    work.active_copy_region =
        Some(work.clone_owned(&work.function.body.id, "DAG fixture region identity")?);
    work.budget.merge_paths = true;
    let merged = work.merged_paths(members);
    work.budget.merge_paths = old_merge;
    work.active_copy_region = old_region;
    let mut merged = merged?;
    assert_eq!(merged.len(), 1);
    let dag = merged.pop().unwrap().observations;
    assert!(dag.prefix.is_some());
    let mut inputs = seed(count, 0, work)?;
    for input in &mut inputs {
        let copied = work.clone_observations(&dag, "DAG fixture prefix")?;
        input.observations.prefix = copied.prefix;
        assert!(!input.observations.items.is_empty());
    }
    Ok(inputs)
}

#[test]
fn complete_fields_work_growth_and_drop_keep_all_input_domains() {
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for count in [0, 1, 7, 8, 9, 16, 17, 64] {
                for fanout in [0, 1, 2, 3] {
                    for stopped in [0, 1, 2] {
                        let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut old_work = SkeletonWork::new(&program, function, &mut old);
                        let mut work = SkeletonWork::new(&program, function, &mut current);
                        let old_inputs = seed(count, stopped, &mut old_work).unwrap();
                        let inputs = seed(count, stopped, &mut work).unwrap();
                        let old_suffixes = seed(fanout, 0, &mut old_work).unwrap();
                        let suffixes = seed(fanout, 0, &mut work).unwrap();
                        let before = skeleton_materializations();
                        let mut expected = before::sequence_skeleton_paths(
                            old_inputs,
                            &old_suffixes,
                            &mut old_work,
                        )
                        .unwrap();
                        let materializations = skeleton_materializations() - before;
                        let before = skeleton_materializations();
                        let mut actual =
                            sequence_skeleton_paths(inputs, &suffixes, &mut work).unwrap();
                        assert_eq!(skeleton_materializations() - before, materializations);
                        assert_expr_paths_equal(&actual, &expected);
                        assert_eq!(actual.capacity(), expected.capacity());
                        let old_capacity = actual.capacity();
                        for _ in actual.len()..=old_capacity {
                            work.push_expr_path(
                                &mut actual,
                                empty_expr_path(),
                                "minimum later growth",
                            )
                            .unwrap();
                            old_work
                                .push_expr_path(
                                    &mut expected,
                                    empty_expr_path(),
                                    "minimum later growth",
                                )
                                .unwrap();
                        }
                        assert_expr_paths_equal(&actual, &expected);
                        assert_eq!(actual.capacity(), expected.capacity());
                        assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                        assert_eq!(current.skeleton_spent, old.skeleton_spent);
                        assert_eq!(current.remaining, old.remaining);
                        drop((actual, expected, suffixes, old_suffixes));
                    }
                }
            }
            // Real DAG observations with retained tails on both sides exercise
            // the unchanged final-suffix move and complete general extension.
            let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut old_work = SkeletonWork::new(&program, function, &mut old);
            let mut work = SkeletonWork::new(&program, function, &mut current);
            let old_inputs = dag_seed(64, &mut old_work).unwrap();
            let inputs = dag_seed(64, &mut work).unwrap();
            let old_suffixes = dag_seed(3, &mut old_work).unwrap();
            let suffixes = dag_seed(3, &mut work).unwrap();
            let before = skeleton_materializations();
            let expected =
                before::sequence_skeleton_paths(old_inputs, &old_suffixes, &mut old_work).unwrap();
            let materializations = skeleton_materializations() - before;
            let before = skeleton_materializations();
            let actual = sequence_skeleton_paths(inputs, &suffixes, &mut work).unwrap();
            assert_eq!(skeleton_materializations() - before, materializations);
            assert_expr_paths_equal(&actual, &expected);
            assert_eq!(actual.capacity(), expected.capacity());
            assert_eq!(current.skeleton_spent, old.skeleton_spent);
            assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
            drop((actual, expected, suffixes, old_suffixes));
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

#[test]
fn paid_probe_reports_no_saving_case_and_exact_finite_sticky_accounts() {
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    for count in [8, 9, 16, 64] {
        let run = |old, limit| {
            with_limit_usage_refusal_site(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let result = (|| {
                        budget.prepare_builder(function)?;
                        budget.reserve_skeleton(function, MAX_REPLAY_WORK_UNITS)?;
                        let mut work = SkeletonWork::new(&program, function, &mut budget);
                        let inputs = seed(count, 0, &mut work)?;
                        let suffixes = seed(2, 0, &mut work)?;
                        let result = if old {
                            before::sequence_skeleton_paths(inputs, &suffixes, &mut work)
                        } else {
                            sequence_skeleton_paths(inputs, &suffixes, &mut work)
                        };
                        result.map(|paths| {
                            let shape = (paths.len(), paths.capacity());
                            drop(paths);
                            shape
                        })
                    })()
                    .map_err(failure);
                    let retry = if result.is_err() {
                        Some(
                            budget
                                .reserve_bytes(function, 0, "minimum sticky retry")
                                .map_err(failure),
                        )
                    } else {
                        None
                    };
                    drop(budget);
                    (result, retry)
                })
            })
        };
        let (((actual, _), allocation), refused, exact, first, site) = run(false, usize::MAX);
        let (((expected, _), old_allocation), old_refused, old_paid, old_first, old_site) =
            run(true, usize::MAX);
        assert_eq!(actual.unwrap(), expected.unwrap());
        assert!(!refused && first.is_none() && site.is_none());
        assert!(!old_refused && old_first.is_none() && old_site.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert!(!old_allocation.invalid && old_allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert_eq!(old_allocation.allocated, old_allocation.freed);
        assert!(allocation.allocated <= exact && old_allocation.allocated <= old_paid);
        let old_capacity = count.next_power_of_two();
        let selected = capacity(count, 1, old_capacity);
        if selected > old_capacity {
            let saved = (selected - old_capacity) * std::mem::size_of::<ExprSkeletonPath>();
            assert_eq!(old_allocation.allocated - allocation.allocated, saved);
            assert_eq!(old_paid + RECEIVING, exact + saved);
        }
        if count == 64 {
            assert!(selected > old_capacity);
        }
        if count == 9 {
            assert_eq!(capacity(9, 1, 16), 16);
            assert_eq!(
                exact,
                old_paid + RECEIVING,
                "paid false probe is reported without refund"
            );
            assert_eq!(allocation.allocated, old_allocation.allocated);
        }
        eprintln!("sequence minimum count={count} current_paid={exact} old_paid={old_paid} current_heap={allocation:?} old_heap={old_allocation:?}");
        for limit in [0, exact - 1, exact] {
            let (((outcome, retry), allocation), refused, paid, first, site) = run(false, limit);
            assert_eq!(outcome.is_ok(), limit == exact);
            assert_eq!(refused, limit != exact);
            if refused {
                assert!(first.is_some() && site.is_some());
                assert!(retry.unwrap().is_err());
            }
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= paid);
        }
    }
}

#[test]
fn pure_capacity_requires_full_removed_backing_and_checks_overflow() {
    assert_eq!(capacity(9, 1, 16), 16);
    assert_eq!(capacity(usize::MAX, 1, 16), 16);
    assert_eq!(
        capacity(MAX_REPLAY_PATHS, 1, MAX_REPLAY_PATHS),
        MAX_REPLAY_PATHS
    );
    for prefixes in [8usize, 16, 64, 256] {
        let old = prefixes.next_power_of_two();
        let selected = capacity(prefixes, 1, old);
        if selected != old {
            assert_eq!(selected, (prefixes + 1).next_power_of_two());
            assert!((selected - old) * std::mem::size_of::<ExprSkeletonPath>() >= RECEIVING);
        }
    }
}

thread_local! { static FIRST_WORK: std::cell::Cell<Option<(usize,usize)>> = const { std::cell::Cell::new(None) }; }
pub(in crate::cleanup_plan::replay) fn record_first(work: &SkeletonWork<'_, '_>) {
    if let Some(remaining) = active_remaining() {
        FIRST_WORK.with(|slot| slot.set(Some((remaining, work.budget.skeleton_spent))));
    }
}

#[test]
fn original_push_work_wins_before_optional_probe_at_real_floor() {
    let program = path_array_main8a::fixture_program();
    let function = &program.functions[0];
    let run = |old, work_limit, floor_offset: Option<usize>| {
        let mut budget = ReplayBudget::unobserved();
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let inputs = seed(64, 0, &mut work).unwrap();
        let suffixes = seed(2, 0, &mut work).unwrap();
        let observed = with_limit_usage_refusal_site(usize::MAX, || {
            measure(|| {
                *work.budget = ReplayBudget::new();
                let outcome = (|| {
                    work.budget.prepare_builder(function)?;
                    work.budget.reserve_skeleton(function, work_limit)?;
                    FIRST_WORK.with(|slot| slot.set(None));
                    if let Some(offset) = floor_offset {
                        assert!(set_active_floor(usize::MAX - offset));
                    }
                    let result = if old {
                        before::sequence_skeleton_paths(inputs, &suffixes, &mut work)
                    } else {
                        sequence_skeleton_paths(inputs, &suffixes, &mut work)
                    };
                    result.map(|paths| {
                        let count = paths.len();
                        drop(paths);
                        count
                    })
                })()
                .map_err(failure);
                let frontier = FIRST_WORK.with(|slot| slot.get());
                let remaining = work.budget.skeleton_remaining;
                *work.budget = ReplayBudget::unobserved();
                (outcome, frontier, remaining)
            })
        });
        drop(suffixes);
        drop(work);
        drop(budget);
        observed
    };
    let (((result, frontier, _), allocation), refused, _, first, site) =
        run(false, MAX_REPLAY_WORK_UNITS, None);
    assert!(result.is_ok() && !refused && first.is_none() && site.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    let (remaining, spent) = frontier.expect("actual first sequenced push work frontier");
    let (((actual, _, left), allocation), _, _, _, _) =
        run(false, spent, Some(usize::MAX - remaining));
    let (((expected, _, _), old_allocation), _, _, _, _) = run(true, spent, None);
    assert_eq!(
        actual.unwrap_err(),
        expected.unwrap_err(),
        "all six Diagnostic fields"
    );
    assert_eq!(left, 0);
    assert!(!allocation.invalid && allocation.live == 0);
    assert!(!old_allocation.invalid && old_allocation.live == 0);
}

#[test]
fn complete_predecessor_is_bound_to_the_single_reviewed_macro_replacement() {
    use sha2::{Digest, Sha256};
    let previous = include_str!("before.rs")
        .split_once("pub(super) fn sequence_skeleton_paths(")
        .unwrap()
        .1;
    let previous = format!("fn sequence_skeleton_paths({previous}");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(previous.trim_end().as_bytes()))
        ),
        "90dba8e16f69638f6631caec08acdad4e5fef2df778635ae223c38bd0cdf9232"
    );
    let root = include_str!("../../../replay.rs");
    let current = root
        .split_once("fn sequence_skeleton_paths(")
        .unwrap()
        .1
        .split_once("\nuse path_summary::has_active_paths;")
        .unwrap()
        .0;
    let current = format!("fn sequence_skeleton_paths({current}");
    let hook = "    #[cfg(test)]\n    if path_array_main44::selected() {\n        return path_array_main44::current_sequence(prefixes, suffixes, work);\n    }\n";
    assert_eq!(current.matches(hook).count(), 1);
    let restored = current
        .replacen(hook, "", 1)
        .replacen(
            "path_array::sequence_minimum::push!(",
            "path_array::push!(",
            1,
        )
        .replacen(
            "            minimum,\n            pending,\n            &mut combined,",
            "            minimum,\n            &mut combined,",
            1,
        );
    assert_eq!(restored.trim_end(), previous.trim_end());
}
