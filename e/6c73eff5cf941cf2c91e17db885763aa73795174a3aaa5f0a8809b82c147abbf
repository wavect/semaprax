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
                    work.release_observation_scratch_for_test();
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
            work.release_observation_scratch_for_test();
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
            work.release_observation_scratch_for_test();
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
                budget
                    .reserve_bytes(
                        function,
                        CONTROLS + 7 * ROW_CLONE_CONTROLS,
                        "mixed refusal row controls",
                    )
                    .unwrap();
            }
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            // Pay the complete real constructor before exhausting the intended
            // suffix-copy frontier; no constructor fee or allowance is bypassed.
            if exhaust_bytes {
                let charged = work.budget.physical.as_ref().unwrap().charged();
                work.budget
                    .physical
                    .as_deref_mut()
                    .unwrap()
                    .reserve(limit - charged)
                    .unwrap();
                assert_eq!(work.budget.physical.as_ref().unwrap().charged(), limit);
            }
            let suffixes = inputs(function, 7);
            let error = if old {
                work.before_merged_suffixes(&suffixes).err().unwrap()
            } else {
                rows(&suffixes, &mut work).err().unwrap()
            };
            assert_eq!(work.budget.skeleton_remaining, 0);
            if exhaust_bytes {
                assert!(error
                    .message
                    .contains("physical reservation exhausted during factored suffix path copies"));
            } else {
                assert!(error
                    .message
                    .contains("work budget exhausted during factored path-state merge"));
            }
            errors.push(error);
        }
        assert_eq!(
            errors[0], errors[1],
            "all six first-failure diagnostic fields"
        );
    }
}

#[test]
fn owned_suffix_floor_foreign_frontiers_keep_six_fields_sticky_and_prefix_drop() {
    use crate::bounded_output::{
        active_remaining, clear_active_floor, set_active_floor, with_limit_usage_refusal,
    };
    use crate::test_allocation_observer::{measure, watch_free_order};
    use skeleton_work::merge_keys_tests::{boundary_diagnostic, boundary_foreign_root, snapshot};
    let program = fixture();
    let function = program.functions().get(0).unwrap();
    let (((), metrics), refused, _, _) = with_limit_usage_refusal(1 << 28, || {
        measure(|| {
            for selected in [false, true] {
                let mut outcomes = Vec::new();
                for old in [true, false] {
                    let foreign = boundary_foreign_root(&program, function);
                    let mut budget = if selected {
                        ReplayBudget::with_physical(function, 1 << 24).unwrap()
                    } else {
                        ReplayBudget::new()
                    };
                    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
                    budget.merge_paths = true;
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    let prefixes = inputs(function, 1);
                    let mut suffixes = inputs(function, 7);
                    for path in &mut suffixes {
                        path.observations.prefix = Some(foreign.clone());
                    }
                    let pointers = [prefixes.as_ptr() as usize, suffixes.as_ptr() as usize];
                    let (result, order) = watch_free_order(&pointers, || {
                        if old {
                            before(prefixes, suffixes, &mut work)
                        } else {
                            current(prefixes, suffixes, &mut work)
                        }
                    });
                    assert!(!order.invalid);
                    if selected {
                        assert_eq!(
                            result.as_ref().err().unwrap(),
                            &boundary_diagnostic(
                                function,
                                "factored DAG crosses its destruction reservation"
                            )
                        );
                        assert_eq!(
                            &order.order[..order.count],
                            &[0, 1],
                            "foreign selected refusal drops prefix before original suffix"
                        );
                    } else {
                        assert!(
                            result.is_ok(),
                            "ordinary immutable foreign children remain admitted"
                        );
                    }
                    outcomes.push(result.map(|paths| {
                        snapshot(paths.into_iter().map(|path| (path, vec![])).collect())
                    }));
                    drop(work);
                    drop(budget);
                    drop(foreign);
                }
                assert_eq!(
                    outcomes[0], outcomes[1],
                    "whole36 language or six-field foreign refusal; selected={selected}"
                );
                drop(outcomes);
            }
        })
    });
    assert!(!refused && !metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    for selected in [false, true] {
        for old in [true, false] {
            let (((), metrics), refused, _, receipt) = with_limit_usage_refusal(1 << 28, || {
                measure(|| {
                    let mut budget = if selected {
                        ReplayBudget::with_physical(function, 1 << 24).unwrap()
                    } else {
                        ReplayBudget::new()
                    };
                    budget
                        .reserve_bytes(function, 1, "owned boundary fixture preflight")
                        .unwrap();
                    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
                    budget.merge_paths = true;
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    let prefixes = inputs(function, 1);
                    let suffixes = inputs(function, 7);
                    // Exactly the new dispatch's actual caller price is acquired
                    // before moving arguments and before selecting the floor.
                    if !old {
                        work.budget
                            .reserve_bytes(
                                function,
                                QUERY + CONTROLS + suffixes.len() * ROW_CLONE_CONTROLS,
                                "owned boundary actual caller controls",
                            )
                            .unwrap();
                    }
                    let pointers = [prefixes.as_ptr() as usize, suffixes.as_ptr() as usize];
                    assert!(set_active_floor(active_remaining().unwrap()));
                    let (error, order) = watch_free_order(&pointers, || {
                        if old {
                            before(prefixes, suffixes, &mut work)
                        } else {
                            sequence(prefixes, suffixes, &mut work)
                        }
                        .err()
                        .unwrap()
                    });
                    assert!(!order.invalid);
                    assert_eq!(&order.order[..order.count], &[0, 1]);
                    let expected = boundary_diagnostic(function, "cleanup replay physical reservation exhausted during factored suffix path copies");
                    assert_eq!(
                        error, expected,
                        "all six floor diagnostic fields against the same original copy frontier"
                    );
                    assert!(work.budget.refusal_spent);
                    clear_active_floor();
                    let remaining = active_remaining();
                    let physical = work.budget.physical.as_ref().map(|p| p.charged());
                    let retry = work
                        .budget
                        .reserve_bytes(function, 0, "owned boundary sticky retry")
                        .unwrap_err();
                    assert_eq!(retry, Diagnostic::io("SPX-H006", String::new()));
                    assert_eq!(active_remaining(), remaining);
                    assert_eq!(work.budget.physical.as_ref().map(|p| p.charged()), physical);
                    assert_eq!(
                        error, expected,
                        "retry cannot replace the selected original failure"
                    );
                    drop((retry, error, expected));
                    drop(work);
                    drop(budget);
                })
            });
            assert!(refused && receipt.is_some());
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
    }
}

fn immediate878(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    assert!(
        suffixes.len() > 1 && work.budget.merge_paths,
        "this witness is the complete immediate owned branch, not its unrelated borrowed fallback"
    );
    super::super::owned_suffix_878::dispatch!(prefixes, suffixes, work);
    unreachable!("the complete immediate dispatch must consume these admitted inputs")
}
#[test]
fn retained_owned_search_keeps_whole_immediate_sequence_languages_work_and_release_order() {
    use crate::test_allocation_observer::{measure, watch_free_order};
    let program = fixture();
    let function = &program.functions[0];
    for prefix_count in [0, 1, 2, 7] {
        for suffix_count in [2, 7, 31] {
            for work_limit in [0, 1, 7, 31, 96, MAX_REPLAY_WORK_UNITS] {
                let mut outcomes = Vec::new();
                for old in [true, false] {
                    let mut budget = ReplayBudget::with_physical(function, 1 << 26).unwrap();
                    budget.skeleton_remaining = work_limit;
                    budget.merge_paths = true;
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    let ((result, order), metrics) = measure(|| {
                        let prefixes = inputs(function, prefix_count);
                        let suffixes = inputs(function, suffix_count);
                        let mut pointers = Vec::new();
                        if !prefixes.is_empty() {
                            pointers.push(prefixes.as_ptr() as usize);
                        }
                        pointers.push(suffixes.as_ptr() as usize);
                        let (result, order) = watch_free_order(&pointers, || {
                            let result = if old {
                                immediate878(prefixes, suffixes, &mut work)
                            } else {
                                current(prefixes, suffixes, &mut work)
                            };
                            let output = result
                                .as_ref()
                                .map(|paths| {
                                    skeleton_work::merge_keys_tests::snapshot(
                                        paths.clone().into_iter().map(|p| (p, vec![])).collect(),
                                    )
                                })
                                .map_err(|error| error.clone());
                            drop(result);
                            work.release_observation_scratch_for_test();
                            output
                        });
                        assert!(!order.invalid);
                        (result, (order.order, order.count))
                    });
                    assert!(!metrics.invalid);
                    // Snapshot output is explicitly test-owned, as in the old owner;
                    // the whole production result and watched carriers die inside.
                    outcomes.push((
                        result,
                        work.budget.remaining,
                        work.budget.skeleton_remaining,
                        work.budget.skeleton_spent,
                        order,
                    ));
                }
                assert_eq!(
                    outcomes[0], outcomes[1],
                    "prefixes={prefix_count} suffixes={suffix_count} work={work_limit}"
                );
            }
        }
    }
}
struct RetainedOwnedRun {
    error: Option<crate::cleanup_plan::replay::merge_census::Failure>,
    report: crate::cleanup_plan::replay::merge_census::Report,
    admission: usize,
    paid: usize,
}
fn retained_owned_run(
    program: &ResolvedProgram,
    old: bool,
    physical: usize,
    allowance: Option<usize>,
) -> (RetainedOwnedRun, crate::test_allocation_observer::Metrics) {
    use crate::cleanup_plan::replay::merge_census::{failure, Scope};
    let function = &program.functions[0];
    let ((out, metrics), _, _, _) =
        crate::bounded_output::with_limit_usage_refusal(1 << 28, || {
            crate::test_allocation_observer::measure(|| {
                let mut budget = ReplayBudget::with_physical(function, physical).unwrap();
                budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
                budget.merge_paths = true;
                let mut work = SkeletonWork::new(program, function, &mut budget);
                let admission = work.budget.physical.as_ref().unwrap().charged();
                let prefixes = inputs(function, 7);
                let suffixes = inputs(function, 31);
                if let Some(allowance) = allowance {
                    assert!(crate::bounded_output::set_active_floor(
                        crate::bounded_output::active_remaining().unwrap() - allowance
                    ));
                }
                let scope = Scope::enter(function, work.budget);
                let result = if old {
                    immediate878(prefixes, suffixes, &mut work)
                } else {
                    current(prefixes, suffixes, &mut work)
                }
                .map(drop);
                let error = result.as_ref().err().map(failure);
                if let Some(first) = &error {
                    crate::bounded_output::clear_active_floor();
                    let paid = work.budget.physical.as_ref().unwrap().charged();
                    let remaining = (
                        work.budget.remaining,
                        work.budget.skeleton_remaining,
                        work.budget.skeleton_spent,
                    );
                    let allocation = crate::test_allocation_observer::current_metrics();
                    let retry = work
                        .budget
                        .reserve_bytes(function, 0, "owned retained sticky retry")
                        .unwrap_err();
                    assert_eq!(retry, Diagnostic::io("SPX-H006", String::new()));
                    assert_eq!(paid, work.budget.physical.as_ref().unwrap().charged());
                    assert_eq!(
                        remaining,
                        (
                            work.budget.remaining,
                            work.budget.skeleton_remaining,
                            work.budget.skeleton_spent
                        )
                    );
                    assert_eq!(
                        crate::test_allocation_observer::current_metrics(),
                        allocation
                    );
                    assert_eq!(failure(result.as_ref().err().unwrap()), *first);
                    drop(retry);
                }
                let paid = work.budget.physical.as_ref().unwrap().charged() - admission;
                let report = scope.report();
                assert!(report.complete && report.foreign == 0);
                assert_eq!(
                    report.cells.iter().map(|cell| cell.acquired).sum::<usize>(),
                    paid
                );
                drop(scope);
                crate::bounded_output::clear_active_floor();
                work.release_observation_scratch_for_test();
                drop(work);
                drop(budget);
                drop(result);
                RetainedOwnedRun {
                    error,
                    report,
                    admission,
                    paid,
                }
            })
        });
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    (out, metrics)
}
#[test]
fn retained_owned_search_real_receiving_frontiers_keep_full_diagnostics_sticky_floor_and_whole_drop(
) {
    let program = fixture();
    let (success, metrics) = retained_owned_run(&program, false, 1 << 26, None);
    assert!(success.error.is_none());
    let (old, old_metrics) = retained_owned_run(&program, true, 1 << 26, None);
    assert!(old.error.is_none());
    eprintln!("OPT881 owned whole878 paid={} heap={} peak={}; retained paid={} heap={} peak={}; actual category requests={:?}/{:?}; unchanged work/caps, no application-fit claim", old.paid, old_metrics.allocated, old_metrics.highwater, success.paid, metrics.allocated, metrics.highwater, old.report.cells, success.report.cells);
    let mut events = Vec::new();
    for category in [1, 2, 3] {
        events.push(success.report.first[category].unwrap());
        events.push(success.report.last[category].unwrap());
    }
    events.push(success.report.first[11].unwrap()); // actual row owner/header bank before construction
    for event in events {
        let before = success.admission + event.charged_before;
        let (short, _) = retained_owned_run(&program, false, before + event.requested - 1, None);
        assert_eq!(
            short.error.unwrap(),
            skeleton_work::merge_keys_tests::retained_primitive_oracle(
                &program,
                event,
                before + event.requested - 1,
                None
            )
        );
        let (exact, _) = retained_owned_run(&program, false, before + event.requested, None);
        assert!(exact.paid >= event.charged_before + event.requested);
        let floor = event.charged_before + event.requested - 1;
        let (floored, _) = retained_owned_run(&program, false, 1 << 26, Some(floor));
        assert_eq!(
            floored.error.unwrap(),
            skeleton_work::merge_keys_tests::retained_primitive_oracle(
                &program,
                event,
                1 << 26,
                Some(floor)
            )
        );
    }
    let (zero, _) = retained_owned_run(&program, false, 1 << 26, Some(0));
    assert!(zero.error.is_some());
    assert_eq!(zero.paid, 0);
}
