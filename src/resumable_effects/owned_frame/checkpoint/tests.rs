use super::*;
use crate::hir::DeclarationId;
use crate::interpreter::resumable::owned_frame::{OwnedFrameInputField, OwnedFrameInputValue};
use crate::resumable_effects::owned_frame::compile_owned_frame_plan;
use std::path::Path;
const SOURCE: &str = r#"module fixture.owned_frame;
@id("fixture.state") record State {
@id("fixture.state.z") objective:Bytes,
@id("fixture.state.a") second:Bytes,
@id("fixture.state.m") budget:i64,
}
@id("fixture.park") fn park(state:own State)->State yields i64->i64 {
let answer=yield state.budget; state
}
@id("fixture.main") fn main()->i64 {0}
"#;
fn fixture() -> (
    CheckedOwnedFramePlan,
    OwnedFrameInput,
    SourceCheckpointScope,
    SourceCheckpointKey,
    String,
    String,
) {
    let program = crate::hir::resolve(
        &crate::parse(SOURCE, Path::new("owned-frame-checkpoint.spx")).unwrap(),
    )
    .unwrap();
    let plan = compile_owned_frame_plan(&program, &DeclarationId::new("fixture.park")).unwrap();
    let input = OwnedFrameInput {
        declaration: DeclarationId::new("fixture.state"),
        fields: vec![
            OwnedFrameInputField {
                identity: DeclarationId::new("fixture.state.z"),
                value: OwnedFrameInputValue::Bytes(vec![0, 7, 0]),
            },
            OwnedFrameInputField {
                identity: DeclarationId::new("fixture.state.a"),
                value: OwnedFrameInputValue::Bytes(Vec::new()),
            },
            OwnedFrameInputField {
                identity: DeclarationId::new("fixture.state.m"),
                value: OwnedFrameInputValue::Scalar(ArgumentValue::Int(4)),
            },
        ],
    };
    let argument_digest = codec::fact_digest(
        b"semaprax.source-owned-frame-arguments.v1\0",
        &codec::input(&plan, &input).unwrap(),
    );
    let generation = format!("sha256:{}", "1".repeat(64));
    (
        plan,
        input,
        SourceCheckpointScope::new("sha256:program", "owned-checkpoint", 0).unwrap(),
        SourceCheckpointKey::new([17; 32]),
        argument_digest,
        generation,
    )
}
fn signed(key: &SourceCheckpointKey, mut value: Value) -> Vec<u8> {
    value.as_object_mut().unwrap().remove("authentication");
    let mac = key.authenticate(MAC_DOMAIN, &codec::canonical(&value));
    value
        .as_object_mut()
        .unwrap()
        .insert("authentication".into(), json!(codec::hex(&mac)));
    let mut bytes = codec::canonical(&value);
    bytes.push(b'\n');
    bytes
}
#[test]
fn owned_frame_inert_checkpoint_roundtrip_preserves_bytes_and_checked_metadata() {
    let (plan, input, scope, key, argument_digest, generation) = fixture();
    let bytes = encode(
        &plan,
        &key,
        &scope,
        &input,
        &argument_digest,
        &ArgumentValue::Int(4),
        &generation,
        3,
        100,
    )
    .unwrap();
    let inert = decode(
        &plan,
        &key,
        &scope,
        &argument_digest,
        &generation,
        3,
        100,
        &bytes,
    )
    .unwrap();
    assert_eq!(
        codec::input(&plan, &inert.input).unwrap(),
        codec::input(&plan, &input).unwrap()
    );
    assert_eq!(inert.request, ArgumentValue::Int(4));
    assert_eq!(inert.sequence, 3);
    assert_eq!(inert.reserved_total, 100);
    assert_eq!(inert.bytes, bytes);
    assert_eq!(digest(&bytes).len(), 71);
    // Authenticated data has no owner token, evaluator entry or restore API.
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["frame"]["leaf_flags"][0]["field"], "fixture.state.z");
    assert_eq!(
        value["frame"]["failure_cleanup"][0]["source"]["projections"][0],
        "fixture.state.a"
    );
}
#[test]
fn owned_frame_checkpoint_tampered_shape_scope_key_generation_and_canonical_refuse() {
    let (plan, input, scope, key, argument_digest, generation) = fixture();
    let bytes = encode(
        &plan,
        &key,
        &scope,
        &input,
        &argument_digest,
        &ArgumentValue::Int(4),
        &generation,
        3,
        100,
    )
    .unwrap();
    let decode_bytes = |candidate: &[u8]| {
        decode(
            &plan,
            &key,
            &scope,
            &argument_digest,
            &generation,
            3,
            100,
            candidate,
        )
        .err()
        .unwrap()
    };
    assert_eq!(
        decode(
            &plan,
            &SourceCheckpointKey::new([18; 32]),
            &scope,
            &argument_digest,
            &generation,
            3,
            100,
            &bytes
        )
        .err(),
        Some(Error::Authentication)
    );
    assert_eq!(
        decode(
            &plan,
            &key,
            &scope,
            &argument_digest,
            &generation,
            4,
            100,
            &bytes
        )
        .err(),
        Some(Error::Binding)
    );
    for field in [
        "flag", "storage", "order", "leaf", "payload", "request", "scope", "schema",
    ] {
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        match field {
            "flag" => value["frame"]["leaf_flags"][0]["live"] = json!(false),
            "storage" => value["frame"]["storage"] = json!({"kind":"provisional_result"}),
            "order" => value["frame"]["fields"].as_array_mut().unwrap().swap(0, 1),
            "leaf" => value["frame"]["fields"][0]["value"] = json!({"tag":"i64","value":0}),
            "payload" => value["frame"]["fields"][0]["value"]["hex"] = json!("01"),
            "request" => value["request"] = json!({"tag":"bool","value":true}),
            "scope" => value["scope"]["policy_epoch"] = json!(1),
            "schema" => value["schema"] = json!("semaprax.source-resumable-checkpoint.v7"),
            _ => unreachable!(),
        }
        assert_eq!(
            decode_bytes(&signed(&key, value)),
            Error::Binding,
            "{field}"
        );
    }
    let mut extra = bytes.clone();
    extra.push(b'\n');
    assert_eq!(decode_bytes(&extra), Error::Malformed);
    let duplicate = String::from_utf8(bytes).unwrap().replacen(
        "\"schema\":",
        "\"schema\":\"x\",\"schema\":",
        1,
    );
    assert_eq!(decode_bytes(duplicate.as_bytes()), Error::Malformed);
}
