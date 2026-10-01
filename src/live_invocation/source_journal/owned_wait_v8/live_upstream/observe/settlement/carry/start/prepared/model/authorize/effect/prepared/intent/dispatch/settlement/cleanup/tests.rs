use super::super::super::super::tests::test_staged;
use super::super::super::tests::{activated, host};
use super::super::tests::ack;
use super::*;

fn recorded<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    staged: LiveContinuedAuthorizationV8<'j>,
    mode: u8,
) -> (
    LiveRecordedContinuedEffectV8<'j>,
    Vec<std::sync::Weak<[u8]>>,
) {
    let (actual, leaves) = activated(journal, staged);
    let mut host = host(mode);
    let actual = actual
        .dispatch(&mut host)
        .unwrap_or_else(|_| panic!("actual target"));
    let selected = actual
        .prepare_settlement()
        .unwrap_or_else(|_| panic!("actual settlement"));
    let LiveContinuedSettlementAcknowledgedV8::Settled(owner) = ack(journal, selected) else {
        panic!("settled")
    };
    let selected = owner
        .prepare_recorded()
        .unwrap_or_else(|_| panic!("actual evidence"));
    let LiveContinuedSettlementAcknowledgedV8::Recorded(owner) = ack(journal, selected) else {
        panic!("recorded")
    };
    assert_eq!(host.calls, 1);
    (owner, leaves)
}
#[test]
fn owned_continued_cleanup_preparation_binds_actual_decision_without_release_or_new_charge() {
    for mode in 0..3 {
        test_staged(
            |journal, staged, _, _| {
                let (owner, leaves) = recorded(journal, staged, mode);
                let before = journal.test_observe_lease().borrow_mut().read().unwrap();
                let accounting = *owner.accounting();
                let registry = journal.test_continued_intent_registry();
                let selected = owner
                    .prepare_decision_cleanup()
                    .unwrap_or_else(|_| panic!("same Decision"));
                selected.validate_live().unwrap();
                let EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                    turn,
                    attempt,
                    staged,
                    ready,
                    consumed,
                    intent,
                    settlement,
                    recorded,
                    operations,
                    operations_digest,
                    ..
                }) = &selected.selected
                else {
                    panic!("cleanup selected")
                };
                assert_eq!((*turn, *attempt), (1, 0));
                assert_eq!(*staged + 1, *ready);
                assert_eq!(*ready + 1, *consumed);
                assert_eq!(*consumed + 1, *intent);
                assert_eq!(*intent + 1, *settlement);
                assert_eq!(*settlement + 1, *recorded);
                assert_eq!(
                    *recorded as usize + 1,
                    selected.owner.phase.current().sequence()
                );
                let (_, execution) = journal.context().ready_runtime().unwrap();
                let expected = crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
                    execution.wait().authorize().disposal(),
                )
                .unwrap();
                assert_eq!(operations, &expected);
                assert!(!operations.as_array().unwrap().is_empty());
                assert!(operations_digest.starts_with("sha256:"));
                assert_eq!(*selected.accounting(), accounting);
                assert_eq!(journal.test_continued_intent_registry(), registry);
                assert_eq!(
                    journal.test_observe_lease().borrow_mut().read().unwrap(),
                    before
                );
                assert!(leaves.iter().all(|leaf| leaf.strong_count() == 1));
                drop(selected);
                assert!(leaves.iter().all(|leaf| leaf.upgrade().is_none()));
            },
            true,
        );
    }
}
#[cfg(unix)]
#[test]
fn owned_continued_cleanup_preparation_refuses_substituted_coordinates_and_operations() {
    for mutation in 0..3 {
        test_staged(
            |journal, staged, _, _| {
                let (owner, leaves) = recorded(journal, staged, 0);
                let before = journal.test_observe_lease().borrow_mut().read().unwrap();
                let mut selected = owner
                    .prepare_decision_cleanup()
                    .unwrap_or_else(|_| panic!("cleanup"));
                let EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                    turn,
                    recorded,
                    operations,
                    ..
                }) = &mut selected.selected
                else {
                    panic!("row")
                };
                match mutation {
                    0 => *turn = 0,
                    1 => *recorded -= 1,
                    _ => *operations = json!([]),
                }
                assert_eq!(selected.validate_live(), Err(SourceJournalError::Binding));
                assert!(journal.begin_session().is_err());
                assert_eq!(
                    journal
                        .test_observe_lease()
                        .borrow()
                        .test_persisted_snapshot()
                        .unwrap(),
                    before
                );
                assert!(leaves.iter().all(|leaf| leaf.strong_count() == 1));
                drop(selected);
                assert!(leaves.iter().all(|leaf| leaf.upgrade().is_none()));
            },
            true,
        );
    }
}
#[cfg(unix)]
#[test]
fn owned_continued_cleanup_preparation_cancel_retains_actual_owner_and_history() {
    test_staged(
        |journal, staged, _, _| {
            let (owner, leaves) = recorded(journal, staged, 0);
            let before = journal.test_observe_lease().borrow_mut().read().unwrap();
            let accounting = *owner.accounting();
            owner
                .phase
                .owner
                .phase
                .owner
                .owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_dispatch_cancellation()
                .cancel();
            let failure = owner
                .prepare_decision_cleanup()
                .err()
                .expect("cancelled before cleanup");
            assert_eq!(failure.error, SourceJournalError::Binding);
            assert_eq!(*failure.owner.accounting(), accounting);
            assert_eq!(
                journal
                    .test_observe_lease()
                    .borrow()
                    .test_persisted_snapshot()
                    .unwrap(),
                before
            );
            assert!(leaves.iter().all(|leaf| leaf.strong_count() == 1));
            drop(failure);
            assert!(leaves.iter().all(|leaf| leaf.upgrade().is_none()));
        },
        true,
    );
}

use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::cleanup::LiveCleanupAcknowledgedV8;
fn started<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: LiveRecordedContinuedEffectV8<'j>,
) -> StartedContinuedDecisionCleanupV8<'j> {
    let selected = owner
        .prepare_decision_cleanup()
        .unwrap_or_else(|_| panic!("cleanup"));
    let verified = journal
        .begin_session()
        .unwrap()
        .append_owned_effect_cleanup(selected.into_append())
        .unwrap_or_else(|_| panic!("fixed Started ACK"));
    let LiveCleanupAcknowledgedV8::ContinuedStarted(owner) = verified
        .advance_cleanup()
        .unwrap_or_else(|_| panic!("Started carrier"))
    else {
        panic!("continued Started")
    };
    owner.validate_live().unwrap();
    *owner
}
#[test]
fn owned_continued_cleanup_actual_release_and_receipt_keep_state_and_spent_hold() {
    for mode in 0..3 {
        test_staged(
            |journal, staged, _, _| {
                let (owner, leaves) = recorded(journal, staged, mode);
                let accounting = *owner.accounting();
                let owner = started(journal, owner);
                assert!(leaves.iter().all(|leaf| leaf.strong_count() == 1));
                let mut releases = 0;
                let released = owner
                    .release_decision(|_| {
                        releases += 1;
                        assert_eq!(leaves[1].strong_count(), 0);
                    })
                    .unwrap_or_else(|_| panic!("actual release"));
                assert_eq!(releases, 1);
                assert_eq!(leaves[0].strong_count(), 1);
                let selected = released
                    .prepare_settled()
                    .unwrap_or_else(|_| panic!("actual receipt"));
                let verified = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_effect_cleanup(selected)
                    .unwrap_or_else(|_| panic!("fixed Settled ACK"));
                let LiveCleanupAcknowledgedV8::ContinuedSettled(settled) = verified
                    .advance_cleanup()
                    .unwrap_or_else(|_| panic!("Settled carrier"))
                else {
                    panic!("continued Settled")
                };
                settled.validate_live().unwrap();
                assert_eq!(*settled.accounting(), accounting);
                assert_eq!(leaves[0].strong_count(), 1);
                assert_eq!(leaves[1].strong_count(), 0);
                drop(settled);
                assert!(leaves.iter().all(|leaf| leaf.upgrade().is_none()));
            },
            true,
        );
    }
}
#[test]
fn owned_continued_outcome_requires_live_settled_ack_and_keeps_one_release() {
    let run = || {
        for cancel_after_settled in [false, true] {
            test_staged(
                |journal, staged, _, _| {
                    let (owner, leaves) = recorded(journal, staged, 0);
                    let owner = started(journal, owner);
                    let mut releases = 0;
                    let released = owner
                        .release_decision(|_| releases += 1)
                        .unwrap_or_else(|_| panic!("actual release"));
                    let selected = released
                        .prepare_settled()
                        .unwrap_or_else(|_| panic!("actual receipt"));
                    let verified = journal
                        .begin_session()
                        .unwrap()
                        .append_owned_effect_cleanup(selected)
                        .unwrap_or_else(|_| panic!("Settled ACK"));
                    let LiveCleanupAcknowledgedV8::ContinuedSettled(owner) = verified
                        .advance_cleanup()
                        .unwrap_or_else(|_| panic!("Settled owner"))
                    else {
                        panic!("continued Settled")
                    };
                    let mut owner = *owner;
                    let before = journal
                        .test_observe_lease()
                        .borrow()
                        .test_persisted_snapshot()
                        .unwrap();
                    if cancel_after_settled {
                        owner
                            .owner
                            .owner
                            .owner
                            .owner
                            .phase
                            .owner
                            .phase
                            .owner
                            .owner
                            .authorization
                            .actual()
                            .unwrap()
                            .owner
                            .test_dispatch_cancellation()
                            .cancel();
                        let failure = owner
                            .mint_outcome()
                            .err()
                            .expect("fresh cancellation refuses Outcome");
                        assert_eq!(failure.error, SourceJournalError::Binding);
                        owner = failure.owner;
                        assert!(!owner.outcome_minted().unwrap_or(false));
                    } else {
                        owner = owner
                            .mint_outcome()
                            .unwrap_or_else(|_| panic!("one physical Outcome"));
                        assert!(owner.outcome_minted().unwrap());
                    }
                    assert_eq!(releases, 1);
                    assert_eq!(
                        journal
                            .test_observe_lease()
                            .borrow()
                            .test_persisted_snapshot()
                            .unwrap(),
                        before
                    );
                    assert_eq!(leaves[1].strong_count(), 0);
                    assert_eq!(leaves[0].strong_count(), 1);
                    drop(owner);
                    assert!(leaves.iter().all(|leaf| leaf.upgrade().is_none()));
                },
                true,
            );
        }
    };
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(run)
        .expect("continued Outcome test thread")
        .join()
        .expect("continued Outcome test completion");
}
#[test]
fn owned_continued_cleanup_incurred_release_survives_cancellation_and_records_observer_panic() {
    for panic_observer in [false, true] {
        test_staged(
            |journal, staged, _, _| {
                let (owner, leaves) = recorded(journal, staged, 0);
                let cancel = owner
                    .phase
                    .owner
                    .phase
                    .owner
                    .owner
                    .authorization
                    .actual()
                    .unwrap()
                    .owner
                    .test_dispatch_cancellation();
                let owner = started(journal, owner);
                cancel.cancel();
                let mut releases = 0;
                let released = owner
                    .release_decision(|_| {
                        releases += 1;
                        if panic_observer {
                            panic!("observer failure after actual drop")
                        }
                    })
                    .unwrap_or_else(|_| panic!("incurred cleanup"));
                assert_eq!(releases, 1);
                assert_eq!(leaves[1].strong_count(), 0);
                assert_eq!(
                    released.receipt().unwrap()["settlement"],
                    if panic_observer {
                        "failed"
                    } else {
                        "completed"
                    }
                );
                let selected = released
                    .prepare_settled()
                    .unwrap_or_else(|_| panic!("incurred receipt"));
                let verified = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_effect_cleanup(selected)
                    .unwrap_or_else(|_| panic!("receipt ACK"));
                let LiveCleanupAcknowledgedV8::ContinuedSettled(settled) = verified
                    .advance_cleanup()
                    .unwrap_or_else(|_| panic!("receipt retained"))
                else {
                    panic!("Settled")
                };
                settled.validate_live().unwrap();
                drop(settled);
                assert!(leaves.iter().all(|leaf| leaf.upgrade().is_none()));
            },
            true,
        );
    }
}
#[cfg(unix)]
#[test]
fn owned_continued_cleanup_physical_ack_faults_retain_actual_boundary_without_retry() {
    for settled in [false, true] {
        for mode in 0..4 {
            test_staged(
                |journal, staged, _, _| {
                    let (owner, leaves) = recorded(journal, staged, 0);
                    let selected = if settled {
                        started(journal, owner)
                            .release_decision(|_| {})
                            .unwrap_or_else(|_| panic!("release"))
                            .prepare_settled()
                            .unwrap_or_else(|_| panic!("receipt"))
                    } else {
                        owner
                            .prepare_decision_cleanup()
                            .unwrap_or_else(|_| panic!("prepare"))
                            .into_append()
                    };
                    let before = journal.test_observe_lease().borrow_mut().read().unwrap();
                    let n = selected.sequence() + 1;
                    {
                        let mut lease = journal.test_observe_lease().borrow_mut();
                        match mode {
                            0 => lease.test_fail_before_write(n),
                            1 => lease.test_fail_after_write(n),
                            2 => lease.test_fail_before_sync(n),
                            _ => lease.test_fail_after_sync(n),
                        }
                    }
                    let failure = journal
                        .begin_session()
                        .unwrap()
                        .append_owned_effect_cleanup(selected)
                        .err()
                        .expect("failed actual append");
                    assert!(matches!(
                        &failure,
                        crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::LiveOwnedEffectCleanupAppendFailureV8::Append {
                            _failure: crate::live_invocation::source_journal::owned_wait_v8::append::AppendFailureV8::InDoubt { .. }, ..
                        }
                    ));
                    assert!(journal.begin_session().is_err());
                    let after = journal
                        .test_observe_lease()
                        .borrow()
                        .test_persisted_snapshot()
                        .unwrap();
                    if mode == 0 {
                        assert_eq!(after, before)
                    } else {
                        assert!(after.len() > before.len());
                        assert!(after.starts_with(&before));
                    }
                    assert_eq!(leaves[0].strong_count(), 1);
                    assert_eq!(leaves[1].strong_count(), if settled { 0 } else { 1 });
                    drop(failure);
                    assert!(leaves.iter().all(|leaf| leaf.upgrade().is_none()));
                },
                true,
            );
        }
    }
}
