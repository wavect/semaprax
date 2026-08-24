//! Deterministic, read-only C Header Emission v1.
//!
//! `semaprax c-header <file>` derives a C11 header from verified program
//! facts for explicitly selected public monomorphic scalar functions. The
//! emitted declaration lines are extracted verbatim from the production
//! native C11 projection (`codegen::emit_c`), so every header declaration
//! matches the ABI the native backend actually emits. Generated comments
//! carry only typed program facts: stable identities, canonical contract
//! text, effect sets, the status/out-parameter contract of the native lane,
//! and by-value ownership facts. No free-form host input reaches the bytes.
//!
//! Diagnostics use the previously unused `SPX-D1xx` family:
//! - `SPX-D101`: invalid options (bounds, duplicates, malformed values).
//! - `SPX-D102`: invalid selection (empty, over budget, unknown target).
//! - `SPX-D103`: output byte-budget exhaustion (fail-closed, no truncation).
//! - `SPX-D104`: derived-text hygiene violation (comment-hostile characters).
//! - `SPX-D105`: envelope or native-projection consistency failure.
//!
//! This tranche performs no header import, no raw-binding import, no safe
//! wrapper generation, no Objective-C mapping, no string or buffer mappings,
//! and compiles nothing.

use std::collections::BTreeSet;
use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::ast::{Function, ParamMode, Program, Type};
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{codegen, format, graph, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.c-header.v1";

/// Hard cap on selected functions per invocation.
pub const MAX_FUNCTIONS: usize = 64;

const DEFAULT_MAX_BYTES: usize = 64 * 1024;

const GUARD_DOMAIN: &[u8] = b"semaprax.c-header.guard.v1\0";
const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.c-header.source.v1\0";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.c-header.payload.v1\0";
const HEADER_DIGEST_DOMAIN: &[u8] = b"semaprax.c-header.header.v1\0";
const DECLARATION_DIGEST_DOMAIN: &[u8] = b"semaprax.c-header.declaration.v1\0";

const REASON_AUTOMATIC_IDENTITY: &str = "automatic_identity";
const REASON_GENERIC_FUNCTION: &str = "generic_function";
const REASON_DECLARED_EFFECTS: &str = "declared_effects";
const REASON_UNSUPPORTED_PARAMETER_MODE: &str = "unsupported_parameter_mode";
const REASON_UNSUPPORTED_PARAMETER_TYPE: &str = "unsupported_parameter_type";
const REASON_UNSUPPORTED_RESULT_TYPE: &str = "unsupported_result_type";

const NONCLAIMS_JSON: &str = "\"no_header_import\",\
\"no_raw_binding_import\",\
\"no_safe_wrapper_generation\",\
\"no_objective_c_mapping\",\
\"no_string_or_buffer_mappings\",\
\"no_compiled_conformance_evidence\",\
\"read_only\"";

const OWNERSHIP_BY_VALUE: &str = "caller-free / by-value scalars";
const STATUS_CONTRACT_NOTE: &str =
    "returns spx_status_token; *spx_result_out is written only at the final success commit";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CHeaderOptions {
    /// Canonical selection: unique nonempty function names or stable IDs in
    /// bytewise sorted order; between one and [`MAX_FUNCTIONS`] entries.
    pub functions: Vec<String>,
    pub max_bytes: usize,
}

impl CHeaderOptions {
    pub fn new(functions: Vec<String>, max_bytes: usize) -> Result<Self, Diagnostic> {
        if functions.is_empty() || functions.len() > MAX_FUNCTIONS {
            return Err(option_error(format!(
                "c-header requires between 1 and {MAX_FUNCTIONS} --function selections"
            )));
        }
        let mut seen = BTreeSet::new();
        for function in &functions {
            if function.is_empty() {
                return Err(option_error(
                    "c-header --function selections must be nonempty".to_owned(),
                ));
            }
            if !seen.insert(function.as_str()) {
                return Err(option_error(format!(
                    "c-header selected `{function}` more than once"
                )));
            }
        }
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "c-header max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self {
            functions,
            max_bytes,
        })
    }
}

impl Default for CHeaderOptions {
    fn default() -> Self {
        Self {
            functions: Vec::new(),
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-D101", message)
}

fn selection_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-D102", message)
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-D105", message)
}

struct EmittedFunction {
    stable_id: String,
    name: String,
    symbol: String,
    signature: String,
    requires: Vec<String>,
    ensures: Vec<String>,
    effects: String,
}

struct ExcludedFunction {
    stable_id: String,
    name: String,
    reason: &'static str,
}

struct Generation {
    envelope: String,
    header: String,
}

/// Generate the canonical `semaprax.c-header.v1` envelope JSON for one
/// verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and the
/// final check or generation fails closed.
pub fn generate(source_path: &Path, options: &CHeaderOptions) -> Result<String, Vec<Diagnostic>> {
    generate_internal(source_path, options).map(|generation| generation.envelope)
}

/// Generate only the bare deterministic header bytes under the same admission
/// and budget rules as [`generate`].
pub fn header_text(
    source_path: &Path,
    options: &CHeaderOptions,
) -> Result<String, Vec<Diagnostic>> {
    generate_internal(source_path, options).map(|generation| generation.header)
}

/// Independently verify one envelope produced by [`generate`].
///
/// Recomputes the outer payload digest over the exact serialized payload
/// bytes, re-checks the declared byte count, and re-authenticates the
/// embedded header digest. Returns the embedded header text on success.
pub fn verify_envelope(envelope: &str) -> Result<String, Diagnostic> {
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
    let Some(header) = payload_value["header"].as_str() else {
        return Err(consistency_error(
            "payload header must be a string".to_owned(),
        ));
    };
    let Some(header_digest) = payload_value["header_sha256"].as_str() else {
        return Err(consistency_error(
            "payload header_sha256 must be a string".to_owned(),
        ));
    };
    if header_digest != domain_digest(HEADER_DIGEST_DOMAIN, header.as_bytes()) {
        return Err(consistency_error(
            "embedded header digest does not match the header text".to_owned(),
        ));
    }
    Ok(header.to_owned())
}

fn generate_internal(
    source_path: &Path,
    options: &CHeaderOptions,
) -> Result<Generation, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

    let mut selected = resolve_selection(&program, &options.functions)?;
    selected.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
    let functions_total = program.functions.len();

    // Contract texts are rendered before entering the bounded assembly so the
    // formatter's internal reservations never interact with the output budget.
    let mut emitted: Vec<EmittedFunction> = Vec::new();
    let mut excluded: Vec<ExcludedFunction> = Vec::new();
    for function in &selected {
        match admission(function) {
            Some(reason) => excluded.push(ExcludedFunction {
                stable_id: function.stable_id.clone(),
                name: function.name.clone(),
                reason,
            }),
            None => emitted.push(EmittedFunction {
                stable_id: function.stable_id.clone(),
                name: hygiene_check(function.name.clone())?,
                symbol: c_function_symbol(&function.stable_id),
                signature: String::new(),
                requires: contract_clauses(&function.requires)?,
                ensures: contract_clauses(&function.ensures)?,
                effects: hygiene_check(function.effects.join(", "))?,
            }),
        }
    }

    let native_text = if emitted.is_empty() {
        None
    } else {
        Some(codegen::emit_c(&program).map_err(|error| vec![error])?)
    };
    if let Some(native_text) = &native_text {
        for function in &mut emitted {
            function.signature =
                extract_native_signature(native_text, &function.symbol, &function.stable_id)?;
        }
    }

    let guard_ids = emitted
        .iter()
        .map(|item| item.stable_id.clone())
        .collect::<Vec<_>>();
    let guard = include_guard(&guard_ids);
    let digest = source_digest(snapshot.source());
    let path_text = source_path.display().to_string();

    let (generation, overflowed) = with_limit(options.max_bytes, || {
        render(
            &path_text,
            &revision,
            &digest,
            &guard,
            options,
            functions_total,
            &emitted,
            &excluded,
        )
    });
    if overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-D103",
            "c-header output exceeds the max-bytes budget; refusing to truncate".to_owned(),
        )]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(generation)
}

fn resolve_selection<'a>(
    program: &'a Program,
    tokens: &[String],
) -> Result<Vec<&'a Function>, Vec<Diagnostic>> {
    let mut selected = Vec::with_capacity(tokens.len());
    let mut claimed = BTreeSet::new();
    for token in tokens {
        let function = program
            .functions
            .iter()
            .find(|candidate| candidate.stable_id == *token || candidate.name == *token)
            .ok_or_else(|| {
                vec![selection_error(format!(
                    "c-header selection `{token}` does not name a function in this program"
                ))]
            })?;
        if !claimed.insert(function.stable_id.as_str()) {
            return Err(vec![selection_error(format!(
                "c-header selections `{token}` and an earlier token both resolve to `{}`",
                function.stable_id
            ))]);
        }
        selected.push(function);
    }
    Ok(selected)
}

/// Closed AST-level admission gate mirroring the widened interop scalar
/// profile (full Copy-scalar surface), plus the explicit-identity
/// requirement.
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
        if !is_admitted_scalar(&param.ty) {
            return Some(REASON_UNSUPPORTED_PARAMETER_TYPE);
        }
    }
    if !is_admitted_scalar(&function.return_type) {
        return Some(REASON_UNSUPPORTED_RESULT_TYPE);
    }
    None
}

/// The full Copy-scalar surface admitted by the interop projections; every
/// member has an exact native C representation in the production projection.
fn is_admitted_scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::I32 | Type::U8 | Type::Char | Type::F32 | Type::F64 | Type::Bool
    )
}

fn contract_clauses(clauses: &[crate::ast::Expr]) -> Result<Vec<String>, Vec<Diagnostic>> {
    clauses
        .iter()
        .map(|clause| hygiene_check(format::expr(clause, 0)))
        .collect()
}

/// Fail closed when derived comment text could terminate a block comment or
/// smuggle control characters into the generated header.
fn hygiene_check(text: String) -> Result<String, Vec<Diagnostic>> {
    let hostile = text.contains("*/")
        || text.contains('\n')
        || text.contains('\r')
        || text.chars().any(|character| {
            character.is_control() || character == '\u{2028}' || character == '\u{2029}'
        });
    if hostile {
        return Err(vec![Diagnostic::io(
            "SPX-D104",
            format!(
                "derived text {text:?} contains characters that are unsafe inside a C block comment"
            ),
        )]);
    }
    Ok(text)
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

/// Include guard derived only from the sorted admitted stable identities, so
/// formatting-only source changes keep the guard stable while renames that
/// change an admitted identity change it.
fn include_guard(ids: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(GUARD_DOMAIN);
    hasher.update((ids.len() as u64).to_le_bytes());
    for id in ids {
        hasher.update((id.len() as u64).to_le_bytes());
        hasher.update(id.as_bytes());
    }
    let digest = hasher.finalize();
    let mut guard = String::from("SPX_HEADER_");
    for byte in &digest[..16] {
        use std::fmt::Write as _;
        let _ = write!(guard, "{byte:02x}");
    }
    guard
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
    guard: &str,
    options: &CHeaderOptions,
    functions_total: usize,
    emitted: &[EmittedFunction],
    excluded: &[ExcludedFunction],
) -> Generation {
    let header = render_header(revision, guard, emitted);

    let function_entries = emitted
        .iter()
        .map(|function| {
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"symbol\":{},\"signature\":{},\
\"declaration_sha256\":{},\"matches_native\":true}}",
                quote_json(&function.stable_id),
                quote_json(&function.name),
                quote_json(&function.symbol),
                quote_json(&function.signature),
                quote_json(&domain_digest(
                    DECLARATION_DIGEST_DOMAIN,
                    function.signature.as_bytes(),
                )),
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
    let header_sha256 = domain_digest(HEADER_DIGEST_DOMAIN, header.as_bytes());

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"limits\":{{\"max_bytes\":{}}},\
\"selection\":{{\"requested\":{},\"functions_total\":{},\"admitted\":{},\"excluded\":{}}},\
\"functions\":[{}],\"exclusions\":[{}],\
\"header_sha256\":{},\"header\":{},\"nonclaims\":[{}]}}",
        SCHEMA,
        quote_json(path_text),
        quote_json(revision),
        quote_json(digest),
        options.max_bytes,
        emitted.len() + excluded.len(),
        functions_total,
        emitted.len(),
        excluded.len(),
        function_entries.budgeted_join(","),
        exclusion_entries.budgeted_join(","),
        quote_json(&header_sha256),
        quote_json(&header),
        NONCLAIMS_JSON,
    );
    let envelope = bformat!(
        "{{\"schema\":\"{}\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        SCHEMA,
        quote_json(&domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload,
    );
    Generation { envelope, header }
}

fn render_header(revision: &str, guard: &str, emitted: &[EmittedFunction]) -> String {
    let mut output = crate::bounded_output::CappedString::new();
    output.push_str("/*\n");
    output.push_str(" * Generated by SEMAPRAX C Header Emission v1 (semaprax.c-header.v1).\n");
    output.push_str(" * This file is compiler-generated deterministic output; do not edit.\n");
    output.push_str(&bformat!(" * Revision: {revision}\n"));
    output.push_str(&bformat!(" * Admitted functions: {}\n", emitted.len()));
    output.push_str(" */\n");
    output.push_str("#ifndef ");
    output.push_str(guard);
    output.push('\n');
    output.push_str("#define ");
    output.push_str(guard);
    output.push_str("\n\n#include <stdbool.h>\n#include <stdint.h>\n");

    for function in emitted {
        output.push_str("\n/*\n");
        output.push_str(&bformat!(" * {}\n", function.name));
        output.push_str(&bformat!(" * stable-id: {}\n", function.stable_id));
        for clause in &function.requires {
            output.push_str(&bformat!(" * requires: {clause}\n"));
        }
        for clause in &function.ensures {
            output.push_str(&bformat!(" * ensures: {clause}\n"));
        }
        let effects = if function.effects.is_empty() {
            "none".to_owned()
        } else {
            function.effects.clone()
        };
        output.push_str(&bformat!(" * effects: {effects}\n"));
        output.push_str(&bformat!(" * status-contract: {STATUS_CONTRACT_NOTE}\n"));
        output.push_str(&bformat!(" * ownership: {OWNERSHIP_BY_VALUE}\n"));
        output.push_str(" */\n");
        output.push_str(&function.signature);
        output.push('\n');
    }

    output.push_str("\n#endif\n");
    output.into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn write_temp(source: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "semaprax-c-header-{}-{}.spx",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::write(&path, source).unwrap();
        path
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    const VALID_SOURCE: &str = r#"
module test.probe;

@id("probe.double")
fn double(value: i64) -> i64
    requires value >= 0
    ensures result == value + value
{
    value + value
}

@id("probe.flag")
fn flag(enabled: bool) -> bool { enabled }

@id("app.main")
fn main() -> i64
    ensures result == 42
{
    if flag(double(21) == 42) { 42 } else { 0 }
}
"#;

    fn double_options() -> CHeaderOptions {
        CHeaderOptions::new(vec!["probe.double".to_owned()], DEFAULT_MAX_BYTES)
            .expect("valid options")
    }

    #[test]
    fn options_reject_out_of_bounds_values() {
        assert!(CHeaderOptions::new(Vec::new(), DEFAULT_MAX_BYTES).is_err());
        assert!(
            CHeaderOptions::new(vec!["a".to_owned(); MAX_FUNCTIONS + 1], DEFAULT_MAX_BYTES)
                .is_err()
        );
        assert!(CHeaderOptions::new(vec![String::new()], DEFAULT_MAX_BYTES).is_err());
        assert!(
            CHeaderOptions::new(vec!["x".to_owned(), "x".to_owned()], DEFAULT_MAX_BYTES).is_err()
        );
        assert!(CHeaderOptions::new(vec!["x".to_owned()], 512).is_err());
        assert!(
            CHeaderOptions::new(vec!["x".to_owned()], graph::MAX_AGENT_CONTEXT_BYTES + 1).is_err()
        );
        assert!(CHeaderOptions::new(vec!["x".to_owned()], graph::MIN_AGENT_CONTEXT_BYTES).is_ok());
    }

    #[test]
    fn include_guard_is_deterministic_and_identity_sensitive() {
        let first = include_guard(&["math.add".to_owned()]);
        assert_eq!(first, include_guard(&["math.add".to_owned()]));
        assert_eq!(first.len(), "SPX_HEADER_".len() + 32);
        assert!(first.starts_with("SPX_HEADER_"));
        assert_ne!(first, include_guard(&["math.sum".to_owned()]));
        assert_ne!(
            first,
            include_guard(&["math.add".to_owned(), "math.sum".to_owned()])
        );
    }

    #[test]
    fn symbols_match_the_native_hex_encoding() {
        assert_eq!(c_function_symbol("app.main"), "spx_decl_6170702e6d61696e");
    }

    #[test]
    fn hygiene_rejects_comment_hostile_text() {
        assert!(hygiene_check("plain".to_owned()).is_ok());
        assert!(hygiene_check("result == left + right".to_owned()).is_ok());
        assert!(hygiene_check("terminates */ here".to_owned()).is_err());
        assert!(hygiene_check("line\nbreak".to_owned()).is_err());
        assert!(hygiene_check("carriage\rreturn".to_owned()).is_err());
    }

    #[test]
    fn golden_header_has_expected_shape_and_is_deterministic() {
        let path = write_temp(VALID_SOURCE);
        let first = header_text(&path, &double_options()).expect("header");
        let second = header_text(&path, &double_options()).expect("header");
        assert_eq!(first, second);
        assert!(first.starts_with("/*\n"));
        assert!(first.contains("#ifndef SPX_HEADER_"));
        assert!(first.contains("#include <stdbool.h>\n#include <stdint.h>\n"));
        assert!(first.contains(" * stable-id: probe.double\n"));
        assert!(first.contains(" * requires: value >= 0\n"));
        assert!(first.contains(" * ensures: result == value + value\n"));
        assert!(first.contains(" * effects: none\n"));
        assert!(first.contains(" * ownership: caller-free / by-value scalars\n"));
        assert!(first.contains(
            "static __attribute__((unused)) spx_status_token spx_decl_70726f62652e646f75626c65(struct spx_context *spx_ctx, int64_t, int64_t *spx_result_out);"
        ));
        assert!(first.ends_with("#endif\n"));
        cleanup(&path);
    }

    #[test]
    fn envelope_round_trips_through_verify_envelope() {
        let path = write_temp(VALID_SOURCE);
        let envelope = generate(&path, &double_options()).expect("envelope");
        let header = verify_envelope(&envelope).expect("verified");
        assert_eq!(header, header_text(&path, &double_options()).unwrap());
        cleanup(&path);
    }

    #[test]
    fn verify_envelope_detects_tampering() {
        let path = write_temp(VALID_SOURCE);
        let envelope = generate(&path, &double_options()).expect("envelope");
        let payload_tampered =
            envelope.replace("\"matches_native\":true", "\"matches_native\":false");
        assert!(verify_envelope(&payload_tampered).is_err());
        let truncated = envelope[..envelope.len() - 4].to_owned();
        assert!(verify_envelope(&truncated).is_err());
        assert!(verify_envelope("not json").is_err());
        cleanup(&path);
    }

    #[test]
    fn signature_matches_the_native_projection_line() {
        let path = write_temp(VALID_SOURCE);
        let program = parse(&std::fs::read_to_string(&path).unwrap(), &path).expect("parses");
        let native = codegen::emit_c(&program).expect("native projection");
        let header = header_text(&path, &double_options()).expect("header");
        for line in header.lines() {
            if line.starts_with("static __attribute__((unused))") {
                assert!(
                    native
                        .lines()
                        .any(|native_line| native_line.trim_end() == line),
                    "header line must appear verbatim in the native projection"
                );
            }
        }
        cleanup(&path);
    }

    #[test]
    fn selection_errors_fail_closed() {
        let path = write_temp(VALID_SOURCE);
        let unknown =
            CHeaderOptions::new(vec!["probe.missing".to_owned()], DEFAULT_MAX_BYTES).unwrap();
        assert!(generate(&path, &unknown).is_err());
        let duplicate_target = CHeaderOptions::new(
            vec!["probe.double".to_owned(), "double".to_owned()],
            DEFAULT_MAX_BYTES,
        )
        .unwrap();
        assert!(generate(&path, &duplicate_target).is_err());
        cleanup(&path);
    }

    #[test]
    fn every_exclusion_reason_is_reachable() {
        let source = r#"
module test.probe;
permit { io.release }

@id("probe.generic")
fn pick<T>(value: T) -> T { value }

@id("probe.effectful")
fn effectful(value: i64) -> i64 uses { io.release } { value }

@id("probe.borrowed")
fn borrowed(target: borrow Buffer, amount: i64) -> i64 { amount }

@id("probe.wide")
fn wide(label: string) -> string { label }

@id("app.main")
fn main() -> i64
    ensures result == 7
{
    7
}

@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}
"#;
        let path = write_temp(source);
        let options = CHeaderOptions::new(
            vec![
                "probe.generic".to_owned(),
                "probe.effectful".to_owned(),
                "probe.borrowed".to_owned(),
                "probe.wide".to_owned(),
            ],
            DEFAULT_MAX_BYTES,
        )
        .unwrap();
        let envelope = generate(&path, &options).expect("all-excluded envelope still succeeds");
        assert!(envelope.contains("\"reason\":\"generic_function\""));
        assert!(envelope.contains("\"reason\":\"declared_effects\""));
        assert!(envelope.contains("\"reason\":\"unsupported_parameter_mode\""));
        assert!(envelope.contains("\"reason\":\"unsupported_parameter_type\""));
        assert!(envelope.contains("\"admitted\":0,\"excluded\":4"));
        let header = verify_envelope(&envelope).expect("verified");
        assert!(header.contains("#include <stdint.h>"));
        assert!(!header.contains("static __attribute__((unused))"));
        cleanup(&path);
    }

    #[test]
    fn private_functions_are_excluded_by_identity_origin() {
        let source = r#"
module test.probe;

fn helper(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64
    ensures result == 1
{
    helper(0)
}
"#;
        let path = write_temp(source);
        let options = CHeaderOptions::new(vec!["helper".to_owned()], DEFAULT_MAX_BYTES).unwrap();
        let envelope = generate(&path, &options).expect("envelope");
        assert!(envelope.contains("\"reason\":\"automatic_identity\""));
        cleanup(&path);
    }

    #[test]
    fn byte_budget_exhaustion_fails_closed_without_truncation() {
        let path = write_temp(VALID_SOURCE);
        let options = CHeaderOptions::new(
            vec!["probe.double".to_owned()],
            graph::MIN_AGENT_CONTEXT_BYTES,
        )
        .unwrap();
        let outcome = generate(&path, &options);
        let errors = outcome.expect_err("tiny budgets must fail closed");
        assert!(
            errors.iter().any(|item| item.code == "SPX-D103"),
            "expected the byte-budget diagnostic"
        );
        cleanup(&path);
    }
}
