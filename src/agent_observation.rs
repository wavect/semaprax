//! Agent Observation Schema v1, derived from a source-owned Agent and the
//! verified HIR type bound to its Observation role.
//!
//! This bounded first slice resolves the role in the same checked module as
//! the Agent declaration. It shares the Proposal compiler's closed
//! monomorphic scalar record/variant shape and decoder machinery. Observation
//! documents are untrusted data and carry no authority.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::agent_proposal::decode::{decode_document, DocumentRules};
use crate::agent_proposal::shape::{self, Shape};
use crate::agent_proposal::{DecodedField, ProposalValue};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::ResolvedAgentTypeRoleKind;
use crate::project::compile_source_agent_declaration;

pub const SCHEMA_V1: &str = "semaprax.agent-observation-schema.v1";
pub const OBSERVATION_SCHEMA: &str = "semaprax.agent-observation.v1";

const SCHEMA_DOMAIN: &[u8] = b"semaprax.agent-observation-schema.digest.v1\0";
const TYPE_REVISION_DOMAIN: &[u8] = b"semaprax.agent-observation-type.revision.v1\0";
const MAX_SCHEMA_BYTES: usize = 262_144;
const MAX_OBSERVATION_BYTES: usize = 65_536;

const NONCLAIMS: [&str; 7] = [
    "no_authorization_value_or_publication_token_from_an_observation",
    "no_capability_grant_effect_or_host_authority",
    "no_trust_in_observation_bytes_without_decoder_validation",
    "no_nested_generic_borrowed_or_resource_bearing_observation_values",
    "no_floating_point_or_lossy_numeric_transport",
    "no_agent_definition_graph_or_runtime_v1_byte_modification",
    "no_cross_module_role_resolution_in_this_slice",
];

pub type ObservationValue = ProposalValue;

pub struct AgentObservationSchema {
    agent_id: String,
    observation_type_id: String,
    observation_type_revision: String,
    source: String,
    digest: String,
}

impl AgentObservationSchema {
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    #[must_use]
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    #[must_use]
    pub fn observation_type_id(&self) -> &str {
        &self.observation_type_id
    }

    #[must_use]
    pub fn observation_type_revision(&self) -> &str {
        &self.observation_type_revision
    }
}

pub struct CompiledAgentObservationSchema {
    schema: AgentObservationSchema,
    definition_digest: String,
    source_revision: String,
    shape: Shape,
}

impl CompiledAgentObservationSchema {
    #[must_use]
    pub fn schema(&self) -> &AgentObservationSchema {
        &self.schema
    }

    #[must_use]
    pub fn definition_digest(&self) -> &str {
        &self.definition_digest
    }

    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    /// Decodes one untrusted observation against this exact schema.
    pub fn decode(&self, source: &str) -> Result<DecodedObservation, Vec<Diagnostic>> {
        decode_document(
            &self.schema.agent_id,
            &self.schema.digest,
            &self.shape,
            source,
            DocumentRules {
                document_schema: OBSERVATION_SCHEMA,
                digest_key: "observation_schema_digest",
                max_document_bytes: MAX_OBSERVATION_BYTES,
                malformed,
                invariant: observation_invariant,
                bytes_field: "observation_bytes",
            },
        )
        .map(DecodedObservation)
        .map_err(|diagnostic| vec![diagnostic])
    }
}

/// One decoded Observation document. It is data and grants no authority.
pub struct DecodedObservation(crate::agent_proposal::DecodedProposal);

impl DecodedObservation {
    #[must_use]
    pub fn agent_id(&self) -> &str {
        self.0.agent_id()
    }

    #[must_use]
    pub fn observation_schema_digest(&self) -> &str {
        self.0.proposal_schema_digest()
    }

    #[must_use]
    pub fn case(&self) -> Option<&str> {
        self.0.case()
    }

    #[must_use]
    pub fn fields(&self) -> &[DecodedField] {
        self.0.fields()
    }

    #[must_use]
    pub fn field(&self, stable_id: &str) -> Option<&ObservationValue> {
        self.0.field(stable_id)
    }

    #[must_use]
    pub fn canonical_json(&self) -> &str {
        self.0.canonical_json()
    }
}

/// Derives one source Agent's Observation schema from the checked same-module
/// HIR type bound to its Observation role.
pub fn compile_source_agent_observation_schema(
    module_source: &str,
    module_path: impl AsRef<Path>,
    agent_id: &str,
) -> Result<CompiledAgentObservationSchema, Vec<Diagnostic>> {
    let path = module_path.as_ref();
    let program = crate::check(module_source, path)?;
    let declaration = select_agent(&program.agents, agent_id)?;
    let compiled_definition = compile_source_agent_declaration(declaration)?;
    let source_revision = crate::graph::revision(&program);
    let resolved = crate::hir::resolve(&program)?;
    let resolved_agent = resolved
        .agents
        .iter()
        .find(|agent| agent.stable_id.as_str() == agent_id)
        .ok_or_else(|| vec![invariant("agent.unresolved")])?;
    let observation_type_id = resolved_agent
        .types
        .iter()
        .find(|role| role.role == ResolvedAgentTypeRoleKind::Observation)
        .map(|role| role.stable_id.as_str().to_owned())
        .ok_or_else(|| vec![invariant("observation_role.missing")])?;
    if compiled_definition.definition().observation_type_id() != observation_type_id {
        return Err(vec![invariant("observation_role.definition_mismatch")]);
    }
    let shape = shape::derive_role(
        &resolved,
        &observation_type_id,
        "observation_type",
        invariant,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let rendered_shape = shape::render(&shape);
    let observation_type_revision = digest(
        TYPE_REVISION_DOMAIN,
        format!(
            "{{\"observation_type_id\":{},\"shape\":{rendered_shape}}}",
            quote_json(&observation_type_id)
        )
        .as_bytes(),
    );
    let source = render_schema(
        agent_id,
        &observation_type_id,
        &observation_type_revision,
        &rendered_shape,
    );
    if source.len() > MAX_SCHEMA_BYTES {
        return Err(vec![invariant("schema_bytes")]);
    }
    Ok(CompiledAgentObservationSchema {
        schema: AgentObservationSchema {
            agent_id: agent_id.to_owned(),
            observation_type_id,
            observation_type_revision,
            digest: digest(SCHEMA_DOMAIN, source.as_bytes()),
            source,
        },
        definition_digest: compiled_definition.definition().digest().to_owned(),
        source_revision,
        shape,
    })
}

/// Rederives an Observation schema and requires exact canonical bytes.
pub fn verify_source_agent_observation_schema_bundle(
    module_source: &str,
    module_path: impl AsRef<Path>,
    agent_id: &str,
    schema_source: &str,
) -> Result<(), Vec<Diagnostic>> {
    if schema_source.len() > MAX_SCHEMA_BYTES {
        return Err(vec![schema_mismatch()]);
    }
    let compiled = compile_source_agent_observation_schema(module_source, module_path, agent_id)?;
    if compiled.schema().canonical_json().as_bytes() != schema_source.as_bytes() {
        return Err(vec![schema_mismatch()]);
    }
    Ok(())
}

fn select_agent<'a>(
    agents: &'a [crate::ast::AgentDeclaration],
    agent_id: &str,
) -> Result<&'a crate::ast::AgentDeclaration, Vec<Diagnostic>> {
    let mut matches = agents.iter().filter(|agent| agent.stable_id == agent_id);
    let Some(agent) = matches.next() else {
        return Err(vec![invariant("agent.unresolved")]);
    };
    if matches.next().is_some() {
        return Err(vec![invariant("agent.duplicate")]);
    }
    Ok(agent)
}

fn render_schema(
    agent_id: &str,
    observation_type_id: &str,
    observation_type_revision: &str,
    rendered_shape: &str,
) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},\"observation_type_id\":{},\"observation_type_revision\":{},\"shape\":{rendered_shape},\"wire\":{{\"observation_schema\":{},\"closed_objects\":true,\"key_order\":\"declaration_order\",\"exact_integer_encoding\":\"decimal_string\",\"max_observation_bytes\":{MAX_OBSERVATION_BYTES}}},\"nonclaims\":[",
        quote_json(SCHEMA_V1),
        quote_json(agent_id),
        quote_json(observation_type_id),
        quote_json(observation_type_revision),
        quote_json(OBSERVATION_SCHEMA),
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

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn invariant(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G566",
        format!("AgentObservationSchema invariant failed: {field}"),
    )
}

fn schema_mismatch() -> Diagnostic {
    Diagnostic::io(
        "SPX-G567",
        "AgentObservationSchema is not the exact replay of its verified source Agent and HIR type",
    )
}

fn malformed() -> Diagnostic {
    Diagnostic::io(
        "SPX-G568",
        format!("AgentObservation is not canonical {OBSERVATION_SCHEMA} JSON"),
    )
}

fn observation_invariant(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G569",
        format!("AgentObservation invariant failed: {field}"),
    )
}
