//! Deterministic, read-only Plugin Manifest Projection v1.
//!
//! [`generate`] projects one verified single-file SEMAPRAX module into one
//! canonical compact JSON envelope (`semaprax.plugin-manifest.v1`)
//! describing a capability-limited plugin descriptor: the provided export
//! inventory (explicit-ID monomorphic effect-free functions with only
//! by-value direct `i64`/`bool` parameters and results, sorted bytewise by
//! stable identity, each carrying its interface types, rendered contract
//! clauses, persistent stable ID, and the exact Native64 prototype line
//! extracted verbatim from the production native C11 projection under its
//! own domain-separated digest), the required host capabilities derived by
//! the same closed five-domain derivation as Build Capability Manifest v1
//! (module permits plus declared function and import effects; every token
//! anywhere in the module must sit inside the closed vocabulary or the whole
//! command fails closed), explicit empty-by-default resource limits, and an
//! explicit closed inventory of unavailable sections. Every other function
//! is recorded as an exclusion with one closed reason mirroring the
//! Canonical ABI Report admission profile exactly.
//!
//! Plugin identity fields are sourced from module metadata conventions. The
//! language has no version metadata today, so the plugin `name` is the
//! module declaration name, while `identity` is the domain-separated
//! SHA-256 digest of the exact source bytes under
//! `semaprax.plugin-manifest.identity.v1` and `version` is the first 16
//! lowercase hex characters of that identity — a build-hash-style version
//! with no semver semantics and no versioning-negotiation machinery.
//!
//! [`verify_envelope`] independently replays one envelope: exact envelope
//! shape, declared byte count, domain-separated payload digest, descriptor
//! counts, closed exclusion reasons, strict stable-id ordering, every
//! embedded export signature digest, the closed capability vocabulary over
//! every listed token, equality of the embedded required-capabilities
//! section with its re-derivation from those tokens, the canonical
//! resource-limits section, the closed unavailable-sections list, and the
//! internal identity/version consistency.
//! [`verify_envelope_against_source`] additionally rebinds the current
//! source bytes to both embedded source digests and fails closed on drift.
//!
//! Diagnostics use the previously unused `SPX-N1xx` family:
//! - `SPX-N101`: invalid options (bounds, malformed values).
//! - `SPX-N102`: a capability token outside the admitted closed vocabulary.
//! - `SPX-N103`: output byte-budget exhaustion (fail-closed, no truncation).
//! - `SPX-N104`: envelope consistency or replay failure.
//!
//! This tranche performs no Component Model runtime or packaging, no host
//! loading or lifecycle management, no versioning negotiation, no
//! resource-limit enforcement, no hostile-plugin execution testing,
//! executes nothing, and changes no source.

use std::collections::BTreeSet;
use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::ast::{Function, ParamMode, Type};
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::capability_manifest;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{codegen, format, graph, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.plugin-manifest.v1";

const DEFAULT_MAX_BYTES: usize = 64 * 1024;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.plugin-manifest.source.v1\0";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.plugin-manifest.payload.v1\0";
const EXPORT_SIGNATURE_DIGEST_DOMAIN: &[u8] = b"semaprax.plugin-manifest.export-signature.v1\0";
const PLUGIN_IDENTITY_DIGEST_DOMAIN: &[u8] = b"semaprax.plugin-manifest.identity.v1\0";

/// Length in lowercase hex characters of the build-hash-style plugin version
/// derived from the plugin identity digest (`sha256:` prefix excluded).
pub const VERSION_HEX_CHARS: usize = 16;

const REASON_AUTOMATIC_IDENTITY: &str = "automatic_identity";
const REASON_GENERIC_FUNCTION: &str = "generic_function";
const REASON_DECLARED_EFFECTS: &str = "declared_effects";
const REASON_UNSUPPORTED_PARAMETER_MODE: &str = "unsupported_parameter_mode";
const REASON_UNSUPPORTED_PARAMETER_TYPE: &str = "unsupported_parameter_type";
const REASON_UNSUPPORTED_RESULT_TYPE: &str = "unsupported_result_type";
const EXCLUSION_REASONS: [&str; 6] = [
    REASON_AUTOMATIC_IDENTITY,
    REASON_GENERIC_FUNCTION,
    REASON_DECLARED_EFFECTS,
    REASON_UNSUPPORTED_PARAMETER_MODE,
    REASON_UNSUPPORTED_PARAMETER_TYPE,
    REASON_UNSUPPORTED_RESULT_TYPE,
];

/// The canonical resource-limits section: explicitly empty by default. No
/// limit is declared by the language today and none is enforced by this
/// projection; replay rejects any other shape.
const RESOURCE_LIMITS_JSON: &str = "{\"fuel\":null,\"memory_bytes\":null,\"table_elements\":null}";

/// Closed inventory of descriptor sections this projection does not provide,
/// in canonical bytewise order.
const UNAVAILABLE_SECTIONS: [&str; 5] = [
    "component_model_packaging",
    "host_lifecycle",
    "hostile_plugin_execution_tests",
    "resource_limit_enforcement",
    "versioning_negotiation",
];
const UNAVAILABLE_SECTIONS_JSON: &str = "[\"component_model_packaging\",\
\"host_lifecycle\",\
\"hostile_plugin_execution_tests\",\
\"resource_limit_enforcement\",\
\"versioning_negotiation\"]";

const NONCLAIMS_JSON: &str = "\"descriptor_projection_only\",\
\"no_component_model_runtime_or_packaging\",\
\"no_host_loading_or_lifecycle\",\
\"no_versioning_negotiation_machinery\",\
\"no_resource_limit_enforcement_or_declared_limits\",\
\"no_hostile_plugin_execution_tests\",\
\"no_target_execution\",\
\"read_only_no_source_changes\"";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluginManifestOptions {
    pub max_bytes: usize,
}

impl PluginManifestOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "plugin-manifest max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for PluginManifestOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-N101", message)
}

fn vocabulary_error(token: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-N102",
        format!(
            "capability `{token}` is outside the admitted bounded vocabulary {}; refusing to emit a plugin manifest",
            capability_manifest::AMBIENT_DOMAINS.join(", ")
        ),
    )
}

fn budget_error() -> Diagnostic {
    Diagnostic::io(
        "SPX-N103",
        "plugin-manifest output exceeds the max-bytes budget; refusing to truncate".to_owned(),
    )
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-N104", message)
}

struct ExportEntry {
    stable_id: String,
    name: String,
    parameters: Vec<&'static str>,
    result: &'static str,
    symbol: String,
    signature: String,
    requires: Vec<String>,
    ensures: Vec<String>,
}

struct ExcludedFunction {
    stable_id: String,
    name: String,
    reason: &'static str,
}

/// One independently authenticated export returned by [`verify_envelope`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedExport {
    pub stable_id: String,
    pub name: String,
    pub native_symbol: String,
    pub native_signature: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VerifiedPluginManifest {
    pub name: String,
    pub identity: String,
    pub version: String,
    pub exports: Vec<VerifiedExport>,
}

/// Derive the build-hash-style plugin version from a plugin identity digest:
/// the first [`VERSION_HEX_CHARS`] lowercase hex characters after the
/// `sha256:` prefix.
pub fn derive_version(identity: &str) -> String {
    let hex = identity.strip_prefix("sha256:").unwrap_or(identity);
    hex.chars().take(VERSION_HEX_CHARS).collect()
}

/// Generate the canonical `semaprax.plugin-manifest.v1` envelope JSON for
/// one verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and
/// the final check or generation fails closed.
pub fn generate(
    source_path: &Path,
    options: &PluginManifestOptions,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

    // Every capability token anywhere in the module must sit inside the
    // closed vocabulary; only the required inventories below drive the
    // required-capabilities section. The derivation mirrors Build
    // Capability Manifest v1 exactly and reuses its helpers.
    let mut tokens = BTreeSet::new();
    for permit in &program.permits {
        require_vocabulary(permit)?;
        tokens.insert(permit.clone());
    }
    for function in &program.functions {
        for effect in &function.effects {
            require_vocabulary(effect)?;
            tokens.insert(effect.clone());
        }
    }
    for interface in &program.interfaces {
        for permit in &interface.permits {
            require_vocabulary(permit)?;
        }
        for import in &interface.imports {
            for effect in &import.effects {
                require_vocabulary(effect)?;
                tokens.insert(effect.clone());
            }
        }
    }

    let mut sorted = program.functions.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
    let functions_total = sorted.len();

    let mut excluded: Vec<ExcludedFunction> = Vec::new();
    let mut admitted: Vec<&Function> = Vec::new();
    for function in sorted {
        match admission(function) {
            Some(reason) => excluded.push(ExcludedFunction {
                stable_id: function.stable_id.clone(),
                name: function.name.clone(),
                reason,
            }),
            None => admitted.push(function),
        }
    }

    let native_text = if admitted.is_empty() {
        None
    } else {
        Some(codegen::emit_c(&program).map_err(|error| vec![error])?)
    };

    let mut exports: Vec<ExportEntry> = Vec::with_capacity(admitted.len());
    for function in admitted {
        let symbol = c_function_symbol(&function.stable_id);
        let signature = match &native_text {
            Some(native_text) => {
                extract_native_signature(native_text, &symbol, &function.stable_id)?
            }
            None => String::new(),
        };
        exports.push(ExportEntry {
            stable_id: function.stable_id.clone(),
            name: function.name.clone(),
            parameters: parameter_types(function),
            result: result_type(function),
            symbol,
            signature,
            requires: function
                .requires
                .iter()
                .map(|clause| format::expr(clause, 0))
                .collect(),
            ensures: function
                .ensures
                .iter()
                .map(|clause| format::expr(clause, 0))
                .collect(),
        });
    }

    let identity = domain_digest(PLUGIN_IDENTITY_DIGEST_DOMAIN, snapshot.source().as_bytes());
    let version = derive_version(&identity);
    let digest = source_digest(snapshot.source());
    let path_text = source_path.display().to_string();
    let (envelope, overflowed) = with_limit(options.max_bytes, || {
        render(
            &path_text,
            &revision,
            &digest,
            &program.module,
            &identity,
            &version,
            options.max_bytes,
            &tokens,
            functions_total,
            &exports,
            &excluded,
        )
    });
    if overflowed {
        return Err(vec![budget_error()]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(envelope)
}

/// Independently verify one envelope produced by [`generate`].
///
/// Recomputes the outer payload digest over the exact serialized payload
/// bytes, re-checks the declared byte count, replays the descriptor counts
/// against the listed inventories, the closed exclusion vocabulary, strict
/// stable-id ordering, every embedded export-signature digest, the closed
/// capability vocabulary over every listed token, the re-derived
/// required-capabilities section, the canonical resource-limits section,
/// the closed unavailable-sections list, and the internal identity/version
/// consistency before returning the verified summaries.
pub fn verify_envelope(envelope: &str) -> Result<VerifiedPluginManifest, Diagnostic> {
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("envelope is not valid JSON: {error}")))?;
    let Some(object) = value.as_object() else {
        return Err(consistency_error(
            "envelope must be a JSON object".to_owned(),
        ));
    };
    let keys: Vec<&str> = object.keys().map(String::as_str).collect();
    if keys != ["bytes", "digest", "payload", "schema"] {
        return Err(consistency_error(format!(
            "envelope keys must be exactly [bytes, digest, payload, schema], found {keys:?}"
        )));
    }
    if object["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "envelope schema must be {SCHEMA}"
        )));
    }
    let Some(envelope_digest) = object["digest"].as_str() else {
        return Err(consistency_error(
            "envelope digest must be a string".to_owned(),
        ));
    };
    let Some(declared_bytes) = object["bytes"].as_u64() else {
        return Err(consistency_error(
            "envelope bytes must be an unsigned integer".to_owned(),
        ));
    };
    const PAYLOAD_KEY: &str = "\"payload\":";
    let Some(offset) = envelope.find(PAYLOAD_KEY) else {
        return Err(consistency_error(
            "envelope is missing its payload member".to_owned(),
        ));
    };
    if !envelope.ends_with('}') {
        return Err(consistency_error("envelope must end with `}`".to_owned()));
    }
    let payload = &envelope[offset + PAYLOAD_KEY.len()..envelope.len() - 1];
    if !payload.starts_with('{') || !payload.ends_with('}') {
        return Err(consistency_error(
            "envelope payload must be a JSON object".to_owned(),
        ));
    }
    if declared_bytes != payload.len() as u64 {
        return Err(consistency_error(format!(
            "envelope declares {declared_bytes} payload bytes but {} are present",
            payload.len()
        )));
    }
    let recomputed = domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes());
    if envelope_digest != recomputed {
        return Err(consistency_error(
            "envelope digest does not match the exact payload bytes".to_owned(),
        ));
    }
    let payload_value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|error| consistency_error(format!("payload is not valid JSON: {error}")))?;
    if payload_value["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "payload schema must be {SCHEMA}"
        )));
    }
    replay_closed_sections(&payload_value)?;

    let Some(plugin) = payload_value["plugin"].as_object() else {
        return Err(consistency_error(
            "payload plugin must be an object".to_owned(),
        ));
    };
    let Some(name) = plugin["name"].as_str() else {
        return Err(consistency_error("plugin name must be a string".to_owned()));
    };
    let Some(identity) = plugin["identity"].as_str() else {
        return Err(consistency_error(
            "plugin identity must be a string".to_owned(),
        ));
    };
    let Some(version) = plugin["version"].as_str() else {
        return Err(consistency_error(
            "plugin version must be a string".to_owned(),
        ));
    };
    if version.len() != VERSION_HEX_CHARS || Some(version) != identity.get(7..7 + VERSION_HEX_CHARS)
    {
        return Err(consistency_error(
            "plugin version must equal the leading hex characters of the plugin identity"
                .to_owned(),
        ));
    }

    let Some(exports) = payload_value["exports"].as_array() else {
        return Err(consistency_error(
            "payload exports must be an array".to_owned(),
        ));
    };
    let mut verified = Vec::with_capacity(exports.len());
    let mut previous_id: Option<&str> = None;
    for export in exports {
        let Some(stable_id) = export["stable_id"].as_str() else {
            return Err(consistency_error(
                "export stable_id must be a string".to_owned(),
            ));
        };
        if let Some(previous) = previous_id {
            if previous.as_bytes() >= stable_id.as_bytes() {
                return Err(consistency_error(format!(
                    "export `{stable_id}` breaks the strict stable-id ordering"
                )));
            }
        }
        previous_id = Some(stable_id);
        let Some(export_name) = export["name"].as_str() else {
            return Err(consistency_error("export name must be a string".to_owned()));
        };
        let Some(symbol) = export["native64"]["symbol"].as_str() else {
            return Err(consistency_error(
                "export native64 symbol must be a string".to_owned(),
            ));
        };
        let Some(signature) = export["native64"]["signature"].as_str() else {
            return Err(consistency_error(
                "export native64 signature must be a string".to_owned(),
            ));
        };
        let Some(signature_digest) = export["native64"]["signature_sha256"].as_str() else {
            return Err(consistency_error(
                "export native64 signature_sha256 must be a string".to_owned(),
            ));
        };
        if signature_digest != domain_digest(EXPORT_SIGNATURE_DIGEST_DOMAIN, signature.as_bytes()) {
            return Err(consistency_error(
                "embedded export signature digest does not match the signature text".to_owned(),
            ));
        }
        verified.push(VerifiedExport {
            stable_id: stable_id.to_owned(),
            name: export_name.to_owned(),
            native_symbol: symbol.to_owned(),
            native_signature: signature.to_owned(),
        });
    }
    Ok(VerifiedPluginManifest {
        name: name.to_owned(),
        identity: identity.to_owned(),
        version: version.to_owned(),
        exports: verified,
    })
}

/// Verify one envelope and additionally bind the current bytes of
/// `source_path` to both embedded source digests (the source snapshot digest
/// and the plugin identity), failing closed on drift.
pub fn verify_envelope_against_source(
    envelope: &str,
    source_path: &Path,
) -> Result<VerifiedPluginManifest, Diagnostic> {
    let verified = verify_envelope(envelope)?;
    let current = std::fs::read(source_path).map_err(|error| {
        consistency_error(format!("cannot read {}: {error}", source_path.display()))
    })?;
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("envelope is not valid JSON: {error}")))?;
    let Some(source_digest) = value["payload"]["source"]["sha256"].as_str() else {
        return Err(consistency_error(
            "payload source sha256 must be a string".to_owned(),
        ));
    };
    if source_digest != domain_digest(SOURCE_DIGEST_DOMAIN, &current) {
        return Err(consistency_error(
            "plugin manifest source digest does not match the current source bytes; \
             the source drifted after the manifest was generated"
                .to_owned(),
        ));
    }
    let expected_identity = domain_digest(PLUGIN_IDENTITY_DIGEST_DOMAIN, &current);
    if verified.identity != expected_identity {
        return Err(consistency_error(
            "plugin identity does not match its derivation from the current source bytes; \
             the source drifted after the manifest was generated"
                .to_owned(),
        ));
    }
    Ok(verified)
}

fn replay_closed_sections(payload: &serde_json::Value) -> Result<(), Diagnostic> {
    let Some(descriptor) = payload["descriptor"].as_object() else {
        return Err(consistency_error(
            "payload descriptor must be an object".to_owned(),
        ));
    };
    let Some(functions_total) = descriptor["functions_total"].as_u64() else {
        return Err(consistency_error(
            "descriptor functions_total must be an unsigned integer".to_owned(),
        ));
    };
    let Some(admitted) = descriptor["exports_admitted"].as_u64() else {
        return Err(consistency_error(
            "descriptor exports_admitted must be an unsigned integer".to_owned(),
        ));
    };
    let Some(excluded) = descriptor["exports_excluded"].as_u64() else {
        return Err(consistency_error(
            "descriptor exports_excluded must be an unsigned integer".to_owned(),
        ));
    };
    let exports_len = payload["exports"].as_array().map_or(0, Vec::len) as u64;
    let exclusions_len = payload["exclusions"].as_array().map_or(0, Vec::len) as u64;
    if functions_total != exports_len + exclusions_len
        || admitted != exports_len
        || excluded != exclusions_len
    {
        return Err(consistency_error(
            "descriptor counts disagree with the listed exports and exclusions".to_owned(),
        ));
    }

    // The capability vocabulary is closed and the required-capabilities
    // section must equal its re-derivation from the listed tokens, exactly
    // as Build Capability Manifest v1 replays its ambient section.
    let mut tokens = BTreeSet::new();
    collect_tokens(payload_value_tokens(payload)?, &mut tokens)?;
    for token in &tokens {
        if !capability_manifest::within_vocabulary(token) {
            return Err(consistency_error(format!(
                "capability `{token}` is outside the admitted bounded vocabulary"
            )));
        }
    }
    let expected_capabilities = capability_manifest::ambient_authority_json(&tokens);
    let expected: serde_json::Value =
        serde_json::from_str(&expected_capabilities).expect("derived section is valid JSON");
    if payload["required_capabilities"] != expected {
        return Err(consistency_error(
            "embedded required capabilities disagree with their derivation from the declared tokens"
                .to_owned(),
        ));
    }

    let expected_resource_limits: serde_json::Value =
        serde_json::from_str(RESOURCE_LIMITS_JSON).expect("resource-limits constant is valid JSON");
    if payload["resource_limits"] != expected_resource_limits {
        return Err(consistency_error(
            "resource_limits must be exactly the canonical empty-by-default section".to_owned(),
        ));
    }

    let Some(unavailable) = payload["unavailable_sections"].as_array() else {
        return Err(consistency_error(
            "payload unavailable_sections must be an array".to_owned(),
        ));
    };
    if unavailable.len() != UNAVAILABLE_SECTIONS.len()
        || unavailable
            .iter()
            .zip(UNAVAILABLE_SECTIONS.iter())
            .any(|(listed, expected)| listed.as_str() != Some(expected))
    {
        return Err(consistency_error(
            "unavailable_sections must be exactly the closed canonical inventory".to_owned(),
        ));
    }

    let Some(exclusions) = payload["exclusions"].as_array() else {
        return Err(consistency_error(
            "payload exclusions must be an array".to_owned(),
        ));
    };
    for exclusion in exclusions {
        let Some(reason) = exclusion["reason"].as_str() else {
            return Err(consistency_error(
                "exclusion reason must be a string".to_owned(),
            ));
        };
        if !EXCLUSION_REASONS.contains(&reason) {
            return Err(consistency_error(format!(
                "exclusion reason `{reason}` is outside the closed vocabulary"
            )));
        }
    }
    Ok(())
}

fn payload_value_tokens(
    payload: &serde_json::Value,
) -> Result<&Vec<serde_json::Value>, Diagnostic> {
    payload["capability_tokens"].as_array().ok_or_else(|| {
        consistency_error("payload capability_tokens must be an array of strings".to_owned())
    })
}

fn collect_tokens(
    values: &Vec<serde_json::Value>,
    tokens: &mut BTreeSet<String>,
) -> Result<(), Diagnostic> {
    for value in values {
        let Some(token) = value.as_str() else {
            return Err(consistency_error(
                "payload capability_tokens must contain only strings".to_owned(),
            ));
        };
        tokens.insert(token.to_owned());
    }
    Ok(())
}

fn require_vocabulary(token: &str) -> Result<(), Vec<Diagnostic>> {
    if !capability_manifest::within_vocabulary(token) {
        return Err(vec![vocabulary_error(token)]);
    }
    Ok(())
}

/// Closed AST-level admission gate mirroring Canonical ABI Report v1 and
/// Interface Package Report v1 exactly.
fn admission(function: &Function) -> Option<&'static str> {
    if !function.explicit_id {
        return Some(REASON_AUTOMATIC_IDENTITY);
    }
    if !function.type_parameters.is_empty() {
        return Some(REASON_GENERIC_FUNCTION);
    }
    if !function.effects.is_empty() {
        return Some(REASON_DECLARED_EFFECTS);
    }
    for param in &function.params {
        if param.mode != ParamMode::Value {
            return Some(REASON_UNSUPPORTED_PARAMETER_MODE);
        }
        if !matches!(param.ty, Type::I64 | Type::Bool) {
            return Some(REASON_UNSUPPORTED_PARAMETER_TYPE);
        }
    }
    if !matches!(function.return_type, Type::I64 | Type::Bool) {
        return Some(REASON_UNSUPPORTED_RESULT_TYPE);
    }
    None
}

fn parameter_types(function: &Function) -> Vec<&'static str> {
    function
        .params
        .iter()
        .map(|param| match param.ty {
            Type::I64 => "i64",
            _ => "bool",
        })
        .collect()
}

fn result_type(function: &Function) -> &'static str {
    match function.return_type {
        Type::I64 => "i64",
        _ => "bool",
    }
}

fn extract_native_signature(
    native_text: &str,
    symbol: &str,
    label: &str,
) -> Result<String, Vec<Diagnostic>> {
    let prefix = format!("static __attribute__((unused)) spx_status_token {symbol}(");
    let matches = native_text
        .lines()
        .filter(|line| line.starts_with(&prefix))
        .map(str::trim_end)
        .filter(|line| line.ends_with(';'))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [signature] => Ok((*signature).to_owned()),
        [] => Err(vec![consistency_error(format!(
            "native projection has no prototype line for `{label}`"
        ))]),
        _ => Err(vec![consistency_error(format!(
            "native projection has multiple prototype lines for `{label}`"
        ))]),
    }
}

/// Mirror of the native backend's monomorphic function symbol convention.
fn c_function_symbol(stable_id: &str) -> String {
    let mut symbol = String::from("spx_decl_");
    for byte in stable_id.bytes() {
        use std::fmt::Write as _;
        let _ = write!(symbol, "{byte:02x}");
    }
    symbol
}

fn source_digest(source: &str) -> String {
    domain_digest(SOURCE_DIGEST_DOMAIN, source.as_bytes())
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

#[allow(clippy::too_many_arguments)]
fn render(
    path_text: &str,
    revision: &str,
    digest: &str,
    module_name: &str,
    identity: &str,
    version: &str,
    max_bytes: usize,
    tokens: &BTreeSet<String>,
    functions_total: usize,
    exports: &[ExportEntry],
    excluded: &[ExcludedFunction],
) -> String {
    let export_entries = exports
        .iter()
        .map(|entry| {
            let parameters = entry
                .parameters
                .iter()
                .map(|parameter| quote_json(parameter))
                .collect::<Vec<_>>();
            let requires = entry
                .requires
                .iter()
                .map(|clause| quote_json(clause))
                .collect::<Vec<_>>();
            let ensures = entry
                .ensures
                .iter()
                .map(|clause| quote_json(clause))
                .collect::<Vec<_>>();
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"parameters\":[{}],\"result\":{},\
\"native64\":{{\"symbol\":{},\"signature\":{},\"signature_sha256\":{}}},\
\"requires\":[{}],\"ensures\":[{}]}}",
                quote_json(&entry.stable_id),
                quote_json(&entry.name),
                parameters.budgeted_join(","),
                quote_json(entry.result),
                quote_json(&entry.symbol),
                quote_json(&entry.signature),
                quote_json(&domain_digest(
                    EXPORT_SIGNATURE_DIGEST_DOMAIN,
                    entry.signature.as_bytes(),
                )),
                requires.budgeted_join(","),
                ensures.budgeted_join(","),
            )
        })
        .collect::<Vec<_>>();
    let exclusion_entries = excluded
        .iter()
        .map(|entry| {
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"reason\":\"{}\"}}",
                quote_json(&entry.stable_id),
                quote_json(&entry.name),
                entry.reason,
            )
        })
        .collect::<Vec<_>>();
    let token_entries = tokens
        .iter()
        .map(|token| quote_json(token))
        .collect::<Vec<_>>();
    let required_capabilities = capability_manifest::ambient_authority_json(tokens);

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"limits\":{{\"max_bytes\":{}}},\
\"plugin\":{{\"name\":{},\"identity\":{},\"version\":{}}},\
\"descriptor\":{{\"functions_total\":{},\"exports_admitted\":{},\"exports_excluded\":{}}},\
\"capability_tokens\":[{}],\
\"required_capabilities\":{},\
\"resource_limits\":{},\
\"exports\":[{}],\"exclusions\":[{}],\
\"unavailable_sections\":{},\"nonclaims\":[{}]}}",
        SCHEMA,
        quote_json(path_text),
        quote_json(revision),
        quote_json(digest),
        max_bytes,
        quote_json(module_name),
        quote_json(identity),
        quote_json(version),
        functions_total,
        exports.len(),
        excluded.len(),
        token_entries.budgeted_join(","),
        required_capabilities,
        RESOURCE_LIMITS_JSON,
        export_entries.budgeted_join(","),
        exclusion_entries.budgeted_join(","),
        UNAVAILABLE_SECTIONS_JSON,
        NONCLAIMS_JSON,
    );
    bformat!(
        "{{\"schema\":\"{}\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        SCHEMA,
        quote_json(&domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_reject_out_of_bounds_values() {
        assert!(PluginManifestOptions::new(512).is_err());
        assert!(PluginManifestOptions::new(graph::MAX_AGENT_CONTEXT_BYTES + 1).is_err());
        assert!(PluginManifestOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).is_ok());
        assert_eq!(
            PluginManifestOptions::default().max_bytes,
            DEFAULT_MAX_BYTES
        );
    }

    #[test]
    fn constants_are_canonical_and_agree() {
        let unavailable: serde_json::Value =
            serde_json::from_str(UNAVAILABLE_SECTIONS_JSON).expect("unavailable constant");
        let listed = unavailable.as_array().expect("array");
        assert_eq!(listed.len(), UNAVAILABLE_SECTIONS.len());
        for (value, token) in listed.iter().zip(UNAVAILABLE_SECTIONS.iter()) {
            assert_eq!(value.as_str(), Some(*token));
        }
        let mut sorted = UNAVAILABLE_SECTIONS;
        sorted.sort_unstable();
        assert_eq!(sorted, UNAVAILABLE_SECTIONS, "must be bytewise sorted");

        let resource_limits: serde_json::Value =
            serde_json::from_str(RESOURCE_LIMITS_JSON).expect("resource-limits constant");
        assert_eq!(resource_limits["fuel"], serde_json::Value::Null);
        assert_eq!(resource_limits["memory_bytes"], serde_json::Value::Null);
        assert_eq!(resource_limits["table_elements"], serde_json::Value::Null);

        let nonclaims: serde_json::Value =
            serde_json::from_str(&format!("[{NONCLAIMS_JSON}]")).expect("nonclaims constant");
        assert_eq!(nonclaims.as_array().map(Vec::len), Some(8));
    }

    #[test]
    fn domain_digest_is_domain_separated() {
        let first = domain_digest(SOURCE_DIGEST_DOMAIN, b"abc");
        let second = domain_digest(PLUGIN_IDENTITY_DIGEST_DOMAIN, b"abc");
        assert_ne!(first, second);
        assert_eq!(first, domain_digest(SOURCE_DIGEST_DOMAIN, b"abc"));
    }

    #[test]
    fn versions_derive_from_the_identity_prefix() {
        let identity = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(derive_version(identity), "0123456789abcdef");
        let other = "sha256:ffffffffffffffff0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_ne!(derive_version(identity), derive_version(other));
        // Non-identity inputs degrade to whatever prefix exists; replay
        // still rejects any version that is not exactly VERSION_HEX_CHARS.
        assert_eq!(derive_version("not-a-digest"), "not-a-digest");
    }

    #[test]
    fn symbols_match_the_native_hex_encoding() {
        assert_eq!(c_function_symbol("app.main"), "spx_decl_6170702e6d61696e");
    }

    #[test]
    fn admission_mirrors_the_scalar_profile() {
        let source = r#"
module test.probe;

@id("probe.ok")
fn ok(value: i64) -> bool { value > 0 }

@id("probe.generic")
fn pick<T>(value: T) -> T { value }
"#;
        let path = std::env::temp_dir().join(format!(
            "semaprax-plugin-manifest-unit-{}.spx",
            std::process::id()
        ));
        std::fs::write(&path, source).unwrap();
        let program = parse(&std::fs::read_to_string(&path).unwrap(), &path).expect("parses");
        let mut functions = program.functions.iter().collect::<Vec<_>>();
        functions.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
        assert_eq!(admission(functions[0]), Some(REASON_GENERIC_FUNCTION));
        assert_eq!(admission(functions[1]), None);
        let _ = std::fs::remove_file(&path);
    }
}
