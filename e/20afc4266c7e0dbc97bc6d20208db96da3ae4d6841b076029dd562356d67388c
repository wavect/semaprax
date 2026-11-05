//! Test-only lower work limit on the actual production funded receiver.
//! No alternate planner, physical allowance, credit or production cap exists.
use super::*;
use std::cell::Cell;
thread_local! { static LIMIT: Cell<Option<usize>> = const { Cell::new(None) }; }

pub(super) fn apply(work: &mut WorkCounter<'_>) {
    if let Some(limit) = LIMIT.with(Cell::get) {
        work.limit = work.limit.min(limit);
    }
}
#[allow(
    dead_code,
    reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
)]
pub(super) fn with_limit<T>(limit: usize, operation: impl FnOnce() -> T) -> T {
    assert!(limit <= MAX_LOAN_PLAN_WORK_V1);
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            LIMIT.with(|limit| limit.set(self.0));
        }
    }
    let _restore = Restore(LIMIT.with(|current| current.replace(Some(limit))));
    operation()
}

#[test]
fn actual_funded_candidate_false_and_plan_routes_keep_exact_short_guided_full_diagnostics() {
    let scalar = crate::hir::resolve(
        &crate::check(
            "module work.boundary; @id(\"work.scalar\") fn main()->i64 {let value=1; value}",
            "work-boundary.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let (owning, index) = boundary_tests::fixture_with_uses(1, true);
    for (program, index, candidate) in [(&scalar, 0, false), (&owning, index, true)] {
        let function = &program.functions[index];
        // Complete saved CFG path selects its original work diagnostic.
        let (selected, used) = saved_planner::build_cfg_plan_with_work_limit(program, function, 0);
        assert_eq!(used, 1);
        let expected = guidance::work_limit(selected.unwrap_err(), function);
        let (exact, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            work.allocation = Some(&owner);
            assert_eq!(
                paid_candidate::inspect(program, function, &owner, &mut work).unwrap(),
                candidate
            );
            if candidate {
                drop(paid_plan::build(program, function, &mut work).unwrap());
            }
            work.used
        });
        assert!(exact > 0 && !refused);
        for limit in [exact, exact - 1] {
            let (((), observed), refused, _) =
                crate::bounded_output::with_limit_usage(usize::MAX, || {
                    crate::test_allocation_observer::measure(|| {
                        let owner = CfgReservation::acquire().unwrap();
                        let result =
                            crate::loan_plan::funded_flat::with_inventory_work_limit(limit, || {
                                crate::loan_plan::funded_flat::before_inventory_for_test(
                                    program, function, &owner,
                                )
                            });
                        assert!(owner.first().is_none());
                        match result {
                            Ok(plan) => {
                                assert_eq!(limit, exact);
                                assert_eq!(plan, function.loan_plan);
                                drop(plan);
                            }
                            Err(PlannerError::Semantic(error)) => {
                                assert_eq!(limit, exact - 1);
                                assert_eq!(error, expected);
                                let before = crate::test_allocation_observer::current_metrics();
                                drop(error);
                                let after = crate::test_allocation_observer::current_metrics();
                                assert_eq!(after.allocated, before.allocated);
                                assert_eq!(after.allocations, before.allocations);
                            }
                            Err(PlannerError::Physical(_)) => {
                                panic!("work guard must not become a physical tag")
                            }
                        }
                        drop(owner);
                    })
                });
            assert!(!refused && !observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
        // Guard restored: the ordinary current production call uses MAX again.
        let (_, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let plan =
                crate::loan_plan::funded_flat::before_inventory_for_test(program, function, &owner)
                    .unwrap_or_else(|_| panic!("restored production cap"));
            assert_eq!(plan, function.loan_plan);
        });
        assert!(!refused);
    }
}
