//! Actual same-root successful cleanup/Outcome, stopped before Reduce ACK.
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::test_executed;
use crate::live_invocation::SourceInvocationClock;
use crate::resumable_effects::CapabilityPolicy;
use std::sync::Arc;
struct Clock;
impl crate::live_invocation::InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        1
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_reduce_reservation_selects_full_retained_f_after_actual_cleanup_without_evaluation() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        test_executed(&journal, &cancel, &policy, &Clock, |executed, weak| {
            let ledger = executed.accounting;
            let before = journal.begin_session().unwrap();
            let sequence = before.sequence();
            let bytes = before.acknowledged_bytes();
            let selected = executed
                .prepare_reduce()
                .unwrap_or_else(|_| panic!("actual Outcome selection"));
            assert_eq!(selected.sequence(), sequence);
            assert_eq!(selected.acknowledged_bytes(), bytes);
            assert_eq!(selected.owner.accounting, ledger);
            let (_, execution) = journal.context().ready_runtime().unwrap();
            assert_eq!(
                selected.selected_row(),
                &EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                    turn: 0,
                    attempt: Some(0),
                    role: SourceStageRole::Reduce,
                    fuel: execution.evaluation_fuel()
                })
            );
            selected.validate_live().unwrap();
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            let after = journal.begin_session().unwrap();
            assert_eq!(after.sequence(), sequence);
            assert_eq!(after.acknowledged_bytes(), bytes);
            // Selection did not enter Reduce or release either physical root.
            drop(selected);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    });
}
#[test]
fn owned_reduce_reservation_cancelled_actual_outcome_preserves_roots_ledger_and_poisons() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        test_executed(&journal, &cancel, &policy, &Clock, |executed, weak| {
            let ledger = executed.accounting;
            cancel.cancel();
            let failure = executed
                .prepare_reduce()
                .err()
                .expect("no Reduce reservation");
            assert_eq!(failure.error, SourceJournalError::Binding);
            assert_eq!(failure._owner.accounting, ledger);
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            drop(failure);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    });
}
