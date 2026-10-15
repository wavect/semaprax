use super::*;
use crate::bounded_output::{
    reserve_active_required, with_limit_usage_refusal, with_reservation_stage,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn scenario(
    observe: bool,
    refused: bool,
    unwind: bool,
) -> (
    bool,
    usize,
    Option<super::super::ReservationRefusal>,
    Option<Trace>,
) {
    take_completed();
    let (_, overflow, consumed, refusal) = with_limit_usage_refusal(10, || {
        let guard = observe.then(|| {
            let (guard, allocation) = crate::test_allocation_observer::measure(|| start(17, 19, 7));
            assert_eq!(allocation.allocations, 0);
            guard
        });
        with_reservation_stage(ReservationStage::FreshResolution, || {
            if refused {
                assert!(super::super::set_active_floor(9));
                with_limit_usage_refusal(3, || assert!(reserve_active_required(2)));
                assert!(!reserve_active_required(0));
            } else if unwind {
                let error = catch_unwind(AssertUnwindSafe(|| {
                    with_limit_usage_refusal(3, || {
                        assert!(reserve_active_required(3));
                        std::panic::panic_any(13usize);
                    });
                }));
                assert!(error.is_err());
            } else {
                with_limit_usage_refusal(3, || assert!(reserve_active_required(2)));
                with_reservation_stage(ReservationStage::HirValidation, || {
                    assert!(reserve_active_required(4))
                });
            }
        });
        let (_, allocation) = crate::test_allocation_observer::measure(|| drop(guard));
        assert_eq!(allocation.allocations, 0);
    });
    (overflow, consumed, refusal, take_completed())
}

#[test]
fn exact_budget_stage_trace_preserves_nested_settlement_refusal_and_unwind_without_storage() {
    for (refused, unwind, expected) in [(false, false, 6), (true, false, 2), (false, true, 3)] {
        let plain = scenario(false, refused, unwind);
        let watched = scenario(true, refused, unwind);
        assert_eq!(
            (plain.0, plain.1, plain.2),
            (watched.0, watched.1, watched.2)
        );
        assert!(plain.3.is_none());
        let trace = watched.3.unwrap();
        assert_eq!((trace.sum(), trace.consumed), (expected, expected));
        assert_eq!((trace.prebound, trace.layouts, trace.mode), (17, 19, 7));
        assert_eq!(
            trace.debits[ReservationStage::FreshResolution as usize + 1],
            if !refused && unwind { 3 } else { 2 }
        );
        assert_eq!(
            trace.debits[ReservationStage::HirValidation as usize + 1],
            if refused || unwind { 0 } else { 4 }
        );
    }
    take_completed();
    let error = catch_unwind(AssertUnwindSafe(|| {
        with_limit_usage_refusal(10, || {
            let _guard = start(0, 0, 6);
            assert!(reserve_active_required(3));
            std::panic::panic_any(17usize);
        });
    }));
    assert!(error.is_err());
    assert_eq!(take_completed().unwrap().consumed, 3);
    // Unwind restored the inactive observer; an unrelated ledger is not traced.
    with_limit_usage_refusal(10, || assert!(reserve_active_required(1)));
    assert!(take_completed().is_none());
    // Snapshot selection mirrors the existing max-consumed-attempt policy.
    take_completed();
    for bytes in [7, 3] {
        with_limit_usage_refusal(10, || {
            let _guard = start(bytes, 0, 6);
            assert!(reserve_active_required(bytes));
        });
    }
    assert_eq!(take_completed().unwrap().consumed, 7);
}

#[test]
fn operation_guards_keep_complete_untagged_refusal_and_actual_zero_allocation() {
    for tagged in [false, true] {
        take_completed();
        let first_line = Cell::new(0);
        let (((), metrics), overflow, used, refusal) = with_limit_usage_refusal(3, || {
            let _trace = start(0, 0, 6);
            crate::test_allocation_observer::measure(|| {
                let _operation = tagged.then(|| operation_guard(Operation::TextContains));
                assert!(reserve_active_required(3));
                first_line.set(line!() + 1);
                assert!(!reserve_active_required(1));
                assert!(!reserve_active_required(0));
            })
        });
        assert_eq!((overflow, used), (true, 3));
        assert!(!metrics.invalid);
        assert_eq!(
            (metrics.allocations, metrics.allocated, metrics.live),
            (0, 0, 0)
        );
        let first = refusal.unwrap();
        assert_eq!(
            (first.requested, first.remaining, first.floor, first.stage),
            (1, 0, 0, None)
        );
        let trace = take_completed().unwrap();
        let site = trace.first_site.unwrap();
        assert_eq!((site.file(), site.line()), (file!(), first_line.get()));
        let operation = trace.first_operation_refusal.unwrap();
        assert_eq!(
            (
                operation.requested,
                operation.remaining,
                operation.floor,
                operation.stage
            ),
            (first.requested, first.remaining, first.floor, first.stage)
        );
        assert_eq!(
            operation.operation,
            tagged.then_some(Operation::TextContains)
        );
        assert_eq!((trace.sum(), trace.consumed), (3, 3));
    }
}

#[test]
fn operation_scope_restores_after_unwind_and_nested_foreign_budget_cannot_tag_parent() {
    take_completed();
    let foreign_line = Cell::new(0);
    let parent_line = Cell::new(0);
    let (_, overflow, used, first) = with_limit_usage_refusal(2, || {
        let _trace = start(0, 0, 6);
        let _parent = operation_guard(Operation::CoreValidation);
        let error = catch_unwind(AssertUnwindSafe(|| {
            let _nested = operation_guard(Operation::TextSubstitution);
            std::panic::panic_any(23usize);
        }));
        assert!(error.is_err());
        with_limit_usage_refusal(1, || {
            // This operation guard authenticates the child budget, which is
            // not the observed parent. Its refusal must not enter the trace.
            let _foreign = operation_guard(Operation::Lifecycle);
            foreign_line.set(line!() + 1);
            assert!(!reserve_active_required(2));
        });
        assert!(reserve_active_required(2));
        parent_line.set(line!() + 1);
        assert!(!reserve_active_required(1));
        let _later = operation_guard(Operation::ScopeCheckpoint);
        assert!(!reserve_active_required(99));
    });
    assert!(overflow && used == 2);
    assert_eq!(first.unwrap().requested, 1);
    let trace = take_completed().unwrap();
    assert_eq!(
        trace.first_operation_refusal.unwrap().operation,
        Some(Operation::CoreValidation)
    );
    assert_eq!(trace.first_operation_refusal.unwrap().requested, 1);
    assert_eq!((trace.sum(), trace.consumed), (2, 2));
    let site = trace.first_site.unwrap();
    assert_eq!((site.file(), site.line()), (file!(), parent_line.get()));
    assert_ne!(site.line(), foreign_line.get());
    assert_eq!(take_last_completed().unwrap().first_site, Some(site));
    // The observer and operation restored before this unrelated ledger.
    with_limit_usage_refusal(0, || assert!(!reserve_active_required(1)));
    assert!(take_completed().is_none());
    assert!(take_last_completed().is_none());
    // A new successful owner must not inherit the settled refusal site.
    with_limit_usage_refusal(1, || {
        let _trace = start(0, 0, 6);
        assert!(reserve_active_required(1));
    });
    assert!(take_completed().unwrap().first_site.is_none());
    assert!(take_last_completed().unwrap().first_site.is_none());
}

#[test]
fn last_attempt_is_distinct_from_maximum_consumed_and_does_not_allocate() {
    take_completed();
    take_last_completed();
    for (mode, used) in [(2, 8), (6, 3)] {
        let (((), metrics), overflow, paid, refusal) = with_limit_usage_refusal(10, || {
            crate::test_allocation_observer::measure(|| {
                let _trace = start(0, 0, mode);
                assert!(reserve_active_required(used));
            })
        });
        assert!(!overflow && refusal.is_none());
        assert_eq!(paid, used);
        assert!(!metrics.invalid);
        assert_eq!(
            (metrics.allocations, metrics.allocated, metrics.live),
            (0, 0, 0)
        );
    }
    let maximum = take_completed().unwrap();
    let last = take_last_completed().unwrap();
    assert_eq!((maximum.mode, maximum.consumed), (2, 8));
    assert_eq!((last.mode, last.consumed), (6, 3));
    assert!(maximum.first_site.is_none() && last.first_site.is_none());
    assert!(take_completed().is_none() && take_last_completed().is_none());
}
