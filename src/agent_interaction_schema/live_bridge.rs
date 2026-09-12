//! Binds the compiler-derived Agent Interaction Schema v1 grammar
//! ([`CompiledInteractionSchema`], issue #109) to the provider-independent
//! `model.invoke` effect's decode seam
//! ([`crate::live_invocation::model_invoke::ProposalDecoder`], the shared
//! #108/#177 contract in `docs/LIVE-INVOCATION-CONTRACT-V1.md`).
//!
//! That contract ships only `live_invocation::fixture::FixtureProposalDecoder`,
//! a toy grammar, and documents the real one as future wiring: "No real
//! compiled proposal grammar... Binding the real one is #109's scope." #109
//! has since landed the real grammar and decoder
//! (`agent_interaction_schema::compile_agent_interaction_schema`,
//! `CompiledInteractionSchema::decode`); this module is that binding. It
//! adds no new admission rule of its own: every acceptance or refusal is
//! `CompiledInteractionSchema::decode`'s existing, independently tested
//! behavior, only reshaped into the `ProposalDecoder` seam's two-case
//! return type.
//!
//! This module does not edit `src/live_invocation/` (leased elsewhere) or
//! `src/agent_runtime_v2/`; it only depends on their already-public trait
//! and fixture surface, exactly as the contract document invites downstream
//! issues to do.

use crate::diagnostic::Diagnostic;
use crate::live_invocation::model_invoke::{ProposalDecoder, ProposalOutcome};

use super::CompiledInteractionSchema;

/// A [`ProposalDecoder`] backed by one real, compiler-derived
/// [`CompiledInteractionSchema`] rather than a fixture/toy grammar.
///
/// `schema_digest` reports exactly `schema().digest()`: a kernel comparing
/// this against a request's `proposal_grammar_digest` rejects schema drift
/// (a decoder bound to a stale or unrelated compiled type) before any
/// dispatch, the same guarantee the contract document requires of every
/// `ProposalDecoder`. `decode` performs no admission logic of its own; it
/// forwards to `CompiledInteractionSchema::decode` and reports its result
/// through the closed `ProposalOutcome` shape, carrying the diagnostic code
/// and message as the refusal reason so a caller sees exactly which
/// admission rule failed, not a generic "malformed" tag.
pub struct SourceInteractionProposalDecoder {
    schema: CompiledInteractionSchema,
}

impl SourceInteractionProposalDecoder {
    /// Wraps one already-compiled interaction schema.
    #[must_use]
    pub fn new(schema: CompiledInteractionSchema) -> Self {
        Self { schema }
    }

    /// Returns the wrapped compiled schema.
    #[must_use]
    pub fn schema(&self) -> &CompiledInteractionSchema {
        &self.schema
    }
}

/// Renders one diagnostic as a stable, specific refusal reason: its code and
/// message, never just "an error occurred".
fn refusal_reason(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .first()
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .unwrap_or_else(|| "agent_interaction_schema: decode produced no diagnostic".to_owned())
}

impl ProposalDecoder for SourceInteractionProposalDecoder {
    fn schema_digest(&self) -> &str {
        self.schema.schema().digest()
    }

    fn decode(&mut self, _turn: u32, response: &[u8]) -> ProposalOutcome {
        match self.schema.decode(response) {
            Ok(value) => ProposalOutcome::Admitted(value.canonical_json().as_bytes().to_vec()),
            Err(diagnostics) => ProposalOutcome::Refused(refusal_reason(&diagnostics)),
        }
    }
}

#[cfg(test)]
mod tests;
