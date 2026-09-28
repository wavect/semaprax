//! Trusted ACK producers exist only here. They bypass the pending successor
//! journal grammar, while retaining genuine checked runtime and physical pins.
use super::super::authorize::effect_tests::{proposal, ready};
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{
    TargetHostError, TargetHostRequest, TargetResponseSink,
};
use crate::live_invocation::source_journal::{
    CheckedOwnedWaitJournalContextV8, SourceOwnedWaitJournalV8,
};
use crate::resumable_effects::owned_frame::v2::compile_owned_reduce_v2;

const RESULT: &[u8] =
    b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"value\",\"9\"]]}\n";
struct Host<'a> {
    calls: usize,
    cancel: Option<&'a AgentCancellation>,
    panic: bool,
}
impl TargetHostHandler for Host<'_> {
    fn dispatch(
        &mut self,
        request: &TargetHostRequest,
        sink: &mut TargetResponseSink,
    ) -> Result<(), TargetHostError> {
        self.calls += 1;
        if self.panic {
            panic!("target callback")
        }
        if let Some(cancel) = self.cancel {
            cancel.cancel();
        }
        let wire = TypedCarrier::new(request.operation().result_type(), RESULT.to_vec())
            .unwrap()
            .encode();
        let _ = sink.write(&wire);
        Ok(())
    }
}
fn authorization(
    inputs: &OwnedEffectInputsV8<'_>,
    ready: &ReadyOwnedAuthorizeV2,
) -> OwnedEffectAuthorizationAckV8 {
    let (basis, _) = checked_basis_facts(inputs, ready.effect_facts().unwrap()).unwrap();
    OwnedEffectAuthorizationAckV8 {
        basis,
        staged: 20,
        ready: 21,
        consumed: 22,
    }
}
fn intent(prepared: &PreparedOwnedEffectV8<'_>) -> OwnedEffectIntentAckV8 {
    OwnedEffectIntentAckV8 {
        basis: prepared.basis.clone(),
        authorization: prepared.authorization_tail,
        intent: 23,
        request: target_protocol::owned_wait_v8::physical::request_digest(&prepared.request),
        operation: prepared.plan.operation().operation_id().into(),
    }
}
fn settlement(
    staged: &StagedOwnedEffectV8<'_>,
) -> (OwnedEffectSettlementAckV8, OwnedEffectCleanupStartedAckV8) {
    let settlement = OwnedEffectSettlementAckV8 {
        basis: staged.prepared.basis.clone(),
        intent: staged.intent,
        settlement: 24,
        evidence: staged.dispatch().unwrap().evidence().digest().into(),
        operation: staged.prepared.plan.operation().operation_id().into(),
        observation: staged.observation().map(<[u8]>::to_vec),
        reason: staged.reason(),
    };
    let started = OwnedEffectCleanupStartedAckV8 {
        basis: staged.prepared.basis.clone(),
        settlement: 24,
        recorded: 25,
        evidence: settlement.evidence.clone(),
        started: 26,
        operations: owned_wait_operations_v8(
            staged
                .prepared
                .inputs
                .execution
                .wait()
                .authorize()
                .disposal(),
        )
        .unwrap(),
        staged: staged.prepared.staged,
        ready: staged.prepared.ready,
        consumed: staged.prepared.authorization_tail,
        intent: staged.intent,
    };
    (settlement, started)
}
fn settled(pending: &PendingOwnedEffectReceiptV8<'_>) -> OwnedEffectCleanupSettledAckV8 {
    OwnedEffectCleanupSettledAckV8 {
        basis: pending.basis.clone(),
        started: pending.started,
        settled: 27,
        receipt: pending.receipt().clone(),
    }
}

#[test]
fn owned_frame_v8_effect_matching_acks_release_seal_then_mint_unique_outcome_and_stage_actual_reduce(
) {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = Arc::new(context);
        let journal = SourceOwnedWaitJournalV8::open(Arc::clone(&context), key, lease).unwrap();
        let (runtime, execution) = context.test_runtime_execution();
        let b = execution.wait();
        let store = journal.hold().unwrap();
        let k = proposal(b, &store.registration().expected_facts().scope);
        let (ready, weak) = ready(b);
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let cancellation = AgentCancellation::new();
        let inputs = OwnedEffectInputsV8 {
            runtime,
            execution,
            proposal: &k,
            store,
            policy: &policy,
            cancellation: &cancellation,
            turn: 0,
            attempt: 0,
        };
        let mut ack = authorization(&inputs, &ready);
        assert_ne!(ack.basis.grant, ack.basis.target_grant);
        ack.basis.grant = ack.basis.target_grant.clone();
        let rejected = prepare_owned_effect_v8(inputs, ready, ack, |_| true)
            .err()
            .expect("physical grant cannot substitute for Ready grant");
        assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 1]);
        let ack = authorization(&rejected.inputs, &rejected.ready);
        let prepared = prepare_owned_effect_v8(rejected.inputs, rejected.ready, ack, |_| true)
            .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        let mut ack = intent(&prepared);
        ack.request = "wrong".into();
        let mut host = Host {
            calls: 0,
            cancel: None,
            panic: false,
        };
        let mut accounting = TargetAccounting::default();
        let rejected =
            dispatch_owned_effect_v8(prepared, ack, &mut accounting, |_| true, &mut host)
                .err()
                .expect("wrong request ACK");
        assert_eq!(host.calls, 0);
        assert_eq!(accounting, TargetAccounting::default());
        let ack = intent(&rejected.prepared);
        let staged =
            dispatch_owned_effect_v8(rejected.prepared, ack, &mut accounting, |_| true, &mut host)
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        assert_eq!(host.calls, 1);
        assert_eq!(accounting.calls(), 1);
        assert_eq!(accounting.fuel(), 1);
        assert_eq!(staged.observation(), Some(RESULT));
        let target_wire = staged.target_evidence_wire().unwrap();
        let result_wire = staged.target_result_wire().unwrap();
        let ordinary = crate::live_invocation::source_journal::SourceJournalEntry::EffectObserved {
            turn: 0,
            attempt: 0,
            operation: staged.prepared.plan.operation().operation_id().into(),
            observation: RESULT.to_vec(),
            observation_digest: crate::live_invocation::source_journal::source_effect_digest(
                RESULT,
            ),
        };
        let checked =
            target_protocol::owned_wait_v8::settlement::checked_owned_effect_settlement_v8(
                target_protocol::owned_wait_v8::settlement::OwnedEffectSettlementInputsV8 {
                    runtime,
                    execution,
                    scope: &staged
                        .prepared
                        .inputs
                        .store
                        .registration()
                        .expected_facts()
                        .scope,
                    turn: 0,
                    attempt: 0,
                    state: &staged.prepared.basis.state,
                    decision: &staged.prepared.basis.decision,
                    proposal: &k,
                },
                &ordinary,
                &target_wire,
                Some(&result_wire),
            )
            .unwrap();
        assert_eq!(checked.request_digest(), intent(&staged.prepared).request);
        assert!(!staged.cleanup_started());
        let (mut ack, start) = settlement(&staged);
        ack.observation = Some(b"substituted".to_vec());
        let mut observed = 0;
        let rejected =
            release_owned_effect_decision_v8(staged, ack, start, |_| true, |_| observed += 1)
                .err()
                .expect("exact accepted settlement payload");
        assert_eq!(observed, 0);
        assert_eq!(weak[1].strong_count(), 1);
        let mut retained = rejected.staged;
        for mode in 0..2 {
            let (ack, mut start) = settlement(&retained);
            if mode == 0 {
                start.recorded = start.settlement;
            } else {
                start.evidence = "wrong recorded evidence".into();
            }
            retained =
                release_owned_effect_decision_v8(retained, ack, start, |_| true, |_| observed += 1)
                    .err()
                    .expect("matching immediate Recorded ACK required before release")
                    .staged;
            assert_eq!(observed, 0);
            assert_eq!(weak[1].strong_count(), 1);
            assert!(!retained.cleanup_started());
        }
        let (ack, start) = settlement(&retained);
        let pending = release_owned_effect_decision_v8(
            retained,
            ack,
            start,
            |_| true,
            |_| {
                observed += 1;
                assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
            },
        )
        .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        assert_eq!(observed, 1);
        assert_eq!(pending.failure(), None);
        let mut ack = settled(&pending);
        ack.receipt["settlement"] = serde_json::json!("failed");
        let rejected = ack_owned_effect_cleanup_v8(pending, ack, |_| true)
            .err()
            .expect("post-release receipt must match");
        assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
        let ack = settled(&rejected.pending);
        let executed = ack_owned_effect_cleanup_v8(rejected.pending, ack, |_| true)
            .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        let Value::Record(outcome) = executed.roots.outcome.as_ref().unwrap() else {
            panic!()
        };
        let metadata = b.lifecycle().owned_wait_outcome_v8();
        let Value::Bytes(bytes) = &outcome.fields[&metadata.bytes_field] else {
            panic!()
        };
        assert_eq!(bytes.bytes.as_ref(), RESULT);
        assert_eq!(
            bytes.allocation, 3,
            "State=1, released seal=2 stay reserved"
        );
        let outcome_weak = Arc::downgrade(&bytes.bytes);
        assert!(executed.roots.allocations.validate(&[
            executed.roots.state.as_ref().unwrap(),
            executed.roots.outcome.as_ref().unwrap()
        ]));
        let plan = compile_owned_reduce_v2(b).unwrap();
        let prepared =
            super::super::reduce::prepare_executed_owned_reduce_v2(executed, &plan, |_| true)
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        let mut fuel = OwnedFrameBudget::new(1000).unwrap();
        let staged =
            super::super::reduce::stage_executed_owned_reduce_v2(prepared, &mut fuel, |_| true)
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        assert!(staged.failure().is_none());
        assert_eq!(staged.effect_settled(), 27);
        assert!(staged.validate_store());
        assert!(fuel.consumed() > 0);
        assert_eq!(host.calls, 1);
        drop(staged);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
        assert!(outcome_weak.upgrade().is_none());
    });
}

#[test]
fn owned_frame_v8_effect_observer_failure_and_post_callback_authority_loss_never_publish_outcome() {
    for lose_authority in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = Arc::new(context);
            let journal = SourceOwnedWaitJournalV8::open(Arc::clone(&context), key, lease).unwrap();
            let (runtime, execution) = context.test_runtime_execution();
            let store = journal.hold().unwrap();
            let k = proposal(
                execution.wait(),
                &store.registration().expected_facts().scope,
            );
            let (ready, weak) = ready(execution.wait());
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let cancellation = AgentCancellation::new();
            let inputs = OwnedEffectInputsV8 {
                runtime,
                execution,
                proposal: &k,
                store,
                policy: &policy,
                cancellation: &cancellation,
                turn: 0,
                attempt: 0,
            };
            let ack = authorization(&inputs, &ready);
            let prepared = prepare_owned_effect_v8(inputs, ready, ack, |_| true)
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
            let ack = intent(&prepared);
            let mut host = Host {
                calls: 0,
                cancel: None,
                panic: false,
            };
            let mut checks = 0;
            let staged = dispatch_owned_effect_v8(
                prepared,
                ack,
                &mut TargetAccounting::default(),
                |_| {
                    checks += 1;
                    !lose_authority || checks == 1
                },
                &mut host,
            )
            .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
            assert_eq!(host.calls, 1);
            if lose_authority {
                assert_eq!(staged.failure(), Some(OwnedEffectFailureV8::AuthorityLost));
                assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 1]);
                let (ack, start) = settlement(&staged);
                let mut observed = 0;
                let rejected = release_owned_effect_decision_v8(
                    staged,
                    ack,
                    start,
                    |_| true,
                    |_| observed += 1,
                )
                .err()
                .expect("lost authority stays in doubt");
                assert_eq!(observed, 0);
                drop(rejected);
            } else {
                let (ack, start) = settlement(&staged);
                let mut observed_dead = None;
                let pending = release_owned_effect_decision_v8(
                    staged,
                    ack,
                    start,
                    |_| true,
                    |_| {
                        observed_dead = Some((weak[0].strong_count(), weak[1].strong_count()));
                        panic!("observer after actual release")
                    },
                )
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
                assert_eq!(
                    observed_dead,
                    Some((1, 0)),
                    "assert outside caught callback"
                );
                assert_eq!(
                    pending.failure(),
                    Some(OwnedEffectFailureV8::ObservationFailed)
                );
                let ack = settled(&pending);
                let rejected = ack_owned_effect_cleanup_v8(pending, ack, |_| true)
                    .err()
                    .expect("failed observation blocks publication");
                drop(rejected);
            }
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}

#[test]
fn owned_frame_v8_effect_cancel_and_host_panic_keep_charges_and_block_success_after_release() {
    for mode in 0..3 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = Arc::new(context);
            let journal = SourceOwnedWaitJournalV8::open(Arc::clone(&context), key, lease).unwrap();
            let (runtime, execution) = context.test_runtime_execution();
            let store = journal.hold().unwrap();
            let k = proposal(
                execution.wait(),
                &store.registration().expected_facts().scope,
            );
            let (ready, weak) = ready(execution.wait());
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let cancellation = AgentCancellation::new();
            let inputs = OwnedEffectInputsV8 {
                runtime,
                execution,
                proposal: &k,
                store,
                policy: &policy,
                cancellation: &cancellation,
                turn: 0,
                attempt: 0,
            };
            let ack = authorization(&inputs, &ready);
            let prepared = prepare_owned_effect_v8(inputs, ready, ack, |_| true)
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
            let ack = intent(&prepared);
            if mode == 0 {
                cancellation.cancel();
            }
            let mut host = Host {
                calls: 0,
                cancel: (mode == 1).then_some(&cancellation),
                panic: mode == 2,
            };
            let mut accounting = TargetAccounting::default();
            let staged =
                dispatch_owned_effect_v8(prepared, ack, &mut accounting, |_| true, &mut host)
                    .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
            assert!(staged.observation().is_none());
            if mode == 0 {
                assert_eq!(host.calls, 0);
                assert_eq!(accounting, TargetAccounting::default());
                assert_eq!(staged.failure(), Some(OwnedEffectFailureV8::Cancelled));
                assert!(
                    staged.dispatch().is_none(),
                    "bare intent remains non-dispatchable staged holder"
                );
                assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 1]);
                drop(staged);
            } else {
                assert_eq!(host.calls, 1);
                assert_eq!(accounting.calls(), 1);
                assert_eq!(accounting.fuel(), 1);
                assert_eq!(
                    staged.reason(),
                    Some(if mode == 1 {
                        SourceEffectFailure::Cancelled
                    } else {
                        SourceEffectFailure::HandlerFailed
                    })
                );
                assert_eq!(
                    staged.dispatch().unwrap().evidence().settlement(),
                    if mode == 1 {
                        Settlement::CancelledAfterDispatch
                    } else {
                        Settlement::HostPanicked
                    }
                );
                let selected = staged.failure();
                let (ack, start) = settlement(&staged);
                let mut observed = Vec::new();
                let pending = release_owned_effect_decision_v8(
                    staged,
                    ack,
                    start,
                    |_| true,
                    |_| observed.push((weak[0].strong_count(), weak[1].strong_count())),
                )
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
                assert_eq!(observed, [(1, 0)]);
                assert_eq!(
                    pending.failure(),
                    selected,
                    "cleanup retains selected cancellation/host failure"
                );
                let ack = settled(&pending);
                let rejected = ack_owned_effect_cleanup_v8(pending, ack, |_| true)
                    .err()
                    .expect("failed effect cannot become Executed");
                assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                drop(rejected);
                assert_eq!(host.calls, 1);
            }
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}

#[test]
fn owned_frame_v8_effect_policy_refusal_and_lost_cleanup_guard_preserve_owners_without_retry() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = Arc::new(context);
        let journal = SourceOwnedWaitJournalV8::open(Arc::clone(&context), key, lease).unwrap();
        let (runtime, execution) = context.test_runtime_execution();
        let store = journal.hold().unwrap();
        let k = proposal(
            execution.wait(),
            &store.registration().expected_facts().scope,
        );
        let (ready, weak) = ready(execution.wait());
        let denied = CapabilityPolicy::new(vec![]).unwrap();
        let allowed = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let cancellation = AgentCancellation::new();
        let inputs = OwnedEffectInputsV8 {
            runtime,
            execution,
            proposal: &k,
            store,
            policy: &denied,
            cancellation: &cancellation,
            turn: 0,
            attempt: 0,
        };
        let ack = authorization(&inputs, &ready);
        let rejected = prepare_owned_effect_v8(inputs, ready, ack, |_| true)
            .err()
            .expect("current caller policy required");
        assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 1]);
        let inputs = OwnedEffectInputsV8 {
            policy: &allowed,
            ..rejected.inputs
        };
        let ack = authorization(&inputs, &rejected.ready);
        let prepared = prepare_owned_effect_v8(inputs, rejected.ready, ack, |_| true)
            .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        let ack = intent(&prepared);
        let mut host = Host {
            calls: 0,
            cancel: None,
            panic: false,
        };
        let staged = dispatch_owned_effect_v8(
            prepared,
            ack,
            &mut TargetAccounting::default(),
            |_| true,
            &mut host,
        )
        .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        let (ack, start) = settlement(&staged);
        let authority = std::cell::Cell::new(true);
        let mut observed = Vec::new();
        let rejected = release_owned_effect_decision_v8(
            staged,
            ack,
            start,
            |_| authority.get(),
            |_| {
                observed.push((weak[0].strong_count(), weak[1].strong_count()));
                authority.set(false);
            },
        )
        .err()
        .expect("authority loss after drop is cleanup in doubt");
        assert_eq!(observed, [(1, 0)]);
        assert!(rejected.staged.cleanup_started);
        let (ack, start) = settlement(&rejected.staged);
        let mut repeated = 0;
        let rejected = release_owned_effect_decision_v8(
            rejected.staged,
            ack,
            start,
            |_| true,
            |_| repeated += 1,
        )
        .err()
        .expect("started cleanup cannot retry even after guard restoration");
        assert_eq!(repeated, 0);
        assert_eq!(host.calls, 1);
        assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
        drop(rejected);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}

#[test]
fn owned_frame_v8_effect_final_guard_cancellation_refuses_outcome_and_reducer_entry() {
    for mode in 0..3 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = Arc::new(context);
            let journal = SourceOwnedWaitJournalV8::open(Arc::clone(&context), key, lease).unwrap();
            let (runtime, execution) = context.test_runtime_execution();
            let store = journal.hold().unwrap();
            let k = proposal(
                execution.wait(),
                &store.registration().expected_facts().scope,
            );
            let (ready, weak) = ready(execution.wait());
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let cancellation = AgentCancellation::new();
            let inputs = OwnedEffectInputsV8 {
                runtime,
                execution,
                proposal: &k,
                store,
                policy: &policy,
                cancellation: &cancellation,
                turn: 0,
                attempt: 0,
            };
            let ack = authorization(&inputs, &ready);
            let prepared = prepare_owned_effect_v8(inputs, ready, ack, |_| true)
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
            let ack = intent(&prepared);
            let mut host = Host {
                calls: 0,
                cancel: None,
                panic: false,
            };
            let staged = dispatch_owned_effect_v8(
                prepared,
                ack,
                &mut TargetAccounting::default(),
                |_| true,
                &mut host,
            )
            .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
            let (ack, start) = settlement(&staged);
            let pending = release_owned_effect_decision_v8(staged, ack, start, |_| true, |_| {})
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
            let ack = settled(&pending);
            if mode == 0 {
                let mut checks = 0;
                let rejected = ack_owned_effect_cleanup_v8(pending, ack, |_| {
                    checks += 1;
                    if checks == 2 {
                        cancellation.cancel();
                    }
                    true
                })
                .err()
                .expect("cancel on last pre-mint callback must be observed");
                assert_eq!(checks, 2);
                assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                assert_eq!(rejected.pending.accepted.as_deref(), Some(RESULT));
                drop(rejected);
            } else {
                let executed = ack_owned_effect_cleanup_v8(pending, ack, |_| true)
                    .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
                let plan = compile_owned_reduce_v2(execution.wait()).unwrap();
                if mode == 1 {
                    let rejected = super::super::reduce::prepare_executed_owned_reduce_v2(
                        executed,
                        &plan,
                        |_| {
                            cancellation.cancel();
                            true
                        },
                    )
                    .err()
                    .expect("cancel on consuming handoff guard preserves Executed");
                    assert!(rejected.executed.roots.outcome.is_some());
                    assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                    drop(rejected);
                } else {
                    let prepared = super::super::reduce::prepare_executed_owned_reduce_v2(
                        executed,
                        &plan,
                        |_| true,
                    )
                    .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
                    let mut fuel = OwnedFrameBudget::new(1000).unwrap();
                    let rejected = super::super::reduce::stage_executed_owned_reduce_v2(
                        prepared,
                        &mut fuel,
                        |_| {
                            cancellation.cancel();
                            true
                        },
                    )
                    .err()
                    .expect("cancel after entry callback prevents evaluator");
                    assert_eq!(fuel.consumed(), 0);
                    assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                    drop(rejected);
                }
            }
            assert_eq!(host.calls, 1);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
