//! Exact old borrowed caller, full ordered languages and original-owner frees.
use super::*;

fn fixture() -> ResolvedProgram {
    let parsed = crate::check(
        "module suffix.owned; @id(\"main\") fn main()->i64 {0}",
        "owned-suffix.spx",
    )
    .unwrap();
    crate::hir::resolve(&parsed).unwrap()
}

fn inputs(function: &ResolvedFunction, count: usize) -> Vec<ExprSkeletonPath> {
    (0..count)
        .map(|i| {
            let mut path = empty_expr_path();
            path.observations
                .items
                .push(std::rc::Rc::new(SkeletonObservation::Boolean {
                    expression: function.body.id.clone(),
                    value: i % 2 == 0,
                }));
            if i % 5 == 4 {
                path.continuation =
                    lexical_skeleton::Continuation::Advancing(path.observations.items[0].clone());
            }
            path.failed = i % 5 == 1;
            path.residual = i % 5 == 2;
            if i % 5 == 3 {
                path.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
            }
            path
        })
        .collect()
}

fn before(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    // Original owned caller keeps the whole suffix outside the unchanged
    // borrowed core. Prefixes move into that core and die before this owner.
    super::before::sequence_skeleton_paths(prefixes, &suffixes, work)
}

fn current(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    dispatch!(prefixes, suffixes, work);
    super::super::sequence_skeleton_paths(prefixes, &suffixes, work)
}

#[test]
fn owned_suffix_retains_original_owner_free_order_on_work_refusals() {
    use crate::test_allocation_observer::{measure, watch_free_order};
    let program = fixture();
    let function = program.functions().get(0).unwrap();
    for allowance in 0..96 {
        let mut observations = Vec::new();
        for old in [true, false] {
            let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
            budget.skeleton_remaining = allowance;
            budget.merge_paths = true;
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let ((outcome, order), metrics) = measure(|| {
                let prefixes = inputs(function, 2);
                let suffixes = inputs(function, 7);
                let mut pointers = vec![prefixes.as_ptr() as usize, suffixes.as_ptr() as usize];
                pointers.extend(
                    prefixes
                        .iter()
                        .chain(&suffixes)
                        .map(|p| p.observations.items.as_ptr() as usize),
                );
                let (outcome, report) = watch_free_order(&pointers, || {
                    let result = if old {
                        before(prefixes, suffixes, &mut work)
                    } else {
                        current(prefixes, suffixes, &mut work)
                    };
                    let outcome = result
                        .as_ref()
                        .map(|paths| paths.len())
                        .map_err(|e| e.clone());
                    drop(result);
                    drop(work.dag_scratch.take());
                    drop(work.ordinary_scratch.take());
                    outcome
                });
                assert!(!report.invalid);
                (outcome, report.order[..report.count].to_vec())
            });
            assert!(!metrics.invalid);
            // Returned diagnostic/order snapshots are test-owned live output.
            observations.push((outcome, order));
        }
        assert_eq!(
            observations[0], observations[1],
            "original owner Drop and all diagnostic fields at work={allowance}"
        );
    }
}

#[test]
fn owned_suffix_all_physical_refusals_drop_prefix_carrier_before_original_suffix() {
    use crate::test_allocation_observer::{measure, watch_free_order};
    let program = fixture();
    let function = program.functions().get(0).unwrap();
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    budget.merge_paths = true;
    let mut work = SkeletonWork::new(&program, function, &mut budget);
    let admission = work.budget.physical.as_ref().unwrap().charged();
    current(inputs(function, 2), inputs(function, 7), &mut work).unwrap();
    let complete = work.budget.physical.as_ref().unwrap().charged() - admission;
    drop(work);
    for allowance in 0..complete {
        let mut budget = ReplayBudget::with_physical(function, admission + allowance).unwrap();
        budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
        budget.merge_paths = true;
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let ((), metrics) = measure(|| {
            let prefixes = inputs(function, 2);
            let suffixes = inputs(function, 7);
            let pointers = [prefixes.as_ptr() as usize, suffixes.as_ptr() as usize];
            let (result, order) =
                watch_free_order(&pointers, || current(prefixes, suffixes, &mut work));
            assert!(result.is_err());
            assert!(!order.invalid);
            assert_eq!(
                &order.order[..order.count],
                &[0, 1],
                "prefix before original suffix at physical={allowance}"
            );
            drop(result);
            drop(work.dag_scratch.take());
            drop(work.ordinary_scratch.take());
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
    }
}

#[test]
fn owned_suffix_matches_full_borrowed_languages_and_work() {
    let parsed = crate::check("module owned.lexical; @id(\"main\") fn main()->i64 {let s=\"held\"; let mut i=0; while i<1 {i=i+1; if i==1 {return 1;}; 0} return 0;}", "owned-lexical.spx").unwrap();
    let lexical = crate::hir::resolve(&parsed).unwrap();
    for program in [fixture(), lexical] {
        let function = program.functions().get(0).unwrap();
        for selected in [false, true] {
            for prefix_count in [0, 1, 2, 7] {
                for suffix_count in [0, 1, 2, 7, 31] {
                    let mut outcomes = Vec::new();
                    for old in [true, false] {
                        let mut budget = if selected {
                            ReplayBudget::with_physical(function, 1 << 24).unwrap()
                        } else {
                            ReplayBudget::new()
                        };
                        budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
                        budget.merge_paths = true;
                        let mut work = SkeletonWork::new(&program, function, &mut budget);
                        if function.cleanup_plan.lexical_control.is_some() {
                            work.active_copy_region = Some(function.body.id.clone());
                        }
                        let prefixes = inputs(function, prefix_count);
                        let suffixes = inputs(function, suffix_count);
                        let result = if old {
                            before(prefixes, suffixes, &mut work)
                        } else {
                            current(prefixes, suffixes, &mut work)
                        }
                        .unwrap();
                        outcomes.push((
                            skeleton_work::merge_keys_tests::snapshot(
                                result.into_iter().map(|p| (p, vec![])).collect(),
                            ),
                            work.budget.skeleton_remaining,
                        ));
                    }
                    assert_eq!(
                        outcomes[0], outcomes[1],
                        "selected={selected} prefix={prefix_count} suffix={suffix_count}"
                    );
                }
            }
        }
    }
}
#[test]
fn owned_suffix_reports_cumulative_backing_through_result_destruction() {
    use crate::test_allocation_observer::measure;
    let program = fixture();
    let function = program.functions().get(0).unwrap();
    for old in [true, false] {
        let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
        budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
        budget.merge_paths = true;
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let baseline = work.budget.physical.as_ref().unwrap().charged();
        let prefixes = inputs(function, 7);
        let suffixes = inputs(function, 31);
        let (held, metrics) = measure(|| {
            let result = if old {
                before(prefixes, suffixes, &mut work)
            } else {
                current(prefixes, suffixes, &mut work)
            }
            .unwrap();
            drop(result);
            drop(work.dag_scratch.take());
            drop(work.ordinary_scratch.take());
            work.budget.physical.as_ref().unwrap().charged() - baseline
        });
        assert!(!metrics.invalid);
        assert!(metrics.allocated <= held && metrics.highwater <= held);
        assert_eq!(metrics.live, 0);
        eprintln!(
            "OPT878 owned suffix old={old}: held={held} allocated={} highwater={}",
            metrics.allocated, metrics.highwater
        );
    }
}

#[test]
fn owned_suffix_copy_refusal_precedes_exhausted_merge_work_in_both_whole_routes() {
    let program = fixture();
    let function = program.functions().get(0).unwrap();
    for exhaust_bytes in [true, false] {
        let mut errors = Vec::new();
        for old in [true, false] {
            let limit = 1 << 24;
            let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
            budget.merge_paths = true;
            budget.skeleton_remaining = 0;
            if !old {
                // Actual new row/control carriers are acquired under the valid
                // owner before the deliberately exhausted physical frontier.
                budget.reserve_bytes(function, CONTROLS + 7 * ROW_CLONE_CONTROLS, "mixed refusal row controls").unwrap();
            }
            if exhaust_bytes {
                let charged = budget.physical.as_ref().unwrap().charged();
                budget.physical.as_deref_mut().unwrap().reserve(limit - charged).unwrap();
            }
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let suffixes = inputs(function, 7);
            let error = if old {
                work.before_merged_suffixes(&suffixes).err().unwrap()
            } else {
                rows(&suffixes, &mut work).err().unwrap()
            };
            assert_eq!(work.budget.skeleton_remaining, 0);
            if exhaust_bytes {
                assert!(error.message.contains("physical reservation exhausted during factored suffix path copies"));
            } else {
                assert!(error.message.contains("work budget exhausted during factored path-state merge"));
            }
            errors.push(error);
        }
        assert_eq!(errors[0], errors[1], "all six first-failure diagnostic fields");
    }
}
