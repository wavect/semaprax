//! Exact generated-Proposal to frozen Agent Runtime v1 final-action compatibility.
//!
//! This module wraps one already-decoded canonical Proposal document as the
//! message of the existing Runtime v1 `final` action. Translation is pure: it
//! grants no host, tool, capability, authorization, or publication authority,
//! and it changes no Runtime v1 action, trace, or evidence schema.

use crate::agent_definition::CompiledAgentDefinition;
use crate::agent_runtime::proposal_compatibility_profile;
use crate::diagnostic::{quote_json, Diagnostic};

use super::CompiledAgentProposalSchema;

const ACTION_SCHEMA: &str = "semaprax.agent-runtime-action.v1";

/// The selected frozen Runtime v1 action kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRuntimeV1ActionKind {
    /// Finish the run with the canonical Proposal document as its message.
    Final,
}

/// Opaque exact bytes of one canonical frozen Runtime v1 final action.
///
/// This value is data only. It carries no host, tool, capability,
/// authorization, or publication authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRuntimeV1ActionBytes {
    source: String,
}

impl AgentRuntimeV1ActionBytes {
    /// Returns the byte-exact canonical Runtime v1 action, including its LF.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.source
    }

    /// Returns the only action kind admitted by this compatibility adapter.
    #[must_use]
    pub fn kind(&self) -> AgentRuntimeV1ActionKind {
        AgentRuntimeV1ActionKind::Final
    }
}

/// One exact, definition-bound generated-Proposal to Runtime v1 adapter.
///
/// Construction authenticates the Proposal schema and Runtime profile pairing.
/// Later decoding remains data-only and renders the already frozen Runtime v1
/// final action rather than introducing a second runtime protocol.
pub struct AgentProposalRuntimeV1Compatibility<'a> {
    proposal: &'a CompiledAgentProposalSchema,
    agent_id: String,
    definition_digest: String,
    proposal_schema_digest: String,
    runtime_profile_digest: String,
    max_provider_response_bytes: u64,
}

impl AgentProposalRuntimeV1Compatibility<'_> {
    /// Returns the exact AgentDefinition digest authenticated at construction.
    #[must_use]
    pub fn definition_digest(&self) -> &str {
        &self.definition_digest
    }

    /// Returns the exact Proposal schema digest authenticated at construction.
    #[must_use]
    pub fn proposal_schema_digest(&self) -> &str {
        &self.proposal_schema_digest
    }

    /// Returns the exact Runtime v1 profile digest authenticated at construction.
    #[must_use]
    pub fn runtime_profile_digest(&self) -> &str {
        &self.runtime_profile_digest
    }

    /// Decodes one untrusted Proposal and wraps its exact canonical bytes in a
    /// frozen Runtime v1 final action.
    ///
    /// Decoding and rendering perform no host or tool call and mint no
    /// capability or authorization value. Proposal diagnostics remain the
    /// existing `SPX-G550`/`SPX-G551` diagnostics.
    pub fn decode_and_render(
        &self,
        proposal_source: &str,
    ) -> Result<AgentRuntimeV1ActionBytes, Vec<Diagnostic>> {
        let proposal = self.proposal.decode(proposal_source)?;
        if proposal.agent_id() != self.agent_id
            || proposal.proposal_schema_digest() != self.proposal_schema_digest
        {
            return Err(vec![incompatible("proposal_binding")]);
        }
        let source = format!(
            "{{\"schema\":{},\"kind\":\"final\",\"message\":{}}}\n",
            quote_json(ACTION_SCHEMA),
            quote_json(proposal.canonical_json())
        );
        if source.len() as u64 > self.max_provider_response_bytes {
            return Err(vec![incompatible("provider_response_bytes")]);
        }
        Ok(AgentRuntimeV1ActionBytes { source })
    }
}

/// Compiles the one admitted generated-Proposal to Runtime v1 final mapping.
///
/// The Proposal may use any record or variant shape already admitted by the
/// Proposal compiler. Any definition, agent, Proposal-role type, or Runtime
/// profile cross-pair fails closed before untrusted Proposal decoding.
pub fn compile_agent_proposal_runtime_v1_compatibility<'a>(
    proposal: &'a CompiledAgentProposalSchema,
    definition: &CompiledAgentDefinition,
) -> Result<AgentProposalRuntimeV1Compatibility<'a>, Vec<Diagnostic>> {
    if proposal.definition_digest() != definition.definition().digest() {
        return Err(vec![incompatible("definition_digest")]);
    }
    if proposal.schema().agent_id() != definition.definition().agent_id() {
        return Err(vec![incompatible("agent_id")]);
    }
    if proposal.schema().proposal_type_id() != definition.definition().proposal_type_id() {
        return Err(vec![incompatible("proposal_type_id")]);
    }
    let profile = proposal_compatibility_profile(definition.runtime_v1_profile())
        .map_err(|_| vec![incompatible("runtime_profile")])?;
    if profile.agent_id() != proposal.schema().agent_id() {
        return Err(vec![incompatible("runtime_profile.agent_id")]);
    }
    Ok(AgentProposalRuntimeV1Compatibility {
        proposal,
        agent_id: profile.agent_id().to_owned(),
        definition_digest: proposal.definition_digest().to_owned(),
        proposal_schema_digest: proposal.schema().digest().to_owned(),
        runtime_profile_digest: profile.profile_digest().to_owned(),
        max_provider_response_bytes: profile.max_provider_response_bytes(),
    })
}

fn incompatible(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G578",
        format!("AgentProposal Runtime v1 compatibility invariant failed: {field}"),
    )
}
