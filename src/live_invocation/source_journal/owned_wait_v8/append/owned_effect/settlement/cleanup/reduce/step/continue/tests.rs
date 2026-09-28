//! Genuine Continue owner and fixed original ACKs; no reconstructed State.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveMovedStepV8;
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::{InvocationClock,SourceInvocationClock};
use crate::resumable_effects::CapabilityPolicy;
use std::cell::Cell;
use std::sync::Arc;
struct Clock {
    now: Cell<i64>,
}
impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        self.now.get()
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
fn with_moved(
    callback: impl for<'j> FnOnce(
        &'j SourceOwnedWaitJournalV8,
        LiveMovedStepV8<'j>,
        Vec<std::sync::Weak<[u8]>>,
        &'j AgentCancellation,
        &'j Clock,
    ),
) {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, _| {
            let context = context.with_cumulative_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock { now: Cell::new(1) };
            super::super::tests::test_moved(&journal, &cancel, &policy, &clock, |moved, weak| {
                callback(&journal, moved, weak, &cancel, &clock)
            });
        },
    );
}
fn ack<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: LiveOwnedContinueAppendV8<'j>,
) -> VerifiedOwnedContinueAppendV8<'j> {
    journal
        .begin_session()
        .unwrap()
        .append_owned_continue(owner)
        .unwrap_or_else(|_| panic!("actual sameFD continuation ACK"))
}
#[test]
fn owned_continue_actual_state_and_observe_acks_preserve_owner_ledger_and_cumulative_funding() {
    with_moved(|journal, moved, weak, _, _| {
        let before = journal.begin_session().unwrap();
        let (r, s, turn, _) = before.inventory.continuation_facts().unwrap();
        let (ordinary_observation, ordinary_consumed) = moved.test_observe_oracle();
        let ledger = *moved.accounting();
        let fuel = journal.context().ordinary().max_steps_per_stage().unwrap() as u64;
        let selected = moved
            .prepare_continue()
            .unwrap_or_else(|_| panic!("actual Continue selection"));
        let current = ack(journal, selected)
            .advance_continue()
            .unwrap_or_else(|_| panic!("actual StateCommitted"));
        let LiveContinueAcknowledgedV8::State(state) = current else {
            panic!("State owner")
        };
        let after_state = journal.begin_session().unwrap();
        let (nr, ns, next, _) = after_state.inventory.continuation_facts().unwrap();
        assert_eq!((nr, ns, next), (r, s, turn + 1));
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        let observed = ack(
            journal,
            state
                .prepare_observe()
                .unwrap_or_else(|_| panic!("fullF Observe obligation")),
        )
        .advance_continue()
        .unwrap_or_else(|_| panic!("sole actual Observe"));
        let LiveContinueAcknowledgedV8::Observed(observed) = observed else {
            panic!("actual observed/failed owner")
        };
        assert!(observed.is_observed());
        assert_eq!(observed.test_observation(), &ordinary_observation);
        assert_eq!(observed.consumed(), ordinary_consumed);
        assert_eq!(observed.turn(), turn + 1);
        assert_eq!(observed.accounting(), &ledger);
        assert!(observed.consumed() > 0 && observed.consumed() <= fuel as usize);
        let after = journal.begin_session().unwrap();
        let (nr, ns, _, _) = after.inventory.continuation_facts().unwrap();
        assert_eq!((nr, ns), (r + fuel, s + 1));
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        drop(observed);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[test]
fn owned_continue_cancel_or_expired_clock_before_ack_keeps_real_state_and_zero_new_stage() {
    for expired in [false, true] {
        with_moved(|journal, moved, weak, cancel, clock| {
            let before = journal.begin_session().unwrap();
            let seq = before.sequence();
            let original = moved
                .prepare_continue()
                .unwrap_or_else(|_| panic!("live selector"));
            if expired {
                clock.now.set(
                    journal
                        .context()
                        .ordinary()
                        .deadline_millis()
                        .checked_add(1)
                        .unwrap(),
                );
            } else {
                cancel.cancel();
            }
            let failure = before
                .append_owned_continue(original)
                .err()
                .expect("actual prewrite guard refusal");
            assert!(matches!(
                failure,
                LiveOwnedContinueAppendFailureV8::Before { .. }
            ));
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            assert!(seq > 0);
            drop(failure);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
#[cfg(unix)]
fn owned_continue_state_and_observe_real_append_faults_never_evaluate_or_remint() {
    for observe in [false, true] {
        for after in [false, true] {
            with_moved(|journal, moved, weak, _, _| {
                let entries_before = crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8();
                let mut obligation = moved
                    .prepare_continue()
                    .unwrap_or_else(|_| panic!("State selection"));
                if observe {
                    let LiveContinueAcknowledgedV8::State(state) = ack(journal, obligation)
                        .advance_continue()
                        .unwrap_or_else(|_| panic!("State ACK"))
                    else {
                        panic!()
                    };
                    obligation = state
                        .prepare_observe()
                        .unwrap_or_else(|_| panic!("Observe original selection"));
                }
                {
                    let number = obligation.sequence() + 1;
                    let mut lease = journal.lease.borrow_mut();
                    if after {
                        lease.test_fail_after_sync(number)
                    } else {
                        lease.test_fail_before_write(number)
                    }
                }
                let failure = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continue(obligation)
                    .err()
                    .expect("actual persistence failure");
                assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8(), entries_before);
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        }
    }
}

#[test]
fn owned_continue_failed_observe_retains_actual_state_and_observed_consumption() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_continued_observe_ensures_store(
        |context, lease, key, _| {
            let context = context.with_cumulative_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock { now: Cell::new(1) };
            super::super::tests::test_moved(&journal, &cancel, &policy, &clock, |moved, weak| {
                let ledger = *moved.accounting();
                let LiveContinueAcknowledgedV8::State(state) = ack(
                    &journal,
                    moved
                        .prepare_continue()
                        .unwrap_or_else(|_| panic!("Continue")),
                )
                .advance_continue()
                .unwrap_or_else(|_| panic!("State ACK")) else {
                    panic!("State")
                };
                let LiveContinueAcknowledgedV8::Observed(failed) = ack(
                    &journal,
                    state
                        .prepare_observe()
                        .unwrap_or_else(|_| panic!("Observe reservation")),
                )
                .advance_continue()
                .unwrap_or_else(|_| panic!("actual failed Observe retained")) else {
                    panic!("Observe outcome")
                };
                assert!(failed.is_failed());
                assert!(!failed.is_observed());
                assert_eq!(failed.turn(), 1);
                assert!(failed.consumed() > 0);
                assert_eq!(failed.accounting(), &ledger);
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                drop(failed);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        },
    );
}
