//! Turn-two physical checkpoint and exact Prepared ACK regressions.
mod completed;
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveLaterStartedPhaseV8;

pub(super) fn run(
    journal: &SourceOwnedWaitJournalV8,
    entered: LiveLaterStartedPhaseV8<'_>,
    adapter: &mut StreamingSourceProposalAdapter<'_>,
    fault: u8,
    weak: &[std::sync::Weak<[u8]>],
) {
    let expected_state = entered.test_ordinary_start().0;
    let start_sequence = entered.sequence();
    let accounting = *entered.test_accounting();
    let prepared = entered
        .prepare_checkpoint()
        .unwrap_or_else(|_| panic!("turn-two actual parked checkpoint"));
    assert!(
        matches!(prepared.selected(), EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitPrepared { turn: 2, attempt: 0, reservation, checkpoint, consumed, .. }) if usize::try_from(*reservation).ok() == Some(start_sequence - 1) && !checkpoint.is_empty() && *consumed > 0)
    );
    let before = journal.lease.try_borrow_mut().unwrap().read().unwrap();
    if fault == 10 {
        journal
            .lease
            .try_borrow_mut()
            .unwrap()
            .test_fail_before_write(start_sequence + 1);
        let failed = journal
            .begin_session()
            .unwrap()
            .append_owned_later_prepared(prepared)
            .err()
            .expect("turn-two Prepared prewrite refusal");
        assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::continued_prepared::later::LiveOwnedLaterPreparedAppendFailureV8::Append { .. }));
        assert_eq!(
            journal
                .lease
                .try_borrow()
                .unwrap()
                .test_persisted_snapshot()
                .unwrap(),
            before
        );
        assert!(journal.begin_session().is_err());
        drop(failed);
    } else {
        let acknowledged = journal
            .begin_session()
            .unwrap()
            .append_owned_later_prepared(prepared)
            .unwrap_or_else(|_| panic!("turn-two Prepared physical ACK"))
            .advance_continued_prepared()
            .unwrap_or_else(|_| panic!("turn-two Prepared retained owner"));
        acknowledged.validate_live().unwrap();
        assert_eq!(acknowledged.sequence(), start_sequence + 1);
        assert_eq!(acknowledged.test_accounting(), &accounting);
        let current = journal.begin_session().unwrap();
        let (_, _, turn, row) = current.inventory.continued_prepared_facts().unwrap();
        assert_eq!(turn, 2);
        assert!(matches!(row, EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitPrepared { turn: 2, .. })));
        let intent = acknowledged
            .prepare_model_intent(adapter)
            .unwrap_or_else(|_| panic!("turn-two physical Model request"));
        assert!(matches!(
            intent.selected(),
            EntryV8::Ordinary(SourceJournalEntry::AttemptIntent {
                turn: 2,
                attempt: 0,
                ..
            })
        ));
        let before_intent = journal.lease.try_borrow_mut().unwrap().read().unwrap();
        if fault == 11 {
            journal
                .lease
                .try_borrow_mut()
                .unwrap()
                .test_fail_before_write(start_sequence + 2);
            let failed = journal
                .begin_session()
                .unwrap()
                .append_owned_later_model_intent(intent)
                .err()
                .expect("turn-two Model Intent prewrite refusal");
            assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::continued_model::later::LiveOwnedLaterModelIntentAppendFailureV8::Append { .. }));
            assert_eq!(
                journal
                    .lease
                    .try_borrow()
                    .unwrap()
                    .test_persisted_snapshot()
                    .unwrap(),
                before_intent
            );
            assert!(journal.begin_session().is_err());
            drop(failed);
        } else {
            let model = journal
                .begin_session()
                .unwrap()
                .append_owned_later_model_intent(intent)
                .unwrap_or_else(|_| panic!("turn-two Model Intent ACK"))
                .advance_continued_model()
                .unwrap_or_else(|_| panic!("retained turn-two Model owner"));
            model.validate_live().unwrap();
            assert_eq!(model.sequence(), start_sequence + 2);
            assert_eq!(model.test_accounting(), &accounting);
            let dispatched = model
                .dispatch_model(adapter)
                .unwrap_or_else(|_| panic!("turn-two guarded SDK dispatch"));
            assert!(dispatched.test_dispatched());
            let settlement = dispatched
                .prepare_settlement()
                .unwrap_or_else(|_| panic!("turn-two actual SDK settlement selector"));
            assert!(matches!(
                settlement.selected(),
                EntryV8::Ordinary(SourceJournalEntry::AttemptSettled {
                    turn: 2,
                    attempt: 0,
                    ..
                })
            ));
            let before_settlement = journal.lease.try_borrow_mut().unwrap().read().unwrap();
            if fault == 12 {
                journal
                    .lease
                    .try_borrow_mut()
                    .unwrap()
                    .test_fail_before_write(start_sequence + 3);
                let failed = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_later_model_settlement(settlement)
                    .err()
                    .expect("turn-two SDK settlement prewrite refusal");
                assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::continued_model::later_settlement::LiveOwnedLaterModelSettlementAppendFailureV8::Append { .. }));
                assert_eq!(
                    journal
                        .lease
                        .try_borrow()
                        .unwrap()
                        .test_persisted_snapshot()
                        .unwrap(),
                    before_settlement
                );
                assert!(journal.begin_session().is_err());
                drop(failed);
            } else {
                let acknowledged = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_later_model_settlement(settlement)
                    .unwrap_or_else(|_| panic!("turn-two SDK settlement physical ACK"))
                    .advance_continued_model()
                    .unwrap_or_else(|_| panic!("turn-two settled owner"));
                acknowledged.validate_live().unwrap();
                assert_eq!(acknowledged.sequence(), start_sequence + 3);
                assert_eq!(acknowledged.test_accounting(), &accounting);
                let current = journal.begin_session().unwrap();
                let (_, _, turn, row) = current.inventory.continued_model_facts().unwrap();
                assert_eq!(turn, 2);
                assert!(matches!(
                    row,
                    EntryV8::Ordinary(SourceJournalEntry::AttemptSettled {
                        turn: 2,
                        attempt: 0,
                        ..
                    })
                ));
                let usage = acknowledged
                    .prepare_usage()
                    .unwrap_or_else(|_| panic!("turn-two actual SDK usage selector"));
                assert!(
                    matches!(usage.selected(), EntryV8::Ordinary(SourceJournalEntry::AttemptUsage { turn: 2, attempt: 0, reported: Some(value) }) if value.input == Some(2) && value.output == Some(3) && value.total == Some(5))
                );
                let before_usage = journal.lease.try_borrow_mut().unwrap().read().unwrap();
                if fault == 13 {
                    journal
                        .lease
                        .try_borrow_mut()
                        .unwrap()
                        .test_fail_before_write(start_sequence + 4);
                    let failed = journal
                        .begin_session()
                        .unwrap()
                        .append_owned_later_model_usage(usage)
                        .err()
                        .expect("turn-two Usage prewrite refusal");
                    assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::continued_model::later_usage::LiveOwnedLaterModelUsageAppendFailureV8::Append { .. }));
                    assert_eq!(
                        journal
                            .lease
                            .try_borrow()
                            .unwrap()
                            .test_persisted_snapshot()
                            .unwrap(),
                        before_usage
                    );
                    assert!(journal.begin_session().is_err());
                    drop(failed);
                } else {
                    let acknowledged = journal
                        .begin_session()
                        .unwrap()
                        .append_owned_later_model_usage(usage)
                        .unwrap_or_else(|_| panic!("turn-two Usage physical ACK"))
                        .advance_continued_model()
                        .unwrap_or_else(|_| panic!("turn-two Usage owner"));
                    acknowledged.validate_live().unwrap();
                    assert_eq!(acknowledged.sequence(), start_sequence + 4);
                    assert_eq!(acknowledged.test_accounting(), &accounting);
                    let current = journal.begin_session().unwrap();
                    let (reserved_before, stages_before, turn, row) =
                        current.inventory.continued_model_facts().unwrap();
                    assert_eq!(turn, 2);
                    assert!(matches!(
                        row,
                        EntryV8::Ordinary(SourceJournalEntry::AttemptUsage {
                            turn: 2,
                            attempt: 0,
                            ..
                        })
                    ));
                    let resume = acknowledged
                        .prepare_resume_reservation()
                        .unwrap_or_else(|_| {
                            panic!("turn-two physical Resume reservation selector")
                        });
                    let (_, execution) = journal.context().test_runtime_execution();
                    let fuel = execution.evaluation_fuel();
                    assert!(
                        matches!(resume.selected(), EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitReserved { turn: 2, attempt: 0, phase: crate::live_invocation::source_journal::owned_wait_v8::model::PhaseV8::Resume, replay_of: None, fuel: reserved_fuel, .. }) if *reserved_fuel == fuel as u64)
                    );
                    let before_resume = journal.lease.try_borrow_mut().unwrap().read().unwrap();
                    if fault == 14 {
                        journal
                            .lease
                            .try_borrow_mut()
                            .unwrap()
                            .test_fail_before_write(start_sequence + 5);
                        let failed = journal
                            .begin_session()
                            .unwrap()
                            .append_owned_later_model_resume(resume)
                            .err()
                            .expect("turn-two Resume reservation prewrite refusal");
                        assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::continued_model::later_resume::LiveOwnedLaterModelResumeAppendFailureV8::Append { .. }));
                        assert_eq!(
                            journal
                                .lease
                                .try_borrow()
                                .unwrap()
                                .test_persisted_snapshot()
                                .unwrap(),
                            before_resume
                        );
                        assert!(journal.begin_session().is_err());
                        drop(failed);
                    } else {
                        let acknowledged = journal
                            .begin_session()
                            .unwrap()
                            .append_owned_later_model_resume(resume)
                            .unwrap_or_else(|_| panic!("turn-two Resume reservation physical ACK"))
                            .advance_continued_model()
                            .unwrap_or_else(|_| panic!("turn-two Resume reservation owner"));
                        acknowledged.validate_live().unwrap();
                        assert_eq!(acknowledged.sequence(), start_sequence + 5);
                        assert_eq!(acknowledged.test_accounting(), &accounting);
                        let current = journal.begin_session().unwrap();
                        let (reserved, stages, turn, row) =
                            current.inventory.continued_model_facts().unwrap();
                        assert_eq!(
                            (reserved, stages, turn),
                            (reserved_before + fuel as u64, stages_before, 2)
                        );
                        assert!(matches!(row, EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitReserved { phase: crate::live_invocation::source_journal::owned_wait_v8::model::PhaseV8::Resume, turn: 2, attempt: 0, replay_of: None, .. })));
                        completed::run(
                            journal,
                            acknowledged,
                            &expected_state,
                            &accounting,
                            weak,
                            fault,
                        );
                    }
                }
            }
        }
    }
}

#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_prepared_prewrite_refusal_retains_physical_park() {
    continued_reduce_chain_step_ack(10, true, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_model_intent_prewrite_refusal_retains_physical_park() {
    continued_reduce_chain_step_ack(11, true, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_model_settlement_prewrite_refusal_retains_dispatched_owner() {
    continued_reduce_chain_step_ack(12, true, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_model_usage_prewrite_refusal_retains_physical_owner() {
    continued_reduce_chain_step_ack(13, true, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_resume_reservation_prewrite_refusal_retains_physical_owner() {
    continued_reduce_chain_step_ack(14, true, false);
}
