//! Independent frozen prompt oracle for the factored old and new formatters.
use super::*;
#[test]
fn owned_wait_sdk_borrowed_prompt_has_exact_frozen_legacy_bytes() {
    let task = crate::agent_lifecycle::LifecycleTask {
        objective: vec![0, 255],
        budget: 3,
    };
    let state = crate::interpreter::retained_call::RetainedValue::I64(7);
    let observation = crate::interpreter::retained_call::RetainedValue::Bool(false);
    let request = ProposalRequest {
        turn: 0,
        attempt: 0,
        task: &task,
        source_revision: "source",
        proposal_schema_digest: "schema",
        state: &state,
        observation: &observation,
        previous_effect: None,
        previous_rejection: None,
        remaining_iterations: 1,
    };
    let expected = r#"{"schema":"semaprax.source-adapter-prompt.v1","task_hex":"00ff","task_budget":3,"source_revision":"source","turn":0,"attempt":0,"remaining_iterations":1,"state":7,"observation":false,"previous_effect_hex":null,"previous_rejection":null,"proposal_schema":{}}"#;
    assert_eq!(canonical_prompt(&request, "{}"), expected);
    assert_eq!(
        canonical_prompt_parts(PromptParts {
            task: &task,
            source_revision: "source",
            turn: 0,
            attempt: 0,
            remaining_iterations: 1,
            state: "7",
            observation: "false",
            previous_effect: None,
            previous_rejection: None,
            schema: "{}"
        }),
        expected
    );
    assert_eq!(
        source_request_digest(expected.as_bytes()),
        source_request_digest(canonical_prompt(&request, "{}").as_bytes())
    );
}
