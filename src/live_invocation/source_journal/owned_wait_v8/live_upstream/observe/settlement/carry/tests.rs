//! Genuine ACKed continuation boundary; no next evaluator or host entry.
use super::super::tests::{ack, with_continued};
use super::*;

#[test]
fn owned_turn_carry_keeps_actual_state_ledger_and_current_ack() {
    with_continued(false, |journal, owner, weak, ledger, _, _, _| {
        let settled = ack(journal, owner);
        let settled = ack(
            journal,
            settled
                .prepare_turn_observed()
                .unwrap_or_else(|_| panic!("TurnObserved")),
        );
        let sequence = journal.begin_session().unwrap().sequence();
        let entries = crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8();
        let carried = settled
            .into_continued_wait()
            .unwrap_or_else(|_| panic!("actual carry"));
        carried.validate_live().unwrap();
        assert_eq!(journal.begin_session().unwrap().sequence(), sequence);
        assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8(), entries);
        let LiveObserveSettlementOwnerV8::Continued(actual) = &carried.owner.owner else {
            panic!()
        };
        assert_eq!(actual.test_accounting(), &ledger);
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        drop(carried);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[test]
fn owned_turn_carry_needs_actual_turnobserved_and_keeps_failed_state() {
    for failed in [false, true] {
        with_continued(failed, |journal, owner, weak, ledger, _, _, _| {
            let settled = ack(journal, owner);
            let rejected = settled
                .into_continued_wait()
                .err()
                .expect("no success TurnObserved ACK");
            assert_eq!(rejected.error, SourceJournalError::Order);
            let LiveObserveSettlementOwnerV8::Continued(actual) = &rejected.owner.owner else {
                panic!()
            };
            assert_eq!(actual.test_accounting(), &ledger);
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            assert!(
                journal.hold().is_ok(),
                "actual failed Observe remains available to its cleanup producer"
            );
            drop(rejected);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
fn owned_turn_carry_postack_cancel_retires_without_new_source_work() {
    with_continued(false, |journal, owner, weak, ledger, _, _, cancel| {
        let settled = ack(journal, owner);
        let settled = ack(
            journal,
            settled
                .prepare_turn_observed()
                .unwrap_or_else(|_| panic!("TurnObserved")),
        );
        let entries = crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8();
        cancel.cancel();
        let rejected = settled
            .into_continued_wait()
            .err()
            .expect("cancelled carry");
        let LiveObserveSettlementOwnerV8::Continued(actual) = &rejected.owner.owner else {
            panic!()
        };
        assert_eq!(actual.test_accounting(), &ledger);
        assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8(), entries);
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        assert!(journal.hold().is_err());
        assert!(journal.begin_session().is_err());
        drop(rejected);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
