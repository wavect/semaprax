//! Turn-two physical checkpoint and exact Prepared ACK regressions.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveLaterStartedPhaseV8;

pub(super) fn run(
    journal: &SourceOwnedWaitJournalV8,
    entered: LiveLaterStartedPhaseV8<'_>,
    fault: u8,
) {
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
        drop(acknowledged);
    }
}

#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_prepared_prewrite_refusal_retains_physical_park() {
    continued_reduce_chain_step_ack(10, true);
}
