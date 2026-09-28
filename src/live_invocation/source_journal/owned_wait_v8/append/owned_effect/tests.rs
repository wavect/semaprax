//! Genuine source/SDK/authorize lineage and physical Ready persistence only.
//! No target dispatch, effect execution, seal release, Outcome or Reduce ACK.
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::tests::test_ready_obligation;
use crate::live_invocation::SourceInvocationClock;
use crate::resumable_effects::CapabilityPolicy;
use std::path::Path;

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
fn physical_path(directory: &Path) -> std::path::PathBuf {
    let entries = std::fs::read_dir(directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1, "exact fixture journal inventory");
    entries.into_iter().next().unwrap()
}
fn bytes(journal: &SourceOwnedWaitJournalV8) -> Vec<u8> {
    journal.lease.try_borrow_mut().unwrap().read().unwrap()
}
fn expected_ready(
    journal: &SourceOwnedWaitJournalV8,
    obligation: &LiveOwnedEffectAppendV8<'_>,
    prefix: &[u8],
) -> Vec<u8> {
    let line = prefix.split_inclusive(|b| *b == b'\n').last().unwrap();
    let mac = wire::parse(&line[..line.len() - 1]).unwrap()["authentication"]
        .as_str()
        .unwrap()
        .to_owned();
    wire::encode(
        obligation.selected_row(),
        &ExpectedRowV8 {
            invocation: journal.context.ordinary().invocation(),
            generation: journal.context.generation(),
            seq: u32::try_from(obligation.sequence()).unwrap(),
            prev_mac: &mac,
            ordinary: journal.context.ordinary(),
        },
        &journal.key,
    )
    .unwrap()
}
#[test]
fn owned_effect_ready_append_preserves_actual_owner_and_exact_predecessor_successor_wire() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (obligation, weak) = test_ready_obligation(&journal, &cancel, &Clock, &policy);
        let before = bytes(&journal);
        let selected = obligation.selected_row().clone();
        let sequence = obligation.sequence();
        assert_eq!(sequence, 20, "actual initialized source fixture prefix");
        assert_eq!(obligation.acknowledged_bytes(), before.len());
        assert!(weak.len() >= 2, "State backing plus actual seal");
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        let encoded = expected_ready(&journal, &obligation, &before);
        let envelope = journal
            .begin_session()
            .unwrap()
            .append_owned_effect(obligation)
            .unwrap_or_else(|_| panic!("actual Ready append"));
        assert_eq!(
            envelope.obligation.sequence(),
            sequence,
            "original obligation has not advanced"
        );
        assert_eq!(envelope.session.sequence(), sequence + 1);
        assert_eq!(envelope.witness.sequence(), sequence + 1);
        assert_eq!(
            envelope.witness.acknowledged_bytes(),
            before.len() + encoded.len()
        );
        assert_eq!(envelope.obligation.selected_row(), &selected);
        assert_ne!(
            envelope.witness.predecessor.authentication,
            envelope.witness.successor.authentication
        );
        let mut expected = before.clone();
        expected.extend(encoded);
        assert_eq!(bytes(&journal), expected);
        envelope.validate_live().unwrap();
        envelope
            .witness
            .validate_predecessor(&journal, sequence, before.len(), &selected)
            .unwrap();
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        // Old pre-Ready validation cannot certify the persisted successor. Do
        // this last: old-prefix detection may irreversibly poison that lineage.
        assert!(envelope.obligation.validate_live().is_err());
        drop(envelope);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[test]
fn owned_effect_ready_append_witness_refuses_intervening_legal_inert_consumed_row() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (obligation, weak) = test_ready_obligation(&journal, &cancel, &Clock, &policy);
        let selected = obligation.selected_row().clone();
        let envelope = journal
            .begin_session()
            .unwrap()
            .append_owned_effect(obligation)
            .unwrap_or_else(|_| panic!());
        let duplicate = journal.begin_session().unwrap().append(selected.clone());
        let Err(AppendFailureV8::CandidateRefused { session, .. }) = duplicate else {
            panic!("duplicate Ready never advances")
        };
        assert_eq!(session.sequence(), envelope.session.sequence());
        envelope.validate_live().unwrap();
        let EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationReady {
            turn,
            attempt,
            grant_digest,
            ..
        }) = selected
        else {
            panic!()
        };
        let advanced = match session.append(EntryV8::Ordinary(
            SourceJournalEntry::AuthorizationConsumed {
                turn,
                attempt,
                grant_digest,
            },
        )) {
            Ok(s) => s,
            Err(_) => panic!("existing inert consumed metadata is legal"),
        };
        assert_eq!(advanced.sequence(), envelope.session.sequence() + 1);
        assert_eq!(envelope.validate_live(), Err(SourceJournalError::Order));
        assert!(journal.hold().is_err());
        assert!(journal.begin_session().is_err());
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(envelope);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[test]
fn owned_effect_ready_append_wrong_container_and_entry_cancel_preserve_zero_append_and_roots() {
    for wrong in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (obligation, weak) = test_ready_obligation(&journal, &cancel, &Clock, &policy);
            let before = bytes(&journal);
            if wrong {
                CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
                    |other_context, other_lease, other_key| {
                        let other = SourceOwnedWaitJournalV8::open(
                            Arc::new(other_context),
                            other_key,
                            other_lease,
                        )
                        .unwrap();
                        let other_before = bytes(&other);
                        let failure = other
                            .begin_session()
                            .unwrap()
                            .append_owned_effect(obligation)
                            .err()
                            .expect("wrong container");
                        assert!(matches!(
                            &failure,
                            LiveOwnedEffectAppendFailureV8::Before { .. }
                        ));
                        assert_eq!(bytes(&other), other_before);
                        assert_eq!(bytes(&journal), before);
                        assert!(weak.iter().all(|w| w.strong_count() == 1));
                        drop(failure);
                        assert!(weak.iter().all(|w| w.upgrade().is_none()));
                    },
                );
            } else {
                cancel.cancel();
                let failure = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_effect(obligation)
                    .err()
                    .expect("cancelled entry");
                assert!(matches!(
                    &failure,
                    LiveOwnedEffectAppendFailureV8::Before { .. }
                ));
                assert_eq!(bytes(&journal), before);
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            }
        });
    }
}
#[cfg(unix)]
#[test]
fn owned_effect_ready_append_physical_faults_retain_owner_without_ack_and_poison_all_handles() {
    for stage in 0..4 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |context, lease, key, directory| {
                let context = context.with_initialization(&lease).unwrap();
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                let cancel = AgentCancellation::new();
                let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                let (obligation, weak) = test_ready_obligation(&journal, &cancel, &Clock, &policy);
                let before = bytes(&journal);
                let encoded = expected_ready(&journal, &obligation, &before);
                // Every existing fixture ACK used exactly one physical append; derive
                // the fault ordinal from its actual retained prefix, not row literals.
                let number = obligation.sequence().checked_add(1).unwrap();
                {
                    let mut lease = journal.lease.try_borrow_mut().unwrap();
                    match stage {
                        0 => lease.test_fail_before_write(number),
                        1 => lease.test_fail_after_write(number),
                        2 => lease.test_fail_before_sync(number),
                        3 => lease.test_fail_after_sync(number),
                        _ => unreachable!(),
                    }
                }
                let held = journal.hold().unwrap();
                let failure = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_effect(obligation)
                    .err()
                    .expect("no Ready witness after uncertain persistence");
                assert!(matches!(
                    &failure,
                    LiveOwnedEffectAppendFailureV8::Append {
                        _failure: AppendFailureV8::InDoubt { .. },
                        ..
                    }
                ));
                let mut expected = before;
                if stage != 0 {
                    expected.extend(encoded);
                }
                assert_eq!(std::fs::read(physical_path(directory)).unwrap(), expected);
                assert_eq!(held.validate_guard(), Err(SourceJournalError::Poisoned));
                assert!(journal.begin_session().is_err());
                assert!(!journal.append_active.get());
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            },
        );
    }
}
