//! Actual host1 and physical ACKs. No raw engine ACK or caller ledger.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{
    TargetAccounting, TargetHostError, TargetHostHandler, TargetHostRequest, TargetResponseSink,
    TypedCarrier,
};
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::OwnedReduceHoldPhaseV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::tests::test_ready_obligation;
use crate::live_invocation::source_journal::{
    SourceEffectFailure, SourceStopReason, SourceStopStatus,
};
use crate::live_invocation::SourceInvocationClock;
use crate::resumable_effects::CapabilityPolicy;
use std::sync::{Arc, Weak};
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
struct Host {
    calls: usize,
    mode: u8,
}
impl TargetHostHandler for Host {
    fn dispatch(
        &mut self,
        request: &TargetHostRequest,
        sink: &mut TargetResponseSink,
    ) -> Result<(), TargetHostError> {
        self.calls += 1;
        match self.mode {
            1 => Err(TargetHostError::Failed),
            2 => sink
                .write(&vec![0; 65537])
                .map_err(|_| TargetHostError::Failed),
            _ => {
                let payload=b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"value\",\"9\"]]}\n".to_vec();
                let wire = TypedCarrier::new(request.operation().result_type(), payload)
                    .unwrap()
                    .encode();
                sink.write(&wire).map_err(|_| TargetHostError::Failed)
            }
        }
    }
}
fn bytes(journal: &SourceOwnedWaitJournalV8) -> Vec<u8> {
    journal.lease.try_borrow_mut().unwrap().read().unwrap()
}
fn obligation<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j AgentCancellation,
    policy: &'j CapabilityPolicy,
    host: &mut Host,
) -> (LiveOwnedEffectSettlementAppendV8<'j>, Vec<Weak<[u8]>>) {
    let (ready, weak) = test_ready_obligation(journal, cancel, &Clock, policy);
    let ready = journal
        .begin_session()
        .unwrap()
        .append_owned_effect(ready)
        .unwrap_or_else(|_| panic!("Ready"));
    let consumed = ready
        .advance_ready()
        .unwrap_or_else(|_| panic!("Ready owner"));
    let consumed = journal
        .begin_session()
        .unwrap()
        .append_owned_authorization_consumed(consumed)
        .unwrap_or_else(|_| panic!("Consumed"));
    let held = consumed
        .reserve_owned_reduce()
        .unwrap_or_else(|_| panic!("same credit"));
    let prepared = held
        .advance_authorization()
        .unwrap_or_else(|_| panic!("Prepared"));
    let intent = prepared
        .prepare_intent()
        .unwrap_or_else(|_| panic!("selected Intent"));
    let intent = journal
        .begin_session()
        .unwrap()
        .append_owned_effect_intent(intent)
        .unwrap_or_else(|_| panic!("Intent ACK"));
    let activated = intent
        .advance_intent()
        .unwrap_or_else(|_| panic!("actual activation"));
    let dispatched = activated
        .dispatch(host)
        .unwrap_or_else(|_| panic!("actual host"));
    assert_eq!(host.calls, 1);
    let actual = dispatched
        .prepare_settlement()
        .unwrap_or_else(|_| panic!("actual selected settlement"));
    assert!(weak.iter().all(|w| w.strong_count() == 1));
    (actual, weak)
}
fn encoded(
    journal: &SourceOwnedWaitJournalV8,
    actual: &LiveOwnedEffectSettlementAppendV8<'_>,
    prefix: &[u8],
) -> Vec<u8> {
    let line = prefix.split_inclusive(|b| *b == b'\n').last().unwrap();
    let envelope = wire::parse(&line[..line.len() - 1]).unwrap();
    wire::encode(
        actual.selected_row(),
        &ExpectedRowV8 {
            invocation: journal.context.ordinary().invocation(),
            generation: journal.context.generation(),
            seq: u32::try_from(actual.sequence()).unwrap(),
            prev_mac: envelope["authentication"].as_str().unwrap(),
            ordinary: journal.context.ordinary(),
        },
        &journal.key,
    )
    .unwrap()
}
fn recorded<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    actual: LiveOwnedEffectSettlementAppendV8<'j>,
) -> LiveOwnedEffectSettlementAppendV8<'j> {
    let envelope = journal
        .begin_session()
        .unwrap()
        .append_owned_effect_settlement(actual)
        .unwrap_or_else(|_| panic!("ordinary settlement ACK"));
    let LiveEffectSettlementAcknowledgedV8::Settled(settled) = envelope
        .advance_settlement()
        .unwrap_or_else(|_| panic!("same actual settled owner"))
    else {
        panic!("ordinary successor");
    };
    settled
        .prepare_recorded()
        .unwrap_or_else(|_| panic!("actual Recorded"))
}
#[test]
fn owned_effect_settlement_append_preserves_live_owner_ledger_credit_and_exact_recorded_bytes() {
    for mode in 0..3 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let mut host = Host { calls: 0, mode };
            let (actual, weak) = obligation(&journal, &cancel, &policy, &mut host);
            match actual.selected_row() {
                EntryV8::Ordinary(SourceJournalEntry::EffectObserved { observation, .. })
                    if mode == 0 =>
                {
                    assert!(!observation.is_empty())
                }
                EntryV8::Ordinary(SourceJournalEntry::EffectFailed { reason, .. }) if mode == 1 => {
                    assert_eq!(*reason, SourceEffectFailure::HandlerFailed)
                }
                EntryV8::Ordinary(SourceJournalEntry::EffectFailed { reason, .. }) if mode == 2 => {
                    assert_eq!(*reason, SourceEffectFailure::ResultLimit)
                }
                _ => panic!("actual selected outcome"),
            }
            let before = bytes(&journal);
            let line = encoded(&journal, &actual, &before);
            let seq = actual.sequence();
            let envelope = journal
                .begin_session()
                .unwrap()
                .append_owned_effect_settlement(actual)
                .unwrap_or_else(|_| panic!("settlement ACK"));
            envelope.validate_live().unwrap();
            assert_eq!(envelope.session.sequence(), seq + 1);
            assert_ne!(
                envelope.witness.predecessor.authentication,
                envelope.witness.successor.authentication
            );
            let mut expected = before;
            expected.extend(line);
            assert_eq!(bytes(&journal), expected);
            let LiveEffectSettlementAcknowledgedV8::Settled(settled) = envelope
                .advance_settlement()
                .unwrap_or_else(|_| panic!("real settled"))
            else {
                panic!("settled");
            };
            let actual = settled
                .prepare_recorded()
                .unwrap_or_else(|_| panic!("Recorded selected"));
            let line = encoded(&journal, &actual, &expected);
            let envelope = journal
                .begin_session()
                .unwrap()
                .append_owned_effect_settlement(actual)
                .unwrap_or_else(|_| panic!("Recorded ACK"));
            envelope.validate_live().unwrap();
            let LiveEffectSettlementAcknowledgedV8::Recorded(actual) = envelope
                .advance_settlement()
                .unwrap_or_else(|_| panic!("real recorded"))
            else {
                panic!("recorded");
            };
            actual.validate_live().unwrap();
            expected.extend(line);
            assert_eq!(bytes(&journal), expected);
            assert_eq!(host.calls, 1);
            assert_ne!(*actual.accounting(), TargetAccounting::default());
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            let registry = journal.prospective_reduce.borrow();
            let r = registry.as_ref().unwrap();
            assert_eq!(r.fuel, 1000);
            assert!(matches!(&r.phase, OwnedReduceHoldPhaseV8::Recorded { .. }));
            let id = r.identity;
            drop(registry);
            assert_eq!(id, 1);
            assert!(journal
                .begin_session()
                .unwrap()
                .append(EntryV8::Ordinary(SourceJournalEntry::Stop {
                    turn: Some(0),
                    attempt: Some(0),
                    reason: SourceStopReason::EffectFailed,
                    status: SourceStopStatus::EffectFailed
                }))
                .is_err());
            assert_eq!(bytes(&journal), expected);
            drop(actual);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
fn owned_effect_settlement_append_reminted_recorded_result_evidence_and_refs_refuse_without_release(
) {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let mut host = Host { calls: 0, mode: 0 };
        let (actual, weak) = obligation(&journal, &cancel, &policy, &mut host);
        let actual = recorded(&journal, actual);
        let prefix = bytes(&journal);
        for mutation in 0..4 {
            let mut row = actual.selected_row().clone();
            let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectSettlementRecorded{intent,settlement,evidence,evidence_digest,result_wire,..})=&mut row else{panic!("Recorded");};
            match mutation {
                0 => *intent = intent.checked_add(1).unwrap(),
                1 => *settlement = settlement.checked_sub(1).unwrap(),
                2 => {
                    *evidence_digest =
                        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                            .into()
                }
                _ => {
                    result_wire.as_mut().unwrap().replace_range(0..2, "ff");
                    assert!(!evidence.is_empty());
                }
            }
            let line = prefix.split_inclusive(|b| *b == b'\n').last().unwrap();
            let header = wire::parse(&line[..line.len() - 1]).unwrap();
            let encoded = wire::encode(
                &row,
                &ExpectedRowV8 {
                    invocation: journal.context.ordinary().invocation(),
                    generation: journal.context.generation(),
                    seq: u32::try_from(actual.sequence()).unwrap(),
                    prev_mac: header["authentication"].as_str().unwrap(),
                    ordinary: journal.context.ordinary(),
                },
                &journal.key,
            )
            .unwrap();
            let lease = journal.lease.try_borrow().unwrap();
            assert!(crate::live_invocation::source_journal::owned_wait_v8::inventory::checked_candidate_inventory_v8(&journal.context,&lease,&journal.key,&prefix,&encoded).is_err());
            drop(lease);
            assert_eq!(bytes(&journal), prefix);
            assert!(weak.iter().all(|w| w.strong_count() == 1));
        }
        actual.validate_live().unwrap();
        assert_eq!(host.calls, 1);
    });
}
#[cfg(unix)]
#[test]
fn owned_effect_settlement_append_all_persistence_faults_retain_actual_owner_and_no_phase_advance()
{
    for recorded_phase in [false, true] {
        for stage in 0..4 {
            CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
                true,
                |context, lease, key, directory| {
                    let context = context.with_initialization(&lease).unwrap();
                    let journal =
                        SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                    let cancel = AgentCancellation::new();
                    let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                    let mut host = Host { calls: 0, mode: 0 };
                    let (actual, weak) = obligation(&journal, &cancel, &policy, &mut host);
                    let actual = if recorded_phase {
                        recorded(&journal, actual)
                    } else {
                        actual
                    };
                    let before = bytes(&journal);
                    let line = encoded(&journal, &actual, &before);
                    let seq = actual.sequence();
                    {
                        let mut lease = journal.lease.try_borrow_mut().unwrap();
                        let n = seq + 1;
                        match stage {
                            0 => lease.test_fail_before_write(n),
                            1 => lease.test_fail_after_write(n),
                            2 => lease.test_fail_before_sync(n),
                            _ => lease.test_fail_after_sync(n),
                        }
                    }
                    let failure = journal
                        .begin_session()
                        .unwrap()
                        .append_owned_effect_settlement(actual)
                        .err()
                        .expect("uncertain ACK");
                    assert!(matches!(
                        &failure,
                        LiveOwnedEffectSettlementAppendFailureV8::Append {
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
                    assert_eq!(r.sequence, seq);
                    assert_eq!(r.fuel, 1000);
                    assert!(if recorded_phase {
                        matches!(&r.phase, OwnedReduceHoldPhaseV8::Settlement { .. })
                    } else {
                        matches!(&r.phase, OwnedReduceHoldPhaseV8::Intent { .. })
                    });
                    drop(registry);
                    assert!(journal.hold().is_err());
                    assert!(journal.begin_session().is_err());
                    assert!(weak.iter().all(|w| w.strong_count() == 1));
                    assert_eq!(host.calls, 1);
                    drop(failure);
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                },
            );
        }
    }
}
#[test]
fn owned_effect_settlement_append_stale_same_container_poison_and_entry_cancel_do_no_io() {
    for cancel_entry in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let mut host = Host { calls: 0, mode: 0 };
            let (actual, weak) = obligation(&journal, &cancel, &policy, &mut host);
            let stale = journal.begin_session().unwrap();
            let actual = if cancel_entry {
                cancel.cancel();
                actual
            } else {
                recorded(&journal, actual)
            };
            let before = bytes(&journal);
            let failure = stale
                .append_owned_effect_settlement(actual)
                .err()
                .expect("stale or cancelled actual owner");
            assert_eq!(bytes(&journal), before);
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            assert_eq!(host.calls, 1);
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(failure);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
fn owned_effect_settlement_append_wrong_container_has_no_io_and_preserves_other_guard() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let mut host = Host { calls: 0, mode: 0 };
        let (actual, weak) = obligation(&journal, &cancel, &policy, &mut host);
        let before = bytes(&journal);
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|other, lease, key| {
            let other = SourceOwnedWaitJournalV8::open(Arc::new(other), key, lease).unwrap();
            let other_before = bytes(&other);
            let failure = other
                .begin_session()
                .unwrap()
                .append_owned_effect_settlement(actual)
                .err()
                .expect("foreign owner refused");
            assert_eq!(bytes(&other), other_before);
            assert_eq!(bytes(&journal), before);
            other.hold().unwrap().validate_guard().unwrap();
            journal.hold().unwrap().validate_guard().unwrap();
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            assert_eq!(host.calls, 1);
            drop(failure);
        });
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
