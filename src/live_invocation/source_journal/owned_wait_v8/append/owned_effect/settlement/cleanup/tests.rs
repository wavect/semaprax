//! Actual host1 and physical ACKs. No raw engine ACK or caller ledger.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::cleanup::{LiveCleanupAcknowledgedV8,LiveOutcomeV8,LiveEffectCleanupFailureV8,LiveExecutedOwnedEffectV8};
use std::cell::Cell;
use crate::agent_lifecycle::authorization::target_protocol::{
    TargetHostError, TargetHostHandler, TargetHostRequest, TargetResponseSink,
    TypedCarrier,
};
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::tests::test_ready_obligation;
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
    clock: &'j dyn SourceInvocationClock,
) -> (crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::LiveOwnedEffectSettlementAppendV8<'j>, Vec<Weak<[u8]>>){
    let (ready, weak) = test_ready_obligation(journal, cancel, clock, policy);
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

fn cleanup<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j AgentCancellation,
    policy: &'j CapabilityPolicy,
    host: &mut Host,
    clock: &'j dyn SourceInvocationClock,
) -> (LiveOwnedEffectCleanupAppendV8<'j>, Vec<Weak<[u8]>>) {
    use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::LiveEffectSettlementAcknowledgedV8;
    let (selected, weak) = obligation(journal, cancel, policy, host, clock);
    let envelope = journal
        .begin_session()
        .unwrap()
        .append_owned_effect_settlement(selected)
        .unwrap_or_else(|_| panic!("ordinary ACK"));
    let LiveEffectSettlementAcknowledgedV8::Settled(owner) = envelope
        .advance_settlement()
        .unwrap_or_else(|_| panic!("settled owner"))
    else {
        panic!("ordinary");
    };
    let selected = owner
        .prepare_recorded()
        .unwrap_or_else(|_| panic!("Recorded selection"));
    let envelope = journal
        .begin_session()
        .unwrap()
        .append_owned_effect_settlement(selected)
        .unwrap_or_else(|_| panic!("Recorded ACK"));
    let LiveEffectSettlementAcknowledgedV8::Recorded(owner) = envelope
        .advance_settlement()
        .unwrap_or_else(|_| panic!("Recorded owner"))
    else {
        panic!("Recorded");
    };
    (
        owner
            .prepare_cleanup()
            .unwrap_or_else(|_| panic!("Started selection")),
        weak,
    )
}
fn started<'j>(journal:&'j SourceOwnedWaitJournalV8,selected:LiveOwnedEffectCleanupAppendV8<'j>)->crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::cleanup::LiveStartedOwnedEffectV8<'j>{
    let envelope = journal
        .begin_session()
        .unwrap()
        .append_owned_effect_cleanup(selected)
        .unwrap_or_else(|_| panic!("Started ACK"));
    let LiveCleanupAcknowledgedV8::Started(owner) = envelope
        .advance_cleanup()
        .unwrap_or_else(|_| panic!("Started owner"))
    else {
        panic!("Started");
    };
    owner
}
fn cleaned<'j>(journal:&'j SourceOwnedWaitJournalV8,selected:LiveOwnedEffectCleanupAppendV8<'j>)->crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::cleanup::LiveCleanedOwnedEffectV8<'j>{
    let envelope = journal
        .begin_session()
        .unwrap()
        .append_owned_effect_cleanup(selected)
        .unwrap_or_else(|_| panic!("Settled ACK"));
    let LiveCleanupAcknowledgedV8::Settled(owner) = envelope
        .advance_cleanup()
        .unwrap_or_else(|_| panic!("Settled owner"))
    else {
        panic!("Settled");
    };
    owner
}
/// Genuine upstream SDK, host1, actual Decision release, actual receipt ACK;
/// the callback receives the opaque actual Executed, never reconstructed roots.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_executed<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j AgentCancellation,
    policy: &'j CapabilityPolicy,
    clock: &'j dyn SourceInvocationClock,
    callback: impl FnOnce(LiveExecutedOwnedEffectV8<'j>, Vec<Weak<[u8]>>),
) {
    let mut host = Host { calls: 0, mode: 0 };
    let (selected, weak) = cleanup(journal, cancel, policy, &mut host, clock);
    let actual = started(journal, selected)
        .release_decision(|_| assert_eq!(weak[1].strong_count(), 0))
        .unwrap_or_else(|_| panic!("actual release"));
    let selected = actual
        .prepare_settled()
        .unwrap_or_else(|_| panic!("real receipt"));
    let LiveOutcomeV8::Executed(executed) = cleaned(journal, selected)
        .advance_outcome()
        .unwrap_or_else(|_| panic!("actual fresh Outcome"))
    else {
        panic!("success");
    };
    let outcome = executed.test_outcome_weak();
    assert_eq!(outcome.strong_count(), 1);
    assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
    assert_eq!(host.calls, 1);
    callback(executed, vec![weak[0].clone(), outcome]);
    assert!(weak.iter().all(|w| w.upgrade().is_none()));
}
#[test]
fn owned_effect_cleanup_append_releases_once_after_started_and_mints_unique_outcome_after_receipt_ack(
) {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        test_executed(&journal, &cancel, &policy, &Clock, |executed, weak| {
            executed.validate_live().unwrap();
            assert_eq!(executed.accounting().calls(), 1);
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(executed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    });
}
#[test]
fn owned_effect_cleanup_append_incurs_release_and_receipt_despite_cancellation_but_refuses_outcome()
{
    for in_observer in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let mut host = Host { calls: 0, mode: 0 };
            let (selected, weak) = cleanup(&journal, &cancel, &policy, &mut host, &Clock);
            let owner = started(&journal, selected);
            if !in_observer {
                cancel.cancel();
            }
            owner.validate_live().unwrap();
            let mut observations = 0;
            let released = owner
                .release_decision(|_| {
                    observations += 1;
                    assert_eq!(weak[1].strong_count(), 0);
                    if in_observer {
                        cancel.cancel();
                    }
                })
                .unwrap_or_else(|_| panic!("incurred release"));
            assert_eq!(observations, 1);
            let selected = released
                .prepare_settled()
                .unwrap_or_else(|_| panic!("actual receipt despite cancel"));
            let cleaned = cleaned(&journal, selected);
            assert!(matches!(
                cleaned.advance_outcome(),
                Err(LiveEffectCleanupFailureV8::Cleaned { .. })
            ));
            assert!(journal.hold().is_err());
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
            assert_eq!(host.calls, 1);
        });
    }
}
#[test]
fn owned_effect_cleanup_append_failed_target_and_failed_observer_never_create_outcome() {
    for mode in 0..3 {
        for panic_observer in [false, true] {
            CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
                let context = context.with_initialization(&lease).unwrap();
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                let cancel = AgentCancellation::new();
                let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                let mut host = Host { calls: 0, mode };
                let (selected, weak) = cleanup(&journal, &cancel, &policy, &mut host, &Clock);
                let released = started(&journal, selected)
                    .release_decision(|_| {
                        if panic_observer {
                            panic!("actual observer failure");
                        }
                    })
                    .unwrap_or_else(|_| panic!("release retains failed receipt"));
                let selected = released
                    .prepare_settled()
                    .unwrap_or_else(|_| panic!("selected actual receipt"));
                let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled{receipt,..})=selected.selected_row() else{panic!("receipt");};
                assert_eq!(
                    receipt["settlement"],
                    if panic_observer {
                        "failed"
                    } else {
                        "completed"
                    }
                );
                let actual = cleaned(&journal, selected)
                    .advance_outcome()
                    .unwrap_or_else(|_| panic!("actual failed/success owner"));
                match actual {
                    LiveOutcomeV8::Executed(owner) => {
                        assert_eq!(mode, 0);
                        assert!(!panic_observer);
                        drop(owner);
                    }
                    LiveOutcomeV8::Failed(owner) => {
                        assert!(mode != 0 || panic_observer);
                        let failure = owner.failure().unwrap();
                        if mode == 0 {
                            assert_eq!(failure,crate::interpreter::resumable::owned_frame::registered_stage::effect::OwnedEffectFailureV8::ObservationFailed);
                        }
                        drop(owner);
                    }
                }
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
                assert_eq!(host.calls, 1);
            });
        }
    }
}
struct ArmedClock {
    armed: Cell<bool>,
    calls: Cell<usize>,
}
impl crate::live_invocation::InvocationClock for ArmedClock {
    fn now_millis(&self) -> i64 {
        self.calls.set(self.calls.get() + 1);
        assert!(
            !self.armed.get(),
            "fresh clock callback after incurred cleanup"
        );
        1
    }
}
impl SourceInvocationClock for ArmedClock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_effect_cleanup_append_incurs_release_without_fresh_clock_then_restores_outcome_clock_guard(
) {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let mut host = Host { calls: 0, mode: 0 };
        let clock = ArmedClock {
            armed: Cell::new(false),
            calls: Cell::new(0),
        };
        let (selected, weak) = cleanup(&journal, &cancel, &policy, &mut host, &clock);
        let owner = started(&journal, selected);
        let before = clock.calls.get();
        clock.armed.set(true);
        let released = owner
            .release_decision(|_| assert_eq!(weak[1].strong_count(), 0))
            .unwrap_or_else(|_| panic!("no fresh clock during release"));
        let selected = released
            .prepare_settled()
            .unwrap_or_else(|_| panic!("real receipt no clock"));
        let cleaned = cleaned(&journal, selected);
        assert_eq!(clock.calls.get(), before);
        assert!(matches!(
            cleaned.advance_outcome(),
            Err(LiveEffectCleanupFailureV8::Cleaned { .. })
        ));
        assert!(clock.calls.get() > before);
        assert!(journal.hold().is_err());
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[cfg(unix)]
#[test]
fn owned_effect_cleanup_append_real_write_sync_faults_never_release_or_mint_without_ack() {
    for settled in [false, true] {
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
                    let (selected, weak) = cleanup(&journal, &cancel, &policy, &mut host, &Clock);
                    let selected = if settled {
                        started(&journal, selected)
                            .release_decision(|_| assert_eq!(weak[1].strong_count(), 0))
                            .unwrap_or_else(|_| panic!("actual release"))
                            .prepare_settled()
                            .unwrap_or_else(|_| panic!("real receipt"))
                    } else {
                        selected
                    };
                    let before = bytes(&journal);
                    let line = encode_row(
                        &journal,
                        selected.selected_row(),
                        selected.sequence(),
                        &before,
                    );
                    let n = selected.sequence() + 1;
                    {
                        let mut lease = journal.lease.try_borrow_mut().unwrap();
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
                        .append_owned_effect_cleanup(selected)
                        .err()
                        .expect("uncertain physical ACK");
                    assert!(matches!(
                        &failure,
                        LiveOwnedEffectCleanupAppendFailureV8::Append {
                            _failure: AppendFailureV8::InDoubt { .. },
                            ..
                        }
                    ));
                    assert_eq!(
                        [weak[0].strong_count(), weak[1].strong_count()],
                        if settled { [1, 0] } else { [1, 1] }
                    );
                    assert!(journal.hold().is_err());
                    assert!(journal.begin_session().is_err());
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
                    drop(failure);
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                    assert_eq!(host.calls, 1);
                },
            );
        }
    }
}

fn encode_row(
    journal: &SourceOwnedWaitJournalV8,
    row: &EntryV8,
    seq: usize,
    prefix: &[u8],
) -> Vec<u8> {
    let line = prefix.split_inclusive(|b| *b == b'\n').last().unwrap();
    let header = wire::parse(&line[..line.len() - 1]).unwrap();
    wire::encode(
        row,
        &ExpectedRowV8 {
            invocation: journal.context.ordinary().invocation(),
            generation: journal.context.generation(),
            seq: u32::try_from(seq).unwrap(),
            prev_mac: header["authentication"].as_str().unwrap(),
            ordinary: journal.context.ordinary(),
        },
        &journal.key,
    )
    .unwrap()
}
#[test]
fn owned_effect_cleanup_append_reminted_started_vector_refs_and_settled_receipt_refuse_without_io()
{
    for settled in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let mut host = Host { calls: 0, mode: 0 };
            let (selected, weak) = cleanup(&journal, &cancel, &policy, &mut host, &Clock);
            let selected = if settled {
                started(&journal, selected)
                    .release_decision(|_| {})
                    .unwrap_or_else(|_| panic!("actual release"))
                    .prepare_settled()
                    .unwrap_or_else(|_| panic!("actual receipt"))
            } else {
                selected
            };
            let prefix = bytes(&journal);
            for mutation in 0..3 {
                let mut row = selected.selected_row().clone();
                use crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8;
                match &mut row {
            EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted{turn,attempt,staged,ready,consumed,intent,settlement,recorded,decision_digest,operations,operations_digest,..})=>match mutation {
              0=>*recorded=recorded.checked_sub(1).unwrap(),
              1=>*decision_digest="sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
              _=>{*operations=serde_json::json!([]);*operations_digest=wire::recipe_digest(wire::RecipeV8::EffectDecisionOperations,&serde_json::json!({"turn":turn,"attempt":attempt,"staged":staged,"ready":ready,"consumed":consumed,"intent":intent,"settlement":settlement,"recorded":recorded,"decision_digest":decision_digest,"operations":operations})).unwrap();}
            },
            EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupSettled{started,receipt,receipt_digest,..})=>match mutation {
              0=>*started=started.checked_sub(1).unwrap(),
              1=>{receipt["operations"]=serde_json::json!([]);*receipt_digest=wire::recipe_digest(wire::RecipeV8::Receipt,receipt).unwrap();}
              _=>*receipt_digest="sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
            },_=>panic!("closed cleanup row"),
          }
                let encoded = encode_row(&journal, &row, selected.sequence(), &prefix);
                let lease = journal.lease.try_borrow().unwrap();
                assert!(crate::live_invocation::source_journal::owned_wait_v8::inventory::checked_candidate_inventory_v8(&journal.context,&lease,&journal.key,&prefix,&encoded).is_err());
                drop(lease);
                assert_eq!(bytes(&journal), prefix);
                assert_eq!(
                    [weak[0].strong_count(), weak[1].strong_count()],
                    if settled { [1, 0] } else { [1, 1] }
                );
            }
            selected.validate_live().unwrap();
            assert_eq!(host.calls, 1);
        });
    }
}
