//! AgentDefinition v2 and AgentDeployment v1: source-owned agent semantics
//! separated from explicit deployment and model bindings.
//!
//! Source owns role, type and operation identities, tool contracts and their
//! effect and capability requirements, model capability requirements, and
//! budget ceilings. A deployment selects concrete providers and models, the
//! granted subset of the source's capabilities and tools, the target features
//! it makes available, and effective limits at or below the source ceilings.
//!
//! Binding compiles one immutable product that authenticates both revisions.
//! It rejects capability expansion, tool expansion, model incompatibility, an
//! unavailable required target feature, and budget widening before anything
//! contacts a provider — binding never contacts one at all. The bound product
//! holds no credential string and performs no environment, filesystem, or
//! network lookup: both document schemas are closed, so a credential key is
//! rejected as noncanonical rather than carried.
//!
//! AgentDefinition v1 is preserved as an exact compatibility projection. A v1
//! document migrates into a v2 definition plus one deployment, and binding
//! that pair reproduces the original v1 bytes, graph, and Runtime v1 profile.

use sha2::{Digest, Sha256};

use crate::agent_definition::{
    compile_agent_definition, render_plain_object, AgentGraph, CompiledAgentDefinition, LIMIT_KEYS,
};
use crate::agent_runtime::{Agent, AgentCancellation, AgentHost};
use crate::diagnostic::{quote_json, Diagnostic};

mod documents;
mod migrate;

use documents::{
    parse_definition_v2, parse_deployment, render_list, tool_capabilities, tool_ids,
    unsigned_limits, DefinitionV2, Deployment, QUALITY_TIERS,
};

pub use migrate::migrate_agent_definition_v1;

/// Schema identity of the source-owned semantic definition.
pub const DEFINITION_V2_SCHEMA: &str = "semaprax.agent-definition.v2";
/// Schema identity of the deployment binding.
pub const DEPLOYMENT_SCHEMA: &str = "semaprax.agent-deployment.v1";
/// Schema identity of the compiled bound product.
pub const BOUND_SCHEMA: &str = "semaprax.agent-bound-deployment.v1";

const DEFINITION_V2_DOMAIN: &[u8] = b"semaprax.agent-definition.digest.v2\0";
const DEPLOYMENT_DOMAIN: &[u8] = b"semaprax.agent-deployment.digest.v1\0";
const BOUND_DOMAIN: &[u8] = b"semaprax.agent-bound-deployment.digest.v1\0";

const MAX_DOCUMENT_BYTES: usize = 1_310_720;
const MAX_BOUND_BYTES: usize = 262_144;
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

const NONCLAIMS: [&str; 8] = [
    "no_credential_secret_key_or_token_material_in_a_bound_deployment",
    "no_implicit_environment_filesystem_network_or_home_lookup",
    "no_capability_effect_tool_or_budget_expansion_beyond_the_source_contract",
    "no_provider_contact_transport_or_billing_during_binding",
    "no_target_feature_implementation_or_backend_admission",
    "no_compiled_transition_execution_checkpoint_or_resume",
    "no_agent_runtime_v1_schema_api_or_kat_modification",
    "runtime_v2_does_not_consume_agent_graph_in_this_slice",
];

/// One admitted canonical AgentDefinition v2: the source-owned semantics.
pub struct AgentDefinitionV2 {
    parsed: DefinitionV2,
    source: String,
    digest: String,
}

impl AgentDefinitionV2 {
    /// Returns the stable semantic identity of the declared agent.
    #[must_use]
    pub fn agent_id(&self) -> &str {
        &self.parsed.agent_id
    }

    /// Returns the admitted document, including its terminal LF.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.source
    }

    /// Returns the domain-separated semantic-definition digest.
    ///
    /// Substituting an eligible provider or model does not change it; changing
    /// a type, an operation identity, a tool contract, an effect or capability
    /// requirement, or a ceiling does.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// One admitted canonical AgentDeployment v1.
pub struct AgentDeployment {
    parsed: Deployment,
    source: String,
    digest: String,
}

impl AgentDeployment {
    /// Returns the deployment's own stable identity.
    #[must_use]
    pub fn deployment_id(&self) -> &str {
        &self.parsed.deployment_id
    }

    /// Returns the semantic-definition digest this deployment claims to bind.
    #[must_use]
    pub fn definition_digest(&self) -> &str {
        &self.parsed.definition_digest
    }

    /// Returns the admitted document, including its terminal LF.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.source
    }

    /// Returns the domain-separated deployment digest.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// The immutable product of binding one deployment to one semantic definition.
pub struct BoundAgentDeployment {
    definition: AgentDefinitionV2,
    deployment: AgentDeployment,
    compiled_v1: CompiledAgentDefinition,
    source: String,
    digest: String,
}

impl BoundAgentDeployment {
    /// Returns the source-owned semantic definition.
    #[must_use]
    pub fn semantic_definition(&self) -> &AgentDefinitionV2 {
        &self.definition
    }

    /// Returns the bound deployment.
    #[must_use]
    pub fn deployment(&self) -> &AgentDeployment {
        &self.deployment
    }

    /// Returns the bound product document, including its terminal LF.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.source
    }

    /// Returns the domain-separated bound-product digest.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Returns the exact AgentDefinition v1 compatibility projection.
    #[must_use]
    pub fn runtime_v1_definition(&self) -> &str {
        self.compiled_v1.definition().canonical_source()
    }

    /// Returns the AgentGraph v1 of the compatibility projection.
    #[must_use]
    pub fn graph(&self) -> &AgentGraph {
        self.compiled_v1.graph()
    }

    /// Returns the exact Agent Runtime Profile v1 projection.
    #[must_use]
    pub fn runtime_v1_profile(&self) -> &str {
        self.compiled_v1.runtime_v1_profile()
    }

    /// Instantiates the bound product through its exact Runtime v1 projection.
    ///
    /// Every provider and tool effect still arrives through `host`. Binding
    /// granted no authority; this call grants none beyond Runtime v1's own.
    pub fn instantiate<H: AgentHost>(
        &self,
        host: H,
        cancellation: AgentCancellation,
    ) -> Result<Agent<H>, Vec<Diagnostic>> {
        self.compiled_v1.instantiate(host, cancellation)
    }
}

/// Admits one canonical AgentDefinition v2 document.
pub fn compile_agent_definition_v2(source: &str) -> Result<AgentDefinitionV2, Vec<Diagnostic>> {
    parse_definition_v2(source)
        .map(|parsed| AgentDefinitionV2 {
            parsed,
            digest: digest(DEFINITION_V2_DOMAIN, source.as_bytes()),
            source: source.to_owned(),
        })
        .map_err(|diagnostic| vec![diagnostic])
}

/// Admits one canonical AgentDeployment v1 document.
pub fn compile_agent_deployment(source: &str) -> Result<AgentDeployment, Vec<Diagnostic>> {
    parse_deployment(source)
        .map(|parsed| AgentDeployment {
            parsed,
            digest: digest(DEPLOYMENT_DOMAIN, source.as_bytes()),
            source: source.to_owned(),
        })
        .map_err(|diagnostic| vec![diagnostic])
}

/// Binds one deployment to one semantic definition.
///
/// Every incompatibility is decided here, from the two documents alone. No
/// provider, tool, filesystem, process, network, or environment access occurs.
pub fn bind_agent_deployment(
    definition_source: &str,
    deployment_source: &str,
) -> Result<BoundAgentDeployment, Vec<Diagnostic>> {
    let definition = compile_agent_definition_v2(definition_source)?;
    let deployment = compile_agent_deployment(deployment_source)?;
    check_compatibility(&definition, &deployment).map_err(|diagnostic| vec![diagnostic])?;
    let v1_source = project_v1(&definition, &deployment).map_err(|diagnostic| vec![diagnostic])?;
    let compiled_v1 = compile_agent_definition(&v1_source)?;
    let source = render_bound(&definition, &deployment, &compiled_v1);
    if source.len() > MAX_BOUND_BYTES {
        return Err(vec![incompatible("bound_bytes")]);
    }
    Ok(BoundAgentDeployment {
        definition,
        deployment,
        compiled_v1,
        digest: digest(BOUND_DOMAIN, source.as_bytes()),
        source,
    })
}

/// Independently rebinds a pair and requires the supplied bound product to
/// equal the result byte for byte.
pub fn verify_bound_agent_deployment_bundle(
    definition_source: &str,
    deployment_source: &str,
    bound_source: &str,
) -> Result<(), Vec<Diagnostic>> {
    if bound_source.len() > MAX_BOUND_BYTES {
        return Err(vec![bound_mismatch()]);
    }
    let bound = bind_agent_deployment(definition_source, deployment_source)?;
    if bound.canonical_json().as_bytes() != bound_source.as_bytes() {
        return Err(vec![bound_mismatch()]);
    }
    Ok(())
}

fn check_compatibility(
    definition: &AgentDefinitionV2,
    deployment: &AgentDeployment,
) -> Result<(), Diagnostic> {
    let source = &definition.parsed;
    let bound = &deployment.parsed;
    if bound.definition_digest != definition.digest {
        return Err(incompatible("definition_digest"));
    }
    if bound
        .granted_capabilities
        .iter()
        .any(|capability| !source.required_capabilities.contains(capability))
    {
        return Err(incompatible("granted_capabilities"));
    }
    if bound
        .allowed_tool_ids
        .iter()
        .any(|tool_id| !source.allowed_tool_ids.contains(tool_id))
    {
        return Err(incompatible("allowed_tool_ids"));
    }
    let declared = tool_ids(&source.tools)?;
    for (tool_id, capabilities) in declared.iter().zip(tool_capabilities(&source.tools)?) {
        if bound.allowed_tool_ids.contains(tool_id)
            && capabilities
                .iter()
                .any(|capability| !bound.granted_capabilities.contains(capability))
        {
            return Err(incompatible("tool_capabilities"));
        }
    }
    if source
        .required_target_features
        .iter()
        .any(|feature| !bound.target_features.contains(feature))
    {
        return Err(incompatible("target_features"));
    }
    let ceilings = unsigned_limits(&source.ceilings).ok_or_else(|| incompatible("ceilings"))?;
    let limits = unsigned_limits(&bound.limits).ok_or_else(|| incompatible("limits"))?;
    if limits
        .iter()
        .zip(&ceilings)
        .any(|(effective, ceiling)| effective > ceiling)
    {
        return Err(incompatible("limits"));
    }
    check_models(source, bound)
}

fn check_models(source: &DefinitionV2, bound: &Deployment) -> Result<(), Diagnostic> {
    let minimum = QUALITY_TIERS
        .iter()
        .position(|tier| *tier == source.minimum_quality_tier)
        .ok_or_else(|| incompatible("minimum_quality_tier"))?;
    for row in bound
        .models
        .as_array()
        .ok_or_else(|| incompatible("models"))?
    {
        let text = |key: &str| row.get(key).and_then(serde_json::Value::as_str);
        let provider_id = text("provider_id").ok_or_else(|| incompatible("models"))?;
        let model_id = text("model_id").ok_or_else(|| incompatible("models"))?;
        if !bound
            .allowed_provider_ids
            .iter()
            .any(|allowed| allowed == provider_id)
            || !bound
                .allowed_model_ids
                .iter()
                .any(|allowed| allowed == model_id)
        {
            return Err(incompatible("selection"));
        }
        if source.required_locality == "local_only"
            && text("locality").ok_or_else(|| incompatible("models"))? != "local"
        {
            return Err(incompatible("required_locality"));
        }
        let tier = text("quality_tier").ok_or_else(|| incompatible("models"))?;
        let rank = QUALITY_TIERS
            .iter()
            .position(|candidate| *candidate == tier)
            .ok_or_else(|| incompatible("models"))?;
        if rank < minimum {
            return Err(incompatible("minimum_quality_tier"));
        }
        let capabilities = row
            .get("capabilities")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| incompatible("models"))?;
        if source.required_model_capabilities.iter().any(|required| {
            !capabilities
                .iter()
                .any(|value| value.as_str() == Some(required.as_str()))
        }) {
            return Err(incompatible("required_model_capabilities"));
        }
    }
    Ok(())
}

/// Renders the exact AgentDefinition v1 compatibility projection.
fn project_v1(
    definition: &AgentDefinitionV2,
    deployment: &AgentDeployment,
) -> Result<String, Diagnostic> {
    let source = &definition.parsed;
    let bound = &deployment.parsed;
    let mut policy = serde_json::Map::new();
    policy.insert(
        "allowed_provider_ids".to_owned(),
        list_value(&bound.allowed_provider_ids),
    );
    policy.insert(
        "allowed_model_ids".to_owned(),
        list_value(&bound.allowed_model_ids),
    );
    policy.insert(
        "required_locality".to_owned(),
        serde_json::Value::String(source.required_locality.clone()),
    );
    policy.insert(
        "minimum_quality_tier".to_owned(),
        serde_json::Value::String(source.minimum_quality_tier.clone()),
    );
    policy.insert(
        "required_model_capabilities".to_owned(),
        list_value(&source.required_model_capabilities),
    );
    policy.insert(
        "granted_capabilities".to_owned(),
        list_value(&bound.granted_capabilities),
    );
    policy.insert(
        "allowed_tool_ids".to_owned(),
        list_value(&bound.allowed_tool_ids),
    );
    let mut runtime = serde_json::Map::new();
    runtime.insert("models".to_owned(), bound.models.clone());
    runtime.insert("tools".to_owned(), source.tools.clone());
    runtime.insert("policy".to_owned(), serde_json::Value::Object(policy));
    runtime.insert("limits".to_owned(), bound.limits.clone());
    crate::agent_definition::render_v1_definition_source(
        &source.agent_id,
        &source.types,
        &source.operations,
        &serde_json::Value::Object(runtime),
    )
}

fn render_bound(
    definition: &AgentDefinitionV2,
    deployment: &AgentDeployment,
    compiled_v1: &CompiledAgentDefinition,
) -> String {
    let source = &definition.parsed;
    let bound = &deployment.parsed;
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},\"definition_digest\":{},\"deployment_id\":{},\"deployment_digest\":{},\"effective\":{{\"allowed_provider_ids\":{},\"allowed_model_ids\":{},\"required_locality\":{},\"minimum_quality_tier\":{},\"required_model_capabilities\":{},\"granted_capabilities\":{},\"allowed_tool_ids\":{},\"target_features\":{},\"limits\":{}}},\"v1_definition_digest\":{},\"agent_graph_digest\":{},\"runtime_v1_profile_digest\":{},\"nonclaims\":[",
        quote_json(BOUND_SCHEMA),
        quote_json(&source.agent_id),
        quote_json(&definition.digest),
        quote_json(&bound.deployment_id),
        quote_json(&deployment.digest),
        render_list(&bound.allowed_provider_ids),
        render_list(&bound.allowed_model_ids),
        quote_json(&source.required_locality),
        quote_json(&source.minimum_quality_tier),
        render_list(&source.required_model_capabilities),
        render_list(&bound.granted_capabilities),
        render_list(&bound.allowed_tool_ids),
        render_list(&bound.target_features),
        render_plain_object(&bound.limits, &LIMIT_KEYS)
            .expect("admitted deployment limits remain canonical"),
        quote_json(compiled_v1.definition().digest()),
        quote_json(compiled_v1.graph().digest()),
        quote_json(&digest(
            b"semaprax.agent-runtime.profile-digest.v1\0",
            compiled_v1.runtime_v1_profile().as_bytes(),
        )),
    );
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(nonclaim));
    }
    output.push_str("]}\n");
    output
}

fn list_value(values: &[String]) -> serde_json::Value {
    serde_json::Value::Array(
        values
            .iter()
            .map(|value| serde_json::Value::String(value.clone()))
            .collect(),
    )
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

pub(crate) fn definition_malformed() -> Diagnostic {
    Diagnostic::io(
        "SPX-G552",
        format!("AgentDefinition v2 is not canonical {DEFINITION_V2_SCHEMA} JSON"),
    )
}

pub(crate) fn definition_invariant(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G553",
        format!("AgentDefinition v2 invariant failed: {field}"),
    )
}

pub(crate) fn deployment_malformed() -> Diagnostic {
    Diagnostic::io(
        "SPX-G554",
        format!("AgentDeployment is not canonical {DEPLOYMENT_SCHEMA} JSON"),
    )
}

pub(crate) fn deployment_invariant(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G555",
        format!("AgentDeployment invariant failed: {field}"),
    )
}

fn incompatible(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G556",
        format!("AgentDeployment is not compatible with its definition: {field}"),
    )
}

fn bound_mismatch() -> Diagnostic {
    Diagnostic::io(
        "SPX-G557",
        "BoundAgentDeployment is not the exact replay of its definition and deployment",
    )
}
