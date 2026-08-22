//! Deterministic, read-only Canonical ABI Report v1.
//!
//! `semaprax abi-report <file>` derives, for explicitly selected public
//! monomorphic scalar functions, both the native fast ABI (the exact Native64
//! C11 prototype extracted verbatim from the production native projection,
//! checked compiler sizes and alignments, by-value parameter passing, and the
//! status/out-parameter contract) and the portable canonical ABI mapping used
//! by the Public Scalar Export Profile v1 Core-Wasm lane (i64/i32 value
//! types, canonical bool boundary normalization, and copy-only behavior).
//!
//! Diagnostics use the previously unused `SPX-A2xx` family:
//! - `SPX-A201`: invalid options (bounds, duplicates, malformed values).
//! - `SPX-A202`: invalid selection (empty, over budget, unknown target).
//! - `SPX-A203`: output byte-budget exhaustion (fail-closed, no truncation).
//! - `SPX-A204`: envelope or backend-projection consistency failure.
//!
//! This tranche performs no interface-semantics mapping beyond the selected
//! scalar exports, no borrowing (the slice is copy-only), no cross-language
//! conformance suite, compiles nothing, executes nothing, and changes no
//! source.

use std::collections::BTreeSet;
use std::path::Path;

use crate::aggregate_layout;
use crate::ast::{Function, ParamMode, Program, Type};
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{ResolvedProgram, ResolvedType};
use crate::{codegen, graph, hir, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.abi-report.v1";

/// Hard cap on selected functions per invocation.
pub const MAX_FUNCTIONS: usize = 64;

const DEFAULT_MAX_BYTES: usize = 64 * 1024;

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.abi-report.payload.v1\0";
const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.abi-report.source.v1\0";
const NATIVE_SIGNATURE_DIGEST_DOMAIN: &[u8] = b"semaprax.abi-report.native-signature.v1\0";
const CANONICAL_SIGNATURE_DIGEST_DOMAIN: &[u8] = b"semaprax.abi-report.canonical-signature.v1\0";

const REASON_AUTOMATIC_IDENTITY: &str = "automatic_identity";
const REASON_GENERIC_FUNCTION: &str = "generic_function";
const REASON_DECLARED_EFFECTS: &str = "declared_effects";
const REASON_UNSUPPORTED_PARAMETER_MODE: &str = "unsupported_parameter_mode";
const REASON_UNSUPPORTED_PARAMETER_TYPE: &str = "unsupported_parameter_type";
const REASON_UNSUPPORTED_RESULT_TYPE: &str = "unsupported_result_type";

const NONCLAIMS_JSON: &str = "\"report_descriptor_only\",\
\"no_interface_semantics_beyond_selected_scalar_exports\",\
\"no_borrowing_copy_only\",\
\"no_cross_language_conformance_suites\",\
\"no_target_execution\",\
\"read_only\"";

const NATIVE_TARGET: &str = "Native64";
const CANONICAL_PROFILE: &str = "semaprax.wasm-scalar.v1";
const BOOL_BOUNDARY_NORMALIZATION: &str = "trap_unless_canonical_0_or_1";
const COPY_BEHAVIOR: &str = "copy";
const PARAMETER_PASSING: &str = "by-value copy";
const STATUS_CONTRACT_RETURNS: &str = "spx_status_token";
const STATUS_CONTRACT_CONTEXT: &str = "struct spx_context *spx_ctx";
const STATUS_CONTRACT_RESULT_WRITTEN_AT: &str = "final success commit";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbiReportOptions {
    /// Canonical selection: unique nonempty function names or stable IDs in
    /// bytewise sorted order; between one and [`MAX_FUNCTIONS`] entries.
    pub functions: Vec<String>,
    pub max_bytes: usize,
}

impl AbiReportOptions {
    pub fn new(functions: Vec<String>, max_bytes: usize) -> Result<Self, Diagnostic> {
        if functions.is_empty() || functions.len() > MAX_FUNCTIONS {
            return Err(option_error(format!(
                "abi-report requires between 1 and {MAX_FUNCTIONS} --function selections"
            )));
        }
        let mut seen = BTreeSet::new();
        for function in &functions {
            if function.is_empty() {
                return Err(option_error(
                    "abi-report --function selections must be nonempty".to_owned(),
                ));
            }
            if !seen.insert(function.as_str()) {
                return Err(option_error(format!(
                    "abi-report selected `{function}` more than once"
                )));
            }
        }
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "abi-report max_bytes must be between {} and {}",
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

impl Default for AbiReportOptions {
    fn default() -> Self {
        Self {
            functions: Vec::new(),
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-A201", message)
}

fn selection_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-A202", message)
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-A204", message)
}

struct ReportedFunction {
    stable_id: String,
    name: String,
    symbol: String,
    signature: String,
    native_parameters: Vec<ScalarFacts>,
    native_result: ScalarFacts,
    result_out_parameter: String,
    wasm_export: String,
    wasm_parameters: Vec<&'static str>,
    wasm_result: &'static str,
}

struct ScalarFacts {
    language_type: &'static str,
    c_type: &'static str,
    size_bytes: u32,
    align_bytes: u32,
}

struct ExcludedFunction {
    stable_id: String,
    name: String,
    reason: &'static str,
}

/// One independently authenticated function summary returned by
/// [`verify_envelope`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedFunction {
    pub stable_id: String,
    pub name: String,
    pub native_symbol: String,
    pub native_signature: String,
    pub wasm_export: String,
    pub wasm_parameters: Vec<String>,
    pub wasm_result: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VerifiedAbiReport {
    pub functions: Vec<VerifiedFunction>,
}

/// Generate the canonical `semaprax.abi-report.v1` envelope JSON for one
/// verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and the
/// final check or generation fails closed.
pub fn generate(source_path: &Path, options: &AbiReportOptions) -> Result<String, Vec<Diagnostic>> {
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

    let mut excluded: Vec<ExcludedFunction> = Vec::new();
    let mut admitted: Vec<&Function> = Vec::new();
    for function in &selected {
        match admission(function) {
            Some(reason) => excluded.push(ExcludedFunction {
                stable_id: function.stable_id.clone(),
                name: function.name.clone(),
                reason,
            }),
            None => admitted.push(function),
        }
    }

    let resolved = if admitted.is_empty() {
        None
    } else {
        Some(hir::resolve(&program)?)
    };

    let mut reported: Vec<ReportedFunction> = Vec::new();
    for function in admitted {
        let facts = scalar_facts(
            resolved.as_ref().expect("resolved above"),
            &function.stable_id,
        )?;
        reported.push(ReportedFunction {
            stable_id: function.stable_id.clone(),
            name: function.name.clone(),
            symbol: c_function_symbol(&function.stable_id),
            signature: String::new(),
            native_parameters: facts.parameters,
            native_result: facts.result,
            result_out_parameter: facts.result_out_parameter,
            wasm_export: raw_wasm_export(&function.stable_id),
            wasm_parameters: facts.wasm_parameters,
            wasm_result: facts.wasm_result,
        });
    }

    if !reported.is_empty() {
        let native_text = codegen::emit_c(&program).map_err(|error| vec![error])?;
        for function in &mut reported {
            function.signature =
                extract_native_signature(&native_text, &function.symbol, &function.stable_id)?;
        }
    }

    let digest = source_digest(snapshot.source());
    let path_text = source_path.display().to_string();

    let (envelope, overflowed) = with_limit(options.max_bytes, || {
        render(
            &path_text,
            &revision,
            &digest,
            options.max_bytes,
            functions_total,
            &reported,
            &excluded,
        )
    });
    if overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-A203",
            "abi-report output exceeds the max-bytes budget; refusing to truncate".to_owned(),
        )]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(envelope)
}

/// Independently verify one envelope produced by [`generate`].
///
/// Recomputes the outer payload digest over the exact serialized payload
/// bytes, re-checks the declared byte count, and re-authenticates every
/// embedded native and canonical signature digest before returning the
/// function summaries.
pub fn verify_envelope(envelope: &str) -> Result<VerifiedAbiReport, Diagnostic> {
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
    let Some(functions) = payload_value["functions"].as_array() else {
        return Err(consistency_error(
            "payload functions must be an array".to_owned(),
        ));
    };
    let mut verified = Vec::with_capacity(functions.len());
    for function in functions {
        let Some(stable_id) = function["stable_id"].as_str() else {
            return Err(consistency_error(
                "function stable_id must be a string".to_owned(),
            ));
        };
        let Some(name) = function["name"].as_str() else {
            return Err(consistency_error(
                "function name must be a string".to_owned(),
            ));
        };
        let Some(native_signature) = function["native"]["signature"].as_str() else {
            return Err(consistency_error(
                "function native signature must be a string".to_owned(),
            ));
        };
        let Some(native_symbol) = function["native"]["symbol"].as_str() else {
            return Err(consistency_error(
                "function native symbol must be a string".to_owned(),
            ));
        };
        let Some(native_signature_digest) = function["native"]["signature_sha256"].as_str() else {
            return Err(consistency_error(
                "function native signature_sha256 must be a string".to_owned(),
            ));
        };
        if native_signature_digest
            != domain_digest(NATIVE_SIGNATURE_DIGEST_DOMAIN, native_signature.as_bytes())
        {
            return Err(consistency_error(
                "embedded native signature digest does not match the signature text".to_owned(),
            ));
        }
        let Some(wasm_export) = function["canonical"]["export"].as_str() else {
            return Err(consistency_error(
                "function canonical export must be a string".to_owned(),
            ));
        };
        let Some(wasm_parameters) = function["canonical"]["parameters"].as_array() else {
            return Err(consistency_error(
                "function canonical parameters must be an array".to_owned(),
            ));
        };
        let mut parameters = Vec::with_capacity(wasm_parameters.len());
        for parameter in wasm_parameters {
            let Some(text) = parameter.as_str() else {
                return Err(consistency_error(
                    "canonical parameters must be strings".to_owned(),
                ));
            };
            parameters.push(text.to_owned());
        }
        let canonical_results = function["canonical"]["results"].as_array();
        let Some(result) = canonical_results
            .and_then(|results| results.first())
            .and_then(|value| value.as_str())
        else {
            return Err(consistency_error(
                "function canonical results must hold one string".to_owned(),
            ));
        };
        let rebuilt = {
            let parameter_refs: Vec<&str> = parameters.iter().map(String::as_str).collect();
            canonical_object_text(wasm_export, &parameter_refs, result)
        };
        let declared_canonical_digest = function["canonical_signature_sha256"]
            .as_str()
            .ok_or_else(|| {
                consistency_error("function canonical_signature_sha256 must be a string".to_owned())
            })?;
        if declared_canonical_digest
            != domain_digest(CANONICAL_SIGNATURE_DIGEST_DOMAIN, rebuilt.as_bytes())
        {
            return Err(consistency_error(
                "embedded canonical signature digest does not match the canonical mapping"
                    .to_owned(),
            ));
        }
        verified.push(VerifiedFunction {
            stable_id: stable_id.to_owned(),
            name: name.to_owned(),
            native_symbol: native_symbol.to_owned(),
            native_signature: native_signature.to_owned(),
            wasm_export: wasm_export.to_owned(),
            wasm_parameters: parameters,
            wasm_result: result.to_owned(),
        });
    }
    Ok(VerifiedAbiReport {
        functions: verified,
    })
}

struct ResolvedFacts {
    parameters: Vec<ScalarFacts>,
    result: ScalarFacts,
    wasm_parameters: Vec<&'static str>,
    wasm_result: &'static str,
    result_out_parameter: String,
}

fn scalar_facts(
    resolved: &ResolvedProgram,
    stable_id: &str,
) -> Result<ResolvedFacts, Vec<Diagnostic>> {
    let function = resolved
        .functions
        .iter()
        .find(|candidate| candidate.id.as_str() == stable_id)
        .ok_or_else(|| {
            vec![consistency_error(format!(
                "admitted function `{stable_id}` is absent from resolved HIR"
            ))]
        })?;
    let mut parameters = Vec::with_capacity(function.params.len());
    let mut wasm_parameters = Vec::with_capacity(function.params.len());
    for param in &function.params {
        let facts = scalar_facts_for(&param.ty)?;
        let wasm = wasm_value_type(&param.ty)?;
        parameters.push(facts);
        wasm_parameters.push(wasm);
    }
    let result = scalar_facts_for(&function.return_type)?;
    let wasm_result = wasm_value_type(&function.return_type)?;
    let result_out_parameter = format!("{} *spx_result_out", result.c_type);
    Ok(ResolvedFacts {
        parameters,
        result,
        wasm_parameters,
        wasm_result,
        result_out_parameter,
    })
}

/// Sizes and alignments come exclusively from the checked compiler layouts.
fn scalar_facts_for(ty: &ResolvedType) -> Result<ScalarFacts, Vec<Diagnostic>> {
    let (size_bytes, align_bytes) =
        aggregate_layout::scalar_size_align(aggregate_layout_target(), ty)
            .map_err(|error| vec![error])?;
    let (language_type, c_type) = match ty {
        ResolvedType::I64 => ("i64", "int64_t"),
        ResolvedType::Bool => ("bool", "bool"),
        other => {
            return Err(vec![consistency_error(format!(
                "type `{}` is outside the admitted scalar profile",
                other.identity_key()
            ))]);
        }
    };
    Ok(ScalarFacts {
        language_type,
        c_type,
        size_bytes,
        align_bytes,
    })
}

fn aggregate_layout_target() -> aggregate_layout::AggregateTarget {
    aggregate_layout::AggregateTarget::Native64
}

/// Mirror of the portable lane's Core-Wasm value-type lowering: `i64` stays
/// `i64` while `bool` narrows to `i32`.
fn wasm_value_type(ty: &ResolvedType) -> Result<&'static str, Vec<Diagnostic>> {
    match ty {
        ResolvedType::I64 => Ok("i64"),
        ResolvedType::Bool => Ok("i32"),
        other => Err(vec![consistency_error(format!(
            "type `{}` has no portable scalar mapping",
            other.identity_key()
        ))]),
    }
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
                    "abi-report selection `{token}` does not name a function in this program"
                ))]
            })?;
        if !claimed.insert(function.stable_id.as_str()) {
            return Err(vec![selection_error(format!(
                "abi-report selections `{token}` and an earlier token both resolve to `{}`",
                function.stable_id
            ))]);
        }
        selected.push(function);
    }
    Ok(selected)
}

/// Closed AST-level admission gate mirroring C Header Emission v1 and the
/// Property-Test Generation scalar profile, plus the explicit-identity
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
        if !matches!(param.ty, Type::I64 | Type::Bool) {
            return Some(REASON_UNSUPPORTED_PARAMETER_TYPE);
        }
    }
    if !matches!(function.return_type, Type::I64 | Type::Bool) {
        return Some(REASON_UNSUPPORTED_RESULT_TYPE);
    }
    None
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

/// Mirror of the portable lane's injective raw export symbol convention.
fn raw_wasm_export(stable_id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut symbol = String::with_capacity("spx_scalar_".len() + stable_id.len() * 2);
    symbol.push_str("spx_scalar_");
    for byte in stable_id.bytes() {
        symbol.push(HEX[(byte >> 4) as usize] as char);
        symbol.push(HEX[(byte & 0x0f) as usize] as char);
    }
    symbol
}

fn source_digest(source: &str) -> String {
    domain_digest(SOURCE_DIGEST_DOMAIN, source.as_bytes())
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

/// The exact canonical-object bytes whose domain-separated digest is embedded
/// beside each reported mapping; the verifier rebuilds these bytes from the
/// parsed payload fields before recomputing the digest.
fn canonical_object_text(export: &str, parameters: &[&str], result: &str) -> String {
    let parameters = parameters
        .iter()
        .map(|parameter| quote_json(parameter))
        .collect::<Vec<_>>()
        .join(",");
    bformat!(
        "{{\"profile\":\"{}\",\"export\":{},\"parameters\":[{}],\"results\":[{}],\
\"bool_boundary\":{{\"parameters\":\"{}\",\"result\":\"{}\"}},\"copy_behavior\":\"{}\"}}",
        CANONICAL_PROFILE,
        quote_json(export),
        parameters,
        quote_json(result),
        BOOL_BOUNDARY_NORMALIZATION,
        BOOL_BOUNDARY_NORMALIZATION,
        COPY_BEHAVIOR,
    )
}

fn render(
    path_text: &str,
    revision: &str,
    digest: &str,
    max_bytes: usize,
    functions_total: usize,
    reported: &[ReportedFunction],
    excluded: &[ExcludedFunction],
) -> String {
    let function_entries = reported
        .iter()
        .map(|function| {
            let native_parameters = function
                .native_parameters
                .iter()
                .enumerate()
                .map(|(index, facts)| {
                    bformat!(
                        "{{\"index\":{},\"type\":{},\"c_type\":{},\"size_bytes\":{},\
\"align_bytes\":{},\"mode\":\"value\"}}",
                        index,
                        quote_json(facts.language_type),
                        quote_json(facts.c_type),
                        facts.size_bytes,
                        facts.align_bytes,
                    )
                })
                .collect::<Vec<_>>();
            let canonical = canonical_object_text(
                &function.wasm_export,
                &function.wasm_parameters,
                function.wasm_result,
            );
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\
\"native\":{{\"target\":\"{}\",\"symbol\":{},\"signature\":{},\"signature_sha256\":{},\
\"matches_native\":true,\"parameters\":[{}],\
\"result\":{{\"type\":{},\"c_type\":{},\"size_bytes\":{},\"align_bytes\":{},\"mode\":\"value\"}},\
\"parameter_passing\":\"{}\",\"status_out_contract\":{{\"returns\":\"{}\",\
\"context_parameter\":\"{}\",\"result_out_parameter\":{},\
\"result_written_at\":\"{}\"}}}},\
\"canonical\":{},\"canonical_signature_sha256\":{}}}",
                quote_json(&function.stable_id),
                quote_json(&function.name),
                NATIVE_TARGET,
                quote_json(&function.symbol),
                quote_json(&function.signature),
                quote_json(&domain_digest(
                    NATIVE_SIGNATURE_DIGEST_DOMAIN,
                    function.signature.as_bytes(),
                )),
                native_parameters.budgeted_join(","),
                quote_json(function.native_result.language_type),
                quote_json(function.native_result.c_type),
                function.native_result.size_bytes,
                function.native_result.align_bytes,
                PARAMETER_PASSING,
                STATUS_CONTRACT_RETURNS,
                STATUS_CONTRACT_CONTEXT,
                quote_json(&function.result_out_parameter),
                STATUS_CONTRACT_RESULT_WRITTEN_AT,
                canonical,
                quote_json(&domain_digest(
                    CANONICAL_SIGNATURE_DIGEST_DOMAIN,
                    canonical.as_bytes(),
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

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"limits\":{{\"max_bytes\":{}}},\
\"selection\":{{\"requested\":{},\"functions_total\":{},\"admitted\":{},\"excluded\":{}}},\
\"functions\":[{}],\"exclusions\":[{}],\"nonclaims\":[{}]}}",
        SCHEMA,
        quote_json(path_text),
        quote_json(revision),
        quote_json(digest),
        max_bytes,
        reported.len() + excluded.len(),
        functions_total,
        reported.len(),
        excluded.len(),
        function_entries.budgeted_join(","),
        exclusion_entries.budgeted_join(","),
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
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn resolved_program(source: &str, path: &Path) -> ResolvedProgram {
        let program = parse(source, path).expect("parses");
        hir::resolve(&program).expect("resolves")
    }

    fn write_temp(source: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "semaprax-abi-report-{}-{}.spx",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::write(&path, source).unwrap();
        path
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn options_reject_out_of_bounds_values() {
        assert!(AbiReportOptions::new(Vec::new(), DEFAULT_MAX_BYTES).is_err());
        assert!(
            AbiReportOptions::new(vec!["a".to_owned(); MAX_FUNCTIONS + 1], DEFAULT_MAX_BYTES)
                .is_err()
        );
        assert!(AbiReportOptions::new(vec![String::new()], DEFAULT_MAX_BYTES).is_err());
        assert!(
            AbiReportOptions::new(vec!["x".to_owned(), "x".to_owned()], DEFAULT_MAX_BYTES).is_err()
        );
        assert!(AbiReportOptions::new(vec!["x".to_owned()], 512).is_err());
        assert!(
            AbiReportOptions::new(vec!["x".to_owned()], graph::MAX_AGENT_CONTEXT_BYTES + 1)
                .is_err()
        );
        assert!(
            AbiReportOptions::new(vec!["x".to_owned()], graph::MIN_AGENT_CONTEXT_BYTES).is_ok()
        );
    }

    #[test]
    fn symbols_match_the_backend_hex_conventions() {
        assert_eq!(c_function_symbol("app.main"), "spx_decl_6170702e6d61696e");
        assert_eq!(raw_wasm_export("math.add"), "spx_scalar_6d6174682e616464");
    }

    #[test]
    fn scalar_facts_come_from_the_checked_layouts() {
        let source = r#"
module test.facts;

@id("facts.mixed")
fn mixed(flag: bool, number: i64) -> bool { flag }

@id("app.main")
fn main() -> i64 { 0 }
"#;
        let path = write_temp(source);
        let text = std::fs::read_to_string(&path).unwrap();
        let resolved = resolved_program(&text, &path);
        let facts = scalar_facts(&resolved, "facts.mixed").expect("facts");

        let native_bool = aggregate_layout::scalar_size_align(
            aggregate_layout::AggregateTarget::Native64,
            &ResolvedType::Bool,
        )
        .unwrap();
        let wasm_bool = aggregate_layout::scalar_size_align(
            aggregate_layout::AggregateTarget::Wasm32,
            &ResolvedType::Bool,
        )
        .unwrap();
        let native_i64 = aggregate_layout::scalar_size_align(
            aggregate_layout::AggregateTarget::Native64,
            &ResolvedType::I64,
        )
        .unwrap();

        assert_eq!(
            (
                facts.parameters[0].size_bytes,
                facts.parameters[0].align_bytes
            ),
            native_bool,
            "reported bool facts must equal the checked Native64 layout"
        );
        assert_ne!(native_bool, wasm_bool);
        assert_eq!(
            (
                facts.parameters[1].size_bytes,
                facts.parameters[1].align_bytes
            ),
            native_i64,
            "reported i64 facts must equal the checked Native64 layout"
        );
        assert_eq!(facts.parameters[0].c_type, "bool");
        assert_eq!(facts.parameters[1].c_type, "int64_t");
        assert_eq!(facts.wasm_parameters, vec!["i32", "i64"]);
        assert_eq!(facts.wasm_result, "i32");
        cleanup(&path);
    }

    #[test]
    fn canonical_object_text_is_stable_and_verifier_friendly() {
        let text = canonical_object_text("spx_scalar_6d6174682e616464", &["i64", "i64"], "i64");
        assert_eq!(
            text,
            "{\"profile\":\"semaprax.wasm-scalar.v1\",\"export\":\"spx_scalar_6d6174682e616464\",\
\"parameters\":[\"i64\",\"i64\"],\"results\":[\"i64\"],\
\"bool_boundary\":{\"parameters\":\"trap_unless_canonical_0_or_1\",\"result\":\
\"trap_unless_canonical_0_or_1\"},\"copy_behavior\":\"copy\"}"
        );
        assert_eq!(
            domain_digest(CANONICAL_SIGNATURE_DIGEST_DOMAIN, text.as_bytes()),
            domain_digest(CANONICAL_SIGNATURE_DIGEST_DOMAIN, text.as_bytes())
        );
    }
}
