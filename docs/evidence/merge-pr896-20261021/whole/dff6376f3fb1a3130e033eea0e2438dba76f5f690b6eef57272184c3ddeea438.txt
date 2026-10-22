use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
fn function() -> ResolvedFunction {
    crate::cleanup_plan::replay::factored::lexical_plan_tests::fixture(
        crate::cleanup_plan::replay::factored::lexical_plan_tests::WHILE,
    )
    .1
}

#[test]
fn actual_paid_map_moves_without_allocation_and_preserves_max_zero_routes_and_last_drop() {
    let function = function();
    for width in [0, 1, 8, 64] {
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut map = BTreeMap::new();
                for key in 0..width {
                    budget
                        .reserve_btree_node::<usize, u64>(&function, "paid unit map")
                        .unwrap();
                    map.insert(key, if key % 2 == 0 { u64::MAX } else { 0 });
                }
                budget
                    .reserve_bytes(
                        &function,
                        physical_clone::rc_backing_bytes::<BTreeMap<usize, u64>>(),
                        "paid unit map Rc",
                    )
                    .unwrap();
                let closure = Rc::new(map);
                budget
                    .reserve_bytes(
                        &function,
                        RECEIVER_BYTES,
                        "factored weighted closure receiving view",
                    )
                    .unwrap();
                let before = crate::test_allocation_observer::current_metrics().allocated;
                let work = budget.remaining;
                let returned = receive(&function, Rc::clone(&closure), &mut budget).unwrap();
                assert_eq!(budget.remaining, work - width);
                assert_eq!(
                    crate::test_allocation_observer::current_metrics().allocated,
                    before
                );
                assert!(Rc::ptr_eq(&closure, &returned));
                assert_eq!(Rc::strong_count(&returned), 2);
                drop(closure);
                assert_eq!(Rc::strong_count(&returned), 1);
                assert_eq!(returned.len(), width);
                for (key, routes) in returned.iter() {
                    assert_eq!(*routes, if key % 2 == 0 { u64::MAX } else { 0 });
                }
                drop(returned);
                drop(budget);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
    }
}

#[test]
fn work_refusal_and_preexisting_financial_refusal_cannot_publish_a_map() {
    let function = function();
    let map = Rc::new(BTreeMap::from([(0usize, u64::MAX), (1usize, 0)]));
    for units in [0, 1, 2] {
        let mut budget = ReplayBudget::unobserved();
        budget.remaining = units;
        let result = receive(&function, Rc::clone(&map), &mut budget);
        assert_eq!(result.is_ok(), units == 2);
        assert_eq!(budget.remaining, 0);
    }
    let (_, refused, paid, first) = with_limit_usage_refusal(0, || {
        assert!(!crate::bounded_output::reserve_active_required(1));
        let mut budget = ReplayBudget::new();
        assert!(budget
            .reserve_bytes(
                &function,
                RECEIVER_BYTES,
                "factored weighted closure receiving view"
            )
            .is_err());
        budget.remaining = usize::MAX;
        assert!(receive(&function, Rc::clone(&map), &mut budget).is_err());
    });
    assert!(refused && first.is_some());
    assert_eq!(paid, 0);
    assert_eq!(Rc::strong_count(&map), 1);
}
