use super::*;
use crate::hir::DeclarationId;
use crate::interpreter::{resumable::ResumableChannelValue, ArgumentValue};
use std::path::Path;
fn fixture() -> (
    CheckedOwnedAgentWaitBindingV8,
    SourceCheckpointKey,
    SourceCheckpointScope,
    CheckedOwnedWaitObservationV8,
    Value,
) {
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
    let b = super::super::compile_owned_agent_wait_v8(
        &source,
        Path::new("checkpoint-v2.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    let scope =
        SourceCheckpointScope::new(b.lifecycle().source_revision(), "checkpoint-v2", 7).unwrap();
    let observation = ResumableChannelValue::Record {
        declaration: DeclarationId::new("fixture.agent.type.observation"),
        fields: vec![ArgumentValue::Int(4), ArgumentValue::Int(0)],
    };
    let checked = super::super::bind_owned_wait_observation_v8(&b, &scope, &observation).unwrap();
    let argument = json!({"declaration":"fixture.agent.type.state","fields":[
        {"identity":"fixture.agent.type.state.objective","value":{"kind":"bytes","hex":"6162"}},
        {"identity":"fixture.agent.type.state.budget","value":{"tag":"i64","value":4}},
        {"identity":"fixture.agent.type.state.epoch","value":{"tag":"i64","value":0}}]});
    (
        b,
        SourceCheckpointKey::new([17; 32]),
        scope,
        checked,
        argument,
    )
}
fn payload(
    b: &CheckedOwnedAgentWaitBindingV8,
    e: &OwnedWaitCheckpointExpectationV8<'_>,
    argument: &Value,
) -> Value {
    let frame = frame(b, argument, e.observation.copy_arguments()).unwrap();
    json!({"schema":SCHEMA,"scope":codec::scope(e.scope).unwrap(),"plan_digest":b.binding(),
        "cleanup_plan_digest":b.cleanup_digest(),"signature":b.signature(),"argument_digest":e.argument_digest,
        "copy_arguments_digest":codec::fact_digest(b"semaprax.source-owned-frame-copy-args.v2\0",e.observation.copy_arguments()),
        "frame_digest":codec::fact_digest(b"semaprax.source-owned-frame-frame.v2\0",&frame),"frame":frame,
        "request":e.observation.copy_arguments()[0]["value"],"request_digest":e.observation.request_digest(),
        "reserved_total":e.reserved_total,"consumed_total":e.consumed_total,"sequence":e.sequence})
}
fn sign(key: &SourceCheckpointKey, payload: Value) -> Vec<u8> {
    let auth = key.authenticate(AUTH, &codec::canonical(&payload));
    let mut bytes =
        codec::canonical(&json!({"payload":payload,"authentication":codec::hex(&auth)}));
    bytes.push(b'\n');
    bytes
}
#[test]
fn owned_wait_checkpoint_v2_checks_exact_envelope_compiler_root_and_copy_commitments() {
    let (b, key, scope, observation, argument) = fixture();
    let digest = codec::fact_digest(b"semaprax.source-owned-frame-args.v2\0", &argument);
    let e = OwnedWaitCheckpointExpectationV8 {
        scope: &scope,
        argument_digest: &digest,
        observation: &observation,
        sequence: 8,
        reserved_total: 200,
        consumed_total: 31,
    };
    let value = payload(&b, &e, &argument);
    let bytes = sign(&key, value.clone());
    let checked = validate_owned_wait_checkpoint_v8(&b, &key, &e, &bytes).unwrap();
    assert_eq!(checked.payload(), &value);
    assert_eq!(
        checked.checkpoint_digest(),
        codec::fact_digest(b"semaprax.source-owned-frame-checkpoint.v2\0", &value)
    );
    assert_eq!(
        checked.outer_digest(),
        codec::digest(b"semaprax.source-agent-owned-wait.checkpoint.v1\0", &bytes)
    );
    assert_ne!(checked.checkpoint_digest(), checked.outer_digest());
    assert!(
        validate_owned_wait_checkpoint_v8(&b, &SourceCheckpointKey::new([18; 32]), &e, &bytes)
            .is_err()
    );
    let mut v1 = codec::parse(&bytes, 65536).unwrap();
    v1["authentication"] = json!(codec::hex(&key.authenticate(
        b"semaprax.source-owned-frame-checkpoint-authentication.v1\0",
        &codec::canonical(&value)
    )));
    let mut oldauth = codec::canonical(&v1);
    oldauth.push(b'\n');
    assert!(validate_owned_wait_checkpoint_v8(&b, &key, &e, &oldauth).is_err());
    let other_scope = SourceCheckpointScope::new(scope.program_root(), "different", 7).unwrap();
    let wrong = OwnedWaitCheckpointExpectationV8 {
        scope: &other_scope,
        ..e
    };
    assert!(validate_owned_wait_checkpoint_v8(&b, &key, &wrong, &bytes).is_err());
}
#[test]
fn owned_wait_checkpoint_v2_signed_hostile_frame_status_counter_and_shape_refuse() {
    let (b, key, scope, observation, argument) = fixture();
    let digest = codec::fact_digest(b"semaprax.source-owned-frame-args.v2\0", &argument);
    let e = OwnedWaitCheckpointExpectationV8 {
        scope: &scope,
        argument_digest: &digest,
        observation: &observation,
        sequence: 8,
        reserved_total: 200,
        consumed_total: 31,
    };
    let original = payload(&b, &e, &argument);
    for field in [
        "schema",
        "scope",
        "plan",
        "cleanup",
        "signature",
        "argument",
        "copy_digest",
        "frame_digest",
        "request_digest",
        "sequence",
        "reserved",
        "consumed",
        "flag",
        "storage",
        "field_order",
        "leaf",
        "cleanup_order",
        "copy",
        "request",
        "extra",
    ] {
        let mut value = original.clone();
        match field {
            "schema" => value["schema"] = json!("semaprax.source-owned-frame-checkpoint.v1"),
            "scope" => value["scope"]["policy_epoch"] = json!(8),
            "plan" => value["plan_digest"] = json!("sha256:".to_owned() + &"0".repeat(64)),
            "cleanup" => {
                value["cleanup_plan_digest"] = json!("sha256:".to_owned() + &"0".repeat(64))
            }
            "signature" => value["signature"]["yield_count"] = json!(2),
            "argument" => value["argument_digest"] = json!("sha256:".to_owned() + &"0".repeat(64)),
            "copy_digest" => {
                value["copy_arguments_digest"] = json!("sha256:".to_owned() + &"0".repeat(64))
            }
            "frame_digest" => value["frame_digest"] = json!("sha256:".to_owned() + &"0".repeat(64)),
            "request_digest" => {
                value["request_digest"] = json!("sha256:".to_owned() + &"0".repeat(64))
            }
            "sequence" => value["sequence"] = json!(7),
            "reserved" => value["reserved_total"] = json!(199),
            "consumed" => value["consumed_total"] = json!(32),
            "flag" => value["frame"]["owned_root"]["leaf_flags"][0]["live"] = json!(false),
            "storage" => {
                value["frame"]["owned_root"]["storage"] = json!({"kind":"provisional_result"})
            }
            "field_order" => value["frame"]["owned_root"]["fields"]
                .as_array_mut()
                .unwrap()
                .swap(0, 1),
            "leaf" => {
                value["frame"]["owned_root"]["fields"][0]["value"] = json!({"tag":"i64","value":0})
            }
            "cleanup_order" => value["frame"]["owned_root"]["failure_cleanup"] = json!([]),
            "copy" => value["frame"]["copy_arguments"][0]["parameter"] = json!("other"),
            "request" => value["request"]["fields"][0]["value"] = json!(5),
            "extra" => value["extra"] = json!(true),
            _ => unreachable!(),
        }
        assert!(
            validate_owned_wait_checkpoint_v8(&b, &key, &e, &sign(&key, value)).is_err(),
            "{field}"
        );
    }
    let bytes = sign(&key, original);
    let mut extra = bytes.clone();
    extra.push(b'\n');
    assert!(validate_owned_wait_checkpoint_v8(&b, &key, &e, &extra).is_err());
    assert!(validate_owned_wait_checkpoint_v8(&b, &key, &e, &bytes[..bytes.len() - 1]).is_err());
    let duplicate = String::from_utf8(bytes.clone()).unwrap().replacen(
        "\"payload\":",
        "\"payload\":null,\"payload\":",
        1,
    );
    assert!(validate_owned_wait_checkpoint_v8(&b, &key, &e, duplicate.as_bytes()).is_err());
    assert!(validate_owned_wait_checkpoint_v8(&b, &key, &e, &vec![b' '; 65537]).is_err());
    let deep = format!("{}null{}", "[".repeat(25), "]".repeat(25));
    assert!(validate_owned_wait_checkpoint_v8(&b, &key, &e, deep.as_bytes()).is_err());
}
#[test]
fn owned_wait_checkpoint_v2_first_retry_sequence_and_full_envelope_cap_are_exact() {
    let (b, key, scope, observation, argument) = fixture();
    let digest = codec::fact_digest(b"semaprax.source-owned-frame-args.v2\0", &argument);
    // Structural unit fixture for the first durable Prepared after interrupted
    // original/fresh Start reservations. Production accounting must come from
    // the independent candidate fold; this test grants no history authority.
    let e = OwnedWaitCheckpointExpectationV8 {
        scope: &scope,
        argument_digest: &digest,
        observation: &observation,
        sequence: 19,
        reserved_total: 600,
        consumed_total: 42,
    };
    let bytes = sign(&key, payload(&b, &e, &argument));
    validate_owned_wait_checkpoint_v8(&b, &key, &e, &bytes).unwrap();
    let old = OwnedWaitCheckpointExpectationV8 { sequence: 8, ..e };
    assert!(validate_owned_wait_checkpoint_v8(&b, &key, &old, &bytes).is_err());
    // A closed-shape error at the exact byte ceiling proves the parser reached
    // schema validation. One additional byte must be refused by the cap first.
    let empty = json!({"payload":{"pad":""},"authentication":"0".repeat(64)});
    let base = codec::canonical(&empty).len() + 1;
    let mut exact = codec::canonical(
        &json!({"payload":{"pad":"x".repeat(65536-base)},"authentication":"0".repeat(64)}),
    );
    exact.push(b'\n');
    assert_eq!(exact.len(), 65536);
    assert_eq!(
        validate_owned_wait_checkpoint_v8(&b, &key, &e, &exact).err(),
        Some(Error::Malformed)
    );
    exact.push(b'\n');
    assert_eq!(
        validate_owned_wait_checkpoint_v8(&b, &key, &e, &exact).err(),
        Some(Error::Capacity)
    );
}
