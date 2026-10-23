//! Whole immediate-old constructor, exact work/physical frontiers and actual
//! allocation through result/report/owner Drop. No forecast is discounted.
use super::*;
use crate::bounded_output::{
    cfg_reservation::CfgReservation, with_limit_usage, with_limit_usage_refusal, ReservationRefusal,
};
use crate::test_allocation_observer::{
    measure_with_capacity, with_phase_trace, Metrics, PhaseTrace,
};

type Observation = ((bool, Metrics), bool, usize, Option<ReservationRefusal>);

fn observe(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expected: &LoanPlan,
    old: bool,
    limit: usize,
) -> (Observation, PhaseTrace) {
    with_phase_trace(|| {
        with_limit_usage_refusal(limit, || {
            measure_with_capacity(1 << 16, || {
                // Cold owner, candidate inventory, CFG, every planner scratch
                // and the receiving result/report all belong to this window.
                let owner = match CfgReservation::acquire() {
                    Ok(owner) => owner,
                    Err(_) => return false,
                };
                let result = if old {
                    funded_flat::before_reachability_release::build(program, function, &owner)
                } else {
                    funded_flat::build(program, function, &owner)
                };
                match result {
                    Ok(plan) => {
                        assert_eq!(&plan, expected);
                        assert_eq!(owner.first(), None);
                        drop(plan);
                        drop(owner);
                        true
                    }
                    Err(storage::PlannerError::Physical(first)) => {
                        assert_eq!(owner.first(), Some(first));
                        assert_eq!(owner.reserve(0), Err(first));
                        assert_eq!(owner.reserve(1), Err(first));
                        let pointer = owner.text_pointer();
                        let report = owner.diagnostic();
                        assert_eq!(report.message.as_ptr(), pointer);
                        drop(report);
                        false
                    }
                    Err(storage::PlannerError::Semantic(error)) => {
                        panic!("intended complete constructor positive: {error:?}")
                    }
                }
            })
        })
    })
}

fn counted(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    old: bool,
    limit: usize,
) -> (Result<LoanPlan, Diagnostic>, usize) {
    let (result, refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let subject = funded_flat::subject_for_test(program, function, &owner).unwrap();
        let mut work = WorkCounter::new(limit);
        work.allocation = Some(&owner);
        let result =
            paid_candidate::inspect_retained(subject, &owner, &mut work).and_then(|inventory| {
                match inventory {
                    Some(inventory) if old => {
                        before_reachability_release::build(program, function, &mut work, inventory)
                    }
                    Some(inventory) => retained::build(program, function, &mut work, inventory),
                    None => Ok(LoanPlan::empty_v1()),
                }
            });
        assert_eq!(owner.first(), None);
        (result, work.used)
    });
    assert!(!refused);
    result
}

fn expression_count(expression: &ResolvedExpr) -> usize {
    let mut total = 1;
    let mut index = 0;
    while let Some(child) = expression.child(index) {
        total += expression_count(child);
        index += 1;
    }
    total
}

fn whole(program: &ResolvedProgram, function: &ResolvedFunction) {
    let expected = saved_planner::build_plan(program, function).unwrap();
    assert!(!expected.loans.is_empty());
    let (old, _) = observe(program, function, &expected, true, usize::MAX);
    let (current, phases) = observe(program, function, &expected, false, usize::MAX);
    assert_eq!(old.0 .0, current.0 .0);
    assert!(current.0 .0 && !current.1);
    assert_eq!((old.2, old.3), (current.2, current.3));
    assert_eq!(current.3, None);
    let old_metrics = old.0 .1;
    let metrics = current.0 .1;
    assert_eq!(metrics.allocated, old_metrics.allocated);
    assert_eq!(metrics.allocations, old_metrics.allocations);
    assert_eq!(metrics.reallocations, old_metrics.reallocations);
    assert!(metrics.highwater <= old_metrics.highwater);
    assert!(metrics.allocated <= current.2);
    assert!(metrics.highwater <= current.2);
    for actual in [old_metrics, metrics] {
        assert!(!actual.invalid);
        assert_eq!(actual.live, 0);
        assert_eq!(actual.allocated, actual.freed);
    }
    assert!(!phases.overflow);
    let (entry, peak, exit) = phases
        .function_peak("loan_plan::paid_reachability_drop")
        .unwrap();
    // Independent HIR census, not the production CFG or a forecast counter.
    // Every body/contract expression owns two points and each loan-mask row
    // contains ceil(actual loans/64) words. The queue is already destroyed.
    let expressions: usize = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .map(expression_count)
        .sum();
    let rows = 2 * expressions * expected.loans.len().div_ceil(64) * 8;
    assert!(rows > 0);
    assert_eq!(entry.live.checked_sub(exit.live), Some(rows));
    assert_eq!(entry.freed.checked_add(rows), Some(exit.freed));
    assert_eq!(entry.allocated, exit.allocated);
    assert_eq!(peak, entry.live);
    eprintln!("OPT903 paid loan row release rows={rows} old={old_metrics:?} current={metrics:?} debit={} phases={phases:?}; actual requested layouts, cumulative reservation unchanged", current.2);

    // The release occurs after all of its existing reservations. Exact,
    // one-short, zero and intermediate physical frontiers stay identical.
    for limit in [0, current.2 / 2, current.2 - 1, current.2] {
        let (old, _) = observe(program, function, &expected, true, limit);
        let (current, phases) = observe(program, function, &expected, false, limit);
        assert_eq!(
            (old.0 .0, old.1, old.2, old.3),
            (current.0 .0, current.1, current.2, current.3)
        );
        assert!(!phases.overflow);
        for actual in [old.0 .1, current.0 .1] {
            assert!(!actual.invalid);
            assert_eq!(actual.live, 0);
            assert_eq!(actual.allocated, actual.freed);
        }
        assert_eq!(current.0 .0, limit == current.2 && !current.1);
    }
}

#[test]
fn cold_complete_paid_constructor_releases_actual_rows_with_identical_receipts_and_drop() {
    for count in [1, 3, 17] {
        let (mut program, index) = boundary_tests::fixture_with_uses(count, true);
        whole(&program, &program.functions[index]);
        // Attached evidence never sizes or authenticates independent scratch.
        program.functions[index].loan_plan.loans.clear();
        program.functions[index].loan_plan.endpoints.reverse();
        whole(&program, &program.functions[index]);
    }
}

#[test]
fn complete_plans_and_exact_work_errors_match_before_release_including_late_overlap() {
    let (mut program, index) = boundary_tests::fixture_with_uses(3, true);
    for hostile in [false, true] {
        if hostile {
            let expected = saved_planner::build_plan(&program, &program.functions[index]).unwrap();
            let owned = expected.loans[0].origin.root.clone();
            let ResolvedExprKind::Block { statements, .. } =
                &mut program.functions[index].body.kind
            else {
                panic!("boundary block")
            };
            let ResolvedStatement::Let { value, .. } = &mut statements[4] else {
                panic!("original first use")
            };
            value.kind = ResolvedExprKind::Place(Place {
                root: owned,
                projections: Vec::new(),
            });
            value.ty = ResolvedType::Bytes;
            value.ownership = OwnershipMode::Own;
        }
        let function = &program.functions[index];
        let (expected, exact) = counted(&program, function, true, MAX_LOAN_PLAN_WORK_V1);
        assert!(exact > 0);
        assert_eq!(expected.is_err(), hostile);
        if let Err(error) = &expected {
            assert_eq!(
                error.message,
                "move, mutation, or transfer overlaps an active shared loan"
            );
        }
        for limit in [0, 1, exact - 1, exact, MAX_LOAN_PLAN_WORK_V1] {
            let old = counted(&program, function, true, limit);
            let current = counted(&program, function, false, limit);
            assert_eq!(
                current, old,
                "complete plans / all Diagnostic fields and actual work"
            );
            if limit >= exact {
                assert_eq!(current.0, expected);
            }
        }
    }
}

#[test]
fn mixed_work_and_physical_first_refusals_keep_full_diagnostics_and_cold_drop() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let function = &program.functions[index];
    let (expected, refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        funded_flat::with_inventory_work_limit(0, || {
            match funded_flat::before_reachability_release::build(&program, function, &owner) {
                Err(storage::PlannerError::Semantic(error)) => error,
                _ => panic!("intended complete old checked-work refusal"),
            }
        })
    });
    assert!(!refused);
    // Independent existing owner-layout plus exact dedicated message bytes.
    let setup = std::mem::size_of::<CfgReservation>() + 46;
    for limit in [0, setup, usize::MAX] {
        let run = |old| {
            with_limit_usage_refusal(limit, || {
                measure_with_capacity(1 << 16, || {
                    let owner = match CfgReservation::acquire() {
                        Ok(owner) => owner,
                        Err(_) => return 0,
                    };
                    let result = funded_flat::with_inventory_work_limit(0, || {
                        if old {
                            funded_flat::before_reachability_release::build(
                                &program, function, &owner,
                            )
                        } else {
                            funded_flat::build(&program, function, &owner)
                        }
                    });
                    match result {
                        Err(storage::PlannerError::Semantic(error)) => {
                            assert_eq!(error, expected, "all six Diagnostic fields");
                            assert_eq!(owner.first(), None);
                            drop(error);
                            drop(owner);
                            2
                        }
                        Err(storage::PlannerError::Physical(first)) => {
                            assert_eq!(owner.reserve(0), Err(first));
                            assert_eq!(owner.reserve(1), Err(first));
                            drop(owner.diagnostic());
                            1
                        }
                        Ok(_) => panic!("zero work cannot admit this function"),
                    }
                })
            })
        };
        let old = run(true);
        let current = run(false);
        assert_eq!(
            current, old,
            "work versus physical selection/debit/real layouts"
        );
        assert_eq!(
            current.0 .0,
            if limit == 0 {
                0
            } else if limit == setup {
                1
            } else {
                2
            }
        );
        assert!(!current.0 .1.invalid);
        assert_eq!(current.0 .1.live, 0);
        assert_eq!(current.0 .1.allocated, current.0 .1.freed);
    }
}

#[test]
fn complete_immediate_old_sources_are_exact_inverses_with_common_helper_custody() {
    let mut current = include_str!("retained.rs").to_owned();
    for name in ["BUILD_TRACE", "ROW_TRACE", "ROW_RELEASE"] {
        let begin = format!("    // OPT903_{name}_BEGIN\n");
        let end = format!("    // OPT903_{name}_END\n");
        assert_eq!(current.matches(&begin).count(), 1);
        assert_eq!(current.matches(&end).count(), 1);
        let start = current.find(&begin).unwrap();
        let finish = current.find(&end).unwrap() + end.len();
        current.replace_range(start..finish, "");
    }
    assert_eq!(current, include_str!("before_reachability_release.rs"));
    let current = include_str!("../funded_flat.rs");
    let current = &current[..current.find("\n#[cfg(test)]\n#[allow(").unwrap()];
    let compact = |text: &str| text.split_whitespace().collect::<String>();
    let old = compact(include_str!(
        "../funded_flat/before_reachability_release.rs"
    ))
    .replace(
        "paid_plan::before_reachability_release::build(program,function,&mutwork,inventory)",
        "paid_plan::build_retained(program,function,&mutwork,inventory)",
    );
    assert_eq!(compact(current), old);
}
