use super::*;

#[test]
fn owned_continue_actual_state_and_observe_acks_preserve_owner_ledger_and_cumulative_funding() {
    with_moved(|journal, moved, weak, _, _| {
        let before = journal.begin_session().unwrap();
        let (r, s, turn, _) = before.inventory.continuation_facts().unwrap();
        let (ordinary_observation, ordinary_consumed) = moved.test_observe_oracle();
        let ledger = *moved.accounting();
        let fuel = journal.context().ordinary().max_steps_per_stage().unwrap() as u64;
        let selected = moved
            .prepare_continue()
            .unwrap_or_else(|_| panic!("actual Continue selection"));
        let current = ack(journal, selected)
            .advance_continue()
            .unwrap_or_else(|_| panic!("actual StateCommitted"));
        let LiveContinueAcknowledgedV8::State(state) = current else {
            panic!("State owner")
        };
        let after_state = journal.begin_session().unwrap();
        let (nr, ns, next, _) = after_state.inventory.continuation_facts().unwrap();
        assert_eq!((nr, ns, next), (r, s, turn + 1));
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        let observed = ack(
            journal,
            state
                .prepare_observe()
                .unwrap_or_else(|_| panic!("fullF Observe obligation")),
        )
        .advance_continue()
        .unwrap_or_else(|_| panic!("sole actual Observe"));
        let LiveContinueAcknowledgedV8::Observed(observed) = observed else {
            panic!("actual observed/failed owner")
        };
        assert!(observed.is_observed());
        assert_eq!(observed.test_observation(), &ordinary_observation);
        assert_eq!(observed.consumed(), ordinary_consumed);
        assert_eq!(observed.turn(), turn + 1);
        assert_eq!(observed.accounting(), &ledger);
        assert!(observed.consumed() > 0 && observed.consumed() <= fuel as usize);
        let after = journal.begin_session().unwrap();
        let (nr, ns, _, _) = after.inventory.continuation_facts().unwrap();
        assert_eq!((nr, ns), (r + fuel, s + 1));
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        drop(observed);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
        false
    });
}
#[test]
fn owned_continue_cancel_or_expired_clock_before_ack_keeps_real_state_and_zero_new_stage() {
    for expired in [false, true] {
        with_moved(|journal, moved, weak, cancel, clock| {
            let before = journal.begin_session().unwrap();
            let seq = before.sequence();
            let original = moved
                .prepare_continue()
                .unwrap_or_else(|_| panic!("live selector"));
            if expired {
                clock.now.set(
                    journal
                        .context()
                        .ordinary()
                        .deadline_millis()
                        .checked_add(1)
                        .unwrap(),
                );
            } else {
                cancel.cancel();
            }
            let failure = before
                .append_owned_continue(original)
                .err()
                .expect("actual prewrite guard refusal");
            assert!(matches!(
                failure,
                LiveOwnedContinueAppendFailureV8::Before { .. }
            ));
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            assert!(seq > 0);
            drop(failure);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
            false
        });
    }
}
