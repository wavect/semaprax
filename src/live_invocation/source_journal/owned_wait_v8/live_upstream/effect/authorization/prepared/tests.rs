//! Uses the genuine source actor, fixed ACKs and actual exclusive Reduce hold.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::HeldOwnedAuthorizationConsumedV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::tests::test_ready_obligation;
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
fn held<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j crate::agent_runtime::AgentCancellation,
    clock: &'j Clock,
    policy: &'j CapabilityPolicy,
) -> (
    HeldOwnedAuthorizationConsumedV8<'j>,
    Vec<std::sync::Weak<[u8]>>,
) {
    let (ready, weak) = test_ready_obligation(journal, cancel, clock, policy);
    let ready = journal
        .begin_session()
        .unwrap()
        .append_owned_effect(ready)
        .unwrap_or_else(|_| panic!("actual Ready ACK"));
    let consumed = ready
        .advance_ready()
        .unwrap_or_else(|_| panic!("actual source promotion"));
    let consumed = journal
        .begin_session()
        .unwrap()
        .append_owned_authorization_consumed(consumed)
        .unwrap_or_else(|_| panic!("actual Consumed ACK"));
    let held = consumed
        .reserve_owned_reduce()
        .unwrap_or_else(|_| panic!("actual exclusive hold"));
    held.validate_live().unwrap();
    (held, weak)
}
#[test]
fn owned_wait_live_prepared_consumes_same_roots_k_refs_and_exclusive_hold_without_work() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let clock = Clock(Cell::new(1));
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (held, weak) = held(&journal, &cancel, &clock, &policy);
        let before = journal.begin_session().unwrap();
        let before_facts = before.fold_for_live_test();
        let before_bytes = before.acknowledged_bytes();
        let actual = held
            .advance_authorization()
            .unwrap_or_else(|_| panic!("actual preparation"));
        actual.validate_live().unwrap();
        assert_eq!(
            (
                actual.session.sequence(),
                actual.session.acknowledged_bytes()
            ),
            (22, before_bytes)
        );
        let (staged, ready, consumed, grant, target, proposal) =
            actual.prepared.live_test_metadata();
        assert_eq!((staged, ready, consumed), (19, 20, 21));
        assert_eq!(grant, actual.commitments.grant_digest());
        assert_eq!(target, actual.commitments.target_grant_digest());
        assert_ne!(grant, target);
        assert_eq!(proposal, actual.proposal.ordinary_digest());
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        let after = journal.begin_session().unwrap().fold_for_live_test();
        assert_eq!(
            (after.stages, after.reserved_total, after.consumed_recorded),
            (
                before_facts.stages,
                before_facts.reserved_total,
                before_facts.consumed_recorded
            )
        );
        // No host or source entry exists on this opaque successor. Dropping the
        // same credit retires the invocation rather than refunding it.
        drop(actual);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
        assert!(journal.hold().is_err());
    });
}
#[test]
fn owned_wait_live_prepared_cancel_or_deadline_before_handoff_retains_owner_and_credit() {
    for deadline in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let clock = Clock(Cell::new(1));
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (held, weak) = held(&journal, &cancel, &clock, &policy);
            if deadline {
                clock.0.set(20);
            } else {
                cancel.cancel();
            }
            let failed = held.advance_authorization().err().expect("no Prepared");
            let LiveEffectAuthorizationFailureV8::Before { error, .. } = &failed else {
                panic!("before handoff");
            };
            assert_eq!(
                *error,
                if deadline {
                    SourceJournalError::Time
                } else {
                    SourceJournalError::Binding
                }
            );
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            drop(failed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
fn owned_wait_live_prepared_post_handoff_guard_loss_quarantines_same_physical_owner() {
    for deadline in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let clock = Clock(Cell::new(1));
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (held, weak) = held(&journal, &cancel, &clock, &policy);
            let actual = held
                .advance_authorization()
                .unwrap_or_else(|_| panic!("actual preparation"));
            if deadline {
                clock.0.set(20);
            } else {
                cancel.cancel();
            }
            assert_eq!(
                actual.validate_live(),
                Err(if deadline {
                    SourceJournalError::Time
                } else {
                    SourceJournalError::Binding
                })
            );
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            assert_eq!(actual.validate_live(), Err(SourceJournalError::Poisoned));
            drop(actual);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}

struct HandoffClock<'a> {
    cancel: &'a crate::agent_runtime::AgentCancellation,
    reads: Cell<usize>,
    armed: Cell<bool>,
    mode: u8,
}
impl crate::live_invocation::InvocationClock for HandoffClock<'_> {
    fn now_millis(&self) -> i64 {
        if !self.armed.get() {
            return 1;
        }
        let read = self.reads.get() + 1;
        self.reads.set(read);
        // Two source guards, two engine admission guards and one engine
        // authorization guard precede the first actual Prepared postguard.
        if read == 6 {
            match self.mode {
                0 => self.cancel.cancel(),
                1 => return 20,
                _ => panic!("actual Prepared postguard clock panic"),
            }
        }
        1
    }
}
impl SourceInvocationClock for HandoffClock<'_> {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_wait_live_prepared_guard_loss_during_handoff_retains_actual_prepared_not_ready() {
    for mode in 0..3 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let clock = HandoffClock {
                cancel: &cancel,
                reads: Cell::new(0),
                armed: Cell::new(false),
                mode,
            };
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (ready, weak) = test_ready_obligation(&journal, &cancel, &clock, &policy);
            let ready = journal
                .begin_session()
                .unwrap()
                .append_owned_effect(ready)
                .unwrap_or_else(|_| panic!("actual Ready ACK"));
            let consumed = ready
                .advance_ready()
                .unwrap_or_else(|_| panic!("actual promotion"));
            let consumed = journal
                .begin_session()
                .unwrap()
                .append_owned_authorization_consumed(consumed)
                .unwrap_or_else(|_| panic!("actual Consumed ACK"));
            let held = consumed
                .reserve_owned_reduce()
                .unwrap_or_else(|_| panic!("actual exclusive hold"));
            clock.armed.set(true);
            let failed = held
                .advance_authorization()
                .err()
                .expect("postpreparation guard fails");
            let LiveEffectAuthorizationFailureV8::Preparation {
                _owner: LiveReadyEffectPreparationRejectionV8::After { _owner: actual, .. },
                _session: session,
                _hold: hold,
                error,
                ..
            } = &failed
            else {
                panic!("retain actual Prepared without rewinding Ready");
            };
            assert_eq!(
                *error,
                match mode {
                    0 => SourceJournalError::Binding,
                    1 => SourceJournalError::Time,
                    _ => SourceJournalError::Poisoned,
                }
            );
            assert_eq!(clock.reads.get(), 6);
            let (staged, ready, consumed, grant, target, _) = actual.live_test_metadata();
            assert_eq!(
                (staged, ready, consumed, session.sequence()),
                (19, 20, 21, 22)
            );
            assert_ne!(grant, target);
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            assert_eq!(
                hold.validate_guard(&journal, session.sequence(), session.acknowledged_bytes()),
                Err(SourceJournalError::Poisoned)
            );
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            // The quarantined opaque rejection exposes no source retry or
            // dispatch path. No later guard can repeat the original callback.
            assert_eq!(clock.reads.get(), 6);
            drop(failed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
