use super::*;
use serde_json::json;
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
fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal
requires observation.budget >= 0
{
    let proposal = yield observation;
    state
}
"#
    );
    super::super::compile_owned_agent_wait_v8(
        &source,
        Path::new("owned-data.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap()
}
fn i64_value(value: i64) -> Value {
    codec::scalar(&ArgumentValue::Int(value)).unwrap()
}
fn field(id: &str, value: Value) -> Value {
    json!({"identity":id,"value":value})
}
fn state() -> Value {
    json!({"declaration":"fixture.agent.type.state","fields":[
        field("fixture.agent.type.state.objective",json!({"kind":"bytes","hex":"6162"})),
        field("fixture.agent.type.state.budget",i64_value(4)),
        field("fixture.agent.type.state.epoch",i64_value(0))]})
}
fn granted() -> Value {
    json!({"declaration":"fixture.agent.type.decision","case":"fixture.agent.type.decision.granted",
        "fields":[field("fixture.agent.type.decision.granted.seal", json!({"kind":"bytes","hex":""})),
        field("fixture.agent.type.decision.granted.budget",i64_value(2))]})
}
#[test]
fn owned_wait_data_binds_exact_state_and_complete_decision_without_owners() {
    let b = binding();
    let original = state();
    validate_owned_wait_state_v8(&b, &original).unwrap();
    for value in [
        {
            let mut v = original.clone();
            v["declaration"] = json!("wrong");
            v
        },
        {
            let mut v = original.clone();
            v["fields"].as_array_mut().unwrap().swap(0, 1);
            v
        },
        {
            let mut v = original.clone();
            v["fields"][1]["value"] = codec::scalar(&ArgumentValue::Bool(true)).unwrap();
            v
        },
        {
            let mut v = original.clone();
            v["extra"] = json!(true);
            v
        },
        {
            let mut v = original.clone();
            v["fields"][0]["value"]["hex"] = json!("aa".repeat(1025));
            v
        },
    ] {
        assert!(validate_owned_wait_state_v8(&b, &value).is_err(), "{value}");
    }
    let original = granted();
    validate_owned_wait_decision_v8(&b, &original).unwrap();
    let refused = json!({"declaration":"fixture.agent.type.decision","case":"fixture.agent.type.decision.refused",
        "fields":[field("fixture.agent.type.decision.refused.code",i64_value(1))]});
    validate_owned_wait_decision_v8(&b, &refused).unwrap();
    for value in [
        {
            let mut v = original.clone();
            v["case"] = json!("fixture.agent.type.decision.refused");
            v
        },
        {
            let mut v = original.clone();
            v["declaration"] = json!("different.decision");
            v
        },
        {
            let mut v = original.clone();
            v["fields"].as_array_mut().unwrap().pop();
            v
        },
        {
            let mut v = original.clone();
            v["fields"].as_array_mut().unwrap().swap(0, 1);
            v
        },
        {
            let mut v = original.clone();
            v["fields"][0]["value"] = i64_value(1);
            v
        },
        {
            let mut v = original.clone();
            v["fields"][1]["value"] = codec::scalar(&ArgumentValue::Int32(1)).unwrap();
            v
        },
    ] {
        assert!(
            validate_owned_wait_decision_v8(&b, &value).is_err(),
            "{value}"
        );
    }
}
#[test]
fn owned_wait_data_keeps_compiler_cleanup_vector_order_and_whole_observed_receipt() {
    let b = binding();
    let mut actions = b.helper().liveness().failure_cleanup.clone();
    actions.extend_from_slice(b.authorize().disposal());
    assert!(actions.len() >= 2);
    let operations = owned_wait_operations_v8(&actions).unwrap();
    validate_owned_wait_operations_v8(&actions, &operations).unwrap();
    assert_eq!(
        operations[0],
        serde_json::from_str::<Value>(&crate::graph_cleanup::finalize_action_json(&actions[0]))
            .unwrap()
    );
    let partial = owned_wait_operations_v8(b.authorize().partial_disposal()).unwrap();
    assert!(codec::canonical(&partial)
        .windows(9)
        .any(|w| w == b"temporary"));
    let mut reversed = operations.clone();
    reversed.as_array_mut().unwrap().reverse();
    assert!(validate_owned_wait_operations_v8(&actions, &reversed).is_err());
    let entries = operations
        .as_array()
        .unwrap()
        .iter()
        .map(|v| json!({"operation":v,"outcome":"completed"}))
        .collect::<Vec<_>>();
    let receipt = json!({"kind":"observed","settlement":"completed","operations":entries});
    validate_owned_wait_observed_receipt_v8(&operations, &receipt).unwrap();
    let mut failed = receipt.clone();
    failed["operations"][0]["outcome"] = json!("failed");
    assert!(validate_owned_wait_observed_receipt_v8(&operations, &failed).is_err());
    failed["settlement"] = json!("failed");
    validate_owned_wait_observed_receipt_v8(&operations, &failed).unwrap();
    let mut omitted = receipt.clone();
    omitted["operations"].as_array_mut().unwrap().pop();
    assert!(validate_owned_wait_observed_receipt_v8(&operations, &omitted).is_err());
    let mut reordered = receipt.clone();
    reordered["operations"].as_array_mut().unwrap().reverse();
    assert!(validate_owned_wait_observed_receipt_v8(&operations, &reordered).is_err());
    let mut wrong = receipt.clone();
    wrong["operations"][0]["outcome"] = json!("skipped");
    assert!(validate_owned_wait_observed_receipt_v8(&operations, &wrong).is_err());
}
#[test]
fn owned_wait_data_failure_requires_actual_function_and_frozen_status_shape() {
    let b = binding();
    let id = &b.helper().function().id;
    for failure in [
        "fuel_exhausted",
        "host_abandoned",
        "answer_type_mismatch",
        "evaluation_rejected",
        "handler_failed",
        "call_depth_exceeded",
    ] {
        let status = json!({"failure":failure,"language_status":null});
        validate_owned_wait_failure_v8(&b, id, &status).unwrap();
        assert!(
            validate_owned_wait_failure_v8(&b, &DeclarationId::new("not.a.role"), &status).is_err()
        );
        let mut wrong = status;
        wrong["language_status"] = json!({});
        assert!(validate_owned_wait_failure_v8(&b, id, &wrong).is_err());
    }
    assert!(validate_owned_wait_failure_v8(
        &b,
        id,
        &json!({"failure":"cleanup_failed","language_status":null})
    )
    .is_err());
    assert!(validate_owned_wait_failure_v8(
        &b,
        id,
        &json!({"failure":"language_failure","language_status":null})
    )
    .is_err());
    assert!(validate_owned_wait_failure_v8(
        &b,
        id,
        &json!({"failure":"fuel_exhausted","language_status":null,"status":0})
    )
    .is_err());
}

#[test]
fn owned_wait_data_language_failure_is_bound_to_actual_compiler_status_source() {
    let b = binding();
    let id = &b.helper().function().id;
    let normalized = crate::conformance::NormalizedStatus::contract(
        crate::cleanup_plan::ContractPhase::Requires,
    );
    let payload: Value = serde_json::from_str(&normalized.to_json()).unwrap();
    let original = json!({"failure":"language_failure","language_status":payload});
    validate_owned_wait_failure_v8(&b, id, &original).unwrap();
    assert!(validate_owned_wait_failure_v8(&b, &b.observe().function().id, &original).is_err());
    for key in ["code", "class", "retryable", "domain_id"] {
        let mut wrong = original.clone();
        wrong["language_status"][key] = if key == "retryable" {
            json!(true)
        } else {
            json!("other")
        };
        assert!(
            validate_owned_wait_failure_v8(&b, id, &wrong).is_err(),
            "{key}"
        );
    }
}
