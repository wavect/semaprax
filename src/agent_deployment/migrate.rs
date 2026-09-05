//! Exact migration of one AgentDefinition v1 into a v2 definition plus one
//! deployment.
//!
//! The migration is evidence, not a conversion convenience: binding the pair
//! it produces reproduces the original v1 document byte for byte, so the v1
//! graph and Runtime v1 profile known answers are preserved by construction.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::agent_definition::compile_agent_definition;
use crate::diagnostic::Diagnostic;

use super::documents::{
    canonical_identifier, render_definition_v2, render_deployment, tool_capabilities, tool_ids,
    DefinitionV2, Deployment,
};
use super::{
    definition_invariant, definition_malformed, deployment_invariant, digest, DEFINITION_V2_DOMAIN,
    OPERATION_ROLES, TYPE_ROLES,
};

/// Migrates one canonical AgentDefinition v1 into an AgentDefinition v2 and
/// one AgentDeployment v1 that reproduce it exactly when bound.
///
/// The source definition is admitted through the unchanged v1 compiler first,
/// so a document this rejects is never split. `deployment_id` is supplied by
/// the caller; the compiler invents no identity.
pub fn migrate_agent_definition_v1(
    v1_source: &str,
    deployment_id: &str,
) -> Result<(String, String), Vec<Diagnostic>> {
    compile_agent_definition(v1_source)?;
    if !canonical_identifier(deployment_id) {
        return Err(vec![deployment_invariant("deployment_id")]);
    }
    split(v1_source, deployment_id).map_err(|diagnostic| vec![diagnostic])
}

fn split(v1_source: &str, deployment_id: &str) -> Result<(String, String), Diagnostic> {
    let value: Value = serde_json::from_str(v1_source.trim_end_matches('\n'))
        .map_err(|_| definition_malformed())?;
    let top = value.as_object().ok_or_else(definition_malformed)?;
    let agent_id = text(top.get("agent_id"))?;
    let types = role_ids(top.get("types"), TYPE_ROLES.len())?;
    let operations = role_ids(top.get("operations"), OPERATION_ROLES.len())?;
    let runtime = top
        .get("runtime_v1")
        .and_then(Value::as_object)
        .ok_or_else(definition_malformed)?;
    let tools = runtime
        .get("tools")
        .cloned()
        .ok_or_else(definition_malformed)?;
    let models = runtime
        .get("models")
        .cloned()
        .ok_or_else(definition_malformed)?;
    let limits = runtime
        .get("limits")
        .cloned()
        .ok_or_else(definition_malformed)?;
    let policy = runtime
        .get("policy")
        .and_then(Value::as_object)
        .ok_or_else(definition_malformed)?;

    let granted_capabilities = list(policy.get("granted_capabilities"))?;
    let policy_tool_ids = list(policy.get("allowed_tool_ids"))?;

    // The source contract must cover every capability its own declared tools
    // need, and may allow every tool it declares. The deployment then narrows
    // to exactly the v1 policy's grants.
    let mut required_capabilities = granted_capabilities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for capabilities in tool_capabilities(&tools)? {
        required_capabilities.extend(capabilities);
    }
    let declared_tool_ids = tool_ids(&tools)?.into_iter().collect::<BTreeSet<_>>();

    let definition = DefinitionV2 {
        agent_id: agent_id.clone(),
        types,
        operations,
        tools,
        required_locality: text(policy.get("required_locality"))?,
        minimum_quality_tier: text(policy.get("minimum_quality_tier"))?,
        required_model_capabilities: list(policy.get("required_model_capabilities"))?,
        required_capabilities: required_capabilities.into_iter().collect(),
        allowed_tool_ids: declared_tool_ids.into_iter().collect(),
        required_target_features: Vec::new(),
        ceilings: limits.clone(),
    };
    let definition_source = render_definition_v2(&definition);

    let deployment = Deployment {
        deployment_id: deployment_id.to_owned(),
        definition_digest: digest(DEFINITION_V2_DOMAIN, definition_source.as_bytes()),
        models,
        allowed_provider_ids: list(policy.get("allowed_provider_ids"))?,
        allowed_model_ids: list(policy.get("allowed_model_ids"))?,
        granted_capabilities,
        allowed_tool_ids: policy_tool_ids,
        target_features: Vec::new(),
        limits,
    };
    Ok((definition_source, render_deployment(&deployment)))
}

fn role_ids(value: Option<&Value>, expected: usize) -> Result<Vec<String>, Diagnostic> {
    let rows = value
        .and_then(Value::as_array)
        .ok_or_else(definition_malformed)?;
    if rows.len() != expected {
        return Err(definition_invariant("roles"));
    }
    rows.iter().map(|row| text(row.get("stable_id"))).collect()
}

fn text(value: Option<&Value>) -> Result<String, Diagnostic> {
    value
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(definition_malformed)
}

fn list(value: Option<&Value>) -> Result<Vec<String>, Diagnostic> {
    value
        .and_then(Value::as_array)
        .ok_or_else(definition_malformed)?
        .iter()
        .map(|item| text(Some(item)))
        .collect()
}
