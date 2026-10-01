//! A physical later Resume and its Completed ACK preserve the actual State.
mod join;
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::TargetAccounting;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveLaterModelResumeReservedV8;

pub(super) fn run(
    journal: &SourceOwnedWaitJournalV8,
    reserved: LiveLaterModelResumeReservedV8<'_>,
    expected_state: &serde_json::Value,
    accounting: &TargetAccounting,
    weak: &[std::sync::Weak<[u8]>],
    fault: u8,
) {
    let sequence = reserved.sequence();
    let entries = crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_resume_entries_v8();
    let (funding, stages, turn, _) = journal
        .begin_session()
        .unwrap()
        .inventory
        .continued_model_facts()
        .unwrap();
    assert_eq!(turn, 2);
    let resumed = reserved
        .resume_actual()
        .unwrap_or_else(|_| panic!("one-use physical turn-two Resume"));
    assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_resume_entries_v8(), entries + 1);
    let (state, consumed, result_digest) = resumed.test_result();
    assert_eq!(&state, expected_state);
    assert!(consumed > 0);
    assert!(weak.iter().any(|owner| owner.strong_count() == 1));
    let completed = resumed
        .prepare_completed()
        .unwrap_or_else(|_| panic!("Completed from successful physical State"));
    let row = completed.selected().clone();
    assert!(
        matches!(&row, EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitCompleted { turn: 2, attempt: 0, reservation, result_digest: digest, consumed: actual_consumed, .. }) if *reservation as usize == sequence - 1 && digest == &result_digest && *actual_consumed == consumed)
    );
    let before = journal.lease.try_borrow_mut().unwrap().read().unwrap();
    if fault == 15 {
        journal
            .lease
            .try_borrow_mut()
            .unwrap()
            .test_fail_before_write(sequence + 1);
        let failed = journal
            .begin_session()
            .unwrap()
            .append_owned_later_model_completed(completed)
            .err()
            .expect("turn-two Completed prewrite refusal");
        assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::continued_model::later_completed::LiveOwnedLaterModelCompletedAppendFailureV8::Append { .. }));
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
        assert!(journal.hold().is_err());
        assert!(
            weak.iter().any(|owner| owner.strong_count() == 1),
            "refused Completed retains the real resumed State"
        );
        drop(failed);
    } else {
        let completed = journal
            .begin_session()
            .unwrap()
            .append_owned_later_model_completed(completed)
            .unwrap_or_else(|_| panic!("turn-two Completed physical ACK"))
            .advance_continued_model()
            .unwrap_or_else(|_| panic!("same resumed State after Completed ACK"));
        completed.validate_live().unwrap();
        assert_eq!(completed.sequence(), sequence + 1);
        assert_eq!(completed.test_accounting(), accounting);
        let current = journal.begin_session().unwrap();
        let (after_funding, after_stages, turn, last) =
            current.inventory.continued_model_facts().unwrap();
        assert_eq!((after_funding, after_stages, turn), (funding, stages, 2));
        assert_eq!(last, &row);
        assert!(weak.iter().any(|owner| owner.strong_count() == 1));
        join::run(journal, completed, weak, fault);
    }
    assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_resume_entries_v8(), entries + 1, "Completed cannot replay source Resume");
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_physical_resume_and_completed_ack() {
    continued_reduce_chain_step_ack(0, true);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_completed_prewrite_refusal_retains_resumed_owner() {
    continued_reduce_chain_step_ack(15, true);
}
