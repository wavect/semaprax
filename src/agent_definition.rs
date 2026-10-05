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
#[cfg(test)]
pub(crate) const GRAPH_DOMAIN_FOR_TESTS: &[u8] = GRAPH_DOMAIN;
const PROFILE_DOMAIN: &[u8] = b"semaprax.agent-runtime.profile-digest.v1\0";
const MAX_DEFINITION_BYTES: usize = 1_310_720;
const MAX_GRAPH_BYTES: usize = 1_572_864;
const MAX_IDENTIFIER_BYTES: usize = 240;
const MAX_JSON_DEPTH: usize = 16;

/// The six type roles. [`TypeRole::ALL`] is the single owner of their wire
/// names and normative order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TypeRole {
    Task,
    State,
    Observation,
    Proposal,
    Outcome,
    Result,
}

impl TypeRole {
    const ALL: [Self; 6] = [
        Self::Task,
        Self::State,
        Self::Observation,
        Self::Proposal,
        Self::Outcome,
        Self::Result,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::State => "state",
            Self::Observation => "observation",
            Self::Proposal => "proposal",
            Self::Outcome => "outcome",
            Self::Result => "result",
        }
    }
}

/// The six operation roles. [`OperationRole::ALL`] is the single owner of
/// their wire names, declared kinds and normative order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OperationRole {
    Initialize,
    Observe,
    Propose,
    Authorize,
    Execute,
    Reduce,
}

impl OperationRole {
    const ALL: [Self; 6] = [
        Self::Initialize,
        Self::Observe,
        Self::Propose,
        Self::Authorize,
        Self::Execute,
        Self::Reduce,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::Observe => "observe",
            Self::Propose => "propose",
            Self::Authorize => "authorize",
            Self::Execute => "execute",
            Self::Reduce => "reduce",
        }
    }

    fn kind(self) -> &'static str {
        match self {
            Self::Propose => "model",
            Self::Execute => "effect",
            Self::Initialize | Self::Observe | Self::Authorize | Self::Reduce => "deterministic",
        }
    }
}
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
    role: TypeRole,
    stable_id: String,
}

#[derive(Clone, Eq, PartialEq)]
struct Operation {
    role: OperationRole,
    stable_id: String,
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

    /// Returns the stable type identity admitted for the Proposal role.
    ///
    /// Additive read-only accessor for the derived proposal grammar. It
    /// changes no admitted byte and grants no authority.
    pub fn proposal_type_id(&self) -> &str {
        self.role_type(TypeRole::Proposal)
    }

    /// Returns the stable type identity admitted for the Observation role.
    ///
    /// This additive read-only accessor changes no admitted byte and grants
    /// no authority.
    pub fn observation_type_id(&self) -> &str {
        self.role_type(TypeRole::Observation)
    }

    /// Returns the stable type identity admitted for one of the six type
    /// roles, named in the normative order `task`, `state`, `observation`,
    /// `proposal`, `outcome`, `result`.
    ///
    /// This additive read-only accessor changes no admitted byte and grants
    /// no authority.
    pub fn type_id(&self, role: &str) -> Option<&str> {
        TypeRole::ALL
            .into_iter()
            .find(|candidate| candidate.name() == role)
            .map(|role| self.role_type(role))
    }

    /// Returns the stable operation identity and declared kind admitted for
    /// one of the six operation roles.
    ///
    /// This additive read-only accessor changes no admitted byte and grants
    /// no authority.
    pub fn operation(&self, role: &str) -> Option<(&str, &str)> {
        OperationRole::ALL
            .into_iter()
            .find(|candidate| candidate.name() == role)
            .map(|role| (self.role_operation(role), role.kind()))
    }

    /// Returns the stable identity admitted for `role`. Admission binds
    /// exactly one identity to every role, so the lookup cannot miss.
    fn role_type(&self, role: TypeRole) -> &str {
        self.types
            .iter()
            .find(|ty| ty.role == role)
            .map(|ty| ty.stable_id.as_str())
            .expect("admission binds every type role exactly once")
    }

    /// Returns the stable identity admitted for operation `role`.
    fn role_operation(&self, role: OperationRole) -> &str {
        self.operations
            .iter()
            .find(|operation| operation.role == role)
            .map(|operation| operation.stable_id.as_str())
            .expect("admission binds every operation role exactly once")
    }

    /// Writes the admitted `"types"` and `"operations"` sections.
    fn write_role_sections(&self, output: &mut String) {
        write_type_roles(
            output,
            self.types.iter().map(|ty| (ty.role, ty.stable_id.as_str())),
        );
        output.push(',');
        write_operation_roles(
            output,
            self.operations
                .iter()
                .map(|operation| (operation.role, operation.stable_id.as_str())),
        );
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

    /// Instantiates the admitted definition through its exact Runtime v1
    /// compatibility projection.
    ///
    /// The caller supplies every provider and tool effect through `host` and
    /// retains cooperative cancellation authority. The compiler-owned graph
    /// and profile remain immutable; no graph fact is accepted as execution
    /// input and no ambient authority is introduced by this bridge.
    pub fn instantiate<H: AgentHost>(
        &self,
        host: H,
        cancellation: AgentCancellation,
    ) -> Result<Agent<H>, Vec<Diagnostic>> {
        Agent::new(self.runtime_v1_profile(), host, cancellation)
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
    verify_compiled_agent_graph(&compiled, graph_source)
}

/// Exact-compares a submitted AgentGraph with the graph of a compilation the
/// caller has just produced from authoritative definition source.
///
/// This is the reuse seam for a composite verifier that already compiled the
/// definition within the same verification call. It never accepts a
/// caller-supplied cache or a submitted artifact as authority, and it keeps
/// the AgentGraph input bound before comparing bytes.
pub(crate) fn verify_compiled_agent_graph(
    compiled: &CompiledAgentDefinition,
    graph_source: &str,
) -> Result<(), Vec<Diagnostic>> {
    if graph_source.len() > MAX_GRAPH_BYTES {
        return Err(vec![graph_mismatch()]);
    }
    if compiled.graph().canonical_json().as_bytes() != graph_source.as_bytes() {
        return Err(vec![graph_mismatch()]);
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    static COMPILATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Number of AgentDefinition compilations performed on this test thread.
#[cfg(test)]
pub(crate) fn compilations_on_this_thread() -> usize {
    COMPILATIONS.with(std::cell::Cell::get)
}

fn compile(source: &str) -> Result<CompiledAgentDefinition, Diagnostic> {
    #[cfg(test)]
    COMPILATIONS.with(|count| count.set(count.get() + 1));
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
    if values.len() != TypeRole::ALL.len() {
        return Err(invariant("types"));
    }
    let mut ids = BTreeSet::new();
    let mut types = Vec::with_capacity(values.len());
    for (value, role) in values.iter().zip(TypeRole::ALL) {
        let row = value.as_object().ok_or_else(malformed)?;
        if !exact_keys(row, &["role", "stable_id"]) || string(row, "role")? != role.name() {
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
    if values.len() != OperationRole::ALL.len() {
        return Err(invariant("operations"));
    }
    let mut ids = BTreeSet::new();
    let mut operations = Vec::with_capacity(values.len());
    for (value, role) in values.iter().zip(OperationRole::ALL) {
        let row = value.as_object().ok_or_else(malformed)?;
        if !exact_keys(row, &["role", "stable_id", "kind"])
            || string(row, "role")? != role.name()
            || string(row, "kind")? != role.kind()
        {
            return Err(invariant("operations.roles"));
        }
        let stable_id = string(row, "stable_id")?.to_owned();
        if !canonical_identifier(&stable_id) || !ids.insert(stable_id.clone()) {
            return Err(invariant("operations.stable_ids"));
        }
        operations.push(Operation { role, stable_id });
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
        render_plain_object(runtime.get("policy").ok_or_else(malformed)?, &POLICY_KEYS,)?,
        render_plain_object(runtime.get("limits").ok_or_else(malformed)?, &LIMIT_KEYS,)?,
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

pub(crate) fn render_models(value: &Value) -> Result<String, Diagnostic> {
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

pub(crate) fn render_tools(value: &Value) -> Result<String, Diagnostic> {
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

pub(crate) fn render_plain_object(value: &Value, keys: &[&str]) -> Result<String, Diagnostic> {
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

/// The exact canonical Runtime v1 policy key order.
pub(crate) const POLICY_KEYS: [&str; 7] = [
    "allowed_provider_ids",
    "allowed_model_ids",
    "required_locality",
    "minimum_quality_tier",
    "required_model_capabilities",
    "granted_capabilities",
    "allowed_tool_ids",
];

/// The exact canonical Runtime v1 limit key order.
pub(crate) const LIMIT_KEYS: [&str; 22] = [
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
];

/// Renders one canonical AgentDefinition v1 document from assembled material.
///
/// This is the compiler-owned projection seam the additive
/// definition/deployment split uses. It renders bytes only; admission stays
/// [`compile_agent_definition`]'s job, so no caller bypasses validation.
pub(crate) fn render_v1_definition_source(
    agent_id: &str,
    types: &[String],
    operations: &[String],
    runtime_v1: &Value,
) -> Result<String, Diagnostic> {
    if types.len() != TypeRole::ALL.len() || operations.len() != OperationRole::ALL.len() {
        return Err(malformed());
    }
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},",
        quote_json(DEFINITION_SCHEMA),
        quote_json(agent_id)
    );
    write_type_roles(
        &mut output,
        TypeRole::ALL
            .into_iter()
            .zip(types.iter().map(String::as_str)),
    );
    output.push(',');
    write_operation_roles(
        &mut output,
        OperationRole::ALL
            .into_iter()
            .zip(operations.iter().map(String::as_str)),
    );
    output.push_str(",\"runtime_v1\":");
    output.push_str(&render_runtime_v1(runtime_v1)?);
    output.push_str("}\n");
    Ok(output)
}

/// Writes one ordered `"types"` section from `(role, stable_id)` rows.
fn write_type_roles<'a>(output: &mut String, rows: impl Iterator<Item = (TypeRole, &'a str)>) {
    output.push_str("\"types\":[");
    for (index, (role, stable_id)) in rows.enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"stable_id\":{}}}",
            quote_json(role.name()),
            quote_json(stable_id)
        ));
    }
    output.push(']');
}

/// Writes one ordered `"operations"` section from `(role, stable_id)` rows,
/// with each role's declared kind.
fn write_operation_roles<'a>(
    output: &mut String,
    rows: impl Iterator<Item = (OperationRole, &'a str)>,
) {
    output.push_str("\"operations\":[");
    for (index, (role, stable_id)) in rows.enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"stable_id\":{},\"kind\":{}}}",
            quote_json(role.name()),
            quote_json(stable_id),
            quote_json(role.kind())
        ));
    }
    output.push(']');
}

fn render_definition(definition: &AgentDefinition) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},",
        quote_json(DEFINITION_SCHEMA),
        quote_json(&definition.agent_id)
    );
    definition.write_role_sections(&mut output);
    output.push_str(",\"runtime_v1\":");
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
        "{{\"schema\":{},\"definition_digest\":{},\"agent_id\":{},",
        quote_json(GRAPH_SCHEMA),
        quote_json(&definition.digest),
        quote_json(&definition.agent_id)
    );
    definition.write_role_sections(&mut output);
    output.push_str(
        ",\"derived_types\":[{\"node_id\":\"@authorized_proposal\",\"kind\":\"opaque_authorized\",\"value_type\":"
    );
    output.push_str(&quote_json(definition.role_type(TypeRole::Proposal)));
    output.push_str(",\"runtime_minted\":true,\"single_use\":true},{\"node_id\":\"@rejection\",\"kind\":\"runtime_rejection\"},{\"node_id\":\"@authorization_result\",\"kind\":\"result\",\"ok\":\"@authorized_proposal\",\"error\":\"@rejection\"},{\"node_id\":\"@suspension\",\"kind\":\"runtime_suspension\"},{\"node_id\":\"@agent_failure\",\"kind\":\"runtime_failure\"},{\"node_id\":\"@agent_step\",\"kind\":\"closed_runtime_variant\",\"variants\":[{\"kind\":\"continue\",\"fields\":[");
    output.push_str(&quote_json(definition.role_type(TypeRole::State)));
    output.push_str("]},{\"kind\":\"complete\",\"fields\":[");
    output.push_str(&quote_json(definition.role_type(TypeRole::Result)));
    output.push_str("]},{\"kind\":\"suspend\",\"fields\":[");
    output.push_str(&quote_json(definition.role_type(TypeRole::State)));
    output.push_str(",\"@suspension\"]},{\"kind\":\"fail\",\"fields\":[\"@agent_failure\"]}]}],\"relationships\":[");
    let typed_relationships = [
        (OperationRole::Initialize, "consumes", TypeRole::Task),
        (OperationRole::Initialize, "returns", TypeRole::State),
        (OperationRole::Observe, "borrows", TypeRole::State),
        (OperationRole::Observe, "returns", TypeRole::Observation),
        (OperationRole::Propose, "borrows", TypeRole::Observation),
        (OperationRole::Propose, "returns", TypeRole::Proposal),
        (OperationRole::Authorize, "borrows", TypeRole::State),
        (OperationRole::Authorize, "borrows", TypeRole::Proposal),
    ];
    for (index, (operation, relationship, ty)) in typed_relationships.into_iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"from\":{},\"relationship\":{},\"to\":{}}}",
            quote_json(definition.role_operation(operation)),
            quote_json(relationship),
            quote_json(definition.role_type(ty))
        ));
    }
    for (operation, relationship, target) in [
        (OperationRole::Authorize, "returns", "@authorization_result"),
        (OperationRole::Execute, "consumes", "@authorized_proposal"),
    ] {
        output.push_str(&format!(
            ",{{\"from\":{},\"relationship\":{},\"to\":{}}}",
            quote_json(definition.role_operation(operation)),
            quote_json(relationship),
            quote_json(target)
        ));
    }
    for (operation, relationship, ty) in [
        (OperationRole::Execute, "returns", TypeRole::Outcome),
        (OperationRole::Reduce, "consumes", TypeRole::State),
        (OperationRole::Reduce, "uses", TypeRole::Proposal),
        (OperationRole::Reduce, "uses", TypeRole::Outcome),
    ] {
        output.push_str(&format!(
            ",{{\"from\":{},\"relationship\":{},\"to\":{}}}",
            quote_json(definition.role_operation(operation)),
            quote_json(relationship),
            quote_json(definition.role_type(ty))
        ));
    }
    output.push_str(&format!(
        ",{{\"from\":{},\"relationship\":\"returns\",\"to\":\"@agent_step\"}}",
        quote_json(definition.role_operation(OperationRole::Reduce))
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
    output.push_str(&quote_json(
        definition.role_operation(OperationRole::Propose),
    ));
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
    output.push_str(&quote_json(definition.role_type(TypeRole::Proposal)));
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
            &LIMIT_KEYS,
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

pub(crate) fn canonical_identifier(value: &str) -> bool {
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

#[cfg(test)]
pub(crate) mod tests;
