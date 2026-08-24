//! Deterministic, read-only Interface Package Report v1.
//!
//! [`generate`] projects one verified single-file SEMAPRAX module into one
//! canonical compact JSON envelope (`semaprax.package-report.v1`): an
//! interface-first package descriptor whose export inventory lists every
//! admitted declaration — explicit-ID monomorphic effect-free functions with
//! only by-value direct Copy-scalar (`i64`, `i32`, `u8`, `f32`, `f64`,
//! `char`, `bool`) parameters and results — sorted bytewise
//! by stable identity, each carrying its interface types, rendered contract
//! clauses, declared effect set, and the exact Native64 prototype line
//! extracted verbatim from the production native C11 projection under its own
//! domain-separated digest. Every other function is recorded as an exclusion
//! with one closed reason, mirroring the Canonical ABI Report admission
//! profile exactly. A fixed target availability matrix marks exactly two
//! admitted targets (`native64`, `wasm32`) available for this profile, and an
//! explicit closed list names the unavailable capabilities the row does not
//! provide.
//!
//! [`verify_envelope`] independently replays one envelope: exact envelope
//! shape, declared byte count, domain-separated payload digest, package
//! counts, the closed target matrix, the closed unavailable-capability list,
//! closed exclusion reasons, sorted export order, and every embedded export
//! signature digest.
//!
//! Diagnostics use the previously unused `SPX-P3xx` family:
//! - `SPX-P301`: invalid options (bounds, malformed values).
//! - `SPX-P302`: output byte-budget exhaustion (fail-closed, no truncation).
//! - `SPX-P303`: envelope or backend-projection consistency failure.
//!
//! This tranche performs no dependency resolution, writes no lockfile,
//! maintains no dependency model, hosts no registry, runs no compatibility
//! engine or conformance tests, attaches no provenance, signatures, licenses,
//! or SBOM, executes nothing, and changes no source.

use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::ast::{Function, ParamMode, Type};
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{codegen, format, graph, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.package-report.v1";

const DEFAULT_MAX_BYTES: usize = 64 * 1024;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report.source.v1\0";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report.payload.v1\0";
const EXPORT_SIGNATURE_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report.export-signature.v1\0";

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

/// The complete target availability matrix: exactly these two targets are
/// admitted, and both are available for the scalar export profile.
const TARGETS_JSON: &str =
    "[{\"target\":\"native64\",\"available\":true},{\"target\":\"wasm32\",\"available\":true}]";

/// Closed inventory of capabilities this report explicitly does not provide,
/// in canonical bytewise order.
const UNAVAILABLE_CAPABILITIES: [&str; 10] = [
    "compatibility_engine",
    "conformance_tests",
    "dependency_model",
    "licenses",
    "lockfile",
    "package_registry",
    "provenance",
    "resolver",
    "sbom",
    "signatures",
];
const UNAVAILABLE_CAPABILITIES_JSON: &str = "[\"compatibility_engine\",\
\"conformance_tests\",\
\"dependency_model\",\
\"licenses\",\
\"lockfile\",\
\"package_registry\",\
\"provenance\",\
\"resolver\",\
\"sbom\",\
\"signatures\"]";

const NONCLAIMS_JSON: &str = "\"report_descriptor_only\",\
\"no_resolver\",\
\"no_lockfile_or_dependency_model\",\
\"no_package_registry_or_hosting\",\
\"no_version_compatibility_engine\",\
\"no_conformance_tests\",\
\"no_provenance_signatures_licenses_or_sbom\",\
\"no_target_execution\",\
\"read_only_no_source_changes\"";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageReportOptions {
    pub max_bytes: usize,
}

impl PackageReportOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "package-report max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for PackageReportOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-P301", message)
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-P303", message)
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
    effects: Vec<String>,
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
pub struct VerifiedPackageReport {
    pub exports: Vec<VerifiedExport>,
}

/// Generate the canonical `semaprax.package-report.v1` envelope JSON for one
/// verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and the
/// final check or generation fails closed.
pub fn generate(
    source_path: &Path,
    options: &PackageReportOptions,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

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
            effects: function.effects.clone(),
        });
    }

    let digest = source_digest(snapshot.source());
    let path_text = source_path.display().to_string();
    let (envelope, overflowed) = with_limit(options.max_bytes, || {
        render(
            &path_text,
            &revision,
            &digest,
            &program.module,
            options.max_bytes,
            functions_total,
            &exports,
            &excluded,
        )
    });
    if overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-P302",
            "package-report output exceeds the max-bytes budget; refusing to truncate".to_owned(),
        )]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(envelope)
}

/// Independently verify one envelope produced by [`generate`].
///
/// Recomputes the outer payload digest over the exact serialized payload
/// bytes, re-checks the declared byte count, replays the closed target
/// matrix, the closed unavailable-capability list, the package counts, the
/// closed exclusion vocabulary, and the sorted export order, and
/// re-authenticates every embedded export signature digest before returning
/// the export summaries.
pub fn verify_envelope(envelope: &str) -> Result<VerifiedPackageReport, Diagnostic> {
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
        let Some(name) = export["name"].as_str() else {
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
            name: name.to_owned(),
            native_symbol: symbol.to_owned(),
            native_signature: signature.to_owned(),
        });
    }
    Ok(VerifiedPackageReport { exports: verified })
}

fn replay_closed_sections(payload: &serde_json::Value) -> Result<(), Diagnostic> {
    let Some(package) = payload["package"].as_object() else {
        return Err(consistency_error(
            "payload package must be an object".to_owned(),
        ));
    };
    let Some(functions_total) = package["functions_total"].as_u64() else {
        return Err(consistency_error(
            "package functions_total must be an unsigned integer".to_owned(),
        ));
    };
    let Some(admitted) = package["exports_admitted"].as_u64() else {
        return Err(consistency_error(
            "package exports_admitted must be an unsigned integer".to_owned(),
        ));
    };
    let Some(excluded) = package["exports_excluded"].as_u64() else {
        return Err(consistency_error(
            "package exports_excluded must be an unsigned integer".to_owned(),
        ));
    };
    let exports_len = payload["exports"].as_array().map_or(0, Vec::len) as u64;
    let exclusions_len = payload["exclusions"].as_array().map_or(0, Vec::len) as u64;
    if functions_total != exports_len + exclusions_len
        || admitted != exports_len
        || excluded != exclusions_len
    {
        return Err(consistency_error(
            "package counts disagree with the listed exports and exclusions".to_owned(),
        ));
    }

    let expected_targets: serde_json::Value =
        serde_json::from_str(TARGETS_JSON).expect("target matrix constant is valid JSON");
    if payload["targets"] != expected_targets {
        return Err(consistency_error(
            "target availability matrix must be exactly the two admitted available targets"
                .to_owned(),
        ));
    }

    let Some(unavailable) = payload["unavailable_capabilities"].as_array() else {
        return Err(consistency_error(
            "payload unavailable_capabilities must be an array".to_owned(),
        ));
    };
    if unavailable.len() != UNAVAILABLE_CAPABILITIES.len()
        || unavailable
            .iter()
            .zip(UNAVAILABLE_CAPABILITIES.iter())
            .any(|(listed, expected)| listed.as_str() != Some(expected))
    {
        return Err(consistency_error(
            "unavailable_capabilities must be exactly the closed canonical inventory".to_owned(),
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

/// Closed AST-level admission gate mirroring the widened Canonical ABI
/// Report v1 profile style: explicit identity, monomorphic, effect-free,
/// by-value direct Copy-scalar (`i64`, `i32`, `u8`, `f32`, `f64`, `char`,
/// `bool`) parameters and result.
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
        if scalar_type_name(&param.ty).is_none() {
            return Some(REASON_UNSUPPORTED_PARAMETER_TYPE);
        }
    }
    if scalar_type_name(&function.return_type).is_none() {
        return Some(REASON_UNSUPPORTED_RESULT_TYPE);
    }
    None
}

/// The widened Copy-scalar surface this report admits, with the exact
/// language-type spelling embedded in the export inventory.
fn scalar_type_name(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::I64 => Some("i64"),
        Type::I32 => Some("i32"),
        Type::Char => Some("char"),
        Type::U8 => Some("u8"),
        Type::F32 => Some("f32"),
        Type::F64 => Some("f64"),
        Type::Bool => Some("bool"),
        _ => None,
    }
}

fn parameter_types(function: &Function) -> Vec<&'static str> {
    function
        .params
        .iter()
        .map(|param| scalar_type_name(&param.ty).expect("admitted scalar parameter"))
        .collect()
}

fn result_type(function: &Function) -> &'static str {
    scalar_type_name(&function.return_type).expect("admitted scalar result")
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
    max_bytes: usize,
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
            let effects = entry
                .effects
                .iter()
                .map(|effect| quote_json(effect))
                .collect::<Vec<_>>();
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"parameters\":[{}],\"result\":{},\
\"native64\":{{\"symbol\":{},\"signature\":{},\"signature_sha256\":{}}},\
\"requires\":[{}],\"ensures\":[{}],\"effects\":[{}]}}",
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
                effects.budgeted_join(","),
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

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"limits\":{{\"max_bytes\":{}}},\
\"package\":{{\"name\":{},\"functions_total\":{},\"exports_admitted\":{},\"exports_excluded\":{}}},\
\"targets\":{},\
\"exports\":[{}],\"exclusions\":[{}],\
\"unavailable_capabilities\":{},\"nonclaims\":[{}]}}",
        SCHEMA,
        quote_json(path_text),
        quote_json(revision),
        quote_json(digest),
        max_bytes,
        quote_json(module_name),
        functions_total,
        exports.len(),
        excluded.len(),
        TARGETS_JSON,
        export_entries.budgeted_join(","),
        exclusion_entries.budgeted_join(","),
        UNAVAILABLE_CAPABILITIES_JSON,
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
        assert!(PackageReportOptions::new(512).is_err());
        assert!(PackageReportOptions::new(graph::MAX_AGENT_CONTEXT_BYTES + 1).is_err());
        assert!(PackageReportOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).is_ok());
        assert_eq!(PackageReportOptions::default().max_bytes, DEFAULT_MAX_BYTES);
    }

    #[test]
    fn constants_are_canonical_and_agree() {
        let targets: serde_json::Value =
            serde_json::from_str(TARGETS_JSON).expect("targets constant");
        assert_eq!(
            targets,
            serde_json::json!([
                {"target": "native64", "available": true},
                {"target": "wasm32", "available": true}
            ])
        );
        let unavailable: serde_json::Value =
            serde_json::from_str(UNAVAILABLE_CAPABILITIES_JSON).expect("unavailable constant");
        let listed = unavailable.as_array().expect("array");
        assert_eq!(listed.len(), UNAVAILABLE_CAPABILITIES.len());
        for (value, token) in listed.iter().zip(UNAVAILABLE_CAPABILITIES.iter()) {
            assert_eq!(value.as_str(), Some(*token));
        }
        let mut sorted = UNAVAILABLE_CAPABILITIES;
        sorted.sort_unstable();
        assert_eq!(sorted, UNAVAILABLE_CAPABILITIES, "must be bytewise sorted");
    }

    #[test]
    fn domain_digest_is_domain_separated() {
        let first = domain_digest(SOURCE_DIGEST_DOMAIN, b"abc");
        let second = domain_digest(PAYLOAD_DIGEST_DOMAIN, b"abc");
        assert_ne!(first, second);
        assert_eq!(first, domain_digest(SOURCE_DIGEST_DOMAIN, b"abc"));
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
            "semaprax-package-report-unit-{}.spx",
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
