use super::*;
use crate::hir::DeclarationId;
use crate::resumable_effects::owned_frame::v2::compile_owned_agent_wait_v8;
use std::path::Path;

fn binding() -> CheckedOwnedAgentWaitBindingV8 {
    let source = include_str!("../../../../../examples/offline-repair-project/src/app.spx");
    let source = format!(
        "{}\n{}",
        source.replace(
            "    runtime_v1 {",
            "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n    runtime_v1 {"
        ),
        r#"
@id("fixture.agent.fn.park")
fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
    let proposal = yield observation;
    state
}
"#
    );
    compile_owned_agent_wait_v8(
        &source,
        Path::new("observation-binding.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap_or_else(|e| panic!("{e:?}"))
}
fn observation() -> ResumableChannelValue {
    ResumableChannelValue::Record {
        declaration: DeclarationId::new("fixture.agent.type.observation"),
        fields: vec![ArgumentValue::Int(10), ArgumentValue::Int(0)],
    }
}
fn scope(binding: &CheckedOwnedAgentWaitBindingV8) -> SourceCheckpointScope {
    SourceCheckpointScope::new(binding.binding(), "observation-test", 7).unwrap()
}
#[test]
fn checked_observation_uses_frozen_ordinary_codec_and_v2_request_recipe() {
    let b = binding();
    let scope = scope(&b);
    let observation = observation();
    let facts = bind_owned_wait_observation_v8(&b, &scope, &observation).unwrap();
    assert!(facts.matches(
        b.binding(),
        &super::super::super::codec::scope(&scope).unwrap()
    ));
    assert!(!facts.matches(
        "sha256:wrong",
        &super::super::super::codec::scope(&scope).unwrap()
    ));
    assert_eq!(
        facts.ordinary_digest(),
        "sha256:96ac3faa91e1f081bba4360e16493719e01839c4b853c0c7939f5ecd130ccd6c"
    );
    assert_eq!(
        facts.copy_arguments(),
        &json!([{"parameter":b.helper().function().params[1].id.as_str(),"value":{"tag":"record","declaration":"fixture.agent.type.observation","fields":[{"tag":"i64","value":10},{"tag":"i64","value":0}]}}])
    );
    assert_eq!(
        facts.request_digest(),
        super::super::super::codec::fact_digest(
            b"semaprax.source-owned-frame-request.v2\0",
            &json!({"scope":{"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":7},"plan_digest":b.binding(),"value":checkpoint::channel_json(&observation)})
        )
    );
    let changed_scope = SourceCheckpointScope::new(b.binding(), "another-invocation", 7).unwrap();
    let changed = bind_owned_wait_observation_v8(&b, &changed_scope, &observation).unwrap();
    assert_eq!(facts.ordinary_digest(), changed.ordinary_digest());
    assert_ne!(facts.request_digest(), changed.request_digest());
    let ResumableChannelValue::Record {
        declaration,
        mut fields,
    } = observation
    else {
        panic!()
    };
    fields.swap(0, 1);
    let changed = bind_owned_wait_observation_v8(
        &b,
        &scope,
        &ResumableChannelValue::Record {
            declaration,
            fields,
        },
    )
    .unwrap();
    assert_ne!(facts.ordinary_digest(), changed.ordinary_digest());
    assert_ne!(facts.request_digest(), changed.request_digest());
}
#[test]
fn checked_observation_refuses_nominal_shape_wrong_width_and_owned_carriers() {
    let b = binding();
    let scope = scope(&b);
    for supplied in [
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("fixture.agent.type.proposal"),
            fields: vec![ArgumentValue::Int(10), ArgumentValue::Int(0)],
        },
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("fixture.agent.type.observation"),
            fields: vec![ArgumentValue::Int(10)],
        },
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("fixture.agent.type.observation"),
            fields: vec![ArgumentValue::Int32(10), ArgumentValue::Int(0)],
        },
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("fixture.agent.type.observation"),
            fields: vec![ArgumentValue::Float64(-0.0), ArgumentValue::Int(0)],
        },
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("fixture.agent.type.observation"),
            fields: vec![ArgumentValue::Char('x' as u32), ArgumentValue::Int(0)],
        },
        ResumableChannelValue::Scalar(ArgumentValue::Int(10)),
        ResumableChannelValue::RecordBytes {
            declaration: DeclarationId::new("fixture.agent.type.observation"),
            fields: vec![
                crate::interpreter::resumable::channel_bytes::ChannelField::Bytes(vec![0]),
            ],
        },
    ] {
        assert_eq!(
            bind_owned_wait_observation_v8(&b, &scope, &supplied)
                .err()
                .unwrap()
                .code,
            "SPX-G583"
        );
    }
    assert!(bind_owned_wait_observation_v8(&b, &scope, &observation()).is_ok());
}
#[test]
fn inert_copy_projection_keeps_existing_sdk_refusals_and_usize_bound() {
    assert!(retained_copy(&ArgumentValue::Usize(u32::MAX as u64)).is_ok());
    for value in [
        ArgumentValue::Usize(u32::MAX as u64 + 1),
        ArgumentValue::Float32(f32::from_bits(0x7fc00001)),
        ArgumentValue::Float64(-0.0),
        ArgumentValue::Char('x' as u32),
        ArgumentValue::BorrowedStr("x".into()),
        ArgumentValue::BorrowedSlice(vec![0]),
    ] {
        assert_eq!(retained_copy(&value).err().unwrap().code, "SPX-G583");
    }
    // These are codec-only projection controls, not wider actual Agent admission:
    // the current checked source Agent Observation profile is i64-only.
    for value in [
        ArgumentValue::Bool(false),
        ArgumentValue::Int32(i32::MIN),
        ArgumentValue::Int(i64::MAX),
        ArgumentValue::Uint8(0),
    ] {
        assert!(retained_copy(&value).is_ok());
    }
}
