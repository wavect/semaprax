//! Agent Proposal Schema v1: the model's proposal grammar, derived from the
//! program's own verified record and variant declarations.
//!
//! The compiler resolves an AgentDefinition's Proposal role to one actual
//! stable-ID record or variant declaration in a checked module, derives a
//! closed schema from the same retained HIR ordinary execution uses, and
//! exposes a typed decoder for untrusted model output.
//!
//! The schema carries semantic identities and exact scalar representations
//! only. Display names never enter it, so a display rename preserves the
//! derived revision while an actual type change invalidates every stale
//! schema and proposal bound to it.
//!
//! A proposal is data. Decoding validates untrusted bytes and returns a
//! [`DecodedProposal`]; it constructs no `Authorized<T>`, no publication
//! token, and no capability, and it performs no provider, tool, filesystem,
//! process, network, or approval effect.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::agent_definition::compile_agent_definition;
use crate::diagnostic::{quote_json, Diagnostic};

mod clients;
pub(crate) mod decode;
pub(crate) mod shape;

pub use clients::{
    verify_agent_proposal_client_bundle, AgentProposalClientBundle,
    AGENT_PROPOSAL_CLIENT_BUNDLE_SCHEMA, MAX_AGENT_PROPOSAL_CLIENT_BUNDLE_BYTES,
    MAX_AGENT_PROPOSAL_CLIENT_MANIFEST_BYTES, MAX_AGENT_PROPOSAL_CLIENT_SOURCE_BYTES,
};
pub use decode::{DecodedField, DecodedProposal, ProposalValue};

use shape::Shape;

/// Schema identity of the derived proposal grammar.
pub const SCHEMA_V1: &str = "semaprax.agent-proposal-schema.v1";
/// Schema identity of one decoded proposal document.
pub const PROPOSAL_SCHEMA: &str = "semaprax.agent-proposal.v1";

const SCHEMA_DOMAIN: &[u8] = b"semaprax.agent-proposal-schema.digest.v1\0";
const TYPE_REVISION_DOMAIN: &[u8] = b"semaprax.agent-proposal-type.revision.v1\0";

const MAX_SCHEMA_BYTES: usize = 262_144;
const MAX_PROPOSAL_BYTES: usize = 65_536;
const MAX_STRING_FIELD_BYTES: usize = 4_096;

const NONCLAIMS: [&str; 7] = [
    "no_authorization_value_or_publication_token_from_a_proposal",
    "no_capability_grant_effect_or_host_authority",
    "no_trust_in_model_output_without_decoder_validation",
    "no_nested_generic_borrowed_or_resource_bearing_proposal_values",
    "no_floating_point_or_lossy_numeric_transport",
    "no_agent_definition_graph_or_runtime_v1_byte_modification",
    "no_generated_consumer_clients_in_this_slice",
];

/// One derived canonical Agent Proposal Schema v1 document.
pub struct AgentProposalSchema {
    agent_id: String,
    proposal_type_id: String,
    proposal_type_revision: String,
    source: String,
    digest: String,
}

impl AgentProposalSchema {
    /// Returns the canonical document, including its terminal LF.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.source
    }

    /// Returns the domain-separated schema digest one proposal binds to.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Returns the agent identity this grammar belongs to.
    #[must_use]
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Returns the resolved Proposal-role stable type identity.
    #[must_use]
    pub fn proposal_type_id(&self) -> &str {
        &self.proposal_type_id
    }

    /// Returns the display-name-independent revision of the proposal type.
    #[must_use]
    pub fn proposal_type_revision(&self) -> &str {
        &self.proposal_type_revision
    }
}

/// The complete output of the Agent Proposal Schema v1 compiler.
pub struct CompiledAgentProposalSchema {
    schema: AgentProposalSchema,
    definition_digest: String,
    source_revision: String,
    shape: Shape,
}

impl CompiledAgentProposalSchema {
    /// Returns the derived schema.
    #[must_use]
    pub fn schema(&self) -> &AgentProposalSchema {
        &self.schema
    }

    /// Returns the digest of the AgentDefinition the Proposal role came from.
    #[must_use]
    pub fn definition_digest(&self) -> &str {
        &self.definition_digest
    }

    /// Returns the graph revision of the module the type was resolved in.
    ///
    /// The revision is a fact about the compiled module, not a binding of the
    /// grammar: an unrelated edit changes it without invalidating a proposal.
    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    /// Decodes one untrusted proposal document against this exact grammar.
    ///
    /// The returned value is data. It carries no authorization, no token, and
    /// no capability, and decoding performs no effect.
    pub fn decode(&self, proposal_source: &str) -> Result<DecodedProposal, Vec<Diagnostic>> {
        decode::decode(
            &self.schema.agent_id,
            &self.schema.digest,
            &self.shape,
            proposal_source,
        )
        .map_err(|diagnostic| vec![diagnostic])
    }
}

/// Derives the closed proposal grammar of one AgentDefinition from one checked
/// module.
///
/// The module is compiled through the ordinary checked pipeline; its
/// diagnostics are returned unchanged. Compilation is pure and grants no
/// provider, tool, filesystem, process, network, or publication authority.
pub fn compile_agent_proposal_schema(
    module_source: &str,
    module_path: impl AsRef<Path>,
    definition_source: &str,
) -> Result<CompiledAgentProposalSchema, Vec<Diagnostic>> {
    let compiled = compile_agent_definition(definition_source)?;
    let program = crate::check(module_source, module_path)?;
    let source_revision = crate::graph::revision(&program);
    let resolved = crate::hir::resolve(&program)?;
    let proposal_type_id = compiled.definition().proposal_type_id().to_owned();
    let shape =
        shape::derive(&resolved, &proposal_type_id).map_err(|diagnostic| vec![diagnostic])?;
    let rendered_shape = shape::render(&shape);
    let proposal_type_revision = digest(
        TYPE_REVISION_DOMAIN,
        format!(
            "{{\"proposal_type_id\":{},\"shape\":{rendered_shape}}}",
            quote_json(&proposal_type_id)
        )
        .as_bytes(),
    );
    let agent_id = compiled.definition().agent_id().to_owned();
    let source = render_schema(
        &agent_id,
        &proposal_type_id,
        &proposal_type_revision,
        &rendered_shape,
    );
    if source.len() > MAX_SCHEMA_BYTES {
        return Err(vec![invariant("schema_bytes")]);
    }
    Ok(CompiledAgentProposalSchema {
        schema: AgentProposalSchema {
            agent_id,
            proposal_type_id,
            proposal_type_revision,
            digest: digest(SCHEMA_DOMAIN, source.as_bytes()),
            source,
        },
        definition_digest: compiled.definition().digest().to_owned(),
        source_revision,
        shape,
    })
}

/// Independently rederives a proposal grammar and requires the supplied
/// document to equal it byte for byte.
pub fn verify_agent_proposal_schema_bundle(
    module_source: &str,
    module_path: impl AsRef<Path>,
    definition_source: &str,
    schema_source: &str,
) -> Result<(), Vec<Diagnostic>> {
    if schema_source.len() > MAX_SCHEMA_BYTES {
        return Err(vec![schema_mismatch()]);
    }
    let compiled = compile_agent_proposal_schema(module_source, module_path, definition_source)?;
    if compiled.schema().canonical_json().as_bytes() != schema_source.as_bytes() {
        return Err(vec![schema_mismatch()]);
    }
    Ok(())
}

fn render_schema(
    agent_id: &str,
    proposal_type_id: &str,
    proposal_type_revision: &str,
    rendered_shape: &str,
) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},\"proposal_type_id\":{},\"proposal_type_revision\":{},\"shape\":{rendered_shape},\"wire\":{{\"proposal_schema\":{},\"closed_objects\":true,\"key_order\":\"declaration_order\",\"exact_integer_encoding\":\"decimal_string\",\"max_proposal_bytes\":{MAX_PROPOSAL_BYTES}}},\"nonclaims\":[",
        quote_json(SCHEMA_V1),
        quote_json(agent_id),
        quote_json(proposal_type_id),
        quote_json(proposal_type_revision),
        quote_json(PROPOSAL_SCHEMA)
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
        "SPX-G548",
        format!("AgentProposalSchema invariant failed: {field}"),
    )
}

fn schema_mismatch() -> Diagnostic {
    Diagnostic::io(
        "SPX-G549",
        "AgentProposalSchema is not the exact replay of its verified source and AgentDefinition",
    )
}

fn malformed() -> Diagnostic {
    Diagnostic::io(
        "SPX-G550",
        format!("AgentProposal is not canonical {PROPOSAL_SCHEMA} JSON"),
    )
}

fn proposal_invariant(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G551",
        format!("AgentProposal invariant failed: {field}"),
    )
}
