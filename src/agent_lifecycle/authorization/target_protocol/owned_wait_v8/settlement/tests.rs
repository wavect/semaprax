//! Genuine typed registry, inert State/Decision facts and existing target wire.
//! This gate makes no source-evaluation or successor ACK claim.
use super::*;
use crate::live_invocation::source_journal::CheckedOwnedWaitJournalContextV8;
use crate::resumable_effects::owned_frame::v2::bind_owned_wait_proposal_v8;
use serde_json::json;

struct Host {
    calls: usize,
    payload: Vec<u8>,
    malformed: bool,
    panic: bool,
}
impl TargetHostHandler for Host {
    fn dispatch(
        &mut self,
        request: &TargetHostRequest,
        sink: &mut TargetResponseSink,
    ) -> Result<(), TargetHostError> {
        self.calls += 1;
        if self.panic {
            panic!("target host panic, caught by existing protocol");
        }
        let wire = if self.malformed {
            b"not a typed carrier".to_vec()
        } else {
            TypedCarrier::new(request.operation().result_type(), self.payload.clone())
                .unwrap()
                .encode()
        };
        let _ = sink.write(&wire);
        Ok(())
    }
}
/// Bounded existing real target handler fixture for the combined journal gate.
/// Its metadata grant is test-only; no source owner or successor ACK is minted.
pub(crate) fn test_effect_exchange(
    inputs: &OwnedEffectSettlementInputsV8<'_>,
) -> (SourceJournalEntry, Vec<u8>, Option<Vec<u8>>) {
    test_effect_exchange_mode(inputs, false)
}
/// Real host-panic exchange: the target protocol produces the evidence and
/// accounting. Changing an ordinary row cannot substitute for this exchange.
pub(crate) fn test_failed_effect_exchange(
    inputs: &OwnedEffectSettlementInputsV8<'_>,
) -> (SourceJournalEntry, Vec<u8>, Option<Vec<u8>>) {
    test_effect_exchange_mode(inputs, true)
}
fn test_effect_exchange_mode(
    inputs: &OwnedEffectSettlementInputsV8<'_>,
    host_panics: bool,
) -> (SourceJournalEntry, Vec<u8>, Option<Vec<u8>>) {
    let request = checked_owned_effect_request_v8(inputs).unwrap();
    let commitments = checked_owned_wait_ready_commitments_v8(
        inputs.runtime,
        inputs.execution,
        inputs.scope,
        inputs.turn,
        inputs.attempt,
        inputs.state,
        inputs.decision,
        inputs.proposal,
    )
    .unwrap();
    let payload =
        b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"value\",\"9\"]]}\n"
            .to_vec();
    assert_eq!(
        request.plan.accepted_result(&payload),
        Some(payload.clone())
    );
    let grant = TargetGrant {
        grant_id: commitments.target_grant_digest().into(),
        authorization_binding: commitments.authorization_binding().into(),
        operation: request.operation().clone(),
        argument_digest: commitments.argument_digest().into(),
        turn: u64::from(inputs.turn),
        granted_budget: commitments.budget(),
    };
    let mut host = Host {
        calls: 0,
        payload: payload.clone(),
        malformed: false,
        panic: host_panics,
    };
    let run = crate::agent_lifecycle::authorization::target_protocol::dispatch(
        grant,
        request.plan.argument().clone(),
        1,
        request.limits(),
        &mut TargetAccounting::default(),
        &AgentCancellation::new(),
        &mut host,
    );
    assert_eq!(host.calls, 1);
    assert_eq!(
        run.evidence().settlement(),
        if host_panics {
            Settlement::HostPanicked
        } else {
            Settlement::Returned
        }
    );
    let ordinary = if host_panics {
        SourceJournalEntry::EffectFailed {
            turn: inputs.turn,
            attempt: inputs.attempt,
            operation: request.operation().operation_id().into(),
            reason: SourceEffectFailure::HandlerFailed,
        }
    } else {
        SourceJournalEntry::EffectObserved {
            turn: inputs.turn,
            attempt: inputs.attempt,
            operation: request.operation().operation_id().into(),
            observation_digest: source_effect_digest(&payload),
            observation: payload,
        }
    };
    (
        ordinary,
        run.evidence().canonical_wire(),
        run.result().map(TypedCarrier::encode),
    )
}
#[cfg(unix)]
#[test]
fn owned_wait_effect_settlement_replays_exact_exchange_and_refuses_result_or_phase_substitution() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, _lease, _key| {
        let (runtime, execution) = context.test_runtime_execution();
        let b = execution.wait();
        let invocation=crate::live_invocation::identity::digest(b"semaprax.live-invocation.source-id.v8\0",
            serde_json::to_string(&json!({"execution":execution.ordinary().invocation(),"owned_wait_binding":b.binding()})).unwrap().as_bytes());
        let scope =
            SourceCheckpointScope::new(b.lifecycle().source_revision(), invocation, 7).unwrap();
        let state = json!({"declaration":"fixture.agent.type.state","fields":[
            {"identity":"fixture.agent.type.state.objective","value":{"kind":"bytes","hex":"6162"}},
            {"identity":"fixture.agent.type.state.budget","value":{"tag":"i64","value":10}},
            {"identity":"fixture.agent.type.state.epoch","value":{"tag":"i64","value":0}}]});
        let decision = json!({"declaration":"fixture.agent.type.decision","case":"fixture.agent.type.decision.granted","fields":[
            {"identity":"fixture.agent.type.decision.granted.seal","value":{"kind":"bytes","hex":"abcd"}},
            {"identity":"fixture.agent.type.decision.granted.budget","value":{"tag":"i64","value":5}}]});
        let document = format!(
            r#"{{"schema":"semaprax.agent-proposal.v1","agent_id":"fixture.agent","proposal_schema_digest":"{}","value":{{"fields":{{"fixture.agent.type.proposal.budget":"5","fixture.agent.type.proposal.urgent":false,"fixture.agent.type.proposal.sequence":"1"}}}}}}
"#,
            b.lifecycle().proposal_schema().schema().digest()
        );
        let decoded = b.lifecycle().proposal_schema().decode(&document).unwrap();
        let proposal = bind_owned_wait_proposal_v8(b, &scope, &decoded).unwrap();
        let inputs = || OwnedEffectSettlementInputsV8 {
            runtime,
            execution,
            scope: &scope,
            turn: 0,
            attempt: 0,
            state: &state,
            decision: &decision,
            proposal: &proposal,
        };
        let request = checked_owned_effect_request_v8(&inputs()).unwrap();
        assert_eq!(request.limits().max_fuel, request.limits().max_calls);
        assert_eq!(request.operation().effect_id(), "read");
        let commitments = checked_owned_wait_ready_commitments_v8(
            runtime, execution, &scope, 0, 0, &state, &decision, &proposal,
        )
        .unwrap();
        let plan = plan_owned_effect_v8(runtime, execution, &scope, &proposal).unwrap();
        let grant = || TargetGrant {
            grant_id: commitments.target_grant_digest().into(),
            authorization_binding: commitments.authorization_binding().into(),
            operation: plan.operation().clone(),
            argument_digest: commitments.argument_digest().into(),
            turn: 0,
            granted_budget: commitments.budget(),
        };
        let payload =
            b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"value\",\"9\"]]}\n"
                .to_vec();
        let mut no_budget = decision.clone();
        no_budget["fields"][1]["value"]["value"] = json!(0);
        assert!(
            checked_owned_effect_request_v8(&OwnedEffectSettlementInputsV8 {
                decision: &no_budget,
                ..inputs()
            })
            .is_err(),
            "zero Decision budget cannot authorize the profile's host work unit"
        );
        assert!(
            checked_owned_effect_request_v8(&OwnedEffectSettlementInputsV8 {
                turn: 1,
                ..inputs()
            })
            .is_err(),
            "a new turn cannot reset this first-effect accounting proof"
        );
        let cancelled = AgentCancellation::new();
        cancelled.cancel();
        let mut untouched = Host {
            calls: 0,
            payload: payload.clone(),
            malformed: false,
            panic: false,
        };
        let pre_dispatch = crate::agent_lifecycle::authorization::target_protocol::dispatch(
            grant(),
            plan.argument().clone(),
            1,
            plan.target_limits(),
            &mut TargetAccounting::default(),
            &cancelled,
            &mut untouched,
        );
        assert_eq!(untouched.calls, 0);
        let refused = SourceJournalEntry::EffectFailed {
            turn: 0,
            attempt: 0,
            operation: plan.operation().operation_id().into(),
            reason: SourceEffectFailure::Cancelled,
        };
        assert!(
            checked_owned_effect_settlement_v8(
                inputs(),
                &refused,
                &pre_dispatch.evidence().canonical_wire(),
                None
            )
            .is_err(),
            "genuine pre-dispatch cancellation is outside dispatched §21"
        );
        for shape in 0..5 {
            let mut host = Host {
                calls: 0,
                payload: if shape == 1 {
                    b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"wrong\",\"9\"]]}\n".to_vec()
                } else if shape == 4 {
                    vec![b'x'; 1025]
                } else {
                    payload.clone()
                },
                malformed: shape == 2,
                panic: shape == 3,
            };
            let run = crate::agent_lifecycle::authorization::target_protocol::dispatch(
                grant(),
                plan.argument().clone(),
                1,
                plan.target_limits(),
                &mut TargetAccounting::default(),
                &AgentCancellation::new(),
                &mut host,
            );
            assert_eq!(host.calls, 1);
            let ordinary = if shape == 0 {
                SourceJournalEntry::EffectObserved {
                    turn: 0,
                    attempt: 0,
                    operation: plan.operation().operation_id().into(),
                    observation: payload.clone(),
                    observation_digest: source_effect_digest(&payload),
                }
            } else {
                SourceJournalEntry::EffectFailed {
                    turn: 0,
                    attempt: 0,
                    operation: plan.operation().operation_id().into(),
                    reason: if shape == 4 {
                        SourceEffectFailure::ResultLimit
                    } else {
                        SourceEffectFailure::HandlerFailed
                    },
                }
            };
            let evidence = run.evidence().canonical_wire();
            let result = run.result().map(TypedCarrier::encode);
            if shape == 2 {
                assert!(
                    checked_owned_effect_settlement_v8(inputs(), &ordinary, &evidence, None)
                        .is_err(),
                    "committed malformed raw result cannot be silently omitted"
                );
                assert_eq!(host.calls, 1);
                continue;
            }
            let checked = checked_owned_effect_settlement_v8(
                inputs(),
                &ordinary,
                &evidence,
                result.as_deref(),
            )
            .unwrap();
            assert_eq!(checked.operation(), plan.operation());
            if shape == 4 {
                assert_eq!(run.evidence().settlement(), Settlement::ResultBudget);
                assert_eq!(run.evidence().accounting().result_bytes(), 1025);
                let mut less = run.evidence().clone();
                less.accounting.result_bytes -= 1;
                assert!(
                    checked_owned_effect_settlement_v8(
                        inputs(),
                        &ordinary,
                        &less.canonical_wire(),
                        None
                    )
                    .is_err(),
                    "overflow charge is the exact sentinel, not any bounded number"
                );
            }

            for field in 0..4 {
                let mut reminted = run.evidence().clone();
                match field {
                    0 => reminted.accounting.calls = reminted.accounting.calls.saturating_add(1),
                    1 => {
                        reminted.accounting.request_bytes =
                            reminted.accounting.request_bytes.saturating_add(1)
                    }
                    2 => {
                        reminted.accounting.result_bytes =
                            reminted.accounting.result_bytes.saturating_add(1)
                    }
                    3 => reminted.accounting.fuel = reminted.accounting.fuel.saturating_add(1),
                    _ => unreachable!(),
                }
                assert!(
                    checked_owned_effect_settlement_v8(
                        inputs(),
                        &ordinary,
                        &reminted.canonical_wire(),
                        result.as_deref()
                    )
                    .is_err(),
                    "canonical reminted evidence cannot alter charge dimension {field}"
                );
            }

            match &ordinary {
                SourceJournalEntry::EffectObserved { observation, .. } => {
                    assert_eq!(checked.accepted_payload(), Some(observation.as_slice()));
                }
                SourceJournalEntry::EffectFailed { .. } => {
                    assert_eq!(checked.accepted_payload(), None);
                }
                _ => unreachable!(),
            }
            assert_eq!(request.request_wire(), checked.request_wire());
            assert_eq!(request.request_digest(), checked.request_digest());
            assert_eq!(checked.evidence().digest(), run.evidence().digest());
            assert_eq!(
                checked.result_wire_limit(),
                plan.target_limits().max_result_bytes
            );
            assert!(evidence.len() <= checked.evidence_wire_limit());
            checked
                .evidence()
                .replay_wire(checked.request_wire())
                .unwrap();
            let mut wrong = ordinary.clone();
            match &mut wrong {
                SourceJournalEntry::EffectObserved { attempt, .. }
                | SourceJournalEntry::EffectFailed { attempt, .. } => *attempt = 1,
                _ => unreachable!(),
            }
            assert!(checked_owned_effect_settlement_v8(
                inputs(),
                &wrong,
                &evidence,
                result.as_deref()
            )
            .is_err());
            if shape < 2 {
                assert!(
                    checked_owned_effect_settlement_v8(inputs(), &ordinary, &evidence, None)
                        .is_err()
                );
                let replacement =
                    TypedCarrier::new(plan.operation().result_type(), b"substituted".to_vec())
                        .unwrap()
                        .encode();
                assert!(checked_owned_effect_settlement_v8(
                    inputs(),
                    &ordinary,
                    &evidence,
                    Some(&replacement)
                )
                .is_err());
            } else {
                assert!(checked_owned_effect_settlement_v8(
                    inputs(),
                    &ordinary,
                    &evidence,
                    Some(b"unexpected")
                )
                .is_err());
            }
            let mut tampered = evidence.clone();
            tampered[0] ^= 1;
            assert!(checked_owned_effect_settlement_v8(
                inputs(),
                &ordinary,
                &tampered,
                result.as_deref()
            )
            .is_err());
            assert_eq!(host.calls, 1, "all replay and hostile checks are pure");
        }
    });
}
