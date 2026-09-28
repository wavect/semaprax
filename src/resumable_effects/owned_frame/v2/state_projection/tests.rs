use super::*;
use crate::hir::DeclarationId;
use crate::interpreter::retained_call::{RetainedField, RetainedRecord, RetainedValue};
use serde_json::json;
use std::path::Path;
fn binding() -> CheckedOwnedAgentWaitBindingV8 {
    let src = include_str!("../../../../../examples/offline-repair-project/src/app.spx");
    let src = format!(
        "{}\n{}",
        src.replace(
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
    super::super::compile_owned_agent_wait_v8(
        &src,
        Path::new("ordinary-state-projection.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap()
}
#[test]
fn owned_wait_ordinary_state_borrowed_projection_is_exact_frozen_wire() {
    let b = binding();
    for (bytes, budget, epoch) in [
        (vec![], 0, 0),
        (vec![0], i64::MIN, i64::MAX),
        (vec![0, 171, 255], i64::MAX, i64::MIN),
    ] {
        let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let state = json!({"declaration":"fixture.agent.type.state","fields":[
            {"identity":"fixture.agent.type.state.objective","value":{"kind":"bytes","hex":hex}},
            {"identity":"fixture.agent.type.state.budget","value":{"tag":"i64","value":budget}},
            {"identity":"fixture.agent.type.state.epoch","value":{"tag":"i64","value":epoch}}]});
        let retained = RetainedValue::Record(RetainedRecord {
            record: DeclarationId::new("fixture.agent.type.state"),
            fields: vec![
                RetainedField {
                    field: DeclarationId::new("fixture.agent.type.state.objective"),
                    value: RetainedValue::Bytes(bytes),
                },
                RetainedField {
                    field: DeclarationId::new("fixture.agent.type.state.budget"),
                    value: RetainedValue::I64(budget),
                },
                RetainedField {
                    field: DeclarationId::new("fixture.agent.type.state.epoch"),
                    value: RetainedValue::I64(epoch),
                },
            ],
        });
        let ordinary = crate::agent_lifecycle::encode_value(&retained);
        assert_eq!(ordinary_state_bytes(&b, &state).unwrap(), ordinary);
        assert_eq!(
            owned_wait_ordinary_state_digest_v8(&b, &state).unwrap(),
            crate::live_invocation::identity::digest(
                b"semaprax.source-state.v2\0",
                ordinary.as_bytes()
            )
        );
        let mut wrong = state.clone();
        wrong["fields"].as_array_mut().unwrap().swap(0, 2);
        assert!(ordinary_state_bytes(&b, &wrong).is_err());
    }
}
