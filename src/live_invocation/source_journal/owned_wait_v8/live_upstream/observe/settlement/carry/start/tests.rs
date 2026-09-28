//! Real ACKs precede the sole helper entry; actual park then selects Prepared.
use super::super::super::tests::{ack, with_continued};
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
                let source_entries = start_entries();
                {
                    let mut lease = journal.test_observe_lease().borrow_mut();
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
                assert_eq!(
                    start_entries(),
                    source_entries,
                    "no helper entry before full-F ACK"
                );
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        }
    }
}

fn reserved<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: LiveOwnedObserveSettlementAppendV8<'j>,
) -> LiveContinuedStartPhaseV8<'j> {
    let created = acknowledge(
        journal,
        carried(journal, owner)
            .prepare_start_created()
            .unwrap_or_else(|_| panic!("Created")),
    );
    acknowledge(
        journal,
        created
            .prepare_start_reservation()
            .unwrap_or_else(|_| panic!("Reserved")),
    )
}
fn start_entries() -> usize {
    crate::interpreter::resumable::owned_frame::registered_stage::reduce::PreparedHeldContinuedWaitV2::test_start_entries()
}
#[test]
fn owned_continued_start_actual_park_and_prepared_ack_match_ordinary_helper() {
    with_continued(false, |journal, owner, weak, ledger, _, _, _| {
        let entries = start_entries();
        let reserved = reserved(journal, owner);
        assert_eq!(
            start_entries(),
            entries,
            "full-F ACK does not execute source"
        );
        let start_seq = reserved.sequence();
        let (reserved_total, acknowledged_consumed, _) = journal
            .begin_session()
            .unwrap()
            .test_observe_inventory()
            .continued_start_checkpoint_basis(&reserved.owner.observation)
            .unwrap();
        let started = reserved
            .enter_actual_source()
            .unwrap_or_else(|_| panic!("sole actual entry"));
        assert_eq!(start_entries(), entries + 1);
        let (state, request, ordinary_steps) = started.owner.test_ordinary_start();
        assert_eq!(started.consumed(), Some(ordinary_steps as u64));
        assert_eq!(
            crate::interpreter::resumable::checkpoint::channel_json(&request),
            started.observation().copy_arguments()[0]["value"]
        );
        assert_eq!(started.owner.test_accounting(), &ledger);
        let copy_arguments = started.observation().copy_arguments().clone();
        let selected = started
            .prepare_checkpoint()
            .unwrap_or_else(|_| panic!("actual checkpoint"));
        let EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitPrepared {
            turn,
            attempt,
            reservation,
            checkpoint,
            consumed,
            ..
        }) = selected.selected()
        else {
            panic!("Prepared")
        };
        assert_eq!(
            (*turn, *attempt, *reservation),
            (1, 0, u32::try_from(start_seq - 1).unwrap())
        );
        assert_eq!(*consumed, ordinary_steps as u64);
        let bytes = crate::live_invocation::identity::unhex(checkpoint).unwrap();
        assert!(bytes.len() <= 65536 && bytes.ends_with(b"\n"));
        let envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let payload = &envelope["payload"];
        assert_eq!(payload["sequence"], start_seq as u64);
        assert_eq!(payload["reserved_total"], reserved_total);
        assert_eq!(
            payload["consumed_total"],
            acknowledged_consumed + ordinary_steps as u64
        );
        assert_eq!(
            payload["request"],
            crate::interpreter::resumable::checkpoint::channel_json(&request)
        );
        assert_eq!(
            serde_json::json!({"declaration":payload["frame"]["owned_root"]["declaration"],"fields":payload["frame"]["owned_root"]["fields"]}),
            state
        );
        assert_eq!(payload["frame"]["copy_arguments"], copy_arguments);
        let prepared = journal
            .begin_session()
            .unwrap()
            .append_owned_continued_prepared(selected)
            .unwrap_or_else(|_| panic!("real Prepared ACK"))
            .advance_continued_prepared()
            .unwrap_or_else(|_| panic!("actual Prepared owner"));
        prepared.validate_live().unwrap();
        assert_eq!(prepared.test_sequence(), start_seq + 1);
        assert_eq!(prepared.test_accounting(), &ledger);
        assert_eq!(
            start_entries(),
            entries + 1,
            "checkpoint and ACK do not replay"
        );
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        drop(prepared);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[cfg(unix)]
#[test]
fn owned_continued_start_prepared_ack_faults_retain_actual_park_without_reentry() {
    for mode in 0..4 {
        with_continued(false, |journal, owner, weak, _, _, _, _| {
            let entries = start_entries();
            let started = reserved(journal, owner)
                .enter_actual_source()
                .unwrap_or_else(|_| panic!("actual source"));
            assert_eq!(start_entries(), entries + 1);
            let selected = started
                .prepare_checkpoint()
                .unwrap_or_else(|_| panic!("Prepared"));
            let number = selected.sequence() + 1;
            {
                let mut lease = journal.test_observe_lease().borrow_mut();
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
                .append_owned_continued_prepared(selected)
                .err()
                .expect("real physical fault");
            assert_eq!(start_entries(), entries + 1);
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            drop(failure);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}

#[test]
fn owned_continued_start_cancel_at_source_and_prepared_boundaries_retains_owner() {
    for boundary in 0..3 {
        with_continued(false, |journal, owner, weak, ledger, _, _, cancel| {
            let entries = start_entries();
            let reserved = reserved(journal, owner);
            if boundary == 0 {
                cancel.cancel();
                let failed = reserved
                    .enter_actual_source()
                    .err()
                    .expect("cancelled before evaluator");
                assert_eq!(start_entries(), entries);
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                drop(failed);
            } else {
                let started = reserved
                    .enter_actual_source()
                    .unwrap_or_else(|_| panic!("actual helper"));
                assert_eq!(start_entries(), entries + 1);
                assert_eq!(started.owner.test_accounting(), &ledger);
                if boundary == 1 {
                    cancel.cancel();
                    let failed = started
                        .prepare_checkpoint()
                        .err()
                        .expect("guard lost after source");
                    assert_eq!(start_entries(), entries + 1);
                    assert!(weak.iter().any(|w| w.strong_count() == 1));
                    assert!(journal.hold().is_err());
                    drop(failed);
                } else {
                    let selected = started
                        .prepare_checkpoint()
                        .unwrap_or_else(|_| panic!("checkpoint"));
                    let prepared = journal
                        .begin_session()
                        .unwrap()
                        .append_owned_continued_prepared(selected)
                        .unwrap_or_else(|_| panic!("ACK"))
                        .advance_continued_prepared()
                        .unwrap_or_else(|_| panic!("Prepared"));
                    cancel.cancel();
                    assert!(prepared.validate_live().is_err());
                    assert_eq!(prepared.test_accounting(), &ledger);
                    assert_eq!(start_entries(), entries + 1);
                    assert!(weak.iter().any(|w| w.strong_count() == 1));
                    assert!(journal.hold().is_err());
                    assert!(journal.begin_session().is_err());
                    drop(prepared);
                }
            }
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
