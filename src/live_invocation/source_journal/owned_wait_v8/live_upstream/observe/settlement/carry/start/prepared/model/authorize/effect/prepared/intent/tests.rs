//! Genuine continued Ready/Consumed/Prepared and fixed Intent ACK; zero host.
pub(super) use super::super::super::super::tests::test_staged;
use super::super::tests::renew;
use super::*;
use crate::live_invocation::SourceInvocationClock;
use std::cell::Cell;
fn entries() -> usize {
    crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_activations()
}
pub(super) fn prepared<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    staged: LiveContinuedAuthorizationV8<'j>,
) -> (
    LivePreparedContinuedEffectV8<'j>,
    Vec<std::sync::Weak<[u8]>>,
) {
    let owner = renew(journal, staged);
    let leaves = owner
        .authorization
        .actual()
        .unwrap()
        .owner
        .test_effect_weak();
    assert!(leaves.len() >= 2);
    assert!(leaves.iter().all(|w| w.strong_count() == 1));
    let actual = owner
        .prepare_actual_effect()
        .unwrap_or_else(|_| panic!("actual Prepared"));
    (actual, leaves)
}
fn bytes(journal: &SourceOwnedWaitJournalV8) -> Vec<u8> {
    journal.test_observe_lease().borrow_mut().read().unwrap()
}
#[test]
fn owned_continued_intent_actual_ack_activates_same_owner_ledger_and_token_without_host() {
    test_staged(
        |journal, staged, _, ledger| {
            assert!(
                ledger.calls() > 0
                    && ledger.fuel() > 0
                    && ledger.request_bytes() > 0
                    && ledger.result_bytes() > 0
            );
            let (owner, leaves) = prepared(journal, staged);
            let old = journal.test_continued_intent_registry();
            let prefix = bytes(journal);
            let n = entries();
            let selected = owner
                .prepare_intent()
                .unwrap_or_else(|_| panic!("true second Intent"));
            assert!(matches!(
                selected.selected(),
                EntryV8::Ordinary(SourceJournalEntry::EffectIntent {
                    turn: 1,
                    attempt: 0,
                    ..
                })
            ));
            let seq = selected.sequence();
            let mut expected = prefix;
            expected.extend(journal.test_continued_intent_encoded(&selected, &expected));
            let ack = journal
                .begin_session()
                .unwrap()
                .append_owned_continued_intent(selected)
                .unwrap_or_else(|_| panic!("actual Intent ACK"));
            assert_eq!(entries(), n);
            assert_eq!(bytes(journal), expected);
            ack.validate_live().unwrap();
            let actual = ack
                .advance_intent()
                .unwrap_or_else(|_| panic!("actual zero-host Activated"));
            actual.validate_live().unwrap();
            assert_eq!(entries(), n + 1);
            assert_eq!(actual.phase.ack.session.sequence(), seq + 1);
            assert_eq!(
                *actual
                    .phase
                    .owner
                    .owner
                    .authorization
                    .completed
                    .owner
                    .accounting(),
                ledger
            );
            let new = journal.test_continued_intent_registry();
            assert_eq!(
                (&new.0, &new.1, &new.2, &new.3, &new.4),
                (&old.0, &old.1, &old.2, &old.3, &old.4)
            );
            assert_ne!(new.5, old.5);
            assert!(actual
                .phase
                .owner
                .owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_continued_activation_after());
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            // Old Consumed ancestry cannot serve as the fresh post-Intent guard.
            let prior = actual.phase.owner.owner.acks.last().unwrap();
            assert!(prior
                .witness
                .validate_current_session(&prior.session)
                .is_err());
            drop(actual);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}
#[cfg(unix)]
#[test]
fn owned_continued_intent_all_physical_faults_are_in_doubt_without_activation() {
    for mode in 0..4 {
        test_staged(
            |journal, staged, _, ledger| {
                let (owner, leaves) = prepared(journal, staged);
                let selected = owner.prepare_intent().unwrap_or_else(|_| panic!("Intent"));
                assert_eq!(
                    *selected
                        .owner
                        .owner
                        .authorization
                        .completed
                        .owner
                        .accounting(),
                    ledger
                );
                let before = bytes(journal);
                let mut expected = before.clone();
                expected.extend(journal.test_continued_intent_encoded(&selected, &before));
                let number = selected.sequence() + 1;
                let n = entries();
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
                    .append_owned_continued_intent(selected)
                    .err()
                    .expect("real fault");
                assert!(failure.test_is_in_doubt(), "mode {mode}");
                assert_eq!(entries(), n);
                {
                    let mut lease = journal.test_observe_lease().borrow_mut();
                    assert!(lease.read().is_err());
                    assert_eq!(
                        lease.test_persisted_snapshot().unwrap(),
                        if mode == 0 { before } else { expected }
                    );
                    assert!(lease.read().is_err());
                }
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                assert!(leaves.iter().all(|w| w.strong_count() == 1));
                drop(failure);
                assert!(leaves.iter().all(|w| w.upgrade().is_none()));
            },
            true,
        );
    }
}
thread_local! {static AT:Cell<usize>=const{Cell::new(usize::MAX)};static READS:Cell<usize>=const{Cell::new(0)};static EXPIRED:Cell<i64>=const{Cell::new(0)};static PANIC:Cell<bool>=const{Cell::new(false)};}
struct Clock;
impl crate::live_invocation::InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        if entries() > AT.with(Cell::get) {
            READS.with(|x| x.set(x.get() + 1));
            if PANIC.with(Cell::get) {
                panic!("actual Activated postguard");
            }
            EXPIRED.with(Cell::get)
        } else {
            1
        }
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_continued_intent_postconversion_failure_retains_actual_activated_and_first_cause() {
    for panic in [false, true] {
        test_staged(
            |journal, staged, _, ledger| {
                let (mut owner, leaves) = prepared(journal, staged);
                let n = entries();
                AT.with(|x| x.set(n));
                READS.with(|x| x.set(0));
                PANIC.with(|x| x.set(panic));
                EXPIRED.with(|x| {
                    x.set(
                        journal
                            .context()
                            .ordinary()
                            .deadline_millis()
                            .checked_add(1)
                            .unwrap(),
                    )
                });
                let ModelOwnerV8::Resumed(actual) = &mut owner.owner.authorization.completed.owner
                else {
                    panic!("continued")
                };
                actual.owner.test_authorize_clock(&Clock);
                let selected = owner.prepare_intent().unwrap_or_else(|_| panic!("Intent"));
                let ack = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continued_intent(selected)
                    .unwrap_or_else(|_| panic!("ACK before activation"));
                let failed = ack.advance_intent().err().expect("actual after boundary");
                let LiveContinuedIntentAcknowledgmentFailureV8::Entered { phase, error } = &failed
                else {
                    panic!("actual engine entry")
                };
                assert_eq!(
                    *error,
                    if panic {
                        SourceJournalError::Poisoned
                    } else {
                        SourceJournalError::Time
                    }
                );
                assert_eq!(entries(), n + 1);
                assert_eq!(READS.with(Cell::get), 1);
                assert!(phase
                    .owner
                    .owner
                    .authorization
                    .actual()
                    .unwrap()
                    .owner
                    .test_continued_activation_after());
                assert_eq!(
                    *phase.owner.owner.authorization.completed.owner.accounting(),
                    ledger
                );
                assert!(leaves.iter().all(|w| w.strong_count() == 1));
                assert_eq!(phase.validate_activated(), Err(*error));
                assert_eq!(READS.with(Cell::get), 1);
                assert!(journal.hold().is_err());
                AT.with(|x| x.set(usize::MAX));
                drop(failed);
                assert!(leaves.iter().all(|w| w.upgrade().is_none()));
            },
            true,
        );
    }
}
#[test]
fn owned_continued_intent_cancel_before_activation_keeps_actual_prepared() {
    test_staged(
        |journal, staged, _, ledger| {
            let (owner, leaves) = prepared(journal, staged);
            let n = entries();
            let selected = owner.prepare_intent().unwrap_or_else(|_| panic!("Intent"));
            // Cancellation after durable ACK must not rewind or activate.
            let ack = journal
                .begin_session()
                .unwrap()
                .append_owned_continued_intent(selected)
                .unwrap_or_else(|_| panic!("ACK"));
            // Cancellation handle comes from the still-retained actual source owner.
            // Use a narrow owning test seam; no production parts access exists.
            ack.test_cancel_actual();
            let failed = ack.advance_intent().err().expect("cancelled");
            let LiveContinuedIntentAcknowledgmentFailureV8::Before { owner, .. } = &failed else {
                panic!("preentry")
            };
            assert_eq!(
                *owner.owner.owner.authorization.completed.owner.accounting(),
                ledger
            );
            assert_eq!(entries(), n);
            assert!(!owner
                .owner
                .owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_continued_activation_after());
            assert!(journal.hold().is_err());
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            drop(failed);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}
#[test]
fn owned_continued_intent_mutated_selected_request_is_rejected_before_write() {
    test_staged(
        |journal, staged, _, _| {
            let (owner, leaves) = prepared(journal, staged);
            let n = entries();
            let mut selected = owner.prepare_intent().unwrap_or_else(|_| panic!("Intent"));
            let before = bytes(journal);
            let EntryV8::Ordinary(SourceJournalEntry::EffectIntent { request_digest, .. }) =
                &mut selected.selected
            else {
                unreachable!()
            };
            *request_digest = "sha256:foreign".into();
            let failed = journal
                .begin_session()
                .unwrap()
                .append_owned_continued_intent(selected)
                .err()
                .expect("mismatch");
            assert_eq!(entries(), n);
            assert!(journal.hold().is_err());
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            // No physical write took place; quarantine does not alter file bytes.
            #[cfg(unix)]
            assert_eq!(
                journal
                    .test_observe_lease()
                    .borrow()
                    .test_persisted_snapshot()
                    .unwrap(),
                before
            );
            #[cfg(not(unix))]
            let _ = before;
            drop(failed);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}
