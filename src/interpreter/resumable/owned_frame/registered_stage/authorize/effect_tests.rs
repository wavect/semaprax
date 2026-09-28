//! Physical release primitive only: trusted test callers bypass journal ACKs.
//! These tests attest neither effect dispatch nor a durable consuming turn.
use super::*;
use crate::hir::DeclarationId;
use crate::resumable_effects::owned_frame::v2::{
    bind_owned_wait_proposal_v8, compile_owned_agent_wait_v8, CheckedOwnedAgentWaitBindingV8,
};
use crate::resumable_effects::source_checkpoint::SourceCheckpointScope;
use std::sync::Weak;
fn binding() -> CheckedOwnedAgentWaitBindingV8 {
    let source = include_str!("../../../../../../examples/offline-repair-project/src/app.spx");
    let source = source.replace(
        "    runtime_v1 {",
        "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n    runtime_v1 {",
    );
    let source = format!("{source}\n@id(\"fixture.agent.fn.park\") fn park(state:own State,observation:Observation)->State yields Observation->Proposal {{ let proposal=yield observation; state }}");
    compile_owned_agent_wait_v8(
        &source,
        std::path::Path::new("physical-effect-roots.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap_or_else(|e| panic!("{e:?}"))
}
pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn ready(
    b: &CheckedOwnedAgentWaitBindingV8,
) -> (ReadyOwnedAuthorizeV2, [Weak<[u8]>; 2]) {
    let state = admit_owned_agent_state_input(
        b.helper(),
        OwnedFrameInput {
            declaration: DeclarationId::new("fixture.agent.type.state"),
            fields: vec![
                OwnedFrameInputField {
                    identity: DeclarationId::new("fixture.agent.type.state.objective"),
                    value: OwnedFrameInputValue::Bytes(vec![]),
                },
                OwnedFrameInputField {
                    identity: DeclarationId::new("fixture.agent.type.state.budget"),
                    value: OwnedFrameInputValue::Scalar(ArgumentValue::Int(10)),
                },
                OwnedFrameInputField {
                    identity: DeclarationId::new("fixture.agent.type.state.epoch"),
                    value: OwnedFrameInputValue::Scalar(ArgumentValue::Int(0)),
                },
            ],
        },
    )
    .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
    let Value::Record(record) = state.root.as_ref().unwrap() else {
        panic!()
    };
    let Value::Bytes(bytes) =
        &record.fields[&DeclarationId::new("fixture.agent.type.state.objective")]
    else {
        panic!()
    };
    let state_backing = Arc::downgrade(&bytes.bytes);
    let prepared = prepare_owned_copy_wait_v2(
        state,
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("fixture.agent.type.observation"),
            fields: vec![ArgumentValue::Int(10), ArgumentValue::Int(0)],
        },
    )
    .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
    let OwnedCopyWaitStepV2::Parked(parked) =
        begin_owned_copy_wait_v2(prepared, &mut OwnedFrameBudget::new(1000).unwrap())
            .unwrap_or_else(|_| panic!("creator"))
    else {
        panic!()
    };
    let OwnedCopyWaitStepV2::Terminal(terminal) = resume_owned_copy_wait_v2(
        parked,
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("fixture.agent.type.proposal"),
            fields: vec![
                ArgumentValue::Int(5),
                ArgumentValue::Bool(false),
                ArgumentValue::Usize(1),
            ],
        },
        &mut OwnedFrameBudget::new(1000).unwrap(),
    )
    .unwrap_or_else(|_| panic!("creator")) else {
        panic!()
    };
    let OwnedCopyWaitSettledV2::Completed(state) =
        settle_owned_copy_wait_v2(terminal, || true, |_| panic!("empty helper completion"))
            .unwrap_or_else(|e| panic!("{:?}", e.diagnostic))
    else {
        panic!()
    };
    let staged = stage_owned_authorize_v2(
        state,
        b.authorize(),
        &mut OwnedFrameBudget::new(1000).unwrap(),
    )
    .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
    assert!(staged.failure().is_none());
    let Value::Variant(decision) = staged.decision.as_ref().unwrap() else {
        panic!()
    };
    let Value::Bytes(seal) = &decision.fields[b.authorize().seal()] else {
        panic!()
    };
    let seal_backing = Arc::downgrade(&seal.bytes);
    let OwnedAuthorizeSettledV2::Ready(ready) =
        settle_owned_authorize_v2(staged, || true, |_| panic!("empty authorize completion"))
            .unwrap_or_else(|e| panic!("{:?}", e.diagnostic))
    else {
        panic!()
    };
    (ready, [state_backing, seal_backing])
}
pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn proposal(
    b: &CheckedOwnedAgentWaitBindingV8,
    scope: &SourceCheckpointScope,
) -> crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8 {
    let source = format!(
        r#"{{"schema":"semaprax.agent-proposal.v1","agent_id":"fixture.agent","proposal_schema_digest":"{}","value":{{"fields":{{"fixture.agent.type.proposal.budget":"5","fixture.agent.type.proposal.urgent":false,"fixture.agent.type.proposal.sequence":"1"}}}}}}"#,
        b.lifecycle().proposal_schema().schema().digest()
    );
    let decoded = b
        .lifecycle()
        .proposal_schema()
        .decode(&format!("{source}\n"))
        .unwrap();
    bind_owned_wait_proposal_v8(b, scope, &decoded).unwrap()
}
#[test]
fn owned_frame_v8_effect_roots_require_actual_source_and_release_real_seal_before_observation() {
    let b = binding();
    let scope =
        SourceCheckpointScope::new(b.lifecycle().source_revision(), "physical-effect-unit", 0)
            .unwrap();
    let k = proposal(&b, &scope);
    let (ready, weak) = ready(&b);
    let ready = ready
        .hold_for_effect(&binding(), &k, &scope, || true)
        .err()
        .expect("different retained source refused");
    assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 1]);
    let holder = ready
        .hold_for_effect(&b, &k, &scope, || true)
        .unwrap_or_else(|_| panic!("actual binding"));
    let mut actual = Vec::new();
    let mut dead_before_observe = Vec::new();
    let released = holder
        .release_decision(
            || true,
            |action| {
                actual.push(action.clone());
                dead_before_observe.push((weak[0].strong_count(), weak[1].strong_count()));
            },
        )
        .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
    assert_eq!(actual, released.operations);
    assert_eq!(
        actual,
        b.authorize()
            .disposal()
            .iter()
            .filter(|a| a
                .active_case
                .as_ref()
                .is_some_and(|c| c.case == *b.authorize().granted()))
            .cloned()
            .collect::<Vec<_>>()
    );
    assert_eq!(dead_before_observe, [(1, 0)]);
    assert!(released.observations_succeeded);
    assert_eq!(weak[0].strong_count(), 1);
    drop(released);
    assert_eq!(weak[0].strong_count(), 0);
}
#[test]
fn owned_frame_v8_effect_release_observer_panic_and_authority_loss_cannot_retry() {
    let b = binding();
    let scope =
        SourceCheckpointScope::new(b.lifecycle().source_revision(), "physical-effect-unit", 0)
            .unwrap();
    let k = proposal(&b, &scope);
    for authority_loss in [false, true] {
        let (ready, weak) = ready(&b);
        let holder = ready
            .hold_for_effect(&b, &k, &scope, || true)
            .unwrap_or_else(|_| panic!("actual binding"));
        let current = std::cell::Cell::new(true);
        let mut observed = 0;
        let result = holder.release_decision(
            || current.get(),
            |_| {
                observed += 1;
                if authority_loss {
                    current.set(false);
                } else {
                    panic!("expected observer panic");
                }
            },
        );
        assert_eq!(observed, 1);
        assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
        let holder = match result {
            Ok(receipt) => {
                assert!(!authority_loss);
                assert!(!receipt.observations_succeeded);
                receipt.holder
            }
            Err(rejection) => {
                assert!(authority_loss);
                rejection.holder
            }
        };
        let mut retry = 0;
        assert!(holder.release_decision(|| true, |_| retry += 1).is_err());
        assert_eq!(retry, 0);
    }
}
