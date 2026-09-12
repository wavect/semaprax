//! Agent Interaction Schema v1: a bounded, nonrecursive rich interaction
//! schema derived from one checked source record or variant type, plus its
//! strict bounded decoder and provider presentation projections.
//!
//! See [`docs/AGENT-INTERACTION-SCHEMA-V1.md`](../docs/AGENT-INTERACTION-SCHEMA-V1.md)
//! for the full specification this module implements.
//!
//! This is an independent derivation, not an edit of the existing flat
//! `agent_proposal`/`agent_observation` scalar schemas: it neither reads nor
//! changes their internals, and their existing scalar behavior is
//! unaffected by this module's existence. Where the two profiles overlap
//! (direct `bool`/`i32`/`i64`/`u8`/`usize`/`string` fields) this module uses
//! byte-identical wire conventions (bare `bool`, decimal-string integers,
//! plain JSON strings), so a genuinely flat type produces a compatible
//! value under either profile.
//!
//! **Scope.** One checked module resolves one named root record or variant
//! declaration. Its fields may be a direct scalar, bounded UTF-8 text,
//! bounded `Bytes`, or a reference to exactly one further monomorphic
//! record/variant declaration reachable the same way, up to a bounded
//! type-graph size and nesting depth. The type graph must be acyclic:
//! arbitrary recursion, generic instantiation, resources, classes, raw
//! views, callbacks and every other unlisted source type are refused with
//! an explicit diagnostic naming exactly which admission rule failed, never
//! silently serialized as opaque JSON.
//!
//! **Whole-value, not streaming.** [`CompiledInteractionSchema::decode`]
//! takes one complete `&[u8]` response and returns one decoded value or one
//! refusal; there is no partial-decode state. A streaming transport can
//! buffer provider output into one complete response before calling this
//! same boundary — this module needs nothing from a streaming extension to
//! exist, matching the existing
//! [Live Invocation Contract v1](../docs/LIVE-INVOCATION-CONTRACT-V1.md)
//! independence between its rich-schema and streaming-extension consumers.
//!
//! A derived schema and a decoded value are both data. Deriving a schema
//! and decoding a value construct no `Authorized<T>`, no publication token,
//! and no capability, and perform no provider, tool, filesystem, process,
//! network, or approval effect.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::patch;

pub(crate) mod decode;
pub mod live_bridge;
mod provider;
pub(crate) mod shape;

mod render;

pub use decode::{DecodedField, DecodedInteractionValue, FieldValue, ScalarValue, TypedValue};

use shape::TypeGraph;

/// Schema identity of the derived canonical interaction schema document.
pub const SCHEMA_V1: &str = "semaprax.agent-interaction-schema.v1";
/// Schema identity of one decoded interaction document.
pub const DOCUMENT_SCHEMA: &str = "semaprax.agent-interaction-value.v1";

const SCHEMA_DOMAIN: &[u8] = b"semaprax.agent-interaction-schema.digest.v1\0";
const REVISION_DOMAIN: &[u8] = b"semaprax.agent-interaction-schema.type-revision.v1\0";

/// The maximum canonical schema document size, in bytes.
pub(crate) const MAX_SCHEMA_BYTES: usize = 262_144;
/// The maximum canonical decoded-value document size, in bytes.
pub(crate) const MAX_DOCUMENT_BYTES: usize = 65_536;

pub(crate) use shape::{MAX_BYTES_FIELD_BYTES, MAX_DEPTH, MAX_STRING_FIELD_BYTES, MAX_TYPES};

/// One derived canonical Agent Interaction Schema v1 document.
#[derive(Debug)]
pub struct AgentInteractionSchema {
    root_type_id: String,
    root_type_revision: String,
    source: String,
    digest: String,
}

impl AgentInteractionSchema {
    /// Returns the canonical document, including its terminal LF.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.source
    }

    /// Returns the domain-separated schema digest one decoded value binds to.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Returns the resolved root stable type identity.
    #[must_use]
    pub fn root_type_id(&self) -> &str {
        &self.root_type_id
    }

    /// Returns the display-name-independent revision of the root type and
    /// its complete transitive type graph.
    #[must_use]
    pub fn root_type_revision(&self) -> &str {
        &self.root_type_revision
    }
}

/// The complete output of the Agent Interaction Schema v1 compiler.
#[derive(Debug)]
pub struct CompiledInteractionSchema {
    schema: AgentInteractionSchema,
    source_revision: String,
    graph: TypeGraph,
}

impl CompiledInteractionSchema {
    /// Returns the derived schema.
    #[must_use]
    pub fn schema(&self) -> &AgentInteractionSchema {
        &self.schema
    }

    /// Returns the graph revision of the module the root type was resolved
    /// in. This is a fact about the compiled module, not a binding of the
    /// schema: an unrelated source edit changes it without invalidating a
    /// previously decoded value.
    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    /// Renders the provider-compatible JSON Schema draft 2020-12 projection
    /// of this exact canonical schema. See [`provider`] for what this
    /// projection is, and is not, permitted to influence.
    #[must_use]
    pub fn provider_json_schema(&self) -> String {
        provider::render_json_schema(&self.graph)
    }

    /// Decodes one untrusted interaction document against this exact
    /// schema.
    ///
    /// `source` is untrusted response bytes, not yet known to be valid
    /// UTF-8. The returned value is data: it carries no authorization, no
    /// token, and no capability, and decoding performs no effect.
    pub fn decode(&self, source: &[u8]) -> Result<DecodedInteractionValue, Vec<Diagnostic>> {
        decode::decode(&self.graph, &self.schema.digest, source)
            .map_err(|diagnostic| vec![diagnostic])
    }
}

/// Derives the closed, bounded Agent Interaction Schema v1 rooted at
/// `root_type_id` from one checked module at `source_path`.
///
/// Read-only: source bytes must remain unchanged between the snapshot and
/// the final check or derivation fails closed, exactly like
/// `capability_manifest::generate`, `region_report::generate` and
/// `assurance_manifest::generate`.
pub fn compile_agent_interaction_schema(
    source_path: &Path,
    root_type_id: &str,
) -> Result<CompiledInteractionSchema, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = crate::check(snapshot.source(), source_path)?;
    let source_revision = crate::graph::revision(&program);
    let resolved = crate::hir::resolve(&program)?;
    crate::hir::validate(&resolved).map_err(|error| vec![error])?;

    let graph = shape::derive(&resolved, root_type_id).map_err(|diagnostic| vec![diagnostic])?;
    let rendered_types = render::render_types(&graph.types);
    let root_type_revision = digest(
        REVISION_DOMAIN,
        render::render_revision_body(&graph.root_type_id, &rendered_types).as_bytes(),
    );
    let source = render::render_schema(&graph, &root_type_revision, &rendered_types);
    if source.len() > MAX_SCHEMA_BYTES {
        return Err(vec![budget_error("schema_bytes")]);
    }
    let schema = AgentInteractionSchema {
        root_type_id: graph.root_type_id.clone(),
        root_type_revision,
        digest: digest(SCHEMA_DOMAIN, source.as_bytes()),
        source,
    };

    patch::validate_source_unchanged(
        &canonical_source_path,
        source_path,
        &snapshot,
        &source_revision,
    )?;
    Ok(CompiledInteractionSchema {
        schema,
        source_revision,
        graph,
    })
}

/// Independently rederives an interaction schema and requires the supplied
/// document to equal it byte for byte.
pub fn verify_agent_interaction_schema_bundle(
    source_path: &Path,
    root_type_id: &str,
    schema_source: &str,
) -> Result<(), Vec<Diagnostic>> {
    if schema_source.len() > MAX_SCHEMA_BYTES {
        return Err(vec![schema_mismatch()]);
    }
    let compiled = compile_agent_interaction_schema(source_path, root_type_id)?;
    if compiled.schema().canonical_json().as_bytes() != schema_source.as_bytes() {
        return Err(vec![schema_mismatch()]);
    }
    Ok(())
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

/// A derivation-time structural admission failure: an unresolved or
/// non-persistent identity, a generic declaration or field-type argument,
/// an empty variant, a cyclic type reference, an over-budget type graph, or
/// any other unsupported source type. `field` names exactly which admission
/// rule failed.
fn invariant(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z202",
        format!("AgentInteractionSchema invariant failed: {field}"),
    )
}

/// The derived schema exceeded its output byte budget. Fails closed; never
/// truncated.
fn budget_error(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z203",
        format!("AgentInteractionSchema exceeded its bounded output budget: {field}"),
    )
}

/// A supplied schema document is not the exact independent replay of its
/// checked source.
fn schema_mismatch() -> Diagnostic {
    Diagnostic::io(
        "SPX-Z204",
        "AgentInteractionSchema is not the exact replay of its verified source",
    )
}

/// A decoded document, already known to be valid UTF-8 within its byte
/// budget, is not canonical `semaprax.agent-interaction-value.v1` JSON: not
/// valid JSON, a malformed top-level or nested envelope shape, or bytes
/// that fail the exact canonical-replay check (which also rejects every
/// duplicate-key document: canonical rendering emits each declared key
/// once, so any repeated key — declared or not — makes the source strictly
/// longer than its canonical replay).
fn malformed() -> Diagnostic {
    Diagnostic::io(
        "SPX-Z205",
        format!("AgentInteractionValue is not canonical {DOCUMENT_SCHEMA} JSON"),
    )
}

/// A decode-time admission rule failed: oversized or non-UTF-8 input bytes,
/// a schema/root-type binding mismatch, an unknown or missing field, a
/// wrong or unknown variant tag, a value that does not fit its declared
/// representation or bound, or nesting past the declared depth bound.
/// `field` names exactly which admission rule failed.
fn decode_invariant(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z206",
        format!("AgentInteractionValue invariant failed: {field}"),
    )
}

#[cfg(test)]
mod tests;
