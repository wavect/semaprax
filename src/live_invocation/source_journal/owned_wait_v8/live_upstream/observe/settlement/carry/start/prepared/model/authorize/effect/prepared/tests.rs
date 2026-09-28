//! Real A/C ACKs and actual preparation; no manufactured Ready or ACK.
use super::super::super::tests::test_staged;
use super::*;
use crate::live_invocation::SourceInvocationClock;
use std::cell::Cell;
fn entries() -> usize {
    crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_effect_preparations_v8()
}
pub(super) fn renew<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    staged: LiveContinuedAuthorizationV8<'j>,
) -> LiveContinuedEffectV8<'j> {
    let ready = super::super::tests::ack(
        journal,
        staged
            .prepare_effect()
            .unwrap_or_else(|_| panic!("Granted")),
    );
    super::super::tests::ack(
        journal,
        ready
            .prepare_next()
            .unwrap_or_else(|_| panic!("Consumed selected")),
    )
}
#[test]
fn owned_continued_preparation_same_roots_credit_ledger_and_true_references() {
    test_staged(
        |journal, staged, weak, ledger| {
            let owner = renew(journal, staged);
            let refs = owner.preparation_references().unwrap();
            let leaves = owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_effect_weak();
            let registry = journal.test_continued_effect_registry();
            let prefix = owner.current().sequence();
            let n = entries();
            let actual = owner
                .prepare_actual_effect()
                .unwrap_or_else(|_| panic!("actual Prepared"));
            assert_eq!(entries(), n + 1);
            actual.validate_live().unwrap();
            assert_eq!(actual.owner.current().sequence(), prefix);
            assert_eq!(journal.test_continued_effect_registry(), registry);
            assert_eq!(
                *actual.owner.authorization.completed.owner.accounting(),
                ledger
            );
            let metadata = actual
                .owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_prepared_effect_metadata()
                .unwrap();
            assert_eq!((metadata.0, metadata.1, metadata.2), refs);
            assert!(metadata.3 > 0);
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            drop(actual);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}
thread_local! { static MODE:Cell<u8>=const{Cell::new(0)}; static AT:Cell<usize>=const{Cell::new(usize::MAX)}; static EXPIRED:Cell<i64>=const{Cell::new(0)}; }
struct Clock;
impl crate::live_invocation::InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        if entries() > AT.with(Cell::get) {
            match MODE.with(Cell::get) {
                1 => panic!("actual Prepared postguard"),
                2 => return EXPIRED.with(Cell::get),
                _ => {}
            }
        }
        1
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_continued_preparation_actual_posthandoff_failure_retains_prepared() {
    for mode in [1, 2] {
        test_staged(
            |journal, staged, _, ledger| {
                let mut owner = renew(journal, staged);
                let leaves = owner
                    .authorization
                    .actual()
                    .unwrap()
                    .owner
                    .test_effect_weak();
                let n = entries();
                MODE.with(|x| x.set(mode));
                AT.with(|x| x.set(n));
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
                let ModelOwnerV8::Resumed(actual) = &mut owner.authorization.completed.owner else {
                    panic!("actual continued")
                };
                actual.owner.test_authorize_clock(&Clock);
                let failed = owner
                    .prepare_actual_effect()
                    .err()
                    .expect("posthandoff refusal");
                assert_eq!(entries(), n + 1);
                let LiveContinuedEffectPreparationFailureV8::After { owner, error } = &failed
                else {
                    panic!("actual prepared boundary")
                };
                assert_eq!(
                    *error,
                    if mode == 1 {
                        SourceJournalError::Poisoned
                    } else {
                        SourceJournalError::Time
                    }
                );
                assert!(owner
                    .owner
                    .authorization
                    .actual()
                    .unwrap()
                    .owner
                    .test_prepared_effect_after());
                assert_eq!(
                    *owner.owner.authorization.completed.owner.accounting(),
                    ledger
                );
                assert!(leaves.iter().all(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                assert!(owner.validate_live().is_err());
                assert_eq!(entries(), n + 1);
                AT.with(|x| x.set(usize::MAX));
                drop(failed);
                assert!(leaves.iter().all(|w| w.upgrade().is_none()));
            },
            true,
        );
    }
}
#[test]
fn owned_continued_preparation_cancel_before_entry_preserves_actual_ready() {
    test_staged(
        |journal, staged, _, ledger| {
            let owner = renew(journal, staged);
            let n = entries();
            let leaves = owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_effect_weak();
            owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_authorize_cancel();
            let failed = owner.prepare_actual_effect().err().expect("cancelled");
            assert!(matches!(
                &failed,
                LiveContinuedEffectPreparationFailureV8::Before { .. }
            ));
            let LiveContinuedEffectPreparationFailureV8::Before { owner, .. } = &failed else {
                unreachable!()
            };
            assert_eq!(*owner.authorization.completed.owner.accounting(), ledger);
            assert_eq!(entries(), n);
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            drop(failed);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}

#[test]
fn owned_continued_preparation_reset_ledger_refuses_before_handoff() {
    test_staged(
        |journal, staged, _, ledger| {
            assert!(ledger.calls() > 0);
            let mut owner = renew(journal, staged);
            let leaves = owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_effect_weak();
            let n = entries();
            let ModelOwnerV8::Resumed(actual) = &mut owner.authorization.completed.owner else {
                panic!("actual continued")
            };
            actual
                .owner
                .test_replace_preparation_accounting(TargetAccounting::default());
            let failed = owner
                .prepare_actual_effect()
                .err()
                .expect("authenticated prior ledger rejects reset");
            assert!(matches!(
                &failed,
                LiveContinuedEffectPreparationFailureV8::Before {
                    error: SourceJournalError::Binding,
                    ..
                }
            ));
            assert_eq!(entries(), n);
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            drop(failed);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}

fn admissions() -> usize {
    crate::live_invocation::source_journal::owned_wait_v8::live_upstream::test_continued_preparation_admissions()
}
thread_local! {
    static LATE_AT:Cell<usize>=const{Cell::new(usize::MAX)};
    static LATE_READS:Cell<usize>=const{Cell::new(0)};
}
struct LateAdmissionClock;
impl crate::live_invocation::InvocationClock for LateAdmissionClock {
    fn now_millis(&self) -> i64 {
        if admissions() > LATE_AT.with(Cell::get) {
            LATE_READS.with(|n| n.set(n.get() + 1));
            EXPIRED.with(Cell::get)
        } else {
            1
        }
    }
}
impl SourceInvocationClock for LateAdmissionClock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_continued_preparation_late_admission_failure_is_before_with_original_error() {
    test_staged(
        |journal, staged, _, ledger| {
            let mut owner = renew(journal, staged);
            let leaves = owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_effect_weak();
            let n = entries();
            let a = admissions();
            LATE_AT.with(|x| x.set(a));
            LATE_READS.with(|x| x.set(0));
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
            let ModelOwnerV8::Resumed(actual) = &mut owner.authorization.completed.owner else {
                panic!("actual continued")
            };
            actual.owner.test_authorize_clock(&LateAdmissionClock);
            let failed = owner
                .prepare_actual_effect()
                .err()
                .expect("later pre-entry clock failure");
            let LiveContinuedEffectPreparationFailureV8::Before { owner, error } = &failed else {
                panic!("Ready never entered preparation")
            };
            assert_eq!(*error, SourceJournalError::Time);
            assert_eq!(entries(), n);
            assert_eq!(admissions(), a + 1);
            assert_eq!(
                LATE_READS.with(Cell::get),
                1,
                "no revalidation after original refusal"
            );
            assert!(!owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_prepared_effect_after());
            assert_eq!(*owner.authorization.completed.owner.accounting(), ledger);
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            assert_eq!(LATE_READS.with(Cell::get), 1);
            LATE_AT.with(|x| x.set(usize::MAX));
            drop(failed);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}
