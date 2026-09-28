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
        assert!(journal.hold().is_err());
        assert!(journal.begin_session().is_err());
        assert_eq!(next.validate_live(), Err(SourceJournalError::Poisoned));
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(next);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}

struct PostPromotionCancelClock<'a> {
    cancellation: &'a crate::agent_runtime::AgentCancellation,
    armed: Cell<bool>,
    calls: Cell<usize>,
}
impl crate::live_invocation::InvocationClock for PostPromotionCancelClock<'_> {
    fn now_millis(&self) -> i64 {
        if self.armed.get() {
            let calls = self.calls.get() + 1;
            self.calls.set(calls);
            // The consuming delegate checks the Ready prefix twice, then the
            // permit checks before settle, at settle entry, and AFTER the move.
            if calls == 5 {
                self.cancellation.cancel();
            }
        }
        1
    }
}
impl SourceInvocationClock for PostPromotionCancelClock<'_> {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}

#[test]
fn owned_wait_live_consumed_post_promotion_guard_loss_retains_ready_and_cannot_replay() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let clock = PostPromotionCancelClock {
            cancellation: &cancel,
            armed: Cell::new(false),
            calls: Cell::new(0),
        };
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (obligation, weak) = test_ready_obligation(&journal, &cancel, &clock, &policy);
        let consumed = obligation.owner.owner.consumed();
        let actual = match journal
            .begin_session()
            .unwrap()
            .append_owned_effect(obligation)
        {
            Ok(actual) => actual,
            Err(_) => panic!("actual Ready ACK"),
        };
        clock.armed.set(true);
        let failed = actual
            .advance_ready()
            .err()
            .expect("post-handoff guard loss");
        let LiveReadyAdvanceFailureV8::Promotion {
            _owner: LiveReadyPromotionOutcomeV8::GuardLost(ready),
            _session: session,
            ..
        } = &failed
        else {
            panic!("must retain the physical Ready after the move");
        };
        assert_eq!(clock.calls.get(), 5);
        assert_eq!(ready.consumed(), consumed);
        assert_eq!(session.sequence(), 21);
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        let actual_leaves = ready.test_weak();
        assert_eq!(actual_leaves.len(), weak.len());
        assert!(weak.iter().all(|old| actual_leaves
            .iter()
            .any(|new| std::sync::Weak::ptr_eq(old, new))));
        assert!(journal.hold().is_err());
        assert!(journal.begin_session().is_err());
        // No consuming retry or release API exists on this quarantine holder;
        // failed store access performs no source work or physical finalizers.
        assert_eq!(clock.calls.get(), 5);
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(failed);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
