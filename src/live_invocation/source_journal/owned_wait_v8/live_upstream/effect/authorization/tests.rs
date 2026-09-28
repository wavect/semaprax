//! Requires root's real fixed Ready append and consuming envelope delegate.
use super::super::tests::test_ready_obligation;
use super::*;
use crate::live_invocation::SourceInvocationClock;
use std::cell::Cell;
use std::sync::Arc;

struct Clock(Cell<i64>);
impl crate::live_invocation::InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        self.0.get()
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}

#[test]
fn owned_wait_live_consumed_promotion_preserves_roots_refs_and_source_charge() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let clock = Clock(Cell::new(1));
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (obligation, weak) = test_ready_obligation(&journal, &cancel, &clock, &policy);
        let consumed = obligation.owner.owner.consumed();
        let before = obligation.owner.session.fold_for_live_test();
        let actual = match journal
            .begin_session()
            .unwrap()
            .append_owned_effect(obligation)
        {
            Ok(actual) => actual,
            Err(_) => panic!("actual Ready ACK"),
        };
        let next = actual
            .advance_ready()
            .unwrap_or_else(|_| panic!("actual promotion"));
        next.validate_live().unwrap();
        assert!(next.belongs_to(&journal));
        assert_eq!((next.staged, next.sequence()), (19, 21));
        assert_eq!(next.owner.consumed(), consumed);
        assert_eq!(next.owner.test_weak().len(), weak.len());
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        assert!(weak.iter().all(|old| next
            .owner
            .test_weak()
            .iter()
            .any(|new| std::sync::Weak::ptr_eq(old, new))));
        assert_eq!(
            next.selected_row(),
            &EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed {
                turn: 0,
                attempt: 0,
                grant_digest: next.commitments.grant_digest().into(),
            })
        );
        assert_ne!(
            next.commitments.grant_digest(),
            next.commitments.target_grant_digest()
        );
        let after = next.session.fold_for_live_test();
        assert_eq!(after.stages, before.stages);
        assert_eq!(after.reserved_total, before.reserved_total);
        assert_eq!(after.consumed_recorded, before.consumed_recorded);
        drop(next);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}

#[test]
fn owned_wait_live_consumed_promotion_cancel_and_expiry_quarantine_actual_staged_owner() {
    for expired in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let clock = Clock(Cell::new(1));
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (obligation, weak) = test_ready_obligation(&journal, &cancel, &clock, &policy);
            let actual = match journal
                .begin_session()
                .unwrap()
                .append_owned_effect(obligation)
            {
                Ok(actual) => actual,
                Err(_) => panic!("actual Ready ACK"),
            };
            if expired {
                clock.0.set(journal.context().ordinary().deadline_millis());
            } else {
                cancel.cancel();
            }
            let failed = actual.advance_ready().err().expect("no promotion");
            assert!(
                matches!(&failed, LiveReadyAdvanceFailureV8::Before { error, .. }
                if *error == if expired { SourceJournalError::Time } else { SourceJournalError::Binding })
            );
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            drop(failed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}

#[test]
fn owned_wait_live_consumed_inert_metadata_does_not_ack_or_rebase_owner() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let clock = Clock(Cell::new(1));
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (obligation, weak) = test_ready_obligation(&journal, &cancel, &clock, &policy);
        let actual = match journal
            .begin_session()
            .unwrap()
            .append_owned_effect(obligation)
        {
            Ok(actual) => actual,
            Err(_) => panic!("actual Ready ACK"),
        };
        let next = actual
            .advance_ready()
            .unwrap_or_else(|_| panic!("actual promotion"));
        let inert = match journal
            .begin_session()
            .unwrap()
            .append(next.selected_row().clone())
        {
            Ok(inert) => inert,
            Err(_) => panic!("ordinary inert consumed metadata"),
        };
        assert_eq!(inert.sequence(), 22);
        assert_eq!(next.sequence(), 21);
        assert_eq!(next.validate_live(), Err(SourceJournalError::Order));
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(next);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
