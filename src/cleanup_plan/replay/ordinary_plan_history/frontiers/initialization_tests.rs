use super::*;

pub(super) fn check_initial_reset_without_heap_allocation() {
    std::thread::spawn(|| {
        let (((), metrics), overflowed, used, refusal) =
            crate::bounded_output::with_limit_usage_refusal(1, || {
                crate::test_allocation_observer::measure(|| {
                    reset();
                    before("first");
                    after();
                    swapped();
                    before("second");
                    after();
                })
            });
        assert!(!overflowed);
        assert_eq!(used, 0);
        assert_eq!(refusal, None);
        assert!(!metrics.invalid);
        assert_eq!(metrics.allocations, 0);
        assert_eq!(metrics.reallocations, 0);
        assert_eq!(metrics.allocated, 0);
        let events = finish();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].phase, "first");
        assert_eq!(events[0].swaps, 0);
        assert_eq!(events[1].phase, "second");
        assert_eq!(events[1].swaps, 1);
        assert!(events
            .iter()
            .all(|event| event.before == 0 && event.after == Some(0)));
        let (((), metrics), overflowed, used, refusal) =
            crate::bounded_output::with_limit_usage_refusal(1, || {
                crate::test_allocation_observer::measure(|| {
                    reset_swaps();
                    before("filtered");
                    after();
                    before("ordinary plan retained drain swap receivers");
                    swapped();
                    after();
                })
            });
        assert!(!overflowed && !metrics.invalid);
        assert_eq!(
            (used, metrics.allocations, metrics.reallocations),
            (0, 0, 0)
        );
        assert_eq!(refusal, None);
        let events = finish();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].phase,
            "ordinary plan retained drain swap receivers"
        );
        let ((), overflowed, used, refusal) =
            crate::bounded_output::with_limit_usage_refusal(1, || {
                reset();
                for _ in 0..4096 {
                    before("complete capacity");
                    after();
                }
                assert!(std::panic::catch_unwind(|| before("one over")).is_err());
            });
        assert!(!overflowed);
        assert_eq!(used, 0);
        assert_eq!(refusal, None);
        let events = finish();
        assert_eq!(events.len(), 4096);
        assert!(events
            .iter()
            .all(|event| event.phase == "complete capacity" && event.after == Some(0)));
    })
    .join()
    .unwrap();
}

#[test]
fn fresh_thread_trace_is_ready_without_prepare_and_keeps_exact_capacity() {
    check_initial_reset_without_heap_allocation();
}
