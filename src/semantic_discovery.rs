//! Semantic Discovery v1: a compact, revision-bound capability catalog plus a
//! deterministic, revision-bound context delta over the existing bounded
//! Agent Context v2 envelope.
//!
//! See [`docs/SEMANTIC-DISCOVERY-V1.md`](../docs/SEMANTIC-DISCOVERY-V1.md) for
//! the full specification. This module composes existing owners rather than
//! restating their facts:
//!
//! - [`generate_discovery_manifest`] renders one small, deterministic catalog
//!   of the read-only semantic operations this compiler exposes (schema
//!   identities, tool classes and surfaces), referencing
//!   [`crate::installed_guidance::installed_query_capabilities`] and
//!   [`crate::installed_diagnostics::installed_diagnostic_catalog`] by digest
//!   rather than re-deriving their content. It is bound to one selected
//!   source file's exact revision, following the same
//!   `patch::canonical_source_path` / `read_source_snapshot` /
//!   `validate_source_unchanged` fail-closed pattern used by
//!   `capability_manifest`, `region_report`, `assurance_manifest` and
//!   `agent_interaction_schema`.
//! - [`compute_context_delta`] takes one client-held, previously acknowledged
//!   `semaprax.agent-context.v2` document plus its claimed revision, and one
//!   freshly authenticated recomputation of that exact same query at the
//!   current source revision (via [`crate::graph::agent_context_v2_json`]),
//!   and returns either an `unchanged` marker, a compact `delta` of only the
//!   added/changed/removed declaration facts keyed by their persistent
//!   `@id`, or an explicit `resync_required` outcome carrying a bounded fresh
//!   snapshot. It never emits a partial diff over data it cannot trust to be
//!   complete.
//!
//! This module performs no host effect, grants no authority, and does not
//! itself invalidate or execute any operation; capability/discovery output is
//! descriptive data a caller must re-validate against the live source before
//! trusting it, via [`verify_discovery_manifest_against_source`].

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::bounded_output::with_limit;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{graph, installed_diagnostics, installed_guidance, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

/// Schema identity of the compact capability discovery envelope.
pub const DISCOVERY_SCHEMA: &str = "semaprax.semantic-discovery.v1";
/// Schema identity of one revision-bound context delta document.
pub const CONTEXT_DELTA_SCHEMA: &str = "semaprax.semantic-discovery.context-delta.v1";

const DEFAULT_DISCOVERY_MAX_BYTES: usize = 8 * 1024;
const DISCOVERY_PAYLOAD_DOMAIN: &[u8] = b"semaprax.semantic-discovery.payload.digest.v1\0";

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z301", message)
}

fn budget_error(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z302",
        format!("semantic discovery output exceeds its bounded budget: {field}"),
    )
}

fn target_not_found(symbol: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z303",
        format!("no context root resolves symbol `{symbol}`"),
    )
}

fn determinism_violation(message: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z304",
        format!("context delta invariant failed: {message}"),
    )
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z305", message)
}

/// Validated bounds for one [`generate_discovery_manifest`] call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiscoveryOptions {
    max_bytes: usize,
}

impl DiscoveryOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "semantic discovery max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self { max_bytes })
    }

    #[must_use]
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_DISCOVERY_MAX_BYTES,
        }
    }
}

/// One catalog entry: a read-only semantic operation this compiler exposes.
///
/// Entries are listed in ascending byte order of `name`, which every test and
/// the renderer both rely on; `SEMANTIC_DISCOVERY_OPERATIONS` is checked for
/// that order in `tests::catalog_is_sorted_by_name`.
struct OperationEntry {
    name: &'static str,
    surface: &'static str,
    tool_class: &'static str,
    payload_schemas: &'static [&'static str],
}

/// The closed, static operation catalog. Two entries (`installed_diagnostic_
/// catalog`, `installed_query_capabilities`) are annotated at render time
/// with their live digest rather than restated here, so this table never
/// drifts from the artifacts it references.
const SEMANTIC_DISCOVERY_OPERATIONS: &[OperationEntry] = &[
    OperationEntry {
        name: "agent_context",
        surface: "cli",
        tool_class: "read_only_query",
        payload_schemas: &["semaprax.agent-context.v1", "semaprax.agent-context.v2"],
    },
    OperationEntry {
        name: "agent_interaction_schema",
        surface: "library",
        tool_class: "read_only_schema",
        payload_schemas: &[crate::agent_interaction_schema::SCHEMA_V1],
    },
    OperationEntry {
        name: "assurance_manifest",
        surface: "library",
        tool_class: "read_only_report",
        payload_schemas: &[crate::assurance_manifest::SCHEMA],
    },
    OperationEntry {
        name: "capability_manifest",
        surface: "cli",
        tool_class: "read_only_report",
        payload_schemas: &[crate::capability_manifest::SCHEMA],
    },
    OperationEntry {
        name: "context_delta",
        surface: "library",
        tool_class: "read_only_delta",
        payload_schemas: &[CONTEXT_DELTA_SCHEMA],
    },
    OperationEntry {
        name: "installed_diagnostic_catalog",
        surface: "cli",
        tool_class: "read_only_help",
        payload_schemas: &[installed_diagnostics::INSTALLED_DIAGNOSTIC_CATALOG_SCHEMA],
    },
    OperationEntry {
        name: "installed_query_capabilities",
        surface: "cli",
        tool_class: "read_only_help",
        payload_schemas: &[installed_guidance::INSTALLED_QUERY_CAPABILITIES_SCHEMA],
    },
    OperationEntry {
        name: "installed_skill",
        surface: "cli",
        tool_class: "read_only_help",
        payload_schemas: &[installed_guidance::INSTALLED_SKILL_SCHEMA],
    },
    OperationEntry {
        name: "region_report",
        surface: "cli",
        tool_class: "read_only_report",
        payload_schemas: &[crate::region_report::SCHEMA],
    },
];

const KNOWN_LIMITATIONS: &[&str] = &[
    "capability_information_is_a_static_catalog_not_a_live_authority_grant",
    "not_a_workspace_scoped_operation_inventory_see_installed_query_capabilities_for_project_scope",
    "operation_listing_summarizes_by_schema_and_digest_and_does_not_restate_referenced_payload_content",
    "an_operation_listed_here_must_still_be_reconfirmed_against_the_current_revision_before_use",
    "context_delta_requires_the_exact_prior_v2_context_document_not_a_bare_revision_string",
];

/// Generate the canonical `semaprax.semantic-discovery.v1` envelope for one
/// verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and the
/// final check or generation fails closed, exactly like
/// `capability_manifest::generate` and `agent_interaction_schema::compile_
/// agent_interaction_schema`.
pub fn generate_discovery_manifest(
    source_path: &Path,
    options: &DiscoveryOptions,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

    let query_capabilities = installed_guidance::installed_query_capabilities()?;
    let diagnostic_catalog = installed_diagnostics::installed_diagnostic_catalog()?;

    let path_text = source_path.display().to_string();
    let (envelope, overflowed) = with_limit(options.max_bytes, || {
        render_discovery(
            &path_text,
            &revision,
            query_capabilities.schema(),
            query_capabilities.digest(),
            diagnostic_catalog.digest(),
            diagnostic_catalog.code_count(),
        )
    });
    if overflowed {
        return Err(vec![budget_error("discovery_manifest_bytes")]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(envelope)
}

/// Independently rebind one `generate_discovery_manifest` envelope to the
/// current bytes of `source_path`, failing closed on drift.
///
/// A cached discovery manifest carries the revision it was generated at. A
/// caller that intends to act on that manifest (for example, to decide that
/// an operation is available) MUST call this first: source drift after
/// generation means the manifest describes a target that no longer exists,
/// and stale capability information must never authorize executing an
/// operation. This is the check that makes "a newly unavailable operation
/// cannot be executed using stale capability information" true for this
/// module's own output.
pub fn verify_discovery_manifest_against_source(
    envelope: &str,
    source_path: &Path,
) -> Result<(), Vec<Diagnostic>> {
    let value: Value = serde_json::from_str(envelope).map_err(|error| {
        vec![consistency_error(format!(
            "envelope is not valid JSON: {error}"
        ))]
    })?;
    let object = value.as_object().ok_or_else(|| {
        vec![consistency_error(
            "envelope must be a JSON object".to_owned(),
        )]
    })?;
    if object.get("schema").and_then(Value::as_str) != Some(DISCOVERY_SCHEMA) {
        return Err(vec![consistency_error(format!(
            "envelope schema must be {DISCOVERY_SCHEMA}"
        ))]);
    }
    let declared_bytes = object.get("bytes").and_then(Value::as_u64).ok_or_else(|| {
        vec![consistency_error(
            "envelope bytes must be an unsigned integer".to_owned(),
        )]
    })?;
    // The digest and byte count bind the EXACT payload bytes as they appear
    // in `envelope`, not a round-tripped re-serialization: `serde_json::Value`
    // re-orders object keys, which would silently change what is hashed.
    // Extract the raw payload substring the same way
    // `capability_manifest::verify_envelope` does.
    const PAYLOAD_KEY: &str = "\"payload\":";
    let Some(offset) = envelope.find(PAYLOAD_KEY) else {
        return Err(vec![consistency_error(
            "envelope is missing its payload member".to_owned(),
        )]);
    };
    if !envelope.ends_with('}') {
        return Err(vec![consistency_error(
            "envelope must end with `}`".to_owned(),
        )]);
    }
    let payload_text = &envelope[offset + PAYLOAD_KEY.len()..envelope.len() - 1];
    if !payload_text.starts_with('{') || !payload_text.ends_with('}') {
        return Err(vec![consistency_error(
            "envelope payload must be a JSON object".to_owned(),
        )]);
    }
    if declared_bytes != payload_text.len() as u64 {
        return Err(vec![consistency_error(format!(
            "envelope declares {declared_bytes} payload bytes but {} are present",
            payload_text.len()
        ))]);
    }
    let declared_digest = object
        .get("digest")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            vec![consistency_error(
                "envelope digest must be a string".to_owned(),
            )]
        })?;
    if declared_digest != domain_digest(DISCOVERY_PAYLOAD_DOMAIN, payload_text.as_bytes()) {
        return Err(vec![consistency_error(
            "envelope digest does not match its exact payload bytes".to_owned(),
        )]);
    }
    let payload: Value = serde_json::from_str(payload_text).map_err(|error| {
        vec![consistency_error(format!(
            "payload is not valid JSON: {error}"
        ))]
    })?;
    let bound_revision = payload["selected_target"]["revision"]
        .as_str()
        .ok_or_else(|| {
            vec![consistency_error(
                "payload selected_target.revision must be a string".to_owned(),
            )]
        })?;
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let current_revision = graph::revision(&program);
    if bound_revision != current_revision {
        return Err(vec![consistency_error(
            "semantic discovery manifest source revision does not match the current source; \
             the source drifted after the manifest was generated"
                .to_owned(),
        )]);
    }
    Ok(())
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn render_discovery(
    path_text: &str,
    revision: &str,
    query_capabilities_schema: &str,
    query_capabilities_digest: &str,
    diagnostic_catalog_digest: &str,
    diagnostic_catalog_code_count: usize,
) -> String {
    let mut operations = Vec::with_capacity(SEMANTIC_DISCOVERY_OPERATIONS.len());
    for entry in SEMANTIC_DISCOVERY_OPERATIONS {
        let schemas = entry
            .payload_schemas
            .iter()
            .map(|schema| quote_json(schema))
            .collect::<Vec<_>>()
            .join(",");
        let extra = match entry.name {
            "installed_query_capabilities" => {
                bformat!(",\"digest\":{}", quote_json(query_capabilities_digest))
            }
            "installed_diagnostic_catalog" => bformat!(
                ",\"digest\":{},\"code_count\":{}",
                quote_json(diagnostic_catalog_digest),
                diagnostic_catalog_code_count
            ),
            _ => String::new(),
        };
        debug_assert!(
            entry.name != "installed_query_capabilities"
                || (entry.payload_schemas.len() == 1
                    && entry.payload_schemas[0] == query_capabilities_schema),
            "installed_query_capabilities catalog entry must match its live schema constant"
        );
        operations.push(bformat!(
            "{{\"name\":{},\"surface\":{},\"tool_class\":{},\"payload_schemas\":[{}]{}}}",
            quote_json(entry.name),
            quote_json(entry.surface),
            quote_json(entry.tool_class),
            schemas,
            extra,
        ));
    }
    let tool_classes = [
        "read_only_delta",
        "read_only_help",
        "read_only_query",
        "read_only_report",
        "read_only_schema",
    ]
    .iter()
    .map(|class| quote_json(class))
    .collect::<Vec<_>>()
    .join(",");
    let limitations = KNOWN_LIMITATIONS
        .iter()
        .map(|item| quote_json(item))
        .collect::<Vec<_>>()
        .join(",");
    let payload = bformat!(
        "{{\"schema\":\"{}\",\"selected_target\":{{\"path\":{},\"revision\":{}}},\
\"tool_classes\":[{}],\"operations\":[{}],\"known_limitations\":[{}]}}",
        DISCOVERY_SCHEMA,
        quote_json(path_text),
        quote_json(revision),
        tool_classes,
        operations.join(","),
        limitations,
    );
    bformat!(
        "{{\"schema\":\"{}\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        DISCOVERY_SCHEMA,
        quote_json(&domain_digest(DISCOVERY_PAYLOAD_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload,
    )
}

/// One parsed, structurally validated `semaprax.agent-context.v2` document.
/// Used for both the trusted freshly-generated `current` document and the
/// untrusted client-supplied `base` document; the untrusted path never
/// panics on malformed input.
struct ParsedContext {
    schema: String,
    revision: String,
    module: String,
    root: String,
    query: Value,
    truncation: Value,
    truncated: bool,
    facts: BTreeMap<String, Value>,
}

fn parse_context_document(text: &str) -> Option<ParsedContext> {
    let value: Value = serde_json::from_str(text).ok()?;
    let object = value.as_object()?;
    let schema = object.get("schema")?.as_str()?.to_owned();
    let revision = object.get("revision")?.as_str()?.to_owned();
    let module = object.get("module")?.as_str()?.to_owned();
    let root = object.get("root")?.as_str()?.to_owned();
    let query = object.get("query")?.clone();
    if !query.is_object() {
        return None;
    }
    let truncation = object.get("truncation")?.clone();
    let truncated = truncation.as_object()?.get("truncated")?.as_bool()?;
    let facts_value = object.get("facts")?.as_array()?;
    let mut facts = BTreeMap::new();
    for fact in facts_value {
        let id = fact.as_object()?.get("id")?.as_str()?.to_owned();
        if facts.insert(id, fact.clone()).is_some() {
            return None;
        }
    }
    Some(ParsedContext {
        schema,
        revision,
        module,
        root,
        query,
        truncation,
        truncated,
        facts,
    })
}

/// One request to [`compute_context_delta`]: the exact query to recompute,
/// plus the client's claimed prior binding.
pub struct ContextDeltaRequest<'a> {
    pub symbol: &'a str,
    pub options: &'a graph::AgentContextV2Options,
    pub claimed_base_revision: &'a str,
    pub base_document: &'a str,
}

/// Recompute one `semaprax.agent-context.v2` query against the current
/// source and return a revision-bound delta against the client-supplied
/// prior document.
///
/// Read-only: source bytes must remain unchanged between the snapshot and
/// the final check or computation fails closed.
///
/// The returned document always has one of three `outcome` values:
/// - `"unchanged"`: `claimed_base_revision` equals the current revision.
/// - `"delta"`: the base was well-formed, exactly the same query and target,
///   not itself truncated, and at a different revision; `diff.added`,
///   `diff.changed` list full current fact JSON keyed by persistent `id`,
///   `diff.removed` lists only the removed ids.
/// - `"resync_required"`: the base could not be safely diffed (malformed,
///   wrong schema, wrong target, a different query shape, an inconsistent
///   claimed revision, the base itself truncated, or the honest delta would
///   exceed the query's own byte budget). The response embeds one bounded
///   fresh `current_document` instead of an incomplete diff.
pub fn compute_context_delta(
    source_path: &Path,
    request: &ContextDeltaRequest<'_>,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let revision = graph::revision(&program);

    let current_document = graph::agent_context_v2_json(&program, request.symbol, request.options)?
        .ok_or_else(|| vec![target_not_found(request.symbol)])?;

    let outcome = build_delta(
        &current_document,
        request.claimed_base_revision,
        request.base_document,
        request.options.max_bytes(),
    )?;

    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(outcome)
}

fn render_resync(current_document: &str, reason: &str) -> Result<String, Vec<Diagnostic>> {
    let ceiling = current_document
        .len()
        .saturating_add(4096)
        .min(graph::MAX_AGENT_CONTEXT_BYTES);
    let (rendered, overflowed) = with_limit(ceiling, || {
        bformat!(
            "{{\"schema\":\"{}\",\"outcome\":\"resync_required\",\"reason\":{},\"current_document\":{}}}",
            CONTEXT_DELTA_SCHEMA,
            quote_json(reason),
            current_document,
        )
    });
    if overflowed {
        return Err(vec![budget_error("resync_current_document")]);
    }
    Ok(rendered)
}

fn build_delta(
    current_document: &str,
    claimed_base_revision: &str,
    base_document: &str,
    delta_max_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    let current = parse_context_document(current_document)
        .expect("internally generated context document must be well-formed");

    let Some(base) = parse_context_document(base_document) else {
        return render_resync(current_document, "malformed_base");
    };
    if base.schema != current.schema {
        return render_resync(current_document, "schema_mismatch");
    }
    if base.revision != claimed_base_revision {
        return render_resync(current_document, "base_revision_mismatch");
    }
    if base.module != current.module || base.root != current.root {
        return render_resync(current_document, "target_mismatch");
    }
    if base.query != current.query {
        return render_resync(current_document, "query_mismatch");
    }
    if base.truncated {
        return render_resync(current_document, "base_truncated");
    }

    if base.revision == current.revision {
        if base.facts != current.facts {
            return Err(vec![determinism_violation(
                "two documents share one revision but disagree on their facts",
            )]);
        }
        let (rendered, overflowed) = with_limit(delta_max_bytes, || {
            bformat!(
                "{{\"schema\":\"{}\",\"outcome\":\"unchanged\",\"revision\":{},\
\"target\":{{\"module\":{},\"root\":{}}}}}",
                CONTEXT_DELTA_SCHEMA,
                quote_json(&current.revision),
                quote_json(&current.module),
                quote_json(&current.root),
            )
        });
        if overflowed {
            return Err(vec![budget_error("unchanged")]);
        }
        return Ok(rendered);
    }

    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut unchanged_count = 0_usize;
    for (id, fact) in &current.facts {
        match base.facts.get(id) {
            None => added.push(fact.to_string()),
            Some(base_fact) => {
                if base_fact == fact {
                    unchanged_count += 1;
                } else {
                    changed.push(fact.to_string());
                }
            }
        }
    }
    let removed = base
        .facts
        .keys()
        .filter(|id| !current.facts.contains_key(*id))
        .map(|id| quote_json(id))
        .collect::<Vec<_>>();

    let (rendered, overflowed) = with_limit(delta_max_bytes, || {
        bformat!(
            "{{\"schema\":\"{}\",\"outcome\":\"delta\",\"target\":{{\"module\":{},\"root\":{}}},\
\"query\":{},\"base_revision\":{},\"current_revision\":{},\
\"diff\":{{\"added\":[{}],\"changed\":[{}],\"removed\":[{}],\"unchanged_count\":{}}},\
\"truncation\":{}}}",
            CONTEXT_DELTA_SCHEMA,
            quote_json(&current.module),
            quote_json(&current.root),
            current.query,
            quote_json(&base.revision),
            quote_json(&current.revision),
            added.join(","),
            changed.join(","),
            removed.join(","),
            unchanged_count,
            current.truncation,
        )
    });
    if overflowed {
        return render_resync(current_document, "delta_exceeds_max_bytes");
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests;
