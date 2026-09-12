//! Canonical, deterministic JSON rendering for
//! `semaprax.smt-proof-certificate.v1`.
//!
//! Mirrors [`super::super::render`]'s conventions (an outer
//! `{schema,digest,bytes,payload}` envelope, one domain-separated SHA-256
//! per digest kind, `bformat!`/`budgeted_join` for bounded output) but keeps
//! its own domain-separation strings and key layout: this is a distinct
//! wire schema, not a variant of the Assurance Manifest envelope, and must
//! not become byte-coupled to that module's internal rendering choices
//! while it is under separate, concurrent development.

use sha2::{Digest as _, Sha256};

use crate::bounded_output::BudgetedJoin as _;
use crate::diagnostic::quote_json;

use super::super::smt_discharge::{Model, ModelValue, ReplayOutcome, BOUNDS_V1};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.smt-proof-certificate.v1";

const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.smt-proof-certificate.source.v1\0";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.smt-proof-certificate.payload.v1\0";
const SCRIPT_DIGEST_DOMAIN: &[u8] = b"semaprax.smt-proof-certificate.script.v1\0";
const ARTIFACT_DIGEST_DOMAIN: &[u8] = b"semaprax.smt-proof-certificate.artifact.v1\0";

/// The one compiled-artifact target this certificate binds to. Deliberately
/// singular ("Artifact binding for one target", issue #186): the Wasm core
/// module is a real, structurally validated compiled binary produced
/// entirely in-process (no external toolchain, no ambient authority), unlike
/// the native backend's C11 emission which is source text still requiring an
/// external, unpinned C toolchain this crate does not invoke.
pub(super) const ARTIFACT_TARGET_WASM_CORE_MODULE_V1: &str = "wasm-core-module-v1";

/// Every field a reader needs to be told, up front, that this certificate
/// does *not* claim — mirrors the Assurance Manifest's own `nonclaims`
/// convention (`super::super::render::NONCLAIMS_JSON`) but scoped to what a
/// standalone proof certificate can and cannot mean.
const NONCLAIMS_JSON: &str = "\"no_assurance_manifest_merge\",\
\"artifact_binding_covers_only_the_wasm_core_module_target_no_native_artifact_bound\",\
\"artifact_binding_does_not_by_itself_prove_the_backend_lowering_preserves_the_source_theorem\",\
\"no_project_test_discovery_or_execution\",\
\"no_target_execution\",\
\"no_native_or_wasm_runtime_execution\",\
\"not_human_approval_or_policy\",\
\"not_signature_or_publication_authority\",\
\"not_safe_compatible_or_target_conformant\",\
\"proved_verdict_requires_a_separate_solver_rerun_of_the_embedded_script_for_full_independence\",\
\"read_only_no_source_changes\"";

pub(super) fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

pub(super) fn source_digest(source: &str) -> String {
    domain_digest(SOURCE_DIGEST_DOMAIN, source.as_bytes())
}

pub(super) fn payload_digest(payload_bytes: &[u8]) -> String {
    domain_digest(PAYLOAD_DIGEST_DOMAIN, payload_bytes)
}

pub(super) fn script_digest(script: &str) -> String {
    domain_digest(SCRIPT_DIGEST_DOMAIN, script.as_bytes())
}

/// Domain-separated digest of the exact compiled artifact bytes this
/// certificate binds to (currently always a Wasm core module). Not the raw
/// bytes themselves: like `source.sha256`, the artifact is bound by digest
/// and independently rebindable either by recompiling from the exact bound
/// source ([`super::verify::verify_certificate_against_source`]) or by
/// hashing an actual artifact a caller already has in hand
/// ([`super::verify::verify_certificate_against_artifact`]).
pub(super) fn artifact_digest(bytes: &[u8]) -> String {
    domain_digest(ARTIFACT_DIGEST_DOMAIN, bytes)
}

/// The certificate's core claim: a genuine `unsat` proof, or a `sat` model
/// that independently replayed as a validated concrete counterexample.
/// There is deliberately no "inconclusive" variant here — see the module
/// doc's "Scope and honest limitations": an inconclusive attempt has
/// nothing to certify, so [`super::export_postcondition_certificate`] never
/// constructs one.
pub(super) enum CertificateBody {
    Proved,
    Refuted {
        model: Model,
        outcome: ReplayOutcome,
    },
}

/// Every input [`render`] needs, kept as one struct so a caller cannot
/// accidentally pass fields positionally out of order.
pub(super) struct RenderInput<'a> {
    pub source_path_text: &'a str,
    pub revision: &'a str,
    pub source_sha256: &'a str,
    pub declaration_id: &'a str,
    pub obligation_id: &'a str,
    pub ensures_index: usize,
    /// `env!("CARGO_PKG_VERSION")` of the compiler that produced this
    /// certificate. Bound so [`super::verify::verify_certificate_against_source`]
    /// can flag toolchain version drift as staleness — one of the issue's
    /// named failure modes ("toolchain version drift can change accepted
    /// proofs") — rather than silently trusting a certificate produced by a
    /// different compiler build.
    pub compiler_version: &'a str,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
    pub solver_identity: &'a str,
    pub solver_version: &'a str,
    /// Domain-separated digest of the exact compiled Wasm core module bytes
    /// the current source/revision produces for the whole enclosing module
    /// (not merely `declaration_id`). Always
    /// [`ARTIFACT_TARGET_WASM_CORE_MODULE_V1`]: see that constant's doc for
    /// why only this one target is bound.
    pub artifact_sha256: &'a str,
    pub artifact_bytes: usize,
    pub script: &'a str,
    pub body: &'a CertificateBody,
}

fn opt_json(value: &Option<String>) -> String {
    match value {
        Some(text) => quote_json(text),
        None => "null".to_owned(),
    }
}

fn opt_usize_json(value: Option<usize>) -> String {
    match value {
        Some(v) => v.to_string(),
        None => "null".to_owned(),
    }
}

fn render_model_entry(name: &str, value: &ModelValue) -> String {
    let (sort, value_text) = match value {
        ModelValue::Int(v) => ("int", v.to_string()),
        ModelValue::Bool(v) => ("bool", v.to_string()),
    };
    bformat!(
        "{{\"name\":{},\"sort\":{},\"value\":{}}}",
        quote_json(name),
        quote_json(sort),
        quote_json(&value_text),
    )
}

fn render_counterexample(model: &Model, outcome: &ReplayOutcome) -> String {
    let entries = model
        .iter()
        .map(|(name, value)| render_model_entry(name, value))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let (kind, detail, ensures_index) = match outcome {
        ReplayOutcome::Trapped { detail } => ("trapped", Some(detail.clone()), None),
        ReplayOutcome::EnsuresViolated { ensures_index } => {
            ("ensures_violated", None, Some(*ensures_index))
        }
        ReplayOutcome::Inconsistent { .. } => {
            unreachable!("an Inconsistent replay outcome is never certified")
        }
    };
    bformat!(
        "{{\"kind\":{},\"detail\":{},\"ensures_index\":{},\"model\":[{}]}}",
        quote_json(kind),
        opt_json(&detail),
        opt_usize_json(ensures_index),
        entries,
    )
}

/// Render the canonical certificate envelope for one discharge attempt.
pub(super) fn render(input: &RenderInput<'_>) -> String {
    let source_json = bformat!(
        "{{\"path\":{},\"revision\":{},\"sha256\":{}}}",
        quote_json(input.source_path_text),
        quote_json(input.revision),
        quote_json(input.source_sha256),
    );
    let solver_json = bformat!(
        "{{\"identity\":{},\"version\":{}}}",
        quote_json(input.solver_identity),
        quote_json(input.solver_version),
    );
    let limits_json = bformat!(
        "{{\"timeout_ms\":{},\"max_output_bytes\":{}}}",
        input.timeout_ms,
        input.max_output_bytes,
    );
    let artifact_json = bformat!(
        "{{\"target\":{},\"bytes\":{},\"sha256\":{}}}",
        quote_json(ARTIFACT_TARGET_WASM_CORE_MODULE_V1),
        input.artifact_bytes,
        quote_json(input.artifact_sha256),
    );
    let (verdict_token, counterexample_json) = match input.body {
        CertificateBody::Proved => ("proved", "null".to_owned()),
        CertificateBody::Refuted { model, outcome } => {
            ("refuted", render_counterexample(model, outcome))
        }
    };

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{},\"declaration_id\":{},\"obligation_id\":{},\
\"ensures_index\":{},\"compiler_version\":{},\"bounds\":{},\"artifact\":{},\"solver\":{},\
\"limits\":{},\"script\":{},\"script_sha256\":{},\"verdict\":{},\"counterexample\":{},\
\"nonclaims\":[{}]}}",
        SCHEMA,
        source_json,
        quote_json(input.declaration_id),
        quote_json(input.obligation_id),
        input.ensures_index,
        quote_json(input.compiler_version),
        quote_json(BOUNDS_V1),
        artifact_json,
        solver_json,
        limits_json,
        quote_json(input.script),
        quote_json(&script_digest(input.script)),
        quote_json(verdict_token),
        counterexample_json,
        NONCLAIMS_JSON,
    );
    bformat!(
        "{{\"schema\":\"{}\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        SCHEMA,
        quote_json(&payload_digest(payload.as_bytes())),
        payload.len(),
        payload,
    )
}
