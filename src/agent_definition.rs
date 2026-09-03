//! Canonical AgentDefinition v1 to AgentGraph v1 compiler.
//!
//! This additive compiler slice gives an agent's semantic roles stable identities
//! while retaining Agent Runtime v1 as the execution kernel. The definition's
//! structured Runtime v1 material compiles to the frozen profile schema without
//! widening its schemas or authority.

use std::collections::BTreeSet;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::agent_runtime::{
    Agent, AgentBoundaryProbe, AgentCancellation, AgentHost, AgentProviderAttempt,
    AgentProviderSink, AgentToolResultSink,
};
use crate::diagnostic::{quote_json, Diagnostic};

const DEFINITION_SCHEMA: &str = "semaprax.agent-definition.v1";
const GRAPH_SCHEMA: &str = "semaprax.agent-graph.v1";
const PROFILE_SCHEMA: &str = "semaprax.agent-runtime-profile.v1";
const DEFINITION_DOMAIN: &[u8] = b"semaprax.agent-definition.digest.v1\0";
const GRAPH_DOMAIN: &[u8] = b"semaprax.agent-graph.digest.v1\0";
const PROFILE_DOMAIN: &[u8] = b"semaprax.agent-runtime.profile-digest.v1\0";
const MAX_DEFINITION_BYTES: usize = 1_310_720;
const MAX_GRAPH_BYTES: usize = 1_572_864;
const MAX_IDENTIFIER_BYTES: usize = 240;
const MAX_JSON_DEPTH: usize = 16;

const TYPE_ROLES: [&str; 6] = [
    "task",
    "state",
    "observation",
    "proposal",
    "outcome",
    "result",
];
const OPERATION_ROLES: [(&str, &str); 6] = [
    ("initialize", "deterministic"),
    ("observe", "deterministic"),
    ("propose", "model"),
    ("authorize", "deterministic"),
    ("execute", "effect"),
    ("reduce", "deterministic"),
];
const RUNTIME_V1_NONCLAIMS: [&str; 24] = [
    "no_compiler_determinism_from_model_output",
    "no_model_output_authority",
    "no_provider_identity_provenance_or_quality_truth",
    "no_secret_input_or_secret_leakage_guarantee_for_caller_supplied_content",
    "no_credential_prompt_state_trace_or_diagnostic_exposure",
    "no_ambient_network_filesystem_process_home_or_environment_authority",
    "no_write_apply_mutation_or_target_execution_tool_authority",
    "no_capability_minting_delegation_or_self_approval",
    "no_human_approval_ui_or_policy",
    "no_semantic_prompt_injection_proof",
    "no_forced_cancellation_or_preemption",
    "no_exactly_once_provider_billing_or_retry",
    "no_durable_memory_persistence_recovery_or_resume",
    "no_crash_reboot_or_power_loss_durability",
    "no_distributed_or_parallel_execution",
    "no_model_quality_accuracy_or_completion_guarantee",
    "no_live_price_or_cost_accuracy_guarantee",
    "no_reusable_authorization_token",
    "no_signature_attestation_or_authenticated_provenance",
    "no_wallet_payment_signing_asset_or_economic_authority",
    "no_privacy_compliance_or_data_residency_guarantee",
    "no_general_formal_proof",
    "no_new_language_graph_cleanup_backend_or_runtime_semantics",
    "no_current_schema_api_or_kat_modification",
];
const NONCLAIMS: [&str; 8] = [
    "no_agent_language_syntax_or_parser_admission",
    "no_generated_model_output_grammar",
    "no_compiled_transition_execution",
    "no_typed_write_effect_or_publication_authority",
    "no_checkpoint_resume_or_reconciliation",
    "no_provider_transport_or_credentials",
    "no_agent_runtime_v1_schema_api_or_kat_modification",
    "runtime_v1_projection_is_a_bounded_compatibility_profile",
];

#[derive(Clone, Eq, PartialEq)]
struct SemanticType {
    role: &'static str,
    stable_id: String,
}

#[derive(Clone, Eq, PartialEq)]
struct Operation {
    role: &'static str,
    stable_id: String,
    kind: &'static str,
}

/// One admitted canonical AgentDefinition v1.
pub struct AgentDefinition {
    agent_id: String,
    types: Vec<SemanticType>,
    operations: Vec<Operation>,
    runtime_v1: Value,
    runtime_v1_profile: String,
    source: String,
    digest: String,
}

/// One compiler-derived canonical AgentGraph v1.
pub struct AgentGraph {
    source: String,
    digest: String,
}

/// The complete output of the bounded AgentDefinition v1 compiler.
pub struct CompiledAgentDefinition {
    definition: AgentDefinition,
    graph: AgentGraph,
}

impl AgentDefinition {
    /// Returns the stable semantic identity of the declared agent.
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Returns the admitted canonical AgentDefinition, including its terminal LF.
    pub fn canonical_source(&self) -> &str {
        &self.source
    }

    /// Returns the domain-separated AgentDefinition digest.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Returns the byte-preserved canonical Agent Runtime Profile v1 projection.
    pub fn runtime_v1_profile(&self) -> &str {
        &self.runtime_v1_profile
    }
}

impl AgentGraph {
    /// Returns the canonical compiler projection, including its terminal LF.
    pub fn canonical_json(&self) -> &str {
        &self.source
    }

    /// Returns the domain-separated AgentGraph digest.
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

impl CompiledAgentDefinition {
    /// Returns the admitted definition.
    pub fn definition(&self) -> &AgentDefinition {
        &self.definition
    }

    /// Returns the compiler-derived AgentGraph.
    pub fn graph(&self) -> &AgentGraph {
        &self.graph
    }

    /// Returns the exact Agent Runtime Profile v1 projection.
    pub fn runtime_v1_profile(&self) -> &str {
        self.definition.runtime_v1_profile()
    }
}

/// Compiles one canonical AgentDefinition v1 into a deterministic AgentGraph v1.
///
/// Compilation is pure and grants no provider, tool, filesystem, process, or
/// publication authority. The Runtime v1 profile is validated through the
/// frozen public constructor and returned byte-for-byte unchanged.
pub fn compile_agent_definition(source: &str) -> Result<CompiledAgentDefinition, Vec<Diagnostic>> {
    compile(source).map_err(|diagnostic| vec![diagnostic])
}

/// Independently recompiles a definition and verifies its exact profile and graph.
pub fn verify_agent_graph_bundle(
    definition_source: &str,
    runtime_v1_profile_source: &str,
    graph_source: &str,
) -> Result<(), Vec<Diagnostic>> {
    if graph_source.len() > MAX_GRAPH_BYTES {
        return Err(vec![graph_mismatch()]);
    }
    let compiled = compile_agent_definition(definition_source)?;
    if compiled.runtime_v1_profile().as_bytes() != runtime_v1_profile_source.as_bytes() {
        return Err(vec![profile_mismatch()]);
    }
    if compiled.graph().canonical_json().as_bytes() != graph_source.as_bytes() {
        return Err(vec![graph_mismatch()]);
    }
    Ok(())
}

fn compile(source: &str) -> Result<CompiledAgentDefinition, Diagnostic> {
    let body = canonical_body(source)?;
    let value: Value = serde_json::from_str(body).map_err(|_| malformed())?;
    if json_depth(&value) > MAX_JSON_DEPTH {
        return Err(invariant("json_depth"));
    }
    let top = value.as_object().ok_or_else(malformed)?;
    if !exact_keys(
        top,
        &["schema", "agent_id", "types", "operations", "runtime_v1"],
    ) || string(top, "schema")? != DEFINITION_SCHEMA
    {
        return Err(malformed());
    }

    let agent_id = string(top, "agent_id")?.to_owned();
    if !canonical_identifier(&agent_id) {
        return Err(invariant("agent_id"));
    }
    let types = parse_types(top)?;
    let operations = parse_operations(top)?;
    let mut semantic_ids = BTreeSet::from([agent_id.clone()]);
    if types
        .iter()
        .map(|ty| &ty.stable_id)
        .chain(operations.iter().map(|operation| &operation.stable_id))
        .any(|stable_id| !semantic_ids.insert(stable_id.clone()))
    {
        return Err(invariant("semantic_ids"));
    }
    let runtime_v1 = top.get("runtime_v1").cloned().ok_or_else(malformed)?;
    let profile_source = render_runtime_v1_profile(&agent_id, &runtime_v1)?;
    validate_profile(&profile_source)?;

    let definition = AgentDefinition {
        agent_id,
        types,
        operations,
        runtime_v1,
        runtime_v1_profile: profile_source,
        source: source.to_owned(),
        digest: digest(DEFINITION_DOMAIN, source.as_bytes()),
    };
    if render_definition(&definition) != source {
        return Err(malformed());
    }
    let graph_source = render_graph(&definition);
    if graph_source.len() > MAX_GRAPH_BYTES {
        return Err(invariant("graph_bytes"));
    }
    let graph = AgentGraph {
        digest: digest(GRAPH_DOMAIN, graph_source.as_bytes()),
        source: graph_source,
    };
    Ok(CompiledAgentDefinition { definition, graph })
}

fn canonical_body(source: &str) -> Result<&str, Diagnostic> {
    if source.len() > MAX_DEFINITION_BYTES {
        return Err(invariant("definition_bytes"));
    }
    let Some(body) = source.strip_suffix('\n') else {
        return Err(malformed());
    };
    if body.is_empty() || body.contains('\n') || body.contains('\r') || body.starts_with('\u{feff}')
    {
        return Err(malformed());
    }
    Ok(body)
}

fn parse_types(top: &Map<String, Value>) -> Result<Vec<SemanticType>, Diagnostic> {
    let values = top
        .get("types")
        .and_then(Value::as_array)
        .ok_or_else(malformed)?;
    if values.len() != TYPE_ROLES.len() {
        return Err(invariant("types"));
    }
    let mut ids = BTreeSet::new();
    let mut types = Vec::with_capacity(values.len());
    for (value, role) in values.iter().zip(TYPE_ROLES) {
        let row = value.as_object().ok_or_else(malformed)?;
        if !exact_keys(row, &["role", "stable_id"]) || string(row, "role")? != role {
            return Err(invariant("types.roles"));
        }
        let stable_id = string(row, "stable_id")?.to_owned();
        if !canonical_identifier(&stable_id) || !ids.insert(stable_id.clone()) {
            return Err(invariant("types.stable_ids"));
        }
        types.push(SemanticType { role, stable_id });
    }
    Ok(types)
}

fn parse_operations(top: &Map<String, Value>) -> Result<Vec<Operation>, Diagnostic> {
    let values = top
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(malformed)?;
    if values.len() != OPERATION_ROLES.len() {
        return Err(invariant("operations"));
    }
    let mut ids = BTreeSet::new();
    let mut operations = Vec::with_capacity(values.len());
    for (value, (role, kind)) in values.iter().zip(OPERATION_ROLES) {
        let row = value.as_object().ok_or_else(malformed)?;
        if !exact_keys(row, &["role", "stable_id", "kind"])
            || string(row, "role")? != role
            || string(row, "kind")? != kind
        {
            return Err(invariant("operations.roles"));
        }
        let stable_id = string(row, "stable_id")?.to_owned();
        if !canonical_identifier(&stable_id) || !ids.insert(stable_id.clone()) {
            return Err(invariant("operations.stable_ids"));
        }
        operations.push(Operation {
            role,
            stable_id,
            kind,
        });
    }
    Ok(operations)
}

fn render_runtime_v1(value: &Value) -> Result<String, Diagnostic> {
    let runtime = value.as_object().ok_or_else(malformed)?;
    if !exact_keys(runtime, &["models", "tools", "policy", "limits"]) {
        return Err(malformed());
    }
    Ok(format!(
        "{{\"models\":{},\"tools\":{},\"policy\":{},\"limits\":{}}}",
        render_models(runtime.get("models").ok_or_else(malformed)?)?,
        render_tools(runtime.get("tools").ok_or_else(malformed)?)?,
        render_plain_object(
            runtime.get("policy").ok_or_else(malformed)?,
            &[
                "allowed_provider_ids",
                "allowed_model_ids",
                "required_locality",
                "minimum_quality_tier",
                "required_model_capabilities",
                "granted_capabilities",
                "allowed_tool_ids",
            ],
        )?,
        render_plain_object(
            runtime.get("limits").ok_or_else(malformed)?,
            &[
                "max_turns",
                "max_provider_attempts",
                "max_retries_per_turn",
                "max_concurrency",
                "max_elapsed_ms",
                "max_provider_request_bytes",
                "max_provider_response_bytes",
                "max_stream_chunks",
                "max_total_provider_input_bytes",
                "max_total_provider_output_bytes",
                "max_reported_model_input_tokens",
                "max_reported_model_output_tokens",
                "max_usd_microunits",
                "max_tool_calls",
                "max_tool_arguments_bytes",
                "max_tool_result_bytes",
                "max_total_tool_bytes",
                "max_retained_state_bytes",
                "max_trace_events",
                "max_trace_bytes",
                "max_evidence_bytes",
                "max_builder_bytes",
            ],
        )?,
    ))
}

fn render_runtime_v1_profile(agent_id: &str, value: &Value) -> Result<String, Diagnostic> {
    let runtime = render_runtime_v1(value)?;
    let members = runtime
        .strip_prefix('{')
        .and_then(|body| body.strip_suffix('}'))
        .ok_or_else(malformed)?;
    let nonclaims = serde_json::to_string(&RUNTIME_V1_NONCLAIMS).map_err(|_| malformed())?;
    Ok(format!(
        "{{\"schema\":{},\"agent_id\":{},{},\"nonclaims\":{}}}\n",
        quote_json(PROFILE_SCHEMA),
        quote_json(agent_id),
        members,
        nonclaims,
    ))
}

fn render_models(value: &Value) -> Result<String, Diagnostic> {
    render_plain_object_array(
        value,
        &[
            "provider_id",
            "model_id",
            "locality",
            "quality_tier",
            "tokenizer_id",
            "max_context_tokens",
            "input_usd_microunits_per_million_tokens",
            "output_usd_microunits_per_million_tokens",
            "capabilities",
        ],
    )
}

fn render_tools(value: &Value) -> Result<String, Diagnostic> {
    let rows = value.as_array().ok_or_else(malformed)?;
    let mut output = String::from("[");
    for (index, value) in rows.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        let row = value.as_object().ok_or_else(malformed)?;
        if !exact_keys(
            row,
            &[
                "tool_id",
                "description",
                "arguments_schema",
                "result_schema",
                "effects",
                "required_capabilities",
            ],
        ) {
            return Err(malformed());
        }
        output.push_str(&format!(
            "{{\"tool_id\":{},\"description\":{},\"arguments_schema\":{},\"result_schema\":{},\"effects\":{},\"required_capabilities\":{}}}",
            render_json(row.get("tool_id").ok_or_else(malformed)?)?,
            render_json(row.get("description").ok_or_else(malformed)?)?,
            render_closed_schema(row.get("arguments_schema").ok_or_else(malformed)?)?,
            render_closed_schema(row.get("result_schema").ok_or_else(malformed)?)?,
            render_json(row.get("effects").ok_or_else(malformed)?)?,
            render_json(row.get("required_capabilities").ok_or_else(malformed)?)?,
        ));
    }
    output.push(']');
    Ok(output)
}

fn render_closed_schema(value: &Value) -> Result<String, Diagnostic> {
    let schema = value.as_object().ok_or_else(malformed)?;
    if !exact_keys(schema, &["type", "fields", "additional_properties"]) {
        return Err(malformed());
    }
    let fields = render_plain_object_array(
        schema.get("fields").ok_or_else(malformed)?,
        &["name", "type", "required", "max_bytes"],
    )?;
    Ok(format!(
        "{{\"type\":{},\"fields\":{},\"additional_properties\":{}}}",
        render_json(schema.get("type").ok_or_else(malformed)?)?,
        fields,
        render_json(schema.get("additional_properties").ok_or_else(malformed)?)?,
    ))
}

fn render_plain_object_array(value: &Value, keys: &[&str]) -> Result<String, Diagnostic> {
    let rows = value.as_array().ok_or_else(malformed)?;
    let mut output = String::from("[");
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&render_plain_object(row, keys)?);
    }
    output.push(']');
    Ok(output)
}

fn render_plain_object(value: &Value, keys: &[&str]) -> Result<String, Diagnostic> {
    let object = value.as_object().ok_or_else(malformed)?;
    if !exact_keys(object, keys) {
        return Err(malformed());
    }
    let mut output = String::from("{");
    for (index, key) in keys.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(key));
        output.push(':');
        output.push_str(&render_json(object.get(*key).ok_or_else(malformed)?)?);
    }
    output.push('}');
    Ok(output)
}

fn render_json(value: &Value) -> Result<String, Diagnostic> {
    serde_json::to_string(value).map_err(|_| malformed())
}

fn validate_profile(profile: &str) -> Result<(), Diagnostic> {
    Agent::new(profile, ValidationHost, AgentCancellation::new())
        .map(|_| ())
        .map_err(|_| profile_failure())
}

fn render_definition(definition: &AgentDefinition) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},\"types\":[",
        quote_json(DEFINITION_SCHEMA),
        quote_json(&definition.agent_id)
    );
    for (index, ty) in definition.types.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"stable_id\":{}}}",
            quote_json(ty.role),
            quote_json(&ty.stable_id)
        ));
    }
    output.push_str("],\"operations\":[");
    for (index, operation) in definition.operations.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"stable_id\":{},\"kind\":{}}}",
            quote_json(operation.role),
            quote_json(&operation.stable_id),
            quote_json(operation.kind)
        ));
    }
    output.push_str("],\"runtime_v1\":");
    output.push_str(
        &render_runtime_v1(&definition.runtime_v1)
            .expect("admitted Runtime v1 projection material remains valid"),
    );
    output.push_str("}\n");
    output
}

fn render_graph(definition: &AgentDefinition) -> String {
    let profile_digest = digest(PROFILE_DOMAIN, definition.runtime_v1_profile.as_bytes());
    let mut output = format!(
        "{{\"schema\":{},\"definition_digest\":{},\"agent_id\":{},\"types\":[",
        quote_json(GRAPH_SCHEMA),
        quote_json(&definition.digest),
        quote_json(&definition.agent_id)
    );
    for (index, ty) in definition.types.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"stable_id\":{}}}",
            quote_json(ty.role),
            quote_json(&ty.stable_id)
        ));
    }
    output.push_str("],\"operations\":[");
    for (index, operation) in definition.operations.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"stable_id\":{},\"kind\":{}}}",
            quote_json(operation.role),
            quote_json(&operation.stable_id),
            quote_json(operation.kind)
        ));
    }
    output.push_str(
        "],\"derived_types\":[{\"node_id\":\"@authorized_proposal\",\"kind\":\"opaque_authorized\",\"value_type\":"
    );
    output.push_str(&quote_json(&definition.types[3].stable_id));
    output.push_str(",\"runtime_minted\":true,\"single_use\":true},{\"node_id\":\"@rejection\",\"kind\":\"runtime_rejection\"},{\"node_id\":\"@authorization_result\",\"kind\":\"result\",\"ok\":\"@authorized_proposal\",\"error\":\"@rejection\"},{\"node_id\":\"@suspension\",\"kind\":\"runtime_suspension\"},{\"node_id\":\"@agent_failure\",\"kind\":\"runtime_failure\"},{\"node_id\":\"@agent_step\",\"kind\":\"closed_runtime_variant\",\"variants\":[{\"kind\":\"continue\",\"fields\":[");
    output.push_str(&quote_json(&definition.types[1].stable_id));
    output.push_str("]},{\"kind\":\"complete\",\"fields\":[");
    output.push_str(&quote_json(&definition.types[5].stable_id));
    output.push_str("]},{\"kind\":\"suspend\",\"fields\":[");
    output.push_str(&quote_json(&definition.types[1].stable_id));
    output.push_str(",\"@suspension\"]},{\"kind\":\"fail\",\"fields\":[\"@agent_failure\"]}]}],\"relationships\":[");
    let typed_relationships = [
        (0, "consumes", 0),
        (0, "returns", 1),
        (1, "borrows", 1),
        (1, "returns", 2),
        (2, "borrows", 2),
        (2, "returns", 3),
        (3, "borrows", 1),
        (3, "borrows", 3),
    ];
    for (index, (operation, relationship, ty)) in typed_relationships.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"from\":{},\"relationship\":{},\"to\":{}}}",
            quote_json(&definition.operations[*operation].stable_id),
            quote_json(relationship),
            quote_json(&definition.types[*ty].stable_id)
        ));
    }
    for (operation, relationship, target) in [
        (3, "returns", "@authorization_result"),
        (4, "consumes", "@authorized_proposal"),
    ] {
        output.push_str(&format!(
            ",{{\"from\":{},\"relationship\":{},\"to\":{}}}",
            quote_json(&definition.operations[operation].stable_id),
            quote_json(relationship),
            quote_json(target)
        ));
    }
    for (operation, relationship, ty) in [
        (4, "returns", 4),
        (5, "consumes", 1),
        (5, "uses", 3),
        (5, "uses", 4),
    ] {
        output.push_str(&format!(
            ",{{\"from\":{},\"relationship\":{},\"to\":{}}}",
            quote_json(&definition.operations[operation].stable_id),
            quote_json(relationship),
            quote_json(&definition.types[ty].stable_id)
        ));
    }
    output.push_str(&format!(
        ",{{\"from\":{},\"relationship\":\"returns\",\"to\":\"@agent_step\"}}",
        quote_json(&definition.operations[5].stable_id)
    ));
    let runtime = definition
        .runtime_v1
        .as_object()
        .expect("admitted Runtime v1 material remains an object");
    let policy = runtime
        .get("policy")
        .and_then(Value::as_object)
        .expect("admitted Runtime v1 policy remains an object");
    output.push_str("],\"model_contract\":{\"operation_id\":");
    output.push_str(&quote_json(&definition.operations[2].stable_id));
    output.push_str(",\"requirements\":{\"required_locality\":");
    output.push_str(&render_admitted(policy, "required_locality"));
    output.push_str(",\"minimum_quality_tier\":");
    output.push_str(&render_admitted(policy, "minimum_quality_tier"));
    output.push_str(",\"required_capabilities\":");
    output.push_str(&render_admitted(policy, "required_model_capabilities"));
    output.push_str("},\"compatibility_route\":{\"allowed_provider_ids\":");
    output.push_str(&render_admitted(policy, "allowed_provider_ids"));
    output.push_str(",\"allowed_model_ids\":");
    output.push_str(&render_admitted(policy, "allowed_model_ids"));
    output.push_str("}},\"context_plan\":{\"task_schema\":\"semaprax.agent-runtime-task.v1\",\"objective\":\"ordered_utf8\",\"context\":\"ordered_provenance_labelled_utf8\",\"deterministic_order\":true},\"proposal_contract\":{\"type_id\":");
    output.push_str(&quote_json(&definition.types[3].stable_id));
    output.push_str(",\"wire_schema\":\"semaprax.agent-runtime-action.v1\",\"variants\":[{\"kind\":\"final\"},{\"kind\":\"tool\",\"allowed_tool_ids\":");
    output.push_str(&render_admitted(policy, "allowed_tool_ids"));
    output.push_str("}],\"untrusted_output\":true},\"capability_manifest\":{\"granted\":");
    output.push_str(&render_admitted(policy, "granted_capabilities"));
    output.push_str(",\"model_cannot_mint\":true},\"effect_bindings\":");
    output.push_str(
        &render_tools(
            runtime
                .get("tools")
                .expect("admitted Runtime v1 tools remain present"),
        )
        .expect("admitted Runtime v1 tools remain canonical"),
    );
    output.push_str(",\"limits\":");
    output.push_str(
        &render_plain_object(
            runtime
                .get("limits")
                .expect("admitted Runtime v1 limits remain present"),
            &[
                "max_turns",
                "max_provider_attempts",
                "max_retries_per_turn",
                "max_concurrency",
                "max_elapsed_ms",
                "max_provider_request_bytes",
                "max_provider_response_bytes",
                "max_stream_chunks",
                "max_total_provider_input_bytes",
                "max_total_provider_output_bytes",
                "max_reported_model_input_tokens",
                "max_reported_model_output_tokens",
                "max_usd_microunits",
                "max_tool_calls",
                "max_tool_arguments_bytes",
                "max_tool_result_bytes",
                "max_total_tool_bytes",
                "max_retained_state_bytes",
                "max_trace_events",
                "max_trace_bytes",
                "max_evidence_bytes",
                "max_builder_bytes",
            ],
        )
        .expect("admitted Runtime v1 limits remain canonical"),
    );
    output.push_str(",\"approval_requirements\":[],\"terminal_conditions\":[\"completed\",\"cancelled\",\"deadline_exceeded\",\"budget_exhausted\",\"provider_failed\",\"tool_failed\",\"policy_rejected\"],\"evidence_obligations\":{\"trace_schema\":\"semaprax.agent-runtime-trace.v1\",\"evidence_schema\":\"semaprax.agent-runtime-evidence.v1\",\"binds_profile_digest\":true,\"binds_task_digest\":true,\"replay_required\":true},\"references\":{\"program_declarations\":[],\"workspace_operations\":[],\"tests\":[],\"validations\":[]},\"runtime_v1_profile_digest\":");
    output.push_str(&quote_json(&profile_digest));
    output.push_str(",\"nonclaims\":[");
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(nonclaim));
    }
    output.push_str("]}\n");
    output
}

fn exact_keys(object: &Map<String, Value>, keys: &[&str]) -> bool {
    object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
}

fn render_admitted(object: &Map<String, Value>, key: &str) -> String {
    render_json(
        object
            .get(key)
            .expect("admitted Runtime v1 material retains every canonical field"),
    )
    .expect("admitted Runtime v1 material remains renderable")
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(malformed)
}

fn canonical_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(values) => 1 + values.iter().map(json_depth).max().unwrap_or(0),
        Value::Object(values) => 1 + values.values().map(json_depth).max().unwrap_or(0),
        _ => 1,
    }
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn malformed() -> Diagnostic {
    Diagnostic::io(
        "SPX-G501",
        format!("AgentDefinition is not canonical {DEFINITION_SCHEMA} JSON"),
    )
}

fn invariant(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G502",
        format!("AgentDefinition invariant failed: {field}"),
    )
}

fn profile_failure() -> Diagnostic {
    Diagnostic::io(
        "SPX-G502",
        "AgentDefinition invariant failed: runtime_v1_profile",
    )
}

fn graph_mismatch() -> Diagnostic {
    Diagnostic::io(
        "SPX-G503",
        "AgentGraph is not the exact replay of its canonical AgentDefinition",
    )
}

fn profile_mismatch() -> Diagnostic {
    Diagnostic::io(
        "SPX-G504",
        "Agent Runtime Profile v1 is not the exact AgentDefinition projection",
    )
}

struct ValidationProbe;

impl AgentBoundaryProbe for ValidationProbe {
    fn policy_epoch(&self) -> u64 {
        0
    }

    fn elapsed_ms(&self) -> u64 {
        0
    }
}

struct ValidationHost;

impl AgentHost for ValidationHost {
    fn policy_epoch(&self) -> u64 {
        0
    }

    fn elapsed_ms(&self) -> u64 {
        0
    }

    fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe> {
        Box::new(ValidationProbe)
    }

    fn tokenize(&mut self, _: &str, _: &str) -> Option<u64> {
        None
    }

    fn attempt_provider(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
        _: u64,
        _: &mut AgentProviderSink,
    ) -> AgentProviderAttempt {
        unreachable!("profile validation never invokes a provider")
    }

    fn invoke_tool(&mut self, _: &str, _: &str, _: &str, _: &mut AgentToolResultSink) -> bool {
        unreachable!("profile validation never invokes a tool")
    }
}
