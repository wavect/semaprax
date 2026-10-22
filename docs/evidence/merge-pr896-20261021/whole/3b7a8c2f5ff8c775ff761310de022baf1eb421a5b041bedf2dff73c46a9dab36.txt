//! The complete immediate old warm-zero owner, with its original body unchanged.
use super::{Canonicalizer, Rc, ReplayBudget};
use crate::bounded_output::{active_remaining, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
fn fixture() -> super::ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(super::super::super::lexical_plan_tests::WHILE)
        .1
}
#[test]
fn held_cached_closure_keeps_original_one_work_unit_and_same_result_without_auth_debit() {
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let (((), allocation), refused, _, first) = with_limit_usage_refusal(1 << 20, || {
        measure(|| {
            let mut c = Canonicalizer::new(&automaton, sink);
            let mut budget = ReplayBudget::new();
            let held = c
                .closure(&function, roots[3], &mut budget)
                .unwrap()
                .unwrap();
            let work = budget.remaining;
            let paid = active_remaining();
            let at = current_metrics().allocated;
            for offset in 1..=8 {
                let again = c
                    .closure(&function, roots[3], &mut budget)
                    .unwrap()
                    .unwrap();
                assert!(Rc::ptr_eq(&held, &again));
                assert_eq!(budget.remaining, work - offset);
                assert_eq!(active_remaining(), paid);
                assert_eq!(current_metrics().allocated, at);
            }
            drop(held);
            drop(c);
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
}
