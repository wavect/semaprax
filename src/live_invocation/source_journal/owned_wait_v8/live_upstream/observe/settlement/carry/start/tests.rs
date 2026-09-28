//! These ACKs fund entry; this prelude performs no new helper evaluation.
use super::super::super::super::tests::{ack, with_continued};
use super::*;

fn carried<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: LiveOwnedObserveSettlementAppendV8<'j>,
) -> LiveContinuedWaitV8<'j> {
    let settled = ack(journal, owner);
    let observed = settled
        .prepare_turn_observed()
        .unwrap_or_else(|_| panic!("actual observed row"));
    ack(journal, observed)
        .into_continued_wait()
        .unwrap_or_else(|_| panic!("actual carried State"))
}
fn acknowledge<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: LiveOwnedContinuedStartAppendV8<'j>,
) -> LiveContinuedStartPhaseV8<'j> {
    journal
        .begin_session()
        .unwrap()
        .append_owned_continued_start(owner)
        .unwrap_or_else(|_| panic!("actual Start lineage ACK"))
        .advance_continued_start()
        .unwrap_or_else(|_| panic!("actual ACK consumer"))
}
#[test]
fn owned_continued_start_created_then_full_f_ack_preserve_owner_and_ledger() {
    with_continued(false, |journal, owner, weak, ledger, _, _, _| {
        let owner = carried(journal, owner);
        let before = journal.begin_session().unwrap().sequence();
        let entries = crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8();
        let created = owner
            .prepare_start_created()
            .unwrap_or_else(|_| panic!("Created"));
        assert!(matches!(
            created.selected(),
            EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitCreated {
                turn: 1,
                attempt: 0,
                ..
            })
        ));
        let created = acknowledge(journal, created);
        assert_eq!(created.sequence(), before + 1);
        let reservation = created
            .prepare_start_reservation()
            .unwrap_or_else(|_| panic!("original Start"));
        let (_, execution) = journal.context().ready_runtime().unwrap();
        assert!(
            matches!(reservation.selected(), EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitReserved{turn:1,attempt:0,phase:journal_model::PhaseV8::Start,replay_of:None,fuel,..}) if *fuel == execution.evaluation_fuel() as u64)
        );
        let reserved = acknowledge(journal, reservation);
        reserved.validate_live().unwrap();
        assert_eq!(reserved.sequence(), before + 2);
        assert_eq!(reserved.continued().unwrap().test_accounting(), &ledger);
        assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8(), entries);
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        drop(reserved);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[test]
fn owned_continued_start_wrong_phase_retains_real_owner_without_append() {
    with_continued(false, |journal, owner, weak, ledger, _, _, _| {
        let owner = carried(journal, owner);
        let created = acknowledge(
            journal,
            owner
                .prepare_start_created()
                .unwrap_or_else(|_| panic!("Created")),
        );
        let reserved = acknowledge(
            journal,
            created
                .prepare_start_reservation()
                .unwrap_or_else(|_| panic!("Reserved")),
        );
        let sequence = reserved.sequence();
        let failure = reserved
            .prepare_start_reservation()
            .err()
            .expect("one original Start only");
        let LiveContinuedStartFailureV8::Selection { owner, error } = &failure else {
            panic!("phase owner retained")
        };
        assert_eq!(*error, SourceJournalError::Order);
        assert_eq!(owner.continued().unwrap().test_accounting(), &ledger);
        assert_eq!(journal.begin_session().unwrap().sequence(), sequence);
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        drop(failure);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[cfg(unix)]
#[test]
fn owned_continued_start_actual_ack_faults_retain_owner_and_retire_all_handles() {
    for reserved_row in [false, true] {
        for mode in 0..4 {
            with_continued(false, |journal, owner, weak, _, _, _, _| {
                let owner = carried(journal, owner);
                let mut selected = owner
                    .prepare_start_created()
                    .unwrap_or_else(|_| panic!("Created"));
                if reserved_row {
                    selected = acknowledge(journal, selected)
                        .prepare_start_reservation()
                        .unwrap_or_else(|_| panic!("Reserved"));
                }
                let number = selected.sequence() + 1;
                let entries = crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8();
                {
                    let mut lease = journal.lease.borrow_mut();
                    match mode {
                        0 => lease.test_fail_before_write(number),
                        1 => lease.test_fail_after_write(number),
                        2 => lease.test_fail_before_sync(number),
                        _ => lease.test_fail_after_sync(number),
                    }
                }
                let failure = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continued_start(selected)
                    .err()
                    .expect("physical fault");
                assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8(), entries);
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        }
    }
}
