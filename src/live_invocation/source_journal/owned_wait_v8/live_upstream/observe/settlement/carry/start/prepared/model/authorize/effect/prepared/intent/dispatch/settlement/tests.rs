//! Actual second ordinary settlement and Recorded ACKs; no release/Reduce.
use super::super::super::tests::test_staged;
use super::super::tests::{activated, host};
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{Settlement, TargetEvidence};
fn bytes(journal: &SourceOwnedWaitJournalV8) -> Vec<u8> {
    journal.test_observe_lease().borrow_mut().read().unwrap()
}
fn ack<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: LiveOwnedContinuedSettlementAppendV8<'j>,
) -> LiveContinuedSettlementAcknowledgedV8<'j> {
    let before = bytes(journal);
    let mut expected = before.clone();
    expected.extend(journal.test_continued_settlement_encoded(&owner, &before));
    let result = journal
        .begin_session()
        .unwrap()
        .append_owned_continued_settlement(owner)
        .unwrap_or_else(|_| panic!("actual settlement ACK"));
    assert_eq!(bytes(journal), expected);
    result
        .advance_settlement()
        .unwrap_or_else(|_| panic!("same Staged handoff"))
}
#[test]
fn owned_continued_settlement_recorded_all_supported_targets_match_current_totals_without_new_debit(
) {
    for mode in 0..3 {
        test_staged(
            |journal, staged, _, prior| {
                let (actual, leaves) = activated(journal, staged);
                let old = journal.test_continued_intent_registry();
                let mut host = host(mode);
                let owner = actual
                    .dispatch(&mut host)
                    .unwrap_or_else(|_| panic!("actual host"));
                let current = *owner.accounting();
                let selected = owner
                    .prepare_settlement()
                    .unwrap_or_else(|_| panic!("real cumulative mapper"));
                let evidence = TargetEvidence::decode(selected.phase.facts.evidence()).unwrap();
                evidence
                    .replay_exchange_wire(&host.request, selected.phase.facts.result())
                    .unwrap();
                assert_eq!(evidence.accounting(), current);
                assert_eq!(
                    evidence.settlement(),
                    match mode {
                        0 => Settlement::Returned,
                        1 => Settlement::HostFailed,
                        _ => Settlement::ResultBudget,
                    }
                );
                assert_eq!(current.calls(), prior.calls() + 1);
                assert!(current.request_bytes() > prior.request_bytes());
                let LiveContinuedSettlementAcknowledgedV8::Settled(settled) =
                    ack(journal, selected)
                else {
                    panic!("ordinary first")
                };
                let selected = settled
                    .prepare_recorded()
                    .unwrap_or_else(|_| panic!("real Recorded"));
                let LiveContinuedSettlementAcknowledgedV8::Recorded(recorded) =
                    ack(journal, selected)
                else {
                    panic!("Recorded second")
                };
                recorded.validate_live().unwrap();
                assert_eq!(*recorded.accounting(), current);
                let proof = recorded
                    .phase
                    .current()
                    .continued_settlement_accounting()
                    .unwrap();
                assert_eq!(proof, current);
                let new = journal.test_continued_intent_registry();
                assert_eq!(
                    (&old.0, &old.1, &old.2, &old.3, &old.4),
                    (&new.0, &new.1, &new.2, &new.3, &new.4)
                );
                assert_ne!(old.5, new.5);
                assert_eq!(host.calls, 1);
                assert!(leaves.iter().all(|w| w.strong_count() == 1));
                drop(recorded);
                assert!(leaves.iter().all(|w| w.upgrade().is_none()));
            },
            true,
        );
    }
}
#[cfg(unix)]
fn assert_continued_settlement_physical_ack_row(phase: usize) {
    for mode in 0..4 {
        test_staged(
            |journal, staged, _, _| {
                let (actual, leaves) = activated(journal, staged);
                let mut host = host(0);
                let owner = actual
                    .dispatch(&mut host)
                    .unwrap_or_else(|_| panic!("host"));
                let mut selected = owner
                    .prepare_settlement()
                    .unwrap_or_else(|_| panic!("settlement"));
                if phase == 1 {
                    let LiveContinuedSettlementAcknowledgedV8::Settled(owner) =
                        ack(journal, selected)
                    else {
                        panic!("settled")
                    };
                    selected = owner
                        .prepare_recorded()
                        .unwrap_or_else(|_| panic!("Recorded"));
                }
                let before = bytes(journal);
                let mut expected = before.clone();
                expected.extend(journal.test_continued_settlement_encoded(&selected, &before));
                let number = selected.sequence() + 1;
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
                    .append_owned_continued_settlement(selected)
                    .err()
                    .expect("physical failure");
                assert!(failure.test_is_in_doubt(), "phase {phase} mode {mode}");
                assert_eq!(host.calls, 1);
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

#[cfg(unix)]
#[test]
fn owned_continued_settlement_both_ack_rows_all_physical_windows_are_in_doubt_row_0() {
    assert_continued_settlement_physical_ack_row(0);
}

#[cfg(unix)]
#[test]
fn owned_continued_settlement_both_ack_rows_all_physical_windows_are_in_doubt_row_1() {
    assert_continued_settlement_physical_ack_row(1);
}
#[test]
fn owned_continued_settlement_selected_ordinary_and_recorded_substitutions_refuse_before_write() {
    for phase in 0..2 {
        test_staged(
            |journal, staged, _, _| {
                let (actual, leaves) = activated(journal, staged);
                let mut host = host(0);
                let owner = actual
                    .dispatch(&mut host)
                    .unwrap_or_else(|_| panic!("host"));
                let mut selected = owner
                    .prepare_settlement()
                    .unwrap_or_else(|_| panic!("settlement"));
                if phase == 1 {
                    let LiveContinuedSettlementAcknowledgedV8::Settled(owner) =
                        ack(journal, selected)
                    else {
                        panic!("settled")
                    };
                    selected = owner
                        .prepare_recorded()
                        .unwrap_or_else(|_| panic!("Recorded"));
                }
                let before = bytes(journal);
                match &mut selected.selected {
                    EntryV8::Ordinary(SourceJournalEntry::EffectObserved {
                        observation_digest,
                        ..
                    }) => *observation_digest = "sha256:foreign".into(),
                    EntryV8::Owned(OwnedBodyV8::OwnedEffectSettlementRecorded {
                        intent, ..
                    }) => *intent = intent.checked_add(1).unwrap(),
                    _ => panic!("actual selected"),
                };
                let failure = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continued_settlement(selected)
                    .err()
                    .expect("mismatch");
                assert_eq!(host.calls, 1);
                assert!(journal.hold().is_err());
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
                assert!(leaves.iter().all(|w| w.strong_count() == 1));
                drop(failure);
                assert!(leaves.iter().all(|w| w.upgrade().is_none()));
            },
            true,
        );
    }
}
