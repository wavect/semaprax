//! Genuine held Prepared → physical Intent ACK only. No target dispatch or cleanup.
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::OwnedReduceHoldPhaseV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::tests::test_ready_obligation;
use crate::live_invocation::SourceInvocationClock;
use crate::resumable_effects::CapabilityPolicy;

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
fn bytes(journal: &SourceOwnedWaitJournalV8) -> Vec<u8> {
    journal.lease.try_borrow_mut().unwrap().read().unwrap()
}
fn obligation<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j AgentCancellation,
    policy: &'j CapabilityPolicy,
) -> (
    LiveOwnedEffectIntentAppendV8<'j>,
    Vec<std::sync::Weak<[u8]>>,
) {
    let (ready, weak) = test_ready_obligation(journal, cancel, &Clock, policy);
    let ready = journal
        .begin_session()
        .unwrap()
        .append_owned_effect(ready)
        .unwrap_or_else(|_| panic!("actual Ready ACK"));
    let consumed = ready
        .advance_ready()
        .unwrap_or_else(|_| panic!("actual Ready promotion"));
    let consumed = journal
        .begin_session()
        .unwrap()
        .append_owned_authorization_consumed(consumed)
        .unwrap_or_else(|_| panic!("actual Consumed ACK"));
    let held = consumed
        .reserve_owned_reduce()
        .unwrap_or_else(|_| panic!("actual Reduce credit"));
    let prepared = held
        .advance_authorization()
        .unwrap_or_else(|_| panic!("actual Prepared"));
    let intent = prepared
        .prepare_intent()
        .unwrap_or_else(|_| panic!("actual Intent selection"));
    assert!(weak.iter().all(|w| w.strong_count() == 1));
    (intent, weak)
}
fn encoded(
    journal: &SourceOwnedWaitJournalV8,
    actual: &LiveOwnedEffectIntentAppendV8<'_>,
    prefix: &[u8],
) -> Vec<u8> {
    let line = prefix.split_inclusive(|b| *b == b'\n').last().unwrap();
    let authentication = wire::parse(&line[..line.len() - 1]).unwrap()["authentication"]
        .as_str()
        .unwrap()
        .to_owned();
    wire::encode(
        actual.selected_row(),
        &ExpectedRowV8 {
            invocation: journal.context.ordinary().invocation(),
            generation: journal.context.generation(),
            seq: u32::try_from(actual.sequence()).unwrap(),
            prev_mac: &authentication,
            ordinary: journal.context.ordinary(),
        },
        &journal.key,
    )
    .unwrap()
}
#[test]
fn owned_effect_intent_append_preserves_actual_owner_and_advances_only_same_reserved_lineage() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (actual, weak) = obligation(&journal, &cancel, &policy);
        let before = bytes(&journal);
        let selected = actual.selected_row().clone();
        let old_sequence = actual.sequence();
        let line = encoded(&journal, &actual, &before);
        let old = {
            let registry = journal.prospective_reduce.borrow();
            let r = registry.as_ref().unwrap();
            (
                r.identity,
                r.fuel,
                r.turn,
                r.attempt,
                r.authentication.clone(),
            )
        };
        let verified = journal
            .begin_session()
            .unwrap()
            .append_owned_effect_intent(actual)
            .unwrap_or_else(|_| panic!("actual fixed Intent ACK"));
        verified.validate_live().unwrap();
        assert_eq!(
            verified.obligation.sequence(),
            old_sequence,
            "unchanged original obligation"
        );
        assert_eq!(
            verified.session.sequence(),
            old_sequence.checked_add(1).unwrap()
        );
        verified
            .witness
            .validate_predecessor(&journal, old_sequence, before.len(), &selected)
            .unwrap();
        let mut after = before.clone();
        after.extend(line);
        assert_eq!(bytes(&journal), after);
        let registry = journal.prospective_reduce.borrow();
        let r = registry.as_ref().unwrap();
        assert_eq!(
            (r.identity, r.fuel, r.turn, r.attempt),
            (old.0, old.1, old.2, old.3)
        );
        assert_ne!(r.authentication, old.4);
        assert_eq!(
            (r.sequence, r.bytes),
            (verified.session.sequence(), after.len())
        );
        assert!(
            matches!(&r.phase, OwnedReduceHoldPhaseV8::Intent { selected: entry } if &EntryV8::Ordinary(entry.clone()) == &selected)
        );
        drop(registry);
        assert!(!journal.append_active.get());
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        // No second generic write can borrow the retained future Reduce credit.
        let refused = journal
            .begin_session()
            .unwrap()
            .append(selected.clone())
            .err()
            .expect("Generic remains denied");
        assert!(matches!(&refused, AppendFailureV8::CandidateRefused { .. }));
        assert_eq!(bytes(&journal), after);
        drop(refused);
        verified.validate_live().unwrap();
        // The old Consumed freshness validator is not a post-Intent authority.
        assert!(verified.obligation.validate_live().is_err());
        assert!(journal.hold().is_err());
        drop(verified);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[cfg(unix)]
#[test]
fn owned_effect_intent_append_persistence_faults_never_advance_credit_or_mint_witness() {
    for stage in 0..4 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |context, lease, key, directory| {
                let context = context.with_initialization(&lease).unwrap();
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                let cancel = AgentCancellation::new();
                let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                let (actual, weak) = obligation(&journal, &cancel, &policy);
                let before = bytes(&journal);
                let line = encoded(&journal, &actual, &before);
                let sequence = actual.sequence();
                {
                    let mut lease = journal.lease.try_borrow_mut().unwrap();
                    let number = sequence.checked_add(1).unwrap();
                    match stage {
                        0 => lease.test_fail_before_write(number),
                        1 => lease.test_fail_after_write(number),
                        2 => lease.test_fail_before_sync(number),
                        3 => lease.test_fail_after_sync(number),
                        _ => unreachable!(),
                    }
                }
                let failure = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_effect_intent(actual)
                    .err()
                    .expect("uncertain physical Intent cannot ACK");
                assert!(matches!(
                    &failure,
                    LiveOwnedEffectIntentAppendFailureV8::Append {
                        _failure: AppendFailureV8::InDoubt { .. },
                        ..
                    }
                ));
                let paths = std::fs::read_dir(directory)
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .collect::<Vec<_>>();
                assert_eq!(paths.len(), 1);
                let mut expected = before;
                if stage != 0 {
                    expected.extend(line);
                }
                assert_eq!(std::fs::read(&paths[0]).unwrap(), expected);
                let registry = journal.prospective_reduce.borrow();
                let r = registry.as_ref().unwrap();
                assert_eq!(r.sequence, sequence);
                assert!(matches!(&r.phase, OwnedReduceHoldPhaseV8::Consumed));
                drop(registry);
                assert!(!journal.append_active.get());
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            },
        );
    }
}

#[test]
fn owned_effect_intent_append_wrong_container_is_zero_io_and_cancelled_lineage_is_retired() {
    for wrong_container in [true, false] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (actual, weak) = obligation(&journal, &cancel, &policy);
            let before = bytes(&journal);
            if wrong_container {
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
                            .append_owned_effect_intent(actual)
                            .err()
                            .expect("foreign journal refused");
                        assert!(matches!(
                            &failure,
                            LiveOwnedEffectIntentAppendFailureV8::Before { .. }
                        ));
                        assert_eq!(bytes(&other), other_before);
                        assert_eq!(bytes(&journal), before);
                        other.hold().unwrap().validate_guard().unwrap();
                        journal.hold().unwrap().validate_guard().unwrap();
                        assert!(weak.iter().all(|w| w.strong_count() == 1));
                        drop(failure);
                        assert!(weak.iter().all(|w| w.upgrade().is_none()));
                        other.hold().unwrap().validate_guard().unwrap();
                    },
                );
            } else {
                cancel.cancel();
                let failure = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_effect_intent(actual)
                    .err()
                    .expect("entry cancellation refused");
                assert!(matches!(
                    &failure,
                    LiveOwnedEffectIntentAppendFailureV8::Before { .. }
                ));
                assert_eq!(bytes(&journal), before);
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            }
        });
    }
}

#[test]
fn owned_effect_intent_append_stale_same_container_session_poisons_without_io() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let stale = journal.begin_session().unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (actual, weak) = obligation(&journal, &cancel, &policy);
        assert!(stale.sequence() < actual.sequence());
        let before = bytes(&journal);
        let failure = stale
            .append_owned_effect_intent(actual)
            .err()
            .expect("actual lineage mismatch");
        assert!(matches!(
            &failure,
            LiveOwnedEffectIntentAppendFailureV8::Before { .. }
        ));
        assert_eq!(bytes(&journal), before);
        assert!(journal.hold().is_err());
        assert!(journal.begin_session().is_err());
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(failure);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}

#[test]
fn owned_effect_intent_append_advances_actual_engine_ack_without_dispatch_or_credit_debit() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (actual, weak) = obligation(&journal, &cancel, &policy);
        let sequence = actual.sequence();
        let old = {
            let registry = journal.prospective_reduce.borrow();
            let r = registry.as_ref().unwrap();
            (r.identity, r.fuel, r.turn, r.attempt)
        };
        let envelope = journal
            .begin_session()
            .unwrap()
            .append_owned_effect_intent(actual)
            .unwrap_or_else(|_| panic!("actual Intent ACK"));
        let activated = envelope
            .advance_intent()
            .unwrap_or_else(|_| panic!("actual same-owner engine ACK"));
        activated.validate_live().unwrap();
        let session = journal.begin_session().unwrap();
        assert_eq!(session.sequence(), sequence.checked_add(1).unwrap());
        let registry = journal.prospective_reduce.borrow();
        let r = registry.as_ref().unwrap();
        assert_eq!(
            (r.identity, r.fuel, r.turn, r.attempt),
            old,
            "no credit debit/replace"
        );
        assert_eq!(r.sequence, session.sequence());
        drop(registry);
        drop(session);
        // No host/handler is supplied to this closed activation route.
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(activated);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
        assert!(journal.hold().is_err(), "retained hold has no refund route");
    });
}

#[test]
fn owned_effect_intent_append_post_ack_file_replacement_remains_poisoned_after_restore() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, directory| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (actual, weak) = obligation(&journal, &cancel, &policy);
            let envelope = journal
                .begin_session()
                .unwrap()
                .append_owned_effect_intent(actual)
                .unwrap_or_else(|_| panic!("actual Intent ACK"));
            envelope.validate_live().unwrap();
            let before = bytes(&journal);
            let paths = std::fs::read_dir(directory)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect::<Vec<_>>();
            assert_eq!(paths.len(), 1);
            let entry = &paths[0];
            let displaced = directory.join("displaced-intent-fixture");
            std::fs::rename(entry, &displaced).unwrap();
            std::fs::write(entry, b"").unwrap();
            assert!(envelope.validate_live().is_err());
            std::fs::remove_file(entry).unwrap();
            std::fs::rename(&displaced, entry).unwrap();
            assert_eq!(std::fs::read(entry).unwrap(), before);
            assert!(envelope.validate_live().is_err());
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(envelope);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        },
    );
}
