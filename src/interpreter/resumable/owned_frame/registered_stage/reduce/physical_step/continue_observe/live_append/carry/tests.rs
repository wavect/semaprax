//! Independent private owner preparation; test-only original ACK fixture.
use super::super::super::tests::with_continue;
use super::*;

#[test]
fn owned_turn_carry_prepares_same_actual_state_without_eval_or_allocation() {
    with_continue(|committed, weak, _, _| {
        let mut budget = OwnedFrameBudget::new(1000).unwrap();
        let outcome = observe_continued_owned_state_v2(committed, &mut budget, || true)
            .unwrap_or_else(|_| panic!("actual Observe"));
        let consumed = budget.consumed();
        let prepared = prepare_continued_copy_wait_v8(outcome)
            .unwrap_or_else(|_| panic!("same-root helper preparation"));
        assert_eq!(prepared.consumed, consumed);
        assert_eq!(budget.consumed(), consumed);
        assert_eq!(weak.strong_count(), 1);
        assert_eq!(prepared.turn, 1);
        prepared.validate_store().unwrap();
        drop(prepared);
        assert!(weak.upgrade().is_none());
    });
}
#[test]
fn owned_turn_carry_preparation_refuses_cancel_and_retains_actual_owner() {
    with_continue(|committed, weak, cancellation, _| {
        let mut budget = OwnedFrameBudget::new(1000).unwrap();
        let outcome = observe_continued_owned_state_v2(committed, &mut budget, || true)
            .unwrap_or_else(|_| panic!("actual Observe"));
        let consumed = budget.consumed();
        cancellation.cancel();
        let rejected = prepare_continued_copy_wait_v8(outcome)
            .err()
            .expect("cancelled before handoff");
        assert!(matches!(
            &rejected,
            ContinuedWaitPreparationFailureV8::Before { .. }
        ));
        assert_eq!(budget.consumed(), consumed);
        assert_eq!(weak.strong_count(), 1);
        drop(rejected);
        assert!(weak.upgrade().is_none());
    });
}
