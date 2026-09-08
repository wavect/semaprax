//! Canonical additive v2 graph; frozen v1 wire and nonclaims are not embedded.
use super::*;

pub(super) fn render(
    agent_id: &str,
    definition_digest: &str,
    proposal_schema_digest: &str,
    binding: &StageBinding,
    source_revision: &str,
    step: &step::StepShape,
) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},\"definition_digest\":{},\"proposal_schema_digest\":{},\"types\":[",
        quote_json("semaprax.agent-iterative-lifecycle.v2"),
        quote_json(agent_id),
        quote_json(definition_digest),
        quote_json(proposal_schema_digest)
    );
    for (index, (role, id)) in binding.types.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"stable_id\":{}}}",
            quote_json(role),
            quote_json(id.as_str())
        ));
    }
    output.push_str("],\"proposal_projection\":[");
    for (index, parameter) in binding.proposal.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"stable_id\":{},\"representation\":{}}}",
            quote_json(parameter.field.as_str()),
            quote_json(parameter.kind.name())
        ));
    }
    output.push_str("],\"stages\":[");
    for (index, stage) in [
        &binding.initialize,
        &binding.observe,
        binding.authorize.stage(),
        &binding.reduce,
    ]
    .iter()
    .enumerate()
    {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"kind\":\"deterministic\",\"operation_id\":{},\"function_id\":{},\"parameters\":[",
            quote_json(stage.role()),
            quote_json(stage.operation_id()),
            quote_json(stage.function_id())
        ));
        for (position, (ownership, ty)) in stage.parameters().iter().enumerate() {
            if position > 0 {
                output.push(',');
            }
            output.push_str(&format!(
                "{{\"ownership\":{},\"type\":{}}}",
                quote_json(ownership),
                quote_json(ty)
            ));
        }
        output.push_str(&format!("],\"result\":{}}}", quote_json(stage.result())));
    }
    output.push_str(&format!(
        "],\"source_revision\":{},\"step\":{},\"decision\":{{\"type\":{},\"grant_case\":{},\"grant_seal_field\":{},\"grant_budget_field\":{},\"refuse_case\":{},\"refuse_code_field\":{}}}",
        quote_json(source_revision), step.canonical_json(),
        quote_json(binding.authorize.decision_type().as_str()),
        quote_json(binding.authorize.grant_case().as_str()),
        quote_json(binding.authorize.grant_seal_field().as_str()),
        quote_json(binding.authorize.grant_budget_field().as_str()),
        quote_json(binding.authorize.refuse_case().as_str()),
        quote_json(binding.authorize.refuse_code_field().as_str())
    ));
    output.push_str(",\"execution\":{\"initialize\":\"once\",\"iteration_order\":[\"observe\",\"propose\",\"authorize\",\"execute\",\"reduce\"],\"continue_target\":\"observe\",\"terminal_cases\":[\"Complete\",\"Suspend\",\"Fail\"],\"authorization\":\"fresh_per_iteration\",\"max_iterations\":4096,\"max_stages\":12289},\"nonclaims\":[\"no_durable_resume_authority_from_suspended_data\",\"no_effect_beyond_explicitly_injected_read\",\"no_ambient_authority\",\"no_hosted_or_native_execution_claim\"]}\n");
    output
}
