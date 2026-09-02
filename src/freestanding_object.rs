//! Deterministic, read-only Freestanding Object Profile v1.
//!
//! `semaprax freestanding-object <file>` derives, for one verified effect-free
//! scalar module, the complete freestanding C11 translation unit whose bytes
//! start from the production native C11 projection (`codegen::emit_c`) with
//! the documented host-process scaffolding excluded: the `int main` entry
//! wrapper, the hosted `<stdio.h>`/`<stdlib.h>` includes, and the
//! exit-code failure reporter. Two bounded substitutions are applied and
//! recorded in every envelope: the hosted stderr/abort invariant reporter is
//! replaced by a closed failstop loop, and each module function is promoted
//! from internal to external linkage so the relocatable object actually
//! exports callable symbols. Four profile assertions — no-runtime,
//! no-allocation, no-blocking, and no-libc-dependency modulo declared
//! compiler-primitive exceptions — are recomputed from explicit checks over
//! the emitted text and re-checked during independent replay.
//!
//! Diagnostics use the previously unused `SPX-A1xx` family:
//! - `SPX-A101`: invalid options (bounds, duplicates, malformed values).
//! - `SPX-A102`: module outside the freestanding scalar admission profile.
//! - `SPX-A103`: output byte-budget exhaustion (fail-closed, no truncation).
//! - `SPX-A104`: envelope or native-projection consistency failure.
//!
//! This tranche performs no MMIO/volatile/atomics lowering, offers no
//! linker-script control, models no interrupt or RTOS environment, targets no
//! board, invokes no toolchain, executes nothing, and changes no source.

use std::path::Path;

use crate::ast::{Function, ParamMode, Program, Type};
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{codegen, graph, hir, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.freestanding.v1";

const DEFAULT_MAX_BYTES: usize = 512 * 1024;

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.freestanding.payload.v1\0";
const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.freestanding.source.v1\0";
const TRANSLATION_UNIT_DIGEST_DOMAIN: &[u8] = b"semaprax.freestanding.translation-unit.v1\0";

const REASON_AUTOMATIC_IDENTITY: &str = "automatic_identity";
const REASON_GENERIC_FUNCTION: &str = "generic_function";
const REASON_DECLARED_EFFECTS: &str = "declared_effects";
const REASON_UNSUPPORTED_PARAMETER_MODE: &str = "unsupported_parameter_mode";
const REASON_UNSUPPORTED_PARAMETER_TYPE: &str = "unsupported_parameter_type";
const REASON_UNSUPPORTED_RESULT_TYPE: &str = "unsupported_result_type";

const NONCLAIMS_JSON: &str = "\"no_mmio_volatile_or_atomics_support\",\
\"no_linker_script_control\",\
\"no_hardware_or_emulator_execution\",\
\"no_interrupt_or_rtos_model\",\
\"no_board_targets\",\
\"relocatable_object_for_effect_free_scalar_profiles_only\",\
\"no_toolchain_invocation_by_this_command\",\
\"read_only\"";

const SCAFFOLDING_EXCLUSIONS_JSON: &str =
    "\"entry_wrapper\",\"stdio_include\",\"stdlib_include\",\"public_failure_reporter\"";
const SCAFFOLDING_SUBSTITUTIONS_JSON: &str = "\"invariant_failstop\",\"external_function_linkage\"";
const ALLOWED_SYMBOLS_JSON: &str = "\
{\"symbol\":\"memcpy\",\"justification\":\"compiler-emitted memory primitive that ISO C \
freestanding environments are expected to provide\"},\
{\"symbol\":\"strcmp\",\"justification\":\"production status-runtime schema and domain \
validation kept verbatim from the native lane\"}";
const OBJECT_RECIPE_JSON: &str = "{\"command_compiles_nothing\":true,\
\"compiler_flags\":[\"-std=c11\",\"-O0\",\"-ffreestanding\",\"-nostdlib\",\
\"-fno-stack-protector\",\"-D_FORTIFY_SOURCE=0\",\"-c\"]}";

/// Exact original invariant-reporter text in the production native projection.
const INVARIANT_FAILURE_ORIGINAL: &str = "static __attribute__((noreturn, unused)) void spx_runtime_invariant_failure(\n    const char *message\n) {\n    fprintf(stderr, \"SEMAPRAX native runtime invariant failure: %s\\n\", message);\n    abort();\n}";

/// Freestanding replacement: same signature, closed failstop instead of the
/// hosted stderr/abort reporter.
const INVARIANT_FAILURE_FAILSTOP: &str = "static __attribute__((noreturn, unused)) void spx_runtime_invariant_failure(\n    const char *message\n) {\n    /* SEMAPRAX freestanding profile: hosted diagnostics are excluded. */\n    (void)message;\n    for (;;) {\n    }\n}";

const PUBLIC_FAILURE_ANCHOR: &str = "static __attribute__((unused)) int spx_public_failure(";
const STDIO_INCLUDE: &str = "#include <stdio.h>\n\n";
const STDLIB_INCLUDE: &str = "#include <stdlib.h>\n";
const ENTRY_WRAPPER_START: &str = "#ifndef SPX_NO_ENTRY_WRAPPER";
const ENTRY_WRAPPER_END: &str = "#endif\n";
const EXTERNAL_LINKAGE_PREFIX: &str = "static __attribute__((unused)) spx_status_token spx_decl_";

const NO_RUNTIME_FORBIDDEN: &[&str] = &[
    "int main(",
    ENTRY_WRAPPER_START,
    "<stdio.h>",
    "<stdlib.h>",
    "printf",
    "fprintf",
    "fputs",
    "stderr",
    "abort(",
    "spx_public_failure",
];
const NO_ALLOCATION_FORBIDDEN: &[&str] = &[
    "malloc",
    "calloc",
    "realloc",
    "aligned_alloc",
    "alloca(",
    "free(",
];
const NO_BLOCKING_FORBIDDEN: &[&str] = &[
    "sleep",
    "nanosleep",
    "pthread_",
    "thrd_",
    "waitpid",
    "sched_yield",
    "flock(",
    "recv(",
    "select(",
    "poll(",
];
const NO_LIBC_DEPENDENCY_FORBIDDEN: &[&str] = &[
    "<stdio.h>",
    "<stdlib.h>",
    "printf",
    "fprintf",
    "fputs",
    "stderr",
    "abort(",
    "spx_public_failure",
    "exit(",
    "getenv",
    "atexit",
    "system(",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreestandingObjectOptions {
    pub max_bytes: usize,
}

impl FreestandingObjectOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "freestanding-object max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for FreestandingObjectOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-A101", message)
}

fn admission_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-A102", message)
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-A104", message)
}

struct EmittedFunction {
    stable_id: String,
    name: String,
    symbol: String,
}

struct Generation {
    envelope: String,
    unit: String,
}

/// One independently authenticated replay of [`verify_envelope`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedFreestandingObject {
    pub translation_unit: String,
}

/// Generate the canonical `semaprax.freestanding.v1` envelope JSON for one
/// verified effect-free scalar module.
///
/// Read-only: source bytes must remain unchanged between the snapshot and the
/// final check or generation fails closed.
pub fn generate(
    source_path: &Path,
    options: &FreestandingObjectOptions,
) -> Result<String, Vec<Diagnostic>> {
    generate_internal(source_path, options).map(|generation| generation.envelope)
}

/// Generate only the bare deterministic translation-unit bytes under the same
/// admission rules as [`generate`].
pub fn unit_text(
    source_path: &Path,
    options: &FreestandingObjectOptions,
) -> Result<String, Vec<Diagnostic>> {
    generate_internal(source_path, options).map(|generation| generation.unit)
}

/// Independently verify one envelope produced by [`generate`].
///
/// Recomputes the outer payload digest over the exact serialized payload
/// bytes, re-checks the declared byte count, re-authenticates the embedded
/// translation-unit digest, and replays every profile assertion against the
/// embedded text before returning it.
pub fn verify_envelope(envelope: &str) -> Result<VerifiedFreestandingObject, Diagnostic> {
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
    let Some(unit) = payload_value["translation_unit"].as_str() else {
        return Err(consistency_error(
            "payload translation_unit must be a string".to_owned(),
        ));
    };
    let Some(unit_digest) = payload_value["translation_unit_sha256"].as_str() else {
        return Err(consistency_error(
            "payload translation_unit_sha256 must be a string".to_owned(),
        ));
    };
    if unit_digest != domain_digest(TRANSLATION_UNIT_DIGEST_DOMAIN, unit.as_bytes()) {
        return Err(consistency_error(
            "embedded translation-unit digest does not match the translation-unit text".to_owned(),
        ));
    }
    replay_profile_assertions(&payload_value, unit)?;
    Ok(VerifiedFreestandingObject {
        translation_unit: unit.to_owned(),
    })
}

fn generate_internal(
    source_path: &Path,
    options: &FreestandingObjectOptions,
) -> Result<Generation, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

    admit_module(&program)?;

    let mut functions: Vec<EmittedFunction> = program
        .functions
        .iter()
        .map(|function| EmittedFunction {
            stable_id: function.stable_id.clone(),
            name: function.name.clone(),
            symbol: c_function_symbol(&function.stable_id),
        })
        .collect();
    functions.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));

    let entry_point = hir::resolve(&program)
        .map_err(|mut errors| {
            let first = if let Some(index) = errors.iter().position(|item| item.severity.is_error())
            {
                errors.swap_remove(index)
            } else {
                errors.pop().unwrap_or_else(|| {
                    Diagnostic::io("SPX-A104", "HIR resolution failed without a diagnostic")
                })
            };
            vec![first]
        })?
        .entrypoint
        .as_str()
        .to_owned();

    let native_text = codegen::emit_c(&program).map_err(|error| vec![error])?;
    let unit = freestanding_translation_unit(&native_text, functions.len())?;

    let digest = source_digest(snapshot.source());
    let path_text = source_path.display().to_string();

    let (generation, overflowed) = with_limit(options.max_bytes, || {
        render(
            &path_text,
            &revision,
            &digest,
            options.max_bytes,
            program.functions.len(),
            &entry_point,
            &functions,
            &unit,
        )
    });
    if overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-A103",
            "freestanding-object output exceeds the max-bytes budget; refusing to truncate"
                .to_owned(),
        )]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(generation)
}

/// Whole-module admission gate: one verified effect-free scalar module.
fn admit_module(program: &Program) -> Result<(), Vec<Diagnostic>> {
    if !program.permits.is_empty() {
        return Err(vec![admission_error(format!(
            "freestanding modules must declare no capability permits; found {:?}",
            program.permits
        ))]);
    }
    if !program.interfaces.is_empty() {
        return Err(vec![admission_error(
            "freestanding modules must declare no interfaces or imports".to_owned(),
        )]);
    }
    if !program.module_uses.is_empty() {
        return Err(vec![admission_error(
            "freestanding modules must use no external declarations".to_owned(),
        )]);
    }
    for function in &program.functions {
        if let Some(reason) = admission(function) {
            return Err(vec![admission_error(format!(
                "freestanding function `{}` is outside the scalar profile: {reason}",
                function.stable_id
            ))]);
        }
    }
    if !program.types.is_empty() {
        return Err(vec![admission_error(
            "freestanding profile admits scalar modules only; type declarations are rejected"
                .to_owned(),
        )]);
    }
    Ok(())
}

/// Closed AST-level admission gate mirroring C Header Emission v1 and the
/// Canonical ABI Report scalar profile, plus the explicit-identity requirement.
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

/// Derive the freestanding translation unit from the production native C11
/// projection. Every edit is anchored on exact unique markers; drift in the
/// production lane fails closed instead of producing stale artifacts.
fn freestanding_translation_unit(
    native_text: &str,
    function_count: usize,
) -> Result<String, Vec<Diagnostic>> {
    if native_text.matches(INVARIANT_FAILURE_ORIGINAL).count() != 1 {
        return Err(vec![consistency_error(
            "native projection has no unique invariant-reporter block".to_owned(),
        )]);
    }
    let mut unit = native_text.replace(INVARIANT_FAILURE_ORIGINAL, INVARIANT_FAILURE_FAILSTOP);
    remove_once(&mut unit, STDIO_INCLUDE, "the stdio include")?;
    remove_once(&mut unit, STDLIB_INCLUDE, "the stdlib include")?;
    let reporter_start = unit.find(PUBLIC_FAILURE_ANCHOR);
    let reporter_end = reporter_start.and_then(|start| {
        unit[start..]
            .find("\n}\n\n")
            .map(|offset| start + offset + "\n}\n\n".len())
    });
    match (reporter_start, reporter_end) {
        (Some(start), Some(end)) => unit.replace_range(start..end, ""),
        _ => {
            return Err(vec![consistency_error(
                "native projection has no public-failure reporter block".to_owned(),
            )])
        }
    }
    let wrapper_start = unit.find(ENTRY_WRAPPER_START);
    match wrapper_start {
        Some(start) if unit[start..].ends_with(ENTRY_WRAPPER_END) => {
            unit.truncate(start);
        }
        _ => {
            return Err(vec![consistency_error(
                "native projection has no trailing entry-wrapper block".to_owned(),
            )])
        }
    }
    let promoted = promote_external_linkage(&mut unit)?;
    if promoted != function_count * 2 {
        return Err(vec![consistency_error(format!(
            "native projection promotes {promoted} linkage sites but {} module functions are admitted",
            function_count
        ))]);
    }
    run_profile_assertions(&unit).map_err(|error| {
        vec![consistency_error(format!(
            "emitted translation unit violates its own profile assertion: {error}"
        ))]
    })?;
    Ok(unit)
}

fn remove_once(text: &mut String, needle: &str, label: &str) -> Result<(), Vec<Diagnostic>> {
    let count = text.matches(needle).count();
    if count != 1 {
        return Err(vec![consistency_error(format!(
            "native projection has {count} occurrences of {label}; expected exactly one"
        ))]);
    }
    *text = text.replace(needle, "");
    Ok(())
}

/// Rewrite both the prototype line and the definition line of every module
/// function from internal to external linkage; runtime helpers stay static.
fn promote_external_linkage(unit: &mut String) -> Result<usize, Vec<Diagnostic>> {
    let mut promoted = 0usize;
    let ends_with_newline = unit.ends_with('\n');
    let rebuilt: Vec<String> = unit
        .lines()
        .map(|line| match line.strip_prefix(EXTERNAL_LINKAGE_PREFIX) {
            Some(rest) => {
                promoted += 1;
                format!("spx_status_token spx_decl_{rest}")
            }
            None => line.to_owned(),
        })
        .collect();
    *unit = rebuilt.join("\n");
    if ends_with_newline {
        unit.push('\n');
    }
    Ok(promoted)
}

/// Explicit textual checks backing the four profile assertions.
fn run_profile_assertions(unit: &str) -> Result<(), &'static str> {
    for token in NO_RUNTIME_FORBIDDEN {
        if unit.contains(token) {
            return Err("no_runtime");
        }
    }
    for token in NO_ALLOCATION_FORBIDDEN {
        if unit.contains(token) {
            return Err("no_allocation");
        }
    }
    for token in NO_BLOCKING_FORBIDDEN {
        if unit.contains(token) {
            return Err("no_blocking");
        }
    }
    for token in NO_LIBC_DEPENDENCY_FORBIDDEN {
        if unit.contains(token) {
            return Err("no_libc_dependency");
        }
    }
    if !unit.contains(INVARIANT_FAILURE_FAILSTOP) {
        return Err("no_runtime");
    }
    Ok(())
}

fn replay_profile_assertions(
    payload_value: &serde_json::Value,
    unit: &str,
) -> Result<(), Diagnostic> {
    let Some(assertions) = payload_value["profile_assertions"].as_object() else {
        return Err(consistency_error(
            "payload profile_assertions must be an object".to_owned(),
        ));
    };
    let expected = [
        "no_allocation",
        "no_blocking",
        "no_libc_dependency",
        "no_runtime",
    ];
    let keys: Vec<&str> = assertions.keys().map(String::as_str).collect();
    if keys != expected {
        return Err(consistency_error(format!(
            "profile_assertions keys must be exactly {expected:?}, found {keys:?}"
        )));
    }
    for key in expected {
        if assertions[key].as_bool() != Some(true) {
            return Err(consistency_error(format!(
                "profile assertion `{key}` must be true"
            )));
        }
    }
    run_profile_assertions(unit).map_err(|failed| {
        consistency_error(format!(
            "replayed translation unit fails its `{failed}` profile assertion"
        ))
    })
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

#[allow(clippy::too_many_arguments)]
fn render(
    path_text: &str,
    revision: &str,
    digest: &str,
    max_bytes: usize,
    functions_total: usize,
    entry_point: &str,
    functions: &[EmittedFunction],
    unit: &str,
) -> Generation {
    let function_entries = functions
        .iter()
        .map(|function| {
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"symbol\":{}}}",
                quote_json(&function.stable_id),
                quote_json(&function.name),
                quote_json(&function.symbol),
            )
        })
        .collect::<Vec<_>>();
    let unit_sha256 = domain_digest(TRANSLATION_UNIT_DIGEST_DOMAIN, unit.as_bytes());

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"limits\":{{\"max_bytes\":{}}},\
\"module\":{{\"functions_total\":{},\"admitted\":{},\"entry_point\":{}}},\
\"functions\":[{}],\
\"profile_assertions\":{{\"no_allocation\":true,\"no_blocking\":true,\
\"no_libc_dependency\":true,\"no_runtime\":true}},\
\"allowed_undefined_symbols\":[{}],\
\"scaffolding_exclusions\":[{}],\
\"scaffolding_substitutions\":[{}],\
\"object_recipe\":{},\
\"translation_unit_sha256\":{},\"translation_unit\":{},\"nonclaims\":[{}]}}",
        SCHEMA,
        quote_json(path_text),
        quote_json(revision),
        quote_json(digest),
        max_bytes,
        functions_total,
        functions.len(),
        quote_json(entry_point),
        function_entries.budgeted_join(","),
        ALLOWED_SYMBOLS_JSON,
        SCAFFOLDING_EXCLUSIONS_JSON,
        SCAFFOLDING_SUBSTITUTIONS_JSON,
        OBJECT_RECIPE_JSON,
        quote_json(&unit_sha256),
        quote_json(unit),
        NONCLAIMS_JSON,
    );
    let envelope = bformat!(
        "{{\"schema\":\"{}\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        SCHEMA,
        quote_json(&domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload,
    );
    Generation {
        envelope,
        unit: unit.to_owned(),
    }
}

#[cfg(test)]
#[path = "freestanding_object/tests.rs"]
mod tests;
