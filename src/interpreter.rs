//! Deterministic Reference Interpreter v1 over verified HIR.
//!
//! [`interpret`] evaluates ONE explicitly selected explicit-ID monomorphic
//! effect-free scalar function directly from the resolved HIR of one verified
//! single-file SEMAPRAX module — no backend toolchain, no code generation, no
//! target execution — and emits one canonical compact JSON envelope
//! (`semaprax.interpret.v1`) whose outcome is either the returned value or
//! the exact compiler-owned normalized failure status, together with fuel
//! accounting (steps used versus budget), an argument echo, and the source
//! digest. Evaluation reuses the existing checked-arithmetic semantics: every
//! integer overflow, division/remainder fault, and false contract clause is
//! reported through `runtime_status` normalization (`semaprax.status.v1`)
//! exactly as the native C11 and Core-Wasm backends report it.
//!
//! The admission profile is closed: the selected function (and every callee
//! reachable from it) must have an explicit stable identity, be monomorphic,
//! declare no effects, take only by-value direct parameters of the admitted
//! scalar types (`i64`, `i32`, `u8`, `char`, `f32`, `f64`, `bool`), and
//! return one direct value of those same types — mixed scalar signatures are
//! admitted. Function bodies may use the admitted
//! scalar surface — `let` (including `let mut`) and assignment statements,
//! blocks, `if`, lazy `&&`/`||`, unary negation/logical not, all admitted
//! binary operators with left-to-right evaluation and sticky failure
//! selection, `i64`/`i32`/`u8`/`char`/`f32`/`f64`/`bool` literals,
//! requires/ensures contracts, and calls to other admitted functions — and
//! nothing else. Aggregate construction/projection/update, variant
//! construction, matching, postfix `?`, import calls, generic calls, place
//! projections, strings at the boundary, and backend-unlowerable scalar
//! operations (`f32`/`f64`/`u8`
//! remainder, `char` arithmetic) are rejected with one closed reason before
//! any evaluation.
//!
//! Two fixed interpreter-capacity limits are fail-closed and never language
//! statuses: a step budget (fuel; each expression node, statement, and
//! contract clause consumes exactly one step) and a call-depth ceiling.
//! Exhausting either stops evaluation and reports a dedicated outcome kind
//! inside the envelope.
//!
//! Diagnostics use the previously unused `SPX-F1xx` family:
//! - `SPX-F101`: invalid options (bounds, malformed values).
//! - `SPX-F102`: function selection or admission failure (closed reason).
//! - `SPX-F103`: argument count/type/literal mismatch.
//! - `SPX-F104`: output byte-budget exhaustion (fail-closed, no truncation).
//! - `SPX-F105`: fail-closed evaluation guard for an impossible post-verify
//!   state (never expected on verified programs).
//! - `SPX-F106`: envelope consistency or replay failure.
//!
//! This tranche contains no JIT/AOT/Cranelift machinery, no incremental
//! persistence, no hot reload, no debugger mapping, executes no target, and
//! changes no source.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::ast::{BinaryOp, Function, ParamMode, Program, Type, UnaryOp};
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::cleanup_plan::{ContractPhase, StatusCase};
use crate::conformance::NormalizedStatus;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedStatement, ResolvedType, ValueId,
};
use crate::runtime_status::{normalize_arithmetic, normalize_contract};
use crate::{graph, hir, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.interpret.v1";

const DEFAULT_MAX_BYTES: usize = 64 * 1024;

/// Default interpreter fuel budget; each evaluated expression node, statement,
/// and contract clause consumes exactly one step.
pub const DEFAULT_MAX_STEPS: usize = 1_000_000;

/// Hard upper bound for the library-level step budget option.
pub const MAX_STEPS_LIMIT: usize = 100_000_000;

/// Fixed call-depth ceiling; exceeding it is an interpreter-capacity outcome,
/// never a language status.
pub const MAX_CALL_DEPTH: usize = 256;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.interpret.source.v1\0";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.interpret.payload.v1\0";

const REASON_AUTOMATIC_IDENTITY: &str = "automatic_identity";
const REASON_GENERIC_FUNCTION: &str = "generic_function";
const REASON_DECLARED_EFFECTS: &str = "declared_effects";
const REASON_UNSUPPORTED_PARAMETER_MODE: &str = "unsupported_parameter_mode";
const REASON_UNSUPPORTED_PARAMETER_TYPE: &str = "unsupported_parameter_type";
const REASON_UNSUPPORTED_RESULT_TYPE: &str = "unsupported_result_type";
const REASON_GENERIC_CALL: &str = "generic_call";
const REASON_IMPORT_CALL: &str = "import_call";
const REASON_RECORD_CONSTRUCTION: &str = "record_construction";
const REASON_VARIANT_CONSTRUCTION: &str = "variant_construction";
const REASON_RECORD_UPDATE: &str = "record_update";
const REASON_RECORD_PROJECTION: &str = "record_projection";
const REASON_MATCH_EXPRESSION: &str = "match_expression";
const REASON_TRY_EXPRESSION: &str = "try_expression";
const REASON_PLACE_PROJECTION: &str = "place_projection";
const REASON_UNSUPPORTED_CALLEE: &str = "unsupported_callee";
const REASON_UNSUPPORTED_SCALAR_OPERATION: &str = "unsupported_scalar_operation";
const REASON_UNSAFE_BOUNDARY: &str = "unsafe_boundary";

const OUTCOME_RETURNED: &str = "returned";
const OUTCOME_FAILED: &str = "failed";
const OUTCOME_FUEL_EXHAUSTED: &str = "fuel_exhausted";
const OUTCOME_CALL_DEPTH_EXCEEDED: &str = "call_depth_exceeded";

const NONCLAIMS_JSON: &str = "\"no_jit_aot_or_cranelift\",\
\"no_incremental_persistence\",\
\"no_hot_reload\",\
\"no_debugger_mapping\",\
\"no_target_execution\",\
\"read_only_evaluation_only\"";
const NONCLAIMS_LIST: [&str; 6] = [
    "no_jit_aot_or_cranelift",
    "no_incremental_persistence",
    "no_hot_reload",
    "no_debugger_mapping",
    "no_target_execution",
    "read_only_evaluation_only",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterpreterOptions {
    pub max_bytes: usize,
    pub max_steps: usize,
}

impl InterpreterOptions {
    pub fn new(max_bytes: usize, max_steps: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "interpret max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
            return Err(option_error(format!(
                "interpret max_steps must be between 1 and {MAX_STEPS_LIMIT}"
            )));
        }
        Ok(Self {
            max_bytes,
            max_steps,
        })
    }
}

impl Default for InterpreterOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            max_steps: DEFAULT_MAX_STEPS,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-F101", message)
}

fn selection_error(reason: &str, detail: String) -> Diagnostic {
    Diagnostic::io(
        "SPX-F102",
        format!("interpreter admission failed ({reason}): {detail}"),
    )
}

fn argument_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-F103", message)
}

fn guard_error(detail: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-F105",
        format!("interpreter refused to continue on an impossible post-verify state: {detail}"),
    )
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-F106", message)
}

/// One CLI-level argument literal: one admitted scalar value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArgumentValue {
    Int(i64),
    Int32(i32),
    Uint8(u8),
    Char(u32),
    Float32(f32),
    Float64(f64),
    Bool(bool),
}

impl ArgumentValue {
    fn type_text(self) -> &'static str {
        match self {
            Self::Int(_) => "i64",
            Self::Int32(_) => "i32",
            Self::Uint8(_) => "u8",
            Self::Char(_) => "char",
            Self::Float32(_) => "f32",
            Self::Float64(_) => "f64",
            Self::Bool(_) => "bool",
        }
    }

    fn render(self) -> String {
        match self {
            Self::Int(value) => value.to_string(),
            // Suffixed widths always render with their explicit suffix: bare
            // decimals canonically denote `i64`, so the suffix is what keeps
            // each rendering uniquely replayable.
            Self::Int32(value) => format!("{value}i32"),
            Self::Uint8(value) => format!("{value}u8"),
            Self::Char(value) => crate::format::canonical_char(value),
            Self::Float32(value) => format!("{:08x}", value.to_bits()),
            Self::Float64(value) => format!("{:016x}", value.to_bits()),
            Self::Bool(value) => value.to_string(),
        }
    }
}

/// Parses one canonical `--arg` literal for the widened scalar surface:
///
/// - `true`/`false`;
/// - a canonical optionally negative decimal integer that fits `i64`;
/// - the same integer with an explicit `i32` or `u8` suffix (exactly the
///   suffixes the language lexer admits; there is deliberately no `i64`
///   suffix), e.g. `7i32` or `200u8`;
/// - a floating-point literal in the language grammar — digits, a required
///   fraction, an optional exponent, and an optional `f32`/`f64` suffix —
///   whose value must be finite, e.g. `1.5`, `-0.0`, `2.5e-3`, `0.25f32`;
/// - a `char` literal in the language's escape syntax: one Unicode scalar
///   between single quotes, with the named escapes `\n`, `\r`, `\t`, `\0`,
///   `\'`, `\\` and `\u{...}` carrying one to six hexadecimal digits.
///
/// Non-canonical or out-of-range literals fail closed (`SPX-F103`).
pub fn parse_argument(text: &str) -> Result<ArgumentValue, Diagnostic> {
    if text == "true" {
        return Ok(ArgumentValue::Bool(true));
    }
    if text == "false" {
        return Ok(ArgumentValue::Bool(false));
    }
    if text.starts_with('\'') {
        return parse_char_argument(text);
    }
    let (sign, remainder) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text),
    };
    let digits_end = remainder
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    let (digits, tail) = remainder.split_at(digits_end);
    if digits.is_empty()
        || (digits.len() > 1 && digits.starts_with('0'))
        || tail.starts_with(|character: char| character.is_ascii_digit())
    {
        return Err(argument_error(format!(
            "argument `{text}` is not a scalar literal"
        )));
    }
    if let Some(fraction) = tail.strip_prefix('.') {
        return parse_float_argument(text, sign, digits, fraction);
    }
    match tail {
        "" => format!("{sign}{digits}")
            .parse::<i64>()
            .map(ArgumentValue::Int)
            .map_err(|_| argument_error(format!("argument `{text}` does not fit i64"))),
        "i32" => format!("{sign}{digits}")
            .parse::<i32>()
            .map(ArgumentValue::Int32)
            .map_err(|_| argument_error(format!("argument `{text}` is outside the i32 range"))),
        "u8" => {
            if sign == "-" {
                return Err(argument_error(format!(
                    "argument `{text}` is outside the u8 range"
                )));
            }
            digits
                .parse::<u8>()
                .map(ArgumentValue::Uint8)
                .map_err(|_| argument_error(format!("argument `{text}` is outside the u8 range")))
        }
        _ => Err(argument_error(format!(
            "argument `{text}` is not a scalar literal"
        ))),
    }
}

/// Parses the character-literal form of one [`parse_argument`] input using
/// exactly the language's escape vocabulary.
fn parse_char_argument(text: &str) -> Result<ArgumentValue, Diagnostic> {
    let invalid = || argument_error(format!("argument `{text}` is not a char literal"));
    let mut characters = text.chars();
    if characters.next() != Some('\'') {
        return Err(invalid());
    }
    let value = match characters.next() {
        Some('\\') => match characters.next() {
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('0') => '\0',
            Some('\'') => '\'',
            Some('\\') => '\\',
            Some('u') => {
                if characters.next() != Some('{') {
                    return Err(invalid());
                }
                let mut digits = String::new();
                for character in characters.by_ref() {
                    if character == '}' {
                        break;
                    }
                    if !character.is_ascii_hexdigit() || digits.len() >= 6 {
                        return Err(invalid());
                    }
                    digits.push(character);
                }
                if digits.is_empty() {
                    return Err(invalid());
                }
                let scalar = u32::from_str_radix(&digits, 16).map_err(|_| invalid())?;
                char::from_u32(scalar).ok_or_else(invalid)?
            }
            _ => return Err(invalid()),
        },
        Some(character) if character != '\'' => character,
        _ => return Err(invalid()),
    };
    if characters.next() != Some('\'') || characters.next().is_some() {
        return Err(invalid());
    }
    Ok(ArgumentValue::Char(value as u32))
}

/// Parses the floating-point form of one [`parse_argument`] input: a required
/// fraction, an optional exponent, and an optional `f32`/`f64` suffix, with a
/// single rounding from decimal digits in the declared precision.
fn parse_float_argument(
    text: &str,
    sign: &str,
    digits: &str,
    fraction_and_more: &str,
) -> Result<ArgumentValue, Diagnostic> {
    let invalid = |detail: &str| {
        argument_error(format!(
            "argument `{text}` is not a float literal ({detail})"
        ))
    };
    let fraction_end = fraction_and_more
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    let (fraction, mut tail) = fraction_and_more.split_at(fraction_end);
    if fraction.is_empty() {
        return Err(invalid("fraction requires at least one digit"));
    }
    let mut body = format!("{sign}{digits}.{fraction}");
    if matches!(tail.as_bytes().first(), Some(b'e') | Some(b'E')) {
        let mut exponent = &tail[1..];
        let negative_exponent = matches!(exponent.as_bytes().first(), Some(b'+') | Some(b'-'));
        if negative_exponent {
            exponent = &exponent[1..];
        }
        let exponent_digits = exponent
            .bytes()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if exponent_digits == 0 {
            return Err(invalid("exponent requires at least one digit"));
        }
        body.push_str(&tail[..1 + usize::from(negative_exponent) + exponent_digits]);
        tail = &tail[1 + usize::from(negative_exponent) + exponent_digits..];
    }
    let wide = match tail {
        "" | "f64" => true,
        "f32" => false,
        _ => return Err(invalid("only `f32` and `f64` suffixes are admitted")),
    };
    if wide {
        match body.parse::<f64>() {
            Ok(value) if value.is_finite() => Ok(ArgumentValue::Float64(value)),
            _ => Err(invalid("literal is outside the declared float range")),
        }
    } else {
        match body.parse::<f32>() {
            Ok(value) if value.is_finite() => Ok(ArgumentValue::Float32(value)),
            _ => Err(invalid("literal is outside the declared float range")),
        }
    }
}

/// One completed interpretation: the authenticated envelope plus whether the
/// outcome was a returned value (`true`) or any failure/capacity outcome
/// (`false`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Interpretation {
    pub envelope: String,
    pub returned: bool,
}

/// Deterministic facts from evaluating one already-validated resolved entry.
///
/// This is the reusable, artifact-free execution seam for authenticated
/// project snapshots. It carries no source, filesystem, process, or backend
/// authority: the caller owns validation and supplies the exact resolved
/// program plus its entry identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedEvaluation {
    pub outcome: ResolvedEvaluationOutcome,
    pub steps_used: usize,
    pub max_steps: usize,
}

/// Closed outcomes for the zero-argument `i64` resolved-entry profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedEvaluationOutcome {
    ReturnedI64(i64),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    /// An impossible state was observed after the caller's HIR validation.
    GuardError(String),
}

/// Evaluate one exact zero-argument `i64` entry from already-validated HIR.
///
/// The selected identity must equal `program.entrypoint`; this API never
/// redirects execution to another function in the closure. The admitted
/// closure is checked again against the deterministic interpreter surface,
/// but the HIR itself is not rebuilt, parsed, or re-resolved. Evaluation runs
/// on the same fixed-size stack as [`interpret`] and performs no I/O.
pub(crate) fn evaluate_resolved_zero_arg_i64(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    max_steps: usize,
) -> Result<ResolvedEvaluation, Vec<Diagnostic>> {
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "resolved evaluation max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        ))]);
    }
    if program.entrypoint.as_str() != entry_id {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!(
                "selection `{entry_id}` is not the resolved entry point `{}`",
                program.entrypoint
            ),
        )]);
    }
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == entry_id)
        .ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("resolved entry `{entry_id}` is absent from the function index"),
            )]
        })?;
    let explicit_entry = program
        .declarations
        .declaration(&entry.id)
        .is_some_and(|declaration| declaration.identity_origin == hir::IdentityOrigin::Explicit);
    if !explicit_entry {
        return Err(vec![selection_error(
            REASON_AUTOMATIC_IDENTITY,
            format!("resolved entry `{entry_id}` does not have an explicit stable identity"),
        )]);
    }
    if !entry.params.is_empty() || entry.return_type != ResolvedType::I64 {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!("resolved entry `{entry_id}` must have type `fn main() -> i64`"),
        )]);
    }
    if !resolved_signature_is_admitted(entry) {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("resolved entry `{entry_id}` is outside the interpreter profile"),
        )]);
    }

    let admitted = admitted_resolved_functions(program);
    scan_closure(entry_id, &admitted)?;

    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("semaprax-resolved-evaluate".to_owned())
            .stack_size(EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, || {
                let (outcome, steps_used) =
                    evaluate_resolved_entry(entry, &[], &admitted, max_steps);
                let outcome = match outcome {
                    Ok(Value::Int(value)) => ResolvedEvaluationOutcome::ReturnedI64(value),
                    Ok(_) => ResolvedEvaluationOutcome::GuardError(
                        "zero-argument i64 entry returned a non-i64 value".to_owned(),
                    ),
                    Err(Flow::Failure(status)) => {
                        ResolvedEvaluationOutcome::LanguageFailure(status)
                    }
                    Err(Flow::Exhausted) => ResolvedEvaluationOutcome::FuelExhausted,
                    Err(Flow::DepthExceeded) => ResolvedEvaluationOutcome::CallDepthExceeded,
                    Err(Flow::Guard(detail)) => {
                        ResolvedEvaluationOutcome::GuardError(detail.to_owned())
                    }
                };
                ResolvedEvaluation {
                    outcome,
                    steps_used,
                    max_steps,
                }
            })
            .map_err(|error| {
                vec![guard_error(&format!(
                    "resolved evaluation thread failed to start: {error}"
                ))]
            })?;
        worker.join().map_err(|_| {
            vec![guard_error(
                "resolved evaluation thread panicked after HIR validation",
            )]
        })
    })
}

/// Interpret one selected function of one verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and the
/// final check or the whole command fails closed.
pub fn interpret(
    source_path: &Path,
    function_token: &str,
    arguments: &[String],
    options: &InterpreterOptions,
) -> Result<Interpretation, Vec<Diagnostic>> {
    let options_owned = *options;
    // Evaluation recurses per SPX call frame; a dedicated thread with a fixed
    // generous stack keeps the call-depth ceiling reachable without native
    // stack exhaustion. The thread changes nothing about the output bytes.
    let source_path = source_path.to_path_buf();
    let function_token = function_token.to_owned();
    let arguments = arguments.to_vec();
    let stack = std::thread::Builder::new()
        .name("semaprax-interpret".to_owned())
        .stack_size(EVALUATION_STACK_BYTES)
        .spawn(move || {
            interpret_on_current_thread(&source_path, &function_token, &arguments, &options_owned)
        })
        .map_err(|error| {
            vec![Diagnostic::io(
                "SPX-F105",
                format!("interpreter evaluation thread failed to start: {error}"),
            )]
        })?;
    stack
        .join()
        .unwrap_or_else(|_| Err(vec![guard_error("evaluation thread panicked")]))
}

/// Fixed evaluation-thread stack size (64 MiB).
pub const EVALUATION_STACK_BYTES: usize = 64 * 1024 * 1024;

fn interpret_on_current_thread(
    source_path: &Path,
    function_token: &str,
    arguments: &[String],
    options: &InterpreterOptions,
) -> Result<Interpretation, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

    let function = select_function(&program, function_token)?;
    if let Some(reason) = admission(function) {
        return Err(vec![selection_error(
            reason,
            format!("function `{}`", function.name),
        )]);
    }
    let parsed_arguments = bind_arguments(function, arguments)?;

    let resolved = hir::resolve(&program)?;
    let entry = resolved
        .functions
        .iter()
        .find(|candidate| candidate.id.as_str() == function.stable_id)
        .ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!(
                    "admitted function `{}` is absent from resolved HIR",
                    function.stable_id
                ),
            )]
        })?;

    let mut admitted: BTreeMap<&str, &ResolvedFunction> = BTreeMap::new();
    for candidate in &resolved.functions {
        if let Some(ast_function) = program
            .functions
            .iter()
            .find(|item| item.stable_id == candidate.id.as_str())
        {
            if admission(ast_function).is_none() {
                admitted.insert(candidate.id.as_str(), candidate);
            }
        }
    }

    scan_closure(entry.id.as_str(), &admitted)?;

    let (evaluated, steps_used) =
        evaluate_resolved_entry(entry, &parsed_arguments, &admitted, options.max_steps);
    let outcome = match evaluated {
        Ok(value) => returned_outcome(&value),
        Err(flow) => match flow {
            Flow::Failure(status) => failed_outcome(&status.to_json()),
            Flow::Exhausted => capacity_outcome(OUTCOME_FUEL_EXHAUSTED),
            Flow::DepthExceeded => capacity_outcome(OUTCOME_CALL_DEPTH_EXCEEDED),
            Flow::Guard(detail) => return Err(vec![guard_error(detail)]),
        },
    };

    let digest = source_digest(snapshot.source());
    let path_text = source_path.display().to_string();
    let arguments_json = parsed_arguments
        .iter()
        .enumerate()
        .map(|(index, (name, value))| {
            bformat!(
                "{{\"index\":{},\"name\":{},\"type\":{},\"value\":{}}}",
                index,
                quote_json(name),
                quote_json(value.type_text()),
                quote_json(&value.render()),
            )
        })
        .collect::<Vec<_>>();
    let exhausted = outcome.kind == OUTCOME_FUEL_EXHAUSTED;

    let (envelope, overflowed) = with_limit(options.max_bytes, || {
        render(&RenderFacts {
            path_text: &path_text,
            revision: &revision,
            digest: &digest,
            function: entry,
            arguments_json: &arguments_json,
            max_bytes: options.max_bytes,
            max_steps: options.max_steps,
            steps_used,
            exhausted,
            outcome_json: &outcome.json,
        })
    });
    if overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-F104",
            "interpret output exceeds the max-bytes budget; refusing to truncate".to_owned(),
        )]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(Interpretation {
        returned: outcome.kind == OUTCOME_RETURNED,
        envelope,
    })
}

fn select_function<'a>(program: &'a Program, token: &str) -> Result<&'a Function, Vec<Diagnostic>> {
    program
        .functions
        .iter()
        .find(|candidate| candidate.stable_id == token || candidate.name == token)
        .ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-F102",
                format!("selection `{token}` does not name a function in this program"),
            )]
        })
}

/// Closed AST-level admission gate mirroring Canonical ABI Report v1: explicit
/// identity, monomorphic, effect-free, by-value direct signature over the
/// admitted scalar types (`i64`, `i32`, `u8`, `char`, `f32`, `f64`, `bool`).
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

/// The widened direct scalar boundary: exactly the primitive scalar types the
/// engine already evaluates; strings, records, variants, and generics stay
/// outside the profile.
fn is_admitted_scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::I32 | Type::U8 | Type::F32 | Type::F64 | Type::Char | Type::Bool
    )
}

fn bind_arguments(
    function: &Function,
    arguments: &[String],
) -> Result<Vec<(String, ArgumentValue)>, Vec<Diagnostic>> {
    if arguments.len() != function.params.len() {
        return Err(vec![argument_error(format!(
            "function `{}` takes {} argument(s), {} were provided",
            function.name,
            function.params.len(),
            arguments.len()
        ))]);
    }
    let mut bound = Vec::with_capacity(arguments.len());
    for (param, text) in function.params.iter().zip(arguments) {
        let value = parse_argument(text).map_err(|error| vec![error])?;
        let type_matches = matches!(
            (&param.ty, value),
            (Type::I64, ArgumentValue::Int(_))
                | (Type::I32, ArgumentValue::Int32(_))
                | (Type::U8, ArgumentValue::Uint8(_))
                | (Type::Char, ArgumentValue::Char(_))
                | (Type::F32, ArgumentValue::Float32(_))
                | (Type::F64, ArgumentValue::Float64(_))
                | (Type::Bool, ArgumentValue::Bool(_))
        );
        if !type_matches {
            return Err(vec![argument_error(format!(
                "parameter `{}` of function `{}` expects {}, but argument `{text}` is {}",
                param.name,
                function.name,
                param.ty,
                value.type_text(),
            ))]);
        }
        bound.push((param.name.clone(), value));
    }
    Ok(bound)
}

/// Walks the selected function's contracts, body, and every transitively
/// reachable admitted callee, rejecting every shape outside the scalar
/// interpreter profile with one closed reason.
fn scan_closure(
    entry_id: &str,
    admitted: &BTreeMap<&str, &ResolvedFunction>,
) -> Result<(), Vec<Diagnostic>> {
    fn scan<'a>(
        expression: &'a ResolvedExpr,
        admitted: &BTreeMap<&'a str, &'a ResolvedFunction>,
        visited: &mut BTreeSet<&'a str>,
        queue: &mut Vec<&'a str>,
    ) -> Result<(), Vec<Diagnostic>> {
        match &expression.kind {
            ResolvedExprKind::ConstructRecord { .. } => {
                Err(reject_scan(expression, REASON_RECORD_CONSTRUCTION))
            }
            ResolvedExprKind::ConstructVariant { .. } => {
                Err(reject_scan(expression, REASON_VARIANT_CONSTRUCTION))
            }
            ResolvedExprKind::UpdateRecord { .. } => {
                Err(reject_scan(expression, REASON_RECORD_UPDATE))
            }
            ResolvedExprKind::Project { .. } => {
                Err(reject_scan(expression, REASON_RECORD_PROJECTION))
            }
            ResolvedExprKind::Upcast { .. } => {
                Err(reject_scan(expression, REASON_RECORD_PROJECTION))
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                // Refutable Match v1: scalar decision chains over admitted
                // Copy scalars with literal/or/binding patterns join the
                // profile; every aggregate match shape stays rejected.
                let scalar = matches!(
                    scrutinee.ty,
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::U8
                        | ResolvedType::Char
                        | ResolvedType::Bool
                );
                let patterns_admitted = arms
                    .iter()
                    .all(|arm| arm.pattern_is_literal_or_irrefutable());
                if !scalar || !patterns_admitted || arms.is_empty() {
                    Err(reject_scan(expression, REASON_MATCH_EXPRESSION))
                } else {
                    Ok(())
                }
            }
            ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => {
                Err(reject_scan(expression, REASON_TRY_EXPRESSION))
            }
            ResolvedExprKind::NativeRustImportCall(_) => {
                Err(reject_scan(expression, REASON_IMPORT_CALL))
            }
            ResolvedExprKind::Place(place) if !place.projections.is_empty() => {
                Err(reject_scan(expression, REASON_PLACE_PROJECTION))
            }
            ResolvedExprKind::Call {
                callee, instance, ..
            } => {
                if instance.is_some() {
                    return Err(reject_scan(expression, REASON_GENERIC_CALL));
                }
                let intrinsic = crate::string_ops::by_id(callee.as_str()).is_some();
                if !intrinsic && !admitted.contains_key(callee.as_str()) {
                    return Err(reject_scan(expression, REASON_UNSUPPORTED_CALLEE));
                }
                Ok(())
            }
            ResolvedExprKind::Block { statements, .. } => {
                for statement in statements {
                    if matches!(statement, ResolvedStatement::Unsafe { .. }) {
                        return Err(reject_scan(expression, REASON_UNSAFE_BOUNDARY));
                    }
                }
                Ok(())
            }
            ResolvedExprKind::Binary { op, .. } => {
                let unsupported = matches!(
                    (*op, &expression.ty),
                    (BinaryOp::Rem, ResolvedType::F32)
                        | (BinaryOp::Rem, ResolvedType::F64)
                        | (BinaryOp::Rem, ResolvedType::U8)
                        | (BinaryOp::Add, ResolvedType::Char)
                        | (BinaryOp::Sub, ResolvedType::Char)
                        | (BinaryOp::Mul, ResolvedType::Char)
                        | (BinaryOp::Div, ResolvedType::Char)
                        | (BinaryOp::Rem, ResolvedType::Char)
                );
                if unsupported {
                    return Err(reject_scan(expression, REASON_UNSUPPORTED_SCALAR_OPERATION));
                }
                Ok(())
            }
            _ => Ok(()),
        }?;
        for child in child_expressions(expression) {
            scan(child, admitted, visited, queue)?;
        }
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            // Callee bodies are enqueued once; the monotone `visited` set
            // makes recursive call cycles terminate.
            if visited.insert(callee.as_str()) {
                queue.push(callee.as_str());
            }
        }
        Ok(())
    }

    let mut visited = BTreeSet::new();
    visited.insert(entry_id);
    let mut frontier: Vec<&str> = vec![entry_id];
    while let Some(id) = frontier.pop() {
        let Some(function) = admitted.get(id) else {
            continue;
        };
        let mut queue: Vec<&str> = Vec::new();
        for clause in function.requires.iter().chain(&function.ensures) {
            scan(clause, admitted, &mut visited, &mut queue)?;
        }
        scan(&function.body, admitted, &mut visited, &mut queue)?;
        frontier.extend(queue);
    }
    Ok(())
}

fn admitted_resolved_functions(
    program: &hir::ResolvedProgram,
) -> BTreeMap<&str, &ResolvedFunction> {
    program
        .functions
        .iter()
        .filter(|function| resolved_signature_is_admitted(function))
        .map(|function| (function.id.as_str(), function))
        .collect()
}

fn resolved_signature_is_admitted(function: &ResolvedFunction) -> bool {
    function.effects.is_empty()
        && function
            .params
            .iter()
            .all(|parameter| parameter.ownership == hir::OwnershipMode::Value)
        && function
            .params
            .iter()
            .all(|parameter| is_admitted_resolved_scalar(&parameter.ty))
        && is_admitted_resolved_scalar(&function.return_type)
}

fn is_admitted_resolved_scalar(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::U8
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Char
            | ResolvedType::Bool
    )
}

/// Refutable Match v1: exact equality between the staged scrutinee value and
/// a literal pattern of the same type.
fn pattern_value_matches(staged: &Value, value: crate::hir::PatternValue) -> bool {
    match (staged, value) {
        (Value::Int(actual), crate::hir::PatternValue::Int(expected)) => *actual == expected,
        (Value::Int32(actual), crate::hir::PatternValue::Int32(expected)) => *actual == expected,
        (Value::Uint8(actual), crate::hir::PatternValue::Uint8(expected)) => *actual == expected,
        (Value::Char(actual), crate::hir::PatternValue::Char(expected)) => *actual == expected,
        (Value::Bool(actual), crate::hir::PatternValue::Bool(expected)) => *actual == expected,
        _ => false,
    }
}

fn reject_scan(expression: &ResolvedExpr, reason: &'static str) -> Vec<Diagnostic> {
    vec![selection_error(
        reason,
        format!("expression `{}`", expression.id),
    )]
}

/// Every directly nested evaluated expression, in deterministic order.
fn child_expressions(expression: &ResolvedExpr) -> Vec<&ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => args.iter().collect(),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().collect(),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => vec![value.as_ref()],
        ResolvedExprKind::Binary { left, right, .. } => vec![left.as_ref(), right.as_ref()],
        ResolvedExprKind::Block { statements, tail } => {
            let mut collected = Vec::new();
            for statement in statements {
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        collected.push(child);
                    }
                }
            }
            collected.push(tail.as_ref());
            collected
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => vec![
            condition.as_ref(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        ],
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.iter().map(|field| &field.value).collect()
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            let mut collected = vec![scrutinee.as_ref()];
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collected.push(guard.as_ref());
                }
                collected.push(&arm.value);
            }
            collected
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            let mut collected = vec![base.as_ref()];
            collected.extend(fields.iter().map(|field| &field.value));
            collected
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_) => Vec::new(),
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Int(i64),
    Int32(i32),
    Uint8(u8),
    Char(u32),
    Float32(f32),
    Float64(f64),
    Bool(bool),
    String(String),
}

enum Flow {
    Failure(NormalizedStatus),
    Exhausted,
    DepthExceeded,
    Guard(&'static str),
}

struct OutcomeJson {
    kind: &'static str,
    json: String,
}

fn returned_outcome(value: &Value) -> OutcomeJson {
    let (type_text, rendered) = match value {
        Value::Int(value) => ("i64", value.to_string()),
        // Suffixed widths always render with their explicit suffix so each
        // canonical rendering replays uniquely against the closed grammars.
        Value::Int32(value) => ("i32", format!("{value}i32")),
        Value::Uint8(value) => ("u8", format!("{value}u8")),
        Value::Char(value) => ("char", crate::format::canonical_char(*value)),
        // Floats render as their exact big-endian IEEE-754 bit pattern so the
        // envelope distinguishes `-0.0`, infinities, and NaN payloads without
        // relying on any platform's decimal formatting.
        Value::Float32(value) => ("f32", format!("{:08x}", value.to_bits())),
        Value::Float64(value) => ("f64", format!("{:016x}", value.to_bits())),
        Value::Bool(value) => ("bool", value.to_string()),
        other => unreachable!("admitted boundary types keep results on scalars, found {other:?}"),
    };
    OutcomeJson {
        kind: OUTCOME_RETURNED,
        json: bformat!(
            "{{\"kind\":\"{}\",\"type\":{},\"value\":{}}}",
            OUTCOME_RETURNED,
            quote_json(type_text),
            quote_json(&rendered),
        ),
    }
}

fn failed_outcome(status_json: &str) -> OutcomeJson {
    OutcomeJson {
        kind: OUTCOME_FAILED,
        json: bformat!(
            "{{\"kind\":\"{}\",\"status\":{}}}",
            OUTCOME_FAILED,
            status_json
        ),
    }
}

fn capacity_outcome(kind: &'static str) -> OutcomeJson {
    OutcomeJson {
        kind,
        json: bformat!("{{\"kind\":\"{kind}\"}}"),
    }
}

type Environment = Vec<(ValueId, Value)>;

struct Evaluator<'a> {
    admitted: &'a BTreeMap<&'a str, &'a ResolvedFunction>,
    steps: usize,
    budget: usize,
}

fn evaluate_resolved_entry<'a>(
    entry: &'a ResolvedFunction,
    arguments: &[(String, ArgumentValue)],
    admitted: &'a BTreeMap<&'a str, &'a ResolvedFunction>,
    budget: usize,
) -> (Result<Value, Flow>, usize) {
    let mut evaluator = Evaluator {
        admitted,
        steps: 0,
        budget,
    };
    let outcome = evaluator.evaluate_entry(entry, arguments);
    (outcome, evaluator.steps)
}

impl Evaluator<'_> {
    /// Charges one step before evaluating a node; `None` means the fuel
    /// budget is exhausted.
    fn charge(&mut self) -> Option<()> {
        if self.steps >= self.budget {
            return None;
        }
        self.steps += 1;
        Some(())
    }

    fn lookup(environment: &Environment, root: &ValueId) -> Option<Value> {
        environment
            .iter()
            .rev()
            .find(|(key, _)| key == root)
            .map(|(_, value)| value.clone())
    }

    fn evaluate_entry(
        &mut self,
        function: &ResolvedFunction,
        arguments: &[(String, ArgumentValue)],
    ) -> Result<Value, Flow> {
        let mut values = Vec::with_capacity(arguments.len());
        for (param, (_, argument)) in function.params.iter().zip(arguments.iter()) {
            let value = match (&param.ty, *argument) {
                (ResolvedType::I64, ArgumentValue::Int(inner)) => Value::Int(inner),
                (ResolvedType::I32, ArgumentValue::Int32(inner)) => Value::Int32(inner),
                (ResolvedType::U8, ArgumentValue::Uint8(inner)) => Value::Uint8(inner),
                (ResolvedType::Char, ArgumentValue::Char(inner)) => Value::Char(inner),
                (ResolvedType::F32, ArgumentValue::Float32(inner)) => Value::Float32(inner),
                (ResolvedType::F64, ArgumentValue::Float64(inner)) => Value::Float64(inner),
                (ResolvedType::Bool, ArgumentValue::Bool(inner)) => Value::Bool(inner),
                _ => return Err(Flow::Guard("argument/parameter binding mismatch")),
            };
            values.push((param.id.clone(), value));
        }
        if values.len() != function.params.len() {
            return Err(Flow::Guard("incomplete parameter frame"));
        }
        self.call_frame(function, values, 0)
    }

    fn call_frame(
        &mut self,
        function: &ResolvedFunction,
        values: Vec<(ValueId, Value)>,
        depth: usize,
    ) -> Result<Value, Flow> {
        if depth >= MAX_CALL_DEPTH {
            return Err(Flow::DepthExceeded);
        }
        let mut frame: Environment = values;
        for clause in &function.requires {
            self.charge().ok_or(Flow::Exhausted)?;
            match self.evaluate(clause, &mut frame, depth)? {
                Value::Bool(true) => {}
                Value::Bool(false) => {
                    return Err(Flow::Failure(normalize_contract(ContractPhase::Requires)))
                }
                _ => return Err(Flow::Guard("non-boolean requires clause")),
            }
        }
        let value = self.evaluate(&function.body, &mut frame, depth)?;
        frame.push((function.result_id.clone(), value.clone()));
        for clause in &function.ensures {
            self.charge().ok_or(Flow::Exhausted)?;
            match self.evaluate(clause, &mut frame, depth)? {
                Value::Bool(true) => {}
                Value::Bool(false) => {
                    return Err(Flow::Failure(normalize_contract(ContractPhase::Ensures)))
                }
                _ => return Err(Flow::Guard("non-boolean ensures clause")),
            }
        }
        Ok(value)
    }

    fn evaluate(
        &mut self,
        expression: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        self.charge().ok_or(Flow::Exhausted)?;
        match &expression.kind {
            ResolvedExprKind::Int(value) => Ok(Value::Int(*value)),
            ResolvedExprKind::Int32(value) => Ok(Value::Int32(*value)),
            ResolvedExprKind::Uint8(value) => Ok(Value::Uint8(*value)),
            ResolvedExprKind::Char(value) => Ok(Value::Char(*value)),
            ResolvedExprKind::Float32(bits) => Ok(Value::Float32(f32::from_bits(*bits))),
            ResolvedExprKind::Float64(bits) => Ok(Value::Float64(f64::from_bits(*bits))),
            ResolvedExprKind::Bool(value) => Ok(Value::Bool(*value)),
            ResolvedExprKind::String(value) => Ok(Value::String(value.clone())),
            ResolvedExprKind::Place(place) => {
                if !place.projections.is_empty() {
                    return Err(Flow::Guard("scalar profile has no place projections"));
                }
                Self::lookup(environment, &place.root).ok_or(Flow::Guard("unresolved scalar place"))
            }
            ResolvedExprKind::Unary { op, value } => {
                let inner = self.evaluate(value, environment, depth)?;
                match (*op, inner) {
                    (UnaryOp::Neg, Value::Int(inner)) => inner.checked_neg().map_or_else(
                        || {
                            Err(Flow::Failure(normalize_arithmetic(
                                StatusCase::NegationOverflow,
                            )))
                        },
                        |negated| Ok(Value::Int(negated)),
                    ),
                    (UnaryOp::Neg, Value::Int32(inner)) => inner.checked_neg().map_or_else(
                        || {
                            Err(Flow::Failure(normalize_arithmetic(
                                StatusCase::NegationOverflow,
                            )))
                        },
                        |negated| Ok(Value::Int32(negated)),
                    ),
                    (UnaryOp::Neg, Value::Float32(inner)) => Ok(Value::Float32(-inner)),
                    (UnaryOp::Neg, Value::Float64(inner)) => Ok(Value::Float64(-inner)),
                    (UnaryOp::Not, Value::Bool(inner)) => Ok(Value::Bool(!inner)),
                    _ => Err(Flow::Guard("unsupported unary operand")),
                }
            }
            ResolvedExprKind::Binary {
                op, left, right, ..
            } => {
                let lhs = self.evaluate(left, environment, depth)?;
                match op {
                    BinaryOp::And => match lhs {
                        Value::Bool(false) => Ok(Value::Bool(false)),
                        Value::Bool(true) => match self.evaluate(right, environment, depth)? {
                            Value::Bool(rhs) => Ok(Value::Bool(rhs)),
                            _ => Err(Flow::Guard("ill-typed conjunction operand")),
                        },
                        _ => Err(Flow::Guard("ill-typed conjunction operand")),
                    },
                    BinaryOp::Or => match lhs {
                        Value::Bool(true) => Ok(Value::Bool(true)),
                        Value::Bool(false) => match self.evaluate(right, environment, depth)? {
                            Value::Bool(rhs) => Ok(Value::Bool(rhs)),
                            _ => Err(Flow::Guard("ill-typed disjunction operand")),
                        },
                        _ => Err(Flow::Guard("ill-typed disjunction operand")),
                    },
                    _ => {
                        let rhs = self.evaluate(right, environment, depth)?;
                        match combine(*op, lhs, rhs) {
                            Some(Ok(value)) => Ok(value),
                            Some(Err(status)) => Err(Flow::Failure(status)),
                            None => Err(Flow::Guard("ill-typed operands")),
                        }
                    }
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let branch = match self.evaluate(condition, environment, depth)? {
                    Value::Bool(true) => then_branch,
                    Value::Bool(false) => else_branch,
                    _ => return Err(Flow::Guard("non-boolean condition")),
                };
                self.evaluate(branch, environment, depth)
            }
            ResolvedExprKind::Call {
                callee,
                instance,
                type_arguments,
                args,
            } => {
                if instance.is_some() || !type_arguments.is_empty() {
                    return Err(Flow::Guard("generic call inside the scalar profile"));
                }
                if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                    // Compiler-owned string operations evaluate in place;
                    // their byte semantics match the native and Wasm backends.
                    self.charge().ok_or(Flow::Exhausted)?;
                    let mut values = Vec::with_capacity(args.len());
                    for argument in args {
                        values.push(self.evaluate(argument, environment, depth)?);
                    }
                    return match op {
                        crate::string_ops::StringOp::Len => match values.first() {
                            Some(Value::String(value)) => Ok(Value::Int(value.len() as i64)),
                            _ => Err(Flow::Guard("ill-typed string operation operand")),
                        },
                        crate::string_ops::StringOp::IsEmpty => match values.first() {
                            Some(Value::String(value)) => Ok(Value::Bool(value.is_empty())),
                            _ => Err(Flow::Guard("ill-typed string operation operand")),
                        },
                        crate::string_ops::StringOp::Concat => {
                            match (values.first(), values.get(1)) {
                                (Some(Value::String(left)), Some(Value::String(right))) => {
                                    Ok(Value::String(format!("{left}{right}")))
                                }
                                _ => Err(Flow::Guard("ill-typed string operation operand")),
                            }
                        }
                        crate::string_ops::StringOp::StartsWith => {
                            match (values.first(), values.get(1)) {
                                (Some(Value::String(value)), Some(Value::String(prefix))) => {
                                    Ok(Value::Bool(value.starts_with(prefix.as_str())))
                                }
                                _ => Err(Flow::Guard("ill-typed string operation operand")),
                            }
                        }
                        crate::string_ops::StringOp::Contains => {
                            match (values.first(), values.get(1)) {
                                (Some(Value::String(value)), Some(Value::String(needle))) => {
                                    Ok(Value::Bool(value.contains(needle.as_str())))
                                }
                                _ => Err(Flow::Guard("ill-typed string operation operand")),
                            }
                        }
                        crate::string_ops::StringOp::LenChars => match values.first() {
                            Some(Value::String(value)) => {
                                Ok(Value::Int(value.chars().count() as i64))
                            }
                            _ => Err(Flow::Guard("ill-typed string operation operand")),
                        },
                        crate::string_ops::StringOp::FromChar => match values.first() {
                            Some(Value::Char(scalar)) => match char::from_u32(*scalar) {
                                Some(value) => Ok(Value::String(value.to_string())),
                                None => Err(Flow::Guard("ill-typed string operation operand")),
                            },
                            _ => Err(Flow::Guard("ill-typed string operation operand")),
                        },
                    };
                }
                let Some(function) = self.admitted.get(callee.as_str()) else {
                    return Err(Flow::Guard("call outside the admitted closure"));
                };
                let mut values: Vec<(ValueId, Value)> = Vec::with_capacity(args.len());
                for (param, argument) in function.params.iter().zip(args.iter()) {
                    let value = self.evaluate(argument, environment, depth)?;
                    values.push((param.id.clone(), value));
                }
                if values.len() != function.params.len() {
                    return Err(Flow::Guard("argument arity mismatch"));
                }
                self.call_frame(function, values, depth + 1)
            }
            ResolvedExprKind::Block { statements, tail } => {
                let base = environment.len();
                let mut interrupted = None;
                for statement in statements {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            match self.evaluate(value, environment, depth) {
                                Ok(value) => environment.push((binding.id.clone(), value)),
                                Err(flow) => {
                                    interrupted = Some(flow);
                                    break;
                                }
                            }
                        }
                        ResolvedStatement::Assign { binding, value, .. } => {
                            match self.evaluate(value, environment, depth) {
                                Ok(value) => {
                                    let Some(slot) = environment
                                        .iter_mut()
                                        .rev()
                                        .find(|(key, _)| *key == binding.id)
                                        .map(|(_, slot)| slot)
                                    else {
                                        interrupted =
                                            Some(Flow::Guard("assignment to an unknown binding"));
                                        break;
                                    };
                                    *slot = value;
                                }
                                Err(flow) => {
                                    interrupted = Some(flow);
                                    break;
                                }
                            }
                        }
                        ResolvedStatement::Unsafe { .. } => {
                            interrupted =
                                Some(Flow::Guard("unsafe boundary outside the admitted surface"));
                            break;
                        }
                        ResolvedStatement::While {
                            condition, body, ..
                        } => {
                            // Bounded While-Loops v1: the condition
                            // re-evaluates before every iteration and every
                            // evaluated node charges fuel, so a non-terminating
                            // loop fails closed through the existing exhausted
                            // budget path.
                            loop {
                                let charge = self.charge();
                                if charge.is_none() {
                                    interrupted = Some(Flow::Exhausted);
                                    break;
                                }
                                let flag = match self.evaluate(condition, environment, depth) {
                                    Ok(Value::Bool(flag)) => flag,
                                    Ok(_) => {
                                        interrupted =
                                            Some(Flow::Guard("non-boolean while condition"));
                                        break;
                                    }
                                    Err(flow) => {
                                        interrupted = Some(flow);
                                        break;
                                    }
                                };
                                if !flag {
                                    break;
                                }
                                if let Err(flow) = self.evaluate(body, environment, depth) {
                                    interrupted = Some(flow);
                                    break;
                                }
                            }
                        }
                    }
                }
                let outcome = match interrupted {
                    Some(flow) => Err(flow),
                    None => self.evaluate(tail, environment, depth),
                };
                environment.truncate(base);
                outcome
            }
            // Refutable Match v1: one scrutinee evaluation, arms tested in
            // order, guards evaluated once after their pattern matched, and
            // failing guards fall through. Every evaluated node charges fuel
            // through the ordinary recursive `evaluate` calls.
            ResolvedExprKind::Match { scrutinee, arms } => {
                let staged = self.evaluate(scrutinee, environment, depth)?;
                for arm in arms {
                    let selected = match &arm.pattern {
                        crate::hir::ResolvedMatchPattern::Wildcard => true,
                        crate::hir::ResolvedMatchPattern::Binding(binding) => {
                            let _ = binding;
                            true
                        }
                        crate::hir::ResolvedMatchPattern::Literal(value) => {
                            pattern_value_matches(&staged, *value)
                        }
                        crate::hir::ResolvedMatchPattern::Or(alternatives) => {
                            let mut matched = false;
                            for alternative in alternatives {
                                match alternative {
                                    crate::hir::ResolvedMatchPattern::Literal(value) => {
                                        matched |= pattern_value_matches(&staged, *value);
                                    }
                                    _ => {
                                        return Err(Flow::Guard(
                                            "or-pattern alternative is not a literal",
                                        ));
                                    }
                                }
                            }
                            matched
                        }
                        crate::hir::ResolvedMatchPattern::Variant { .. }
                        | crate::hir::ResolvedMatchPattern::Record { .. } => {
                            return Err(Flow::Guard(
                                "aggregate match shape reached scalar evaluation",
                            ));
                        }
                    };
                    if !selected {
                        continue;
                    }
                    // Binding arms capture the staged scrutinee value.
                    let mut bound: Option<(ValueId, Value)> = None;
                    if let crate::hir::ResolvedMatchPattern::Binding(binding) = &arm.pattern {
                        bound = Some((binding.id.clone(), staged.clone()));
                        environment.push((binding.id.clone(), staged.clone()));
                    }
                    let guard_ok = match &arm.guard {
                        Some(guard) => match self.evaluate(guard.as_ref(), environment, depth)? {
                            Value::Bool(flag) => flag,
                            _ => return Err(Flow::Guard("non-boolean match guard")),
                        },
                        None => true,
                    };
                    if !guard_ok {
                        if bound.is_some() {
                            environment.pop();
                        }
                        continue;
                    }
                    let outcome = self.evaluate(&arm.value, environment, depth);
                    if bound.is_some() {
                        environment.pop();
                    }
                    return outcome;
                }
                Err(Flow::Guard("refutable match selected no arm"))
            }
            ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Upcast { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::NativeRustImportCall(_) => Err(Flow::Guard(
                "aggregate/import/match/try shape reached evaluation",
            )),
        }
    }
}

/// Typed binary evaluation. `None` marks an ill-typed combination (impossible
/// on verified programs); `Some(Err(..))` is the exact compiler-owned failure
/// status selected by the checked operation.
fn combine(op: BinaryOp, lhs: Value, rhs: Value) -> Option<Result<Value, NormalizedStatus>> {
    let arithmetic = |outcome: Option<Value>, case: StatusCase| {
        outcome.map_or(Some(Err(normalize_arithmetic(case))), |value| {
            Some(Ok(value))
        })
    };
    let ordered = |less: bool, equal: bool| -> Option<Result<Value, NormalizedStatus>> {
        let value = match op {
            BinaryOp::Eq => equal,
            BinaryOp::Ne => !equal,
            BinaryOp::Lt => less,
            BinaryOp::Le => less || equal,
            BinaryOp::Gt => !less && !equal,
            BinaryOp::Ge => !less,
            _ => return None,
        };
        Some(Ok(Value::Bool(value)))
    };
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => match op {
            BinaryOp::Add => arithmetic(a.checked_add(b).map(Value::Int), StatusCase::AddOverflow),
            BinaryOp::Sub => arithmetic(a.checked_sub(b).map(Value::Int), StatusCase::SubOverflow),
            BinaryOp::Mul => arithmetic(a.checked_mul(b).map(Value::Int), StatusCase::MulOverflow),
            BinaryOp::Div => {
                if b == 0 {
                    arithmetic(None, StatusCase::DivisionByZero)
                } else if a == i64::MIN && b == -1 {
                    arithmetic(None, StatusCase::DivisionOverflow)
                } else {
                    Some(Ok(Value::Int(a / b)))
                }
            }
            BinaryOp::Rem => {
                if b == 0 {
                    arithmetic(None, StatusCase::RemainderByZero)
                } else if a == i64::MIN && b == -1 {
                    arithmetic(None, StatusCase::RemainderOverflow)
                } else {
                    Some(Ok(Value::Int(a % b)))
                }
            }
            _ => ordered(a < b, a == b),
        },
        (Value::Int32(a), Value::Int32(b)) => match op {
            BinaryOp::Add => {
                arithmetic(a.checked_add(b).map(Value::Int32), StatusCase::AddOverflow)
            }
            BinaryOp::Sub => {
                arithmetic(a.checked_sub(b).map(Value::Int32), StatusCase::SubOverflow)
            }
            BinaryOp::Mul => {
                arithmetic(a.checked_mul(b).map(Value::Int32), StatusCase::MulOverflow)
            }
            BinaryOp::Div => {
                if b == 0 {
                    arithmetic(None, StatusCase::DivisionByZero)
                } else if a == i32::MIN && b == -1 {
                    arithmetic(None, StatusCase::DivisionOverflow)
                } else {
                    Some(Ok(Value::Int32(a / b)))
                }
            }
            BinaryOp::Rem => {
                if b == 0 {
                    arithmetic(None, StatusCase::RemainderByZero)
                } else if a == i32::MIN && b == -1 {
                    arithmetic(None, StatusCase::RemainderOverflow)
                } else {
                    Some(Ok(Value::Int32(a % b)))
                }
            }
            _ => ordered(a < b, a == b),
        },
        (Value::Uint8(a), Value::Uint8(b)) => match op {
            BinaryOp::Add => {
                arithmetic(a.checked_add(b).map(Value::Uint8), StatusCase::AddOverflow)
            }
            BinaryOp::Sub => {
                arithmetic(a.checked_sub(b).map(Value::Uint8), StatusCase::SubOverflow)
            }
            BinaryOp::Mul => {
                arithmetic(a.checked_mul(b).map(Value::Uint8), StatusCase::MulOverflow)
            }
            // `u8` division has no overflow case, so `None` selects exactly
            // the zero-divisor status.
            BinaryOp::Div => a.checked_div(b).map_or_else(
                || arithmetic(None, StatusCase::DivisionByZero),
                |quotient| Some(Ok(Value::Uint8(quotient))),
            ),
            // Like division, `u8` remainder can only fail on a zero divisor.
            BinaryOp::Rem => a.checked_rem(b).map_or_else(
                || arithmetic(None, StatusCase::RemainderByZero),
                |remainder| Some(Ok(Value::Uint8(remainder))),
            ),
            _ => ordered(a < b, a == b),
        },
        (Value::Char(a), Value::Char(b)) => ordered(a < b, a == b),
        (Value::Float32(a), Value::Float32(b)) => match op {
            BinaryOp::Add => Some(Ok(Value::Float32(a + b))),
            BinaryOp::Sub => Some(Ok(Value::Float32(a - b))),
            BinaryOp::Mul => Some(Ok(Value::Float32(a * b))),
            BinaryOp::Div => Some(Ok(Value::Float32(a / b))),
            _ => float_ordered(op, a.partial_cmp(&b), a == b).map(Ok),
        },
        (Value::Float64(a), Value::Float64(b)) => match op {
            BinaryOp::Add => Some(Ok(Value::Float64(a + b))),
            BinaryOp::Sub => Some(Ok(Value::Float64(a - b))),
            BinaryOp::Mul => Some(Ok(Value::Float64(a * b))),
            BinaryOp::Div => Some(Ok(Value::Float64(a / b))),
            _ => float_ordered(op, a.partial_cmp(&b), a == b).map(Ok),
        },
        (Value::Bool(a), Value::Bool(b)) => match op {
            BinaryOp::Eq => Some(Ok(Value::Bool(a == b))),
            BinaryOp::Ne => Some(Ok(Value::Bool(a != b))),
            _ => None,
        },
        // Owned strings compare by exact UTF-8 contents; any other operator
        // over strings is ill-typed on verified programs.
        (Value::String(a), Value::String(b)) => match op {
            BinaryOp::Eq => Some(Ok(Value::Bool(a == b))),
            BinaryOp::Ne => Some(Ok(Value::Bool(a != b))),
            _ => None,
        },
        _ => None,
    }
}

/// IEEE-754 comparisons: unordered operands (NaN) compare false everywhere
/// except `!=`, exactly like the hardware backends.
fn float_ordered(op: BinaryOp, ordering: Option<std::cmp::Ordering>, equal: bool) -> Option<Value> {
    use std::cmp::Ordering;
    let value = match op {
        BinaryOp::Eq => equal,
        BinaryOp::Ne => !equal,
        BinaryOp::Lt => ordering == Some(Ordering::Less),
        BinaryOp::Le => matches!(ordering, Some(Ordering::Less) | Some(Ordering::Equal)),
        BinaryOp::Gt => ordering == Some(Ordering::Greater),
        BinaryOp::Ge => matches!(ordering, Some(Ordering::Greater) | Some(Ordering::Equal)),
        _ => return None,
    };
    Some(Value::Bool(value))
}

struct RenderFacts<'a> {
    path_text: &'a str,
    revision: &'a str,
    digest: &'a str,
    function: &'a ResolvedFunction,
    arguments_json: &'a [String],
    max_bytes: usize,
    max_steps: usize,
    steps_used: usize,
    exhausted: bool,
    outcome_json: &'a str,
}

fn render(facts: &RenderFacts<'_>) -> String {
    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"function\":{{\"stable_id\":{},\"name\":{}}},\
\"arguments\":[{}],\
\"limits\":{{\"max_bytes\":{},\"max_steps\":{}}},\
\"fuel\":{{\"steps_used\":{},\"budget\":{},\"exhausted\":{}}},\
\"outcome\":{},\"nonclaims\":[{}]}}",
        SCHEMA,
        quote_json(facts.path_text),
        quote_json(facts.revision),
        quote_json(facts.digest),
        quote_json(facts.function.id.as_str()),
        quote_json(&facts.function.name),
        facts.arguments_json.budgeted_join(","),
        facts.max_bytes,
        facts.max_steps,
        facts.steps_used,
        facts.max_steps,
        facts.exhausted,
        facts.outcome_json,
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

/// Independently verify one envelope produced by [`interpret`].
///
/// Recomputes the outer payload digest over the exact serialized payload
/// bytes, re-checks the declared byte count, and replays every closed
/// derivation the payload carries: argument/fuel/outcome shapes and
/// vocabularies, fuel-budget invariants (`exhausted` implies
/// `steps_used == budget`), canonical literal grammars, and exact
/// compiler-owned normalized-status reconstruction for failed outcomes.
pub fn verify_envelope(envelope: &str) -> Result<(), Diagnostic> {
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
    if envelope_digest != domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes()) {
        return Err(consistency_error(
            "envelope digest does not match the exact payload bytes".to_owned(),
        ));
    }

    let payload_value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|error| consistency_error(format!("payload is not valid JSON: {error}")))?;
    let Some(payload_object) = payload_value.as_object() else {
        return Err(consistency_error(
            "payload must be a JSON object".to_owned(),
        ));
    };
    let mut payload_keys: Vec<&str> = payload_object.keys().map(String::as_str).collect();
    payload_keys.sort_unstable();
    if payload_keys
        != [
            "arguments",
            "fuel",
            "function",
            "limits",
            "nonclaims",
            "outcome",
            "schema",
            "source",
        ]
    {
        return Err(consistency_error(format!(
            "payload keys must be exactly [arguments, function, fuel, limits, nonclaims, outcome, schema, source], found {payload_keys:?}"
        )));
    }
    if payload_object["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "payload schema must be {SCHEMA}"
        )));
    }

    let Some(source) = payload_object["source"].as_object() else {
        return Err(consistency_error(
            "payload source must be an object".to_owned(),
        ));
    };
    if source.keys().map(String::as_str).collect::<Vec<_>>() != ["path", "revision", "sha256"] {
        return Err(consistency_error(
            "payload source keys must be exactly [path, revision, sha256]".to_owned(),
        ));
    }
    for key in ["path", "revision", "sha256"] {
        if source[key].as_str().is_none_or(str::is_empty) {
            return Err(consistency_error(format!(
                "payload source `{key}` must be a nonempty string"
            )));
        }
    }
    if !source["sha256"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:"))
    {
        return Err(consistency_error(
            "payload source sha256 must start with `sha256:`".to_owned(),
        ));
    }

    let Some(function) = payload_object["function"].as_object() else {
        return Err(consistency_error(
            "payload function must be an object".to_owned(),
        ));
    };
    if function.keys().map(String::as_str).collect::<Vec<_>>() != ["name", "stable_id"] {
        return Err(consistency_error(
            "payload function keys must be exactly [name, stable_id]".to_owned(),
        ));
    }
    for key in ["name", "stable_id"] {
        if function[key].as_str().is_none_or(str::is_empty) {
            return Err(consistency_error(format!(
                "payload function `{key}` must be a nonempty string"
            )));
        }
    }

    let Some(arguments) = payload_object["arguments"].as_array() else {
        return Err(consistency_error(
            "payload arguments must be an array".to_owned(),
        ));
    };
    for (index, argument) in arguments.iter().enumerate() {
        let Some(argument) = argument.as_object() else {
            return Err(consistency_error(
                "each payload argument must be an object".to_owned(),
            ));
        };
        if argument.keys().map(String::as_str).collect::<Vec<_>>()
            != ["index", "name", "type", "value"]
        {
            return Err(consistency_error(
                "argument keys must be exactly [index, name, type, value]".to_owned(),
            ));
        }
        if argument["index"].as_u64() != Some(index as u64) {
            return Err(consistency_error(
                "argument indices must be sequential from zero".to_owned(),
            ));
        }
        if argument["name"].as_str().is_none_or(str::is_empty) {
            return Err(consistency_error(
                "argument name must be a nonempty string".to_owned(),
            ));
        }
        let type_text = match argument["type"].as_str() {
            Some(type_text @ ("i64" | "i32" | "u8" | "char" | "f32" | "f64" | "bool")) => type_text,
            _ => {
                return Err(consistency_error(
                    "argument type must be one of i64, i32, u8, char, f32, f64, bool".to_owned(),
                ))
            }
        };
        let Some(value_text) = argument["value"].as_str() else {
            return Err(consistency_error(
                "argument value must be a string".to_owned(),
            ));
        };
        if !canonical_scalar_value_matches(type_text, value_text) {
            return Err(consistency_error(format!(
                "argument value `{value_text}` is not the canonical rendering of `{type_text}`"
            )));
        }
    }

    let Some(limits) = payload_object["limits"].as_object() else {
        return Err(consistency_error(
            "payload limits must be an object".to_owned(),
        ));
    };
    if limits.keys().map(String::as_str).collect::<Vec<_>>() != ["max_bytes", "max_steps"] {
        return Err(consistency_error(
            "payload limits keys must be exactly [max_bytes, max_steps]".to_owned(),
        ));
    }
    let Some(max_bytes) = limits["max_bytes"].as_u64() else {
        return Err(consistency_error(
            "limits max_bytes must be an unsigned integer".to_owned(),
        ));
    };
    if !(graph::MIN_AGENT_CONTEXT_BYTES as u64..=graph::MAX_AGENT_CONTEXT_BYTES as u64)
        .contains(&max_bytes)
    {
        return Err(consistency_error(
            "limits max_bytes is outside the admitted bounds".to_owned(),
        ));
    }
    let Some(max_steps) = limits["max_steps"].as_u64() else {
        return Err(consistency_error(
            "limits max_steps must be an unsigned integer".to_owned(),
        ));
    };
    if !(1..=MAX_STEPS_LIMIT as u64).contains(&max_steps) {
        return Err(consistency_error(
            "limits max_steps is outside the admitted bounds".to_owned(),
        ));
    }

    let Some(fuel) = payload_object["fuel"].as_object() else {
        return Err(consistency_error(
            "payload fuel must be an object".to_owned(),
        ));
    };
    if fuel.keys().map(String::as_str).collect::<Vec<_>>() != ["budget", "exhausted", "steps_used"]
    {
        return Err(consistency_error(
            "payload fuel keys must be exactly [budget, exhausted, steps_used]".to_owned(),
        ));
    }
    if fuel["budget"].as_u64() != Some(max_steps) {
        return Err(consistency_error(
            "fuel budget must equal limits max_steps".to_owned(),
        ));
    }
    let Some(steps_used) = fuel["steps_used"].as_u64() else {
        return Err(consistency_error(
            "fuel steps_used must be an unsigned integer".to_owned(),
        ));
    };
    let Some(exhausted) = fuel["exhausted"].as_bool() else {
        return Err(consistency_error(
            "fuel exhausted must be a boolean".to_owned(),
        ));
    };
    if steps_used > max_steps {
        return Err(consistency_error(
            "fuel steps_used exceeds the declared budget".to_owned(),
        ));
    }
    if exhausted && steps_used != max_steps {
        return Err(consistency_error(
            "exhausted fuel must report steps_used equal to the budget".to_owned(),
        ));
    }

    let Some(outcome) = payload_object["outcome"].as_object() else {
        return Err(consistency_error(
            "payload outcome must be an object".to_owned(),
        ));
    };
    match outcome["kind"].as_str() {
        Some(OUTCOME_RETURNED) => {
            if outcome.keys().map(String::as_str).collect::<Vec<_>>() != ["kind", "type", "value"] {
                return Err(consistency_error(
                    "returned outcomes must carry exactly [kind, type, value]".to_owned(),
                ));
            }
            let type_text =
                match outcome["type"].as_str() {
                    Some(type_text @ ("i64" | "i32" | "u8" | "char" | "f32" | "f64" | "bool")) => {
                        type_text
                    }
                    _ => return Err(consistency_error(
                        "returned outcome type must be one of i64, i32, u8, char, f32, f64, bool"
                            .to_owned(),
                    )),
                };
            let Some(value_text) = outcome["value"].as_str() else {
                return Err(consistency_error(
                    "returned outcome value must be a string".to_owned(),
                ));
            };
            if !canonical_scalar_value_matches(type_text, value_text) {
                return Err(consistency_error(format!(
                    "returned outcome value `{value_text}` is not the canonical rendering of `{type_text}`"
                )));
            }
        }
        Some(OUTCOME_FAILED) => {
            if outcome.keys().map(String::as_str).collect::<Vec<_>>() != ["kind", "status"] {
                return Err(consistency_error(
                    "failed outcomes must carry exactly [kind, status]".to_owned(),
                ));
            }
            verify_status(&outcome["status"])?;
        }
        Some(kind @ (OUTCOME_FUEL_EXHAUSTED | OUTCOME_CALL_DEPTH_EXCEEDED)) => {
            if outcome.keys().map(String::as_str).collect::<Vec<_>>() != ["kind"] {
                return Err(consistency_error(format!(
                    "capacity outcome `{kind}` must carry exactly [kind]"
                )));
            }
            if kind == OUTCOME_FUEL_EXHAUSTED && !exhausted {
                return Err(consistency_error(
                    "fuel_exhausted outcome contradicts the fuel section".to_owned(),
                ));
            }
        }
        _ => {
            return Err(consistency_error(format!(
                "outcome kind must be one of [{OUTCOME_RETURNED}, {OUTCOME_FAILED}, \
                 {OUTCOME_FUEL_EXHAUSTED}, {OUTCOME_CALL_DEPTH_EXCEEDED}]"
            )))
        }
    }

    let Some(nonclaims) = payload_object["nonclaims"].as_array() else {
        return Err(consistency_error(
            "payload nonclaims must be an array".to_owned(),
        ));
    };
    let rendered: Vec<Option<&str>> = nonclaims.iter().map(|item| item.as_str()).collect();
    if rendered
        != NONCLAIMS_LIST
            .iter()
            .map(|&claim| Some(claim))
            .collect::<Vec<_>>()
    {
        return Err(consistency_error(
            "payload nonclaims must equal the fixed closed list".to_owned(),
        ));
    }
    Ok(())
}

/// Replays one canonical scalar value rendering against its declared type:
/// integers and booleans through the closed [`parse_argument`] grammar, chars
/// as canonical language literals, and floats as exact big-endian IEEE-754
/// bit patterns (`f32` eight, `f64` sixteen lowercase hexadecimal digits).
fn canonical_scalar_value_matches(type_text: &str, value_text: &str) -> bool {
    match type_text {
        "i64" => matches!(parse_argument(value_text), Ok(ArgumentValue::Int(_))),
        "i32" => matches!(parse_argument(value_text), Ok(ArgumentValue::Int32(_))),
        "u8" => matches!(parse_argument(value_text), Ok(ArgumentValue::Uint8(_))),
        "bool" => matches!(parse_argument(value_text), Ok(ArgumentValue::Bool(_))),
        "char" => matches!(
            parse_argument(value_text),
            Ok(ArgumentValue::Char(value))
                if crate::format::canonical_char(value) == value_text
        ),
        "f32" | "f64" => {
            let width = if type_text == "f32" { 8 } else { 16 };
            value_text.len() == width
                && value_text
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        }
        _ => false,
    }
}

/// Rebuilds the exact compiler-owned normalized status from its closed
/// vocabulary and compares it byte-for-byte with the embedded rendering.
fn verify_status(status: &serde_json::Value) -> Result<(), Diagnostic> {
    let Some(status) = status.as_object() else {
        return Err(consistency_error(
            "failed outcome status must be an object".to_owned(),
        ));
    };
    if status.keys().map(String::as_str).collect::<Vec<_>>()
        != ["class", "code", "domain_id", "retryable", "schema"]
    {
        return Err(consistency_error(
            "status keys must be exactly [class, code, domain_id, retryable, schema]".to_owned(),
        ));
    }
    if status["schema"].as_str() != Some(crate::conformance::NORMALIZED_STATUS_SCHEMA_V1) {
        return Err(consistency_error(format!(
            "status schema must be {}",
            crate::conformance::NORMALIZED_STATUS_SCHEMA_V1
        )));
    }
    let Some(code) = status["code"].as_u64() else {
        return Err(consistency_error(
            "status code must be an unsigned integer".to_owned(),
        ));
    };
    if status["retryable"].as_bool() != Some(false) {
        return Err(consistency_error(
            "compiler-owned statuses are never retryable".to_owned(),
        ));
    }
    let rebuilt = match status["domain_id"].as_str() {
        Some(crate::conformance::ARITHMETIC_STATUS_DOMAIN_V1) => {
            if status["class"].as_str() != Some("arithmetic") {
                return Err(consistency_error(
                    "arithmetic statuses must declare class arithmetic".to_owned(),
                ));
            }
            let Some(case) = status_case_from_code(u32::try_from(code).unwrap_or(u32::MAX)) else {
                return Err(consistency_error(
                    "arithmetic status code is outside the closed v1 table".to_owned(),
                ));
            };
            normalize_arithmetic(case).to_json()
        }
        Some(crate::conformance::CONTRACT_STATUS_DOMAIN_V1) => {
            if status["class"].as_str() != Some("contract") {
                return Err(consistency_error(
                    "contract statuses must declare class contract".to_owned(),
                ));
            }
            if code == u64::from(crate::conformance::CONTRACT_REQUIRES_FALSE_CODE) {
                normalize_contract(ContractPhase::Requires).to_json()
            } else if code == u64::from(crate::conformance::CONTRACT_ENSURES_FALSE_CODE) {
                normalize_contract(ContractPhase::Ensures).to_json()
            } else {
                return Err(consistency_error(
                    "contract status code is outside the closed v1 table".to_owned(),
                ));
            }
        }
        _ => {
            return Err(consistency_error(
                "interpreted failures only ever carry compiler-owned status domains".to_owned(),
            ))
        }
    };
    if rebuilt
        != format!(
            "{{\"schema\":{},\"domain_id\":{},\"code\":{},\"class\":{},\"retryable\":false}}",
            quote_json(crate::conformance::NORMALIZED_STATUS_SCHEMA_V1),
            quote_json(status["domain_id"].as_str().unwrap_or_default()),
            code,
            quote_json(status["class"].as_str().unwrap_or_default()),
        )
    {
        return Err(consistency_error(
            "embedded status does not equal its compiler-owned reconstruction".to_owned(),
        ));
    }
    Ok(())
}

/// Verify one envelope and additionally bind the current bytes of
/// `source_path` to the embedded source digest, failing closed on drift.
pub fn verify_envelope_against_source(
    envelope: &str,
    source_path: &Path,
) -> Result<(), Diagnostic> {
    verify_envelope(envelope)?;
    let current = std::fs::read(source_path).map_err(|error| {
        consistency_error(format!("cannot read {}: {error}", source_path.display()))
    })?;
    let bound = bound_source_digest(envelope)?;
    if bound != domain_digest(SOURCE_DIGEST_DOMAIN, &current) {
        return Err(consistency_error(
            "interpret source digest does not match the current source bytes; \
             the source drifted after the interpretation was generated"
                .to_owned(),
        ));
    }
    Ok(())
}

fn bound_source_digest(envelope: &str) -> Result<String, Diagnostic> {
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("envelope is not valid JSON: {error}")))?;
    let Some(digest) = value["payload"]["source"]["sha256"].as_str() else {
        return Err(consistency_error(
            "payload source sha256 must be a string".to_owned(),
        ));
    };
    Ok(digest.to_owned())
}

/// The closed v1 arithmetic status table, mirrored for replay.
fn status_case_from_code(code: u32) -> Option<StatusCase> {
    Some(match code {
        1 => StatusCase::AddOverflow,
        2 => StatusCase::SubOverflow,
        3 => StatusCase::MulOverflow,
        4 => StatusCase::DivisionByZero,
        5 => StatusCase::DivisionOverflow,
        6 => StatusCase::RemainderByZero,
        7 => StatusCase::RemainderOverflow,
        8 => StatusCase::NegationOverflow,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(source: &str) -> hir::ResolvedProgram {
        let path = Path::new("resolved-evaluation.spx");
        let program = parse(source, path).unwrap();
        let diagnostics = verify::verify(&program);
        assert!(
            !diagnostics.iter().any(|item| item.severity.is_error()),
            "source must verify before the resolved evaluator is called: {diagnostics:?}"
        );
        hir::resolve(&program).unwrap()
    }

    #[test]
    fn resolved_zero_arg_evaluation_returns_deterministic_i64_and_fuel_facts() {
        let program = resolved(
            "module test.resolved_return;\n\n@id(\"math.add\")\nfn add(left: i64, right: i64) -> i64 { left + right }\n\n@id(\"app.main\")\nfn main() -> i64 { add(19, 23) }\n",
        );
        let first = evaluate_resolved_zero_arg_i64(&program, "app.main", 1_000).unwrap();
        let second = evaluate_resolved_zero_arg_i64(&program, "app.main", 1_000).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.outcome, ResolvedEvaluationOutcome::ReturnedI64(42));
        assert!(first.steps_used > 0);
        assert_eq!(first.max_steps, 1_000);
    }

    #[test]
    fn resolved_zero_arg_evaluation_keeps_language_and_capacity_failures_distinct() {
        let failure = resolved(
            "module test.resolved_failure;\n\n@id(\"app.main\")\nfn main() -> i64 { 1 / 0 }\n",
        );
        let failure = evaluate_resolved_zero_arg_i64(&failure, "app.main", 100).unwrap();
        match failure.outcome {
            ResolvedEvaluationOutcome::LanguageFailure(status) => {
                assert_eq!(status, normalize_arithmetic(StatusCase::DivisionByZero));
            }
            other => panic!("expected language failure, found {other:?}"),
        }

        let fuel =
            resolved("module test.resolved_fuel;\n\n@id(\"app.main\")\nfn main() -> i64 { 42 }\n");
        let fuel = evaluate_resolved_zero_arg_i64(&fuel, "app.main", 1).unwrap();
        assert_eq!(fuel.outcome, ResolvedEvaluationOutcome::FuelExhausted);
        assert_eq!(fuel.steps_used, 1);

        let depth = resolved(
            "module test.resolved_depth;\n\n@id(\"test.recurse\")\nfn recurse() -> i64 { recurse() }\n\n@id(\"app.main\")\nfn main() -> i64 { recurse() }\n",
        );
        let depth = evaluate_resolved_zero_arg_i64(&depth, "app.main", MAX_STEPS_LIMIT).unwrap();
        assert_eq!(depth.outcome, ResolvedEvaluationOutcome::CallDepthExceeded);
        assert!(depth.steps_used < depth.max_steps);
    }

    #[test]
    fn resolved_zero_arg_evaluation_rejects_redirection_signature_and_budget_drift() {
        let program = resolved(
            "module test.resolved_reject;\n\n@id(\"test.other\")\nfn other() -> i64 { 0 }\n\n@id(\"app.main\")\nfn main() -> i64 { other() }\n",
        );
        assert_eq!(
            evaluate_resolved_zero_arg_i64(&program, "test.other", 100).unwrap_err()[0].code,
            "SPX-F102"
        );
        assert_eq!(
            evaluate_resolved_zero_arg_i64(&program, "app.main", 0).unwrap_err()[0].code,
            "SPX-F101"
        );

        let mut wrong_signature = resolved(
            "module test.resolved_signature;\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
        );
        wrong_signature
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap()
            .return_type = ResolvedType::Bool;
        assert_eq!(
            evaluate_resolved_zero_arg_i64(&wrong_signature, "app.main", 100).unwrap_err()[0].code,
            "SPX-F102"
        );
    }

    #[test]
    fn resolved_zero_arg_evaluation_reports_impossible_post_validation_state_as_guard() {
        let mut program =
            resolved("module test.resolved_guard;\n\n@id(\"app.main\")\nfn main() -> i64 { 42 }\n");
        let entry = program
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        entry.body.kind = ResolvedExprKind::Bool(true);
        let outcome = evaluate_resolved_zero_arg_i64(&program, "app.main", 100).unwrap();
        assert_eq!(
            outcome.outcome,
            ResolvedEvaluationOutcome::GuardError(
                "zero-argument i64 entry returned a non-i64 value".to_owned()
            )
        );
    }

    #[test]
    fn options_reject_out_of_bounds_values() {
        assert!(InterpreterOptions::new(512, DEFAULT_MAX_STEPS).is_err());
        assert!(
            InterpreterOptions::new(graph::MAX_AGENT_CONTEXT_BYTES + 1, DEFAULT_MAX_STEPS).is_err()
        );
        assert!(InterpreterOptions::new(graph::MIN_AGENT_CONTEXT_BYTES, 1).is_ok());
        let defaults = InterpreterOptions::default();
        assert_eq!(defaults.max_bytes, DEFAULT_MAX_BYTES);
        assert_eq!(defaults.max_steps, DEFAULT_MAX_STEPS);
        assert!(InterpreterOptions::new(65536, 0).is_err());
        assert!(InterpreterOptions::new(65536, MAX_STEPS_LIMIT).is_ok());
    }

    fn literal(text: &str) -> ArgumentValue {
        parse_argument(text).expect(text)
    }

    #[test]
    fn argument_literals_are_canonical() {
        assert_eq!(literal("true"), ArgumentValue::Bool(true));
        assert_eq!(literal("false"), ArgumentValue::Bool(false));
        assert_eq!(literal("0"), ArgumentValue::Int(0));
        assert_eq!(literal("-0"), ArgumentValue::Int(0));
        assert_eq!(
            literal("-9223372036854775808"),
            ArgumentValue::Int(i64::MIN)
        );
        assert_eq!(literal("9223372036854775807"), ArgumentValue::Int(i64::MAX));
        for hostile in [
            "",
            "+1",
            "007",
            "0x10",
            "1_000",
            "trueish",
            "-",
            "9223372036854775808",
            "TRUE",
            " false",
            "false ",
            "\u{0661}\u{0662}",
        ] {
            assert!(parse_argument(hostile).is_err(), "{hostile}");
        }
    }

    #[test]
    fn widened_scalar_literals_are_canonical() {
        // Suffixed integers mirror the language lexer: only `i32` and `u8`.
        assert_eq!(literal("7i32"), ArgumentValue::Int32(7));
        assert_eq!(literal("-2147483648i32"), ArgumentValue::Int32(i32::MIN));
        assert_eq!(literal("2147483647i32"), ArgumentValue::Int32(i32::MAX));
        assert_eq!(literal("200u8"), ArgumentValue::Uint8(200));
        assert_eq!(literal("255u8"), ArgumentValue::Uint8(u8::MAX));
        for hostile in [
            "7i64",
            "7u16",
            "2147483648i32",
            "-2147483649i32",
            "256u8",
            "-1u8",
            "007i32",
            "0u8x",
            "7 i32",
        ] {
            assert!(parse_argument(hostile).is_err(), "{hostile}");
        }

        // Floats follow the language grammar: required fraction, optional
        // exponent, optional `f32`/`f64` suffix, finite values only.
        assert_eq!(literal("1.5"), ArgumentValue::Float64(1.5));
        match literal("-0.0") {
            ArgumentValue::Float64(value) => {
                assert!(value.is_sign_negative());
                assert_eq!(value, 0.0f64);
            }
            other => panic!("-0.0 parsed as {other:?}"),
        }
        assert_eq!(literal("2.5e-3"), ArgumentValue::Float64(2.5e-3));
        assert_eq!(literal("1.0f64"), ArgumentValue::Float64(1.0));
        match literal("0.25f32") {
            ArgumentValue::Float32(value) => assert_eq!(value, 0.25f32),
            other => panic!("0.25f32 parsed as {other:?}"),
        }
        // An f32 literal rounds once in f32 precision.
        match literal("0.1f32") {
            ArgumentValue::Float32(value) => assert_eq!(value.to_bits(), 0.1f32.to_bits()),
            other => panic!("0.1f32 parsed as {other:?}"),
        }
        for hostile in [
            "inf", "-inf", "nan", "1.", ".5", "1e5", "1.5x", "1.5e", "1.5e+", "1.0e9999", "00.5",
            "1.5f32x", "1.5.6",
        ] {
            assert!(parse_argument(hostile).is_err(), "{hostile}");
        }

        // Chars use the language escape vocabulary.
        assert_eq!(literal("'a'"), ArgumentValue::Char('a' as u32));
        assert_eq!(literal("'\\n'"), ArgumentValue::Char('\n' as u32));
        assert_eq!(literal("'\\0'"), ArgumentValue::Char('\0' as u32));
        assert_eq!(literal("'\\\\'"), ArgumentValue::Char('\\' as u32));
        assert_eq!(literal("'\\''"), ArgumentValue::Char('\'' as u32));
        assert_eq!(literal("'\\u{2603}'"), ArgumentValue::Char(0x2603));
        for hostile in [
            "''",
            "'ab'",
            "'a",
            "a'",
            "'\\x41'",
            "'\\u{}'",
            "'\\u{110000}'",
            "'\\u{d800}'",
            "'\\u{1234567}'",
            "'",
            "'\\q'",
        ] {
            assert!(parse_argument(hostile).is_err(), "{hostile}");
        }
    }

    #[test]
    fn widened_scalar_renderings_are_canonical_and_replayable() {
        let cases = [
            (ArgumentValue::Int(-22), ("i64", "-22")),
            (ArgumentValue::Int32(-7), ("i32", "-7i32")),
            (ArgumentValue::Uint8(255), ("u8", "255u8")),
            (ArgumentValue::Bool(true), ("bool", "true")),
        ];
        for (value, (type_text, rendered)) in cases {
            assert_eq!(value.type_text(), type_text);
            assert_eq!(value.render(), rendered);
            assert!(canonical_scalar_value_matches(type_text, &value.render()));
        }
        // Char rendering is the canonical language literal and replays.
        let snowman = ArgumentValue::Char(0x2603);
        assert_eq!(snowman.type_text(), "char");
        assert_eq!(snowman.render(), "'\\u{2603}'");
        assert!(canonical_scalar_value_matches("char", "'\\u{2603}'"));
        assert!(canonical_scalar_value_matches("char", "'a'"));
        assert!(!canonical_scalar_value_matches("char", "'\\u{61}'"));
        // Float rendering is the exact big-endian bit pattern.
        assert_eq!(
            ArgumentValue::Float64(-2.5).render(),
            format!("{:016x}", (-2.5f64).to_bits())
        );
        assert_eq!(
            ArgumentValue::Float32(2.0).render(),
            format!("{:08x}", 2.0f32.to_bits())
        );
        assert!(canonical_scalar_value_matches(
            "f64",
            &ArgumentValue::Float64(f64::INFINITY).render()
        ));
        assert!(canonical_scalar_value_matches(
            "f32",
            &ArgumentValue::Float32(f32::NAN).render()
        ));
        assert!(!canonical_scalar_value_matches("f32", "4000000"));
        assert!(!canonical_scalar_value_matches("f64", "40000000000000G0"));
    }

    #[test]
    fn arithmetic_status_table_matches_the_compiler_v1_codes() {
        let cases = [
            (StatusCase::AddOverflow, 1),
            (StatusCase::SubOverflow, 2),
            (StatusCase::MulOverflow, 3),
            (StatusCase::DivisionByZero, 4),
            (StatusCase::DivisionOverflow, 5),
            (StatusCase::RemainderByZero, 6),
            (StatusCase::RemainderOverflow, 7),
            (StatusCase::NegationOverflow, 8),
        ];
        for (case, code) in cases {
            assert_eq!(status_case_from_code(code), Some(case));
            assert_eq!(status_case_from_code(case.code()), Some(case));
            assert_eq!(normalize_arithmetic(case).code(), code);
        }
        assert_eq!(status_case_from_code(0), None);
        assert_eq!(status_case_from_code(9), None);
    }

    #[test]
    fn checked_semantics_match_the_native_helpers_exactly() {
        use crate::conformance::StatusClass;
        // i64 boundaries.
        match combine(BinaryOp::Add, Value::Int(i64::MAX), Value::Int(1)) {
            Some(Err(status)) => {
                assert_eq!(status.code(), StatusCase::AddOverflow.code());
                assert_eq!(status.class(), StatusClass::Arithmetic);
            }
            other => panic!("expected overflow status, found {other:?}"),
        }
        assert!(matches!(
            combine(BinaryOp::Div, Value::Int(-5), Value::Int(0)),
            Some(Err(_))
        ));
        assert_eq!(
            combine(BinaryOp::Div, Value::Int(i64::MIN), Value::Int(-1)),
            Some(Err(normalize_arithmetic(StatusCase::DivisionOverflow)))
        );
        assert_eq!(
            combine(BinaryOp::Rem, Value::Int(i64::MIN), Value::Int(-1)),
            Some(Err(normalize_arithmetic(StatusCase::RemainderOverflow)))
        );
        assert_eq!(
            combine(BinaryOp::Div, Value::Int(-7), Value::Int(2)),
            Some(Ok(Value::Int(-3)))
        );
        assert_eq!(
            combine(BinaryOp::Rem, Value::Int(-7), Value::Int(2)),
            Some(Ok(Value::Int(-1)))
        );

        // u8 and i32 range checks select the same stable statuses.
        assert_eq!(
            combine(BinaryOp::Add, Value::Uint8(250), Value::Uint8(10)),
            Some(Err(normalize_arithmetic(StatusCase::AddOverflow)))
        );
        assert_eq!(
            combine(BinaryOp::Mul, Value::Int32(100_000), Value::Int32(100_000)),
            Some(Err(normalize_arithmetic(StatusCase::MulOverflow)))
        );

        // IEEE-754 comparisons stay total; NaN is unordered everywhere.
        let nan = f64::NAN;
        assert_eq!(
            combine(BinaryOp::Lt, Value::Float64(nan), Value::Float64(1.0)),
            Some(Ok(Value::Bool(false)))
        );
        assert_eq!(
            combine(BinaryOp::Ne, Value::Float64(nan), Value::Float64(nan)),
            Some(Ok(Value::Bool(true)))
        );
        assert_eq!(
            combine(BinaryOp::Eq, Value::Float64(0.0), Value::Float64(-0.0)),
            Some(Ok(Value::Bool(true)))
        );

        // Lazy operators never reach typed combination; ill-typed pairs fail.
        assert_eq!(
            combine(BinaryOp::And, Value::Bool(true), Value::Bool(false)),
            None
        );
        assert_eq!(
            combine(BinaryOp::Add, Value::Bool(true), Value::Int(1)),
            None
        );
    }

    #[test]
    fn domain_digest_is_domain_separated() {
        assert_ne!(
            domain_digest(SOURCE_DIGEST_DOMAIN, b"abc"),
            domain_digest(PAYLOAD_DIGEST_DOMAIN, b"abc")
        );
        assert_eq!(
            domain_digest(SOURCE_DIGEST_DOMAIN, b"abc"),
            domain_digest(SOURCE_DIGEST_DOMAIN, b"abc")
        );
    }
}
