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
//! plus verified internal Portable Indexed Byte Data v1 fixed arrays, owned
//! immutable `Bytes`, non-escaping byte views, and compiler-owned byte
//! operations. Exact flat monomorphic records containing direct `Bytes` and
//! Copy-scalar fields, flat monomorphic owned-byte variants, plus
//! compiler-owned `Option<Bytes>` and the exact
//! `Result<Bytes, i64|bool>`/`Result<i64|bool, Bytes>` instances, are admitted
//! only inside that resolved closure. Their explicit `match own`/`match borrow`
//! forms preserve unique ownership without widening the public interpreter
//! boundary. Other aggregate construction/projection/
//! update, variant construction or matching, postfix `?`, import calls,
//! generic calls, place projections, strings at the boundary, and
//! backend-unlowerable scalar
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

mod api_admission;
mod expression_children;
mod failure_detail;
pub mod internal_strings;
mod nested_owned;
pub(crate) mod network;
mod prepared;
mod resolved_case;

pub use failure_detail::{ContractArgument, ContractFailureDetail};
pub(crate) use resolved_case::evaluate_resolved_zero_arg_i64_function;

use api_admission::{
    owned_utf8_api_result_matches, public_api_argument_matches, public_api_parameter_type_matches,
    public_api_result_is_admitted, require_acyclic_public_api_closure,
    resolved_owned_utf8_signature_is_admitted, validate_flat_owned_record_result_shape,
    validate_public_api_borrowed_input_bound,
};
use expression_children::child_expressions;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

use sha2::{Digest as _, Sha256};

use crate::ast::{BinaryOp, Function, ParamMode, Program, Type, UnaryOp};
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::cleanup_plan::{ContractPhase, StatusCase};
use crate::conformance::{NormalizedStatus, Retryability, StatusClass};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedStatement,
    ResolvedType, ValueId,
};
use crate::runtime_status::{normalize_arithmetic, normalize_contract};
use crate::{graph, hir, parse, patch, verify};

pub(crate) use prepared::{
    evaluate_prepared_resolved_zero_arg_i64, prepare_resolved_zero_arg_i64,
    trace_child_expressions, PreparedCancellation, PreparedResolvedEvaluation,
    PreparedResolvedEvaluationOutcome, PreparedResolvedI64, ResolvedTraceEvent, ResolvedTracePhase,
    MAX_PREPARED_INDEX_BYTES, MAX_PREPARED_ORIGIN_NODES,
};

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

/// Fixed cumulative logical UTF-8 materialization count for one retained
/// Project-v10 evaluation. Callers cannot widen this interpreter capacity.
pub const MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS: u64 = 4_096;

/// Fixed cumulative logical UTF-8 payload bytes for one retained Project-v10
/// evaluation. This bounds logical string payload, not allocator metadata.
pub const MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES: u64 = 65_536;

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
#[derive(Clone, Debug, PartialEq)]
pub enum ArgumentValue {
    Int(i64),
    Int32(i32),
    Uint8(u8),
    Usize(u64),
    Char(u32),
    Float32(f32),
    Float64(f64),
    Bool(bool),
    BorrowedStr(String),
    BorrowedSlice(Vec<u8>),
}

impl ArgumentValue {
    fn type_text(&self) -> &'static str {
        match self {
            Self::Int(_) => "i64",
            Self::Int32(_) => "i32",
            Self::Uint8(_) => "u8",
            Self::Usize(_) => "usize",
            Self::Char(_) => "char",
            Self::Float32(_) => "f32",
            Self::Float64(_) => "f64",
            Self::Bool(_) => "bool",
            Self::BorrowedStr(_) => "str",
            Self::BorrowedSlice(_) => "Slice<u8>",
        }
    }

    fn render(&self) -> String {
        match self {
            Self::Int(value) => value.to_string(),
            // Suffixed widths always render with their explicit suffix: bare
            // decimals canonically denote `i64`, so the suffix is what keeps
            // each rendering uniquely replayable.
            Self::Int32(value) => format!("{value}i32"),
            Self::Uint8(value) => format!("{value}u8"),
            Self::Usize(value) => format!("{value}usize"),
            Self::Char(value) => crate::format::canonical_char(*value),
            Self::Float32(value) => format!("{:08x}", value.to_bits()),
            Self::Float64(value) => format!("{:016x}", value.to_bits()),
            Self::Bool(value) => value.to_string(),
            Self::BorrowedStr(value) => {
                serde_json::to_string(value).expect("Rust strings always serialize as JSON")
            }
            Self::BorrowedSlice(value) => serde_json::to_string(value)
                .expect("byte arrays always serialize as canonical JSON"),
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
    if text.starts_with('[') {
        let value = serde_json::from_str::<Vec<u8>>(text).map_err(|_| {
            argument_error(format!(
                "argument `{text}` is not a canonical byte array literal"
            ))
        })?;
        if serde_json::to_string(&value).ok().as_deref() != Some(text) {
            return Err(argument_error(format!(
                "argument `{text}` is not a canonical byte array literal"
            )));
        }
        if value.len() as u64 > crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES {
            return Err(argument_error(format!(
                "borrowed `Slice<u8>` argument exceeds the {}-byte profile limit",
                crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES
            )));
        }
        return Ok(ArgumentValue::BorrowedSlice(value));
    }
    if text.starts_with('"') {
        let value = serde_json::from_str::<String>(text).map_err(|_| {
            argument_error(format!(
                "argument `{text}` is not a canonical UTF-8 string literal"
            ))
        })?;
        if serde_json::to_string(&value).ok().as_deref() != Some(text) {
            return Err(argument_error(format!(
                "argument `{text}` is not a canonical UTF-8 string literal"
            )));
        }
        if value.len() > crate::str_ops::MAX_BORROWED_STR_BYTES {
            return Err(argument_error(format!(
                "borrowed `str` argument exceeds the {}-byte profile limit",
                crate::str_ops::MAX_BORROWED_STR_BYTES
            )));
        }
        return Ok(ArgumentValue::BorrowedStr(value));
    }
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
        "usize" => {
            if sign == "-" {
                return Err(argument_error(format!(
                    "argument `{text}` is outside the usize range"
                )));
            }
            digits
                .parse::<u64>()
                .map(ArgumentValue::Usize)
                .map_err(|_| {
                    argument_error(format!("argument `{text}` is outside the usize range"))
                })
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
    /// The violated clause and call frame when `outcome` is a contract failure
    /// observed by an evaluator that records frame detail.
    pub failure: Option<ContractFailureDetail>,
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

/// One normalized public owned-data value returned by the reference
/// interpreter. The outer invocation outcome remains separate from a
/// successful language-level `Result::Err`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedDataValue {
    Bytes(Vec<u8>),
    OptionBytes(Option<Vec<u8>>),
    ResultBytesI64(Result<Vec<u8>, i64>),
}

/// Closed outcomes for the read-only public owned-data interpreter profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedDataEvaluationOutcome {
    Returned(OwnedDataValue),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    GuardError(String),
}

/// Boundary cleanup observed while copying an interpreter-owned result into
/// its public host value. Each active returned `Bytes` carrier produces
/// exactly one event; `None` and `Result::Err` produce none.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedDataCleanupEvent {
    CopyOutAndSettleBytes,
}

/// Deterministic, authority-free facts for one owned-data interpretation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedDataEvaluation {
    pub function_id: DeclarationId,
    pub outcome: OwnedDataEvaluationOutcome,
    pub cleanup_events: Vec<OwnedDataCleanupEvent>,
    pub steps_used: usize,
    pub max_steps: usize,
}

/// One borrowed Project-v8/v9 public invocation argument. Borrowed host
/// carriers are snapshotted before evaluation and never become interpreter
/// ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicApiArgument<'a> {
    I64(i64),
    Bool(bool),
    BorrowStr(&'a str),
    BorrowSliceU8(&'a [u8]),
}

/// One normalized Project-v8 value returned by the reference interpreter.
/// Active byte payloads are copied into ordinary host-owned vectors first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicApiValue {
    I64(i64),
    Bool(bool),
    Usize(u64),
    Bytes(Vec<u8>),
    OptionBytes(Option<Vec<u8>>),
    ResultBytesI64(Result<Vec<u8>, i64>),
}

/// Closed outcome vocabulary for one Project-v8 public invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicApiEvaluationOutcome {
    Returned(PublicApiValue),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    GuardError(String),
}

/// Authority-free execution facts for one exact selected Project-v8 export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicApiEvaluation {
    pub function_id: DeclarationId,
    pub outcome: PublicApiEvaluationOutcome,
    pub cleanup_events: Vec<OwnedDataCleanupEvent>,
    pub steps_used: usize,
    pub max_steps: usize,
}

/// One normalized Project-v10 value. Owned UTF-8 is kept distinct from the
/// Project-v8 vocabulary so older callers cannot silently acquire a new
/// result shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedUtf8ApiValue {
    I64(i64),
    Bool(bool),
    Usize(u64),
    Bytes(Vec<u8>),
    OptionBytes(Option<Vec<u8>>),
    ResultBytesI64(Result<Vec<u8>, i64>),
    Utf8(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedUtf8ApiEvaluationOutcome {
    Returned(OwnedUtf8ApiValue),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    Utf8MaterializationLimitExceeded {
        attempted_materializations: u64,
        attempted_bytes: u64,
    },
    GuardError(String),
}

/// Public-boundary settlement after a private interpreter carrier has been
/// consumed. Internal drops do not create settlement evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedUtf8SettlementEvent {
    CopyOutAndSettleBytes,
    CopyOutAndSettleUtf8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedUtf8ApiEvaluation {
    pub function_id: DeclarationId,
    pub outcome: OwnedUtf8ApiEvaluationOutcome,
    pub settlement_events: Vec<OwnedUtf8SettlementEvent>,
    pub steps_used: usize,
    pub max_steps: usize,
    pub utf8_materializations_used: u64,
    pub utf8_materializations_max: u64,
    pub utf8_bytes_used: u64,
    pub utf8_bytes_max: u64,
}

/// One identity-bound Project-v9 flat-record member. The vector containing
/// these values is always in authenticated descriptor order; it is not a
/// target layout or an offset-bearing aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlatOwnedRecordMemberValue {
    I64(i64),
    Bool(bool),
    Usize(u64),
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlatOwnedRecordMember {
    pub field_id: DeclarationId,
    pub value: FlatOwnedRecordMemberValue,
}

/// Target-neutral copy-out of one authenticated Project-v9 result record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlatOwnedRecordValue {
    pub record_id: DeclarationId,
    pub fields: Vec<FlatOwnedRecordMember>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlatOwnedRecordEvaluationOutcome {
    Returned(FlatOwnedRecordValue),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    GuardError(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlatOwnedRecordEvaluation {
    pub function_id: DeclarationId,
    pub outcome: FlatOwnedRecordEvaluationOutcome,
    pub cleanup_events: Vec<OwnedDataCleanupEvent>,
    pub steps_used: usize,
    pub max_steps: usize,
}

/// Closed outcomes for one hosted Language Command I/O v1 invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandEvaluationOutcome {
    ReturnedBool(bool),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    GuardError(String),
}

/// Deterministic execution facts for one hosted language-command invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandEvaluation {
    pub outcome: CommandEvaluationOutcome,
    pub steps_used: usize,
    pub max_steps: usize,
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
    let evaluated = resolved_case::evaluate_resolved_zero_arg_i64_function(
        program,
        entry_id,
        max_steps,
        true,
        PreparedCancellation::Never,
    )?;
    let outcome = match evaluated.outcome {
        PreparedResolvedEvaluationOutcome::ReturnedI64(value) => {
            ResolvedEvaluationOutcome::ReturnedI64(value)
        }
        PreparedResolvedEvaluationOutcome::LanguageFailure(status) => {
            ResolvedEvaluationOutcome::LanguageFailure(status)
        }
        PreparedResolvedEvaluationOutcome::FuelExhausted => {
            ResolvedEvaluationOutcome::FuelExhausted
        }
        PreparedResolvedEvaluationOutcome::CallDepthExceeded => {
            ResolvedEvaluationOutcome::CallDepthExceeded
        }
        PreparedResolvedEvaluationOutcome::Cancelled { .. } => {
            ResolvedEvaluationOutcome::GuardError(
                "unexpected cancellation in legacy resolved evaluation".to_owned(),
            )
        }
        PreparedResolvedEvaluationOutcome::GuardError(detail) => {
            ResolvedEvaluationOutcome::GuardError(detail)
        }
    };
    Ok(ResolvedEvaluation {
        outcome,
        steps_used: evaluated.steps_used,
        max_steps: evaluated.max_steps,
        failure: evaluated.failure,
    })
}

/// Evaluate one exact public owned-data export from already-validated HIR.
///
/// This is a read-only reference lane: it accepts exactly one invocation-
/// borrowed `Slice<u8>` and one of the three Project-v8 result shapes
/// (`Bytes`, `Option<Bytes>`, or `Result<Bytes, i64>`). It performs no target
/// execution and grants no filesystem, process, publication, or raw-carrier
/// authority. Copy-out consumes the interpreter carrier and records its one
/// normalized settlement event.
pub fn evaluate_resolved_owned_data(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    input: &[u8],
    max_steps: usize,
) -> Result<OwnedDataEvaluation, Vec<Diagnostic>> {
    hir::validate(program).map_err(|diagnostic| vec![diagnostic])?;
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "owned-data evaluation max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        ))]);
    }
    if input.len() as u64 > crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES {
        return Err(vec![argument_error(format!(
            "owned-data argument exceeds {} bytes",
            crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES
        ))]);
    }
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == entry_id)
        .ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("resolved owned-data export `{entry_id}` is absent"),
            )]
        })?;
    let explicit_entry = program
        .declarations
        .declaration(&entry.id)
        .is_some_and(|declaration| declaration.identity_origin == hir::IdentityOrigin::Explicit);
    if !explicit_entry {
        return Err(vec![selection_error(
            REASON_AUTOMATIC_IDENTITY,
            format!("resolved owned-data export `{entry_id}` has no explicit stable identity"),
        )]);
    }
    let [parameter] = entry.params.as_slice() else {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_PARAMETER_TYPE,
            format!("resolved owned-data export `{entry_id}` must take one borrowed Slice<u8>"),
        )]);
    };
    if parameter.ty != ResolvedType::SliceU8 || parameter.ownership != hir::OwnershipMode::Borrow {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_PARAMETER_TYPE,
            format!("resolved owned-data export `{entry_id}` must take one borrowed Slice<u8>"),
        )]);
    }
    if !owned_data_result_is_admitted(&entry.return_type) {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!(
                "resolved owned-data export `{entry_id}` must return Bytes, Option<Bytes>, or Result<Bytes, i64>"
            ),
        )]);
    }
    if !entry.effects.is_empty() {
        return Err(vec![selection_error(
            REASON_DECLARED_EFFECTS,
            format!("resolved owned-data export `{entry_id}` declares effects"),
        )]);
    }

    hir::analyze_byte_data_capacity(program).map_err(|diagnostic| vec![diagnostic])?;
    let admitted = admitted_resolved_functions(program);
    scan_closure(entry_id, &admitted, &program.declarations, true)?;
    let arguments = [(
        parameter.name.clone(),
        ArgumentValue::BorrowedSlice(input.to_vec()),
    )];
    let return_type = entry.return_type.clone();

    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("semaprax-owned-data-evaluate".to_owned())
            .stack_size(EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, || {
                let (evaluated, steps_used, _) = evaluate_resolved_entry(
                    entry,
                    &arguments,
                    &admitted,
                    &program.declarations,
                    max_steps,
                    false,
                );
                let mut cleanup_events = Vec::with_capacity(1);
                let outcome = match evaluated {
                    Ok(value) => {
                        match copy_out_owned_data(value, &return_type, &mut cleanup_events) {
                            Ok(value) => OwnedDataEvaluationOutcome::Returned(value),
                            Err(detail) => {
                                OwnedDataEvaluationOutcome::GuardError(detail.to_owned())
                            }
                        }
                    }
                    Err(Flow::Failure(status)) => {
                        OwnedDataEvaluationOutcome::LanguageFailure(status)
                    }
                    Err(Flow::Exhausted) => OwnedDataEvaluationOutcome::FuelExhausted,
                    Err(Flow::DepthExceeded) => OwnedDataEvaluationOutcome::CallDepthExceeded,
                    Err(Flow::Cancelled { .. }) => OwnedDataEvaluationOutcome::GuardError(
                        "unexpected cancellation in legacy owned-data evaluation".to_owned(),
                    ),
                    Err(Flow::Utf8MaterializationLimitExceeded { .. }) => {
                        OwnedDataEvaluationOutcome::GuardError(
                            "unexpected UTF-8 materialization limit in legacy owned-data evaluation"
                                .to_owned(),
                        )
                    }
                    Err(Flow::Guard(detail)) => {
                        OwnedDataEvaluationOutcome::GuardError(detail.to_owned())
                    }
                };
                OwnedDataEvaluation {
                    function_id: entry.id.clone(),
                    outcome,
                    cleanup_events,
                    steps_used,
                    max_steps,
                }
            })
            .map_err(|error| {
                vec![guard_error(&format!(
                    "owned-data evaluation thread failed to start: {error}"
                ))]
            })?;
        worker.join().map_err(|_| {
            vec![guard_error(
                "owned-data evaluation thread panicked after HIR validation",
            )]
        })
    })
}

/// Evaluate one exact Project-v8 public export from already-validated HIR.
///
/// Selection authority remains with the caller's authenticated Project
/// revision. This function accepts no descriptor, filesystem or process
/// authority and does not infer an export set from the program.
pub(crate) fn evaluate_resolved_public_api(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    arguments: &[PublicApiArgument<'_>],
    max_steps: usize,
) -> Result<PublicApiEvaluation, Vec<Diagnostic>> {
    hir::validate(program).map_err(|diagnostic| vec![diagnostic])?;
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "public API evaluation max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        ))]);
    }
    if arguments.len() > crate::project::MAX_PUBLIC_API_PARAMETERS {
        return Err(vec![argument_error(format!(
            "public API invocation exceeds {} parameters",
            crate::project::MAX_PUBLIC_API_PARAMETERS
        ))]);
    }
    let mut borrowed_bytes = 0usize;
    for argument in arguments {
        let length = match argument {
            PublicApiArgument::BorrowStr(value) => value.len(),
            PublicApiArgument::BorrowSliceU8(value) => value.len(),
            PublicApiArgument::I64(_) | PublicApiArgument::Bool(_) => 0,
        };
        borrowed_bytes = borrowed_bytes.checked_add(length).ok_or_else(|| {
            vec![argument_error(
                "public API cumulative borrowed input byte count overflowed".to_owned(),
            )]
        })?;
        if borrowed_bytes > crate::project::MAX_PUBLIC_API_BORROWED_INPUT_BYTES {
            return Err(vec![argument_error(format!(
                "public API cumulative borrowed input exceeds {} bytes",
                crate::project::MAX_PUBLIC_API_BORROWED_INPUT_BYTES
            ))]);
        }
    }
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == entry_id)
        .ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("resolved public API export `{entry_id}` is absent"),
            )]
        })?;
    let explicit_entry = program
        .declarations
        .declaration(&entry.id)
        .is_some_and(|declaration| declaration.identity_origin == hir::IdentityOrigin::Explicit);
    if !explicit_entry {
        return Err(vec![selection_error(
            REASON_AUTOMATIC_IDENTITY,
            format!("resolved public API export `{entry_id}` has no explicit stable identity"),
        )]);
    }
    if entry.params.len() != arguments.len() {
        return Err(vec![argument_error(format!(
            "resolved public API export `{entry_id}` takes {} argument(s), {} were provided",
            entry.params.len(),
            arguments.len()
        ))]);
    }
    for (index, (parameter, argument)) in entry.params.iter().zip(arguments).enumerate() {
        let admitted = public_api_argument_matches(parameter, argument);
        if !admitted {
            return Err(vec![selection_error(
                REASON_UNSUPPORTED_PARAMETER_TYPE,
                format!(
                    "resolved public API export `{entry_id}` parameter {index} disagrees with its exact Project-v8 argument"
                ),
            )]);
        }
    }
    if !public_api_result_is_admitted(&entry.return_type) {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!("resolved public API export `{entry_id}` has a non-v8 result type"),
        )]);
    }
    if !entry.effects.is_empty() {
        return Err(vec![selection_error(
            REASON_DECLARED_EFFECTS,
            format!("resolved public API export `{entry_id}` declares effects"),
        )]);
    }
    if !entry.requires.is_empty() || !entry.ensures.is_empty() {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("resolved public API export `{entry_id}` declares contracts"),
        )]);
    }

    hir::analyze_byte_data_capacity(program).map_err(|diagnostic| vec![diagnostic])?;
    let admitted = admitted_resolved_functions(program);
    if !admitted.contains_key(entry_id) {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("resolved public API export `{entry_id}` is outside the interpreter profile"),
        )]);
    }
    let closure = scan_closure(entry_id, &admitted, &program.declarations, true)?;
    if closure.len() > crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!(
                "public API selected closure exceeds {} functions",
                crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS
            ),
        )]);
    }
    for id in &closure {
        let function = admitted.get(id.as_str()).ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("public API closure function `{id}` is not admitted"),
            )]
        })?;
        if !function.effects.is_empty()
            || !function.requires.is_empty()
            || !function.ensures.is_empty()
        {
            return Err(vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("public API closure function `{id}` must be effect- and contract-free"),
            )]);
        }
    }
    require_acyclic_public_api_closure(entry_id, &admitted)?;

    // Snapshot borrowed carriers before spawning the fixed-stack evaluator.
    let arguments = entry
        .params
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            let value = match argument {
                PublicApiArgument::I64(value) => ArgumentValue::Int(*value),
                PublicApiArgument::Bool(value) => ArgumentValue::Bool(*value),
                PublicApiArgument::BorrowStr(value) => {
                    ArgumentValue::BorrowedStr((*value).to_owned())
                }
                PublicApiArgument::BorrowSliceU8(value) => {
                    ArgumentValue::BorrowedSlice((*value).to_vec())
                }
            };
            (parameter.name.clone(), value)
        })
        .collect::<Vec<_>>();
    let return_type = entry.return_type.clone();

    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("semaprax-public-api-evaluate".to_owned())
            .stack_size(EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, || {
                let (evaluated, steps_used, _) = evaluate_resolved_entry(
                    entry,
                    &arguments,
                    &admitted,
                    &program.declarations,
                    max_steps,
                    false,
                );
                let mut cleanup_events = Vec::with_capacity(1);
                let outcome = match evaluated {
                    Ok(value) => {
                        match copy_out_public_api(value, &return_type, &mut cleanup_events) {
                            Ok(value) => PublicApiEvaluationOutcome::Returned(value),
                            Err(detail) => {
                                PublicApiEvaluationOutcome::GuardError(detail.to_owned())
                            }
                        }
                    }
                    Err(Flow::Failure(status)) => {
                        PublicApiEvaluationOutcome::LanguageFailure(status)
                    }
                    Err(Flow::Exhausted) => PublicApiEvaluationOutcome::FuelExhausted,
                    Err(Flow::DepthExceeded) => PublicApiEvaluationOutcome::CallDepthExceeded,
                    Err(Flow::Cancelled { .. }) => PublicApiEvaluationOutcome::GuardError(
                        "unexpected cancellation in public API evaluation".to_owned(),
                    ),
                    Err(Flow::Utf8MaterializationLimitExceeded { .. }) => {
                        PublicApiEvaluationOutcome::GuardError(
                            "unexpected UTF-8 materialization limit in Project-v8 evaluation"
                                .to_owned(),
                        )
                    }
                    Err(Flow::Guard(detail)) => {
                        PublicApiEvaluationOutcome::GuardError(detail.to_owned())
                    }
                };
                PublicApiEvaluation {
                    function_id: entry.id.clone(),
                    outcome,
                    cleanup_events,
                    steps_used,
                    max_steps,
                }
            })
            .map_err(|error| {
                vec![guard_error(&format!(
                    "public API evaluation thread failed to start: {error}"
                ))]
            })?;
        worker.join().map_err(|_| {
            vec![guard_error(
                "public API evaluation thread panicked after HIR validation",
            )]
        })
    })
}

/// Evaluate one exact Project-v9 flat owned-record export from validated HIR.
///
/// The caller supplies the independently replayed descriptor export. This
/// lane rechecks its complete identity/type inventory against HIR, then keeps
/// the runtime record private until its sole byte carrier has been copied and
/// settled. Descriptor order is preserved without exposing a target layout.
pub(crate) fn evaluate_resolved_flat_owned_record_api(
    program: &hir::ResolvedProgram,
    expected: &crate::project::FlatOwnedRecordExport,
    arguments: &[PublicApiArgument<'_>],
    max_steps: usize,
) -> Result<FlatOwnedRecordEvaluation, Vec<Diagnostic>> {
    hir::validate(program).map_err(|diagnostic| vec![diagnostic])?;
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "flat owned-record evaluation max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        ))]);
    }
    if arguments.len() > crate::project::MAX_PUBLIC_API_PARAMETERS {
        return Err(vec![argument_error(format!(
            "flat owned-record invocation exceeds {} parameters",
            crate::project::MAX_PUBLIC_API_PARAMETERS
        ))]);
    }
    validate_public_api_borrowed_input_bound(arguments, "flat owned-record")?;

    let entry_id = expected.stable_id().as_str();
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == entry_id)
        .ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("resolved flat owned-record export `{entry_id}` is absent"),
            )]
        })?;
    if entry.id == program.entrypoint
        || program
            .declarations
            .declaration(&entry.id)
            .is_none_or(|declaration| declaration.identity_origin != hir::IdentityOrigin::Explicit)
    {
        return Err(vec![selection_error(
            REASON_AUTOMATIC_IDENTITY,
            format!(
                "resolved flat owned-record export `{entry_id}` is not an explicit non-entry identity"
            ),
        )]);
    }
    if entry.params.len() != expected.parameters().len() || entry.params.len() != arguments.len() {
        return Err(vec![argument_error(format!(
            "resolved flat owned-record export `{entry_id}` argument inventory disagrees with its descriptor"
        ))]);
    }
    for (index, ((parameter, expected_parameter), argument)) in entry
        .params
        .iter()
        .zip(expected.parameters())
        .zip(arguments)
        .enumerate()
    {
        let (expected_id, expected_name, expected_type) = expected_parameter;
        if parameter.id.as_str() != expected_id
            || parameter.name != *expected_name
            || !public_api_parameter_type_matches(parameter, *expected_type)
            || !public_api_argument_matches(parameter, argument)
        {
            return Err(vec![selection_error(
                REASON_UNSUPPORTED_PARAMETER_TYPE,
                format!(
                    "resolved flat owned-record export `{entry_id}` parameter {index} disagrees with its exact descriptor and argument"
                ),
            )]);
        }
    }
    validate_flat_owned_record_result_shape(program, entry, expected)?;
    if !entry.effects.is_empty() {
        return Err(vec![selection_error(
            REASON_DECLARED_EFFECTS,
            format!("resolved flat owned-record export `{entry_id}` declares effects"),
        )]);
    }
    if !entry.requires.is_empty() || !entry.ensures.is_empty() {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("resolved flat owned-record export `{entry_id}` declares contracts"),
        )]);
    }

    hir::analyze_byte_data_capacity(program).map_err(|diagnostic| vec![diagnostic])?;
    let admitted = admitted_resolved_functions(program);
    if !admitted.contains_key(entry_id) {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!(
                "resolved flat owned-record export `{entry_id}` is outside the interpreter profile"
            ),
        )]);
    }
    let closure = scan_closure(entry_id, &admitted, &program.declarations, true)?;
    if closure.len() > crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!(
                "flat owned-record selected closure exceeds {} functions",
                crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS
            ),
        )]);
    }
    for id in &closure {
        let function = admitted.get(id.as_str()).ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("flat owned-record closure function `{id}` is not admitted"),
            )]
        })?;
        if !function.effects.is_empty()
            || !function.requires.is_empty()
            || !function.ensures.is_empty()
        {
            return Err(vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!(
                    "flat owned-record closure function `{id}` must be effect- and contract-free"
                ),
            )]);
        }
    }
    require_acyclic_public_api_closure(entry_id, &admitted)?;

    let arguments = entry
        .params
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            let value = match argument {
                PublicApiArgument::I64(value) => ArgumentValue::Int(*value),
                PublicApiArgument::Bool(value) => ArgumentValue::Bool(*value),
                PublicApiArgument::BorrowStr(value) => {
                    ArgumentValue::BorrowedStr((*value).to_owned())
                }
                PublicApiArgument::BorrowSliceU8(value) => {
                    ArgumentValue::BorrowedSlice((*value).to_vec())
                }
            };
            (parameter.name.clone(), value)
        })
        .collect::<Vec<_>>();

    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("semaprax-flat-owned-record-evaluate".to_owned())
            .stack_size(EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, || {
                let (evaluated, steps_used, _) = evaluate_resolved_entry(
                    entry,
                    &arguments,
                    &admitted,
                    &program.declarations,
                    max_steps,
                    false,
                );
                let mut cleanup_events = Vec::with_capacity(1);
                let outcome = match evaluated {
                    Ok(value) => {
                        match copy_out_flat_owned_record(value, expected, &mut cleanup_events) {
                            Ok(value) => FlatOwnedRecordEvaluationOutcome::Returned(value),
                            Err(detail) => {
                                FlatOwnedRecordEvaluationOutcome::GuardError(detail.to_owned())
                            }
                        }
                    }
                    Err(Flow::Failure(status)) => {
                        FlatOwnedRecordEvaluationOutcome::LanguageFailure(status)
                    }
                    Err(Flow::Exhausted) => FlatOwnedRecordEvaluationOutcome::FuelExhausted,
                    Err(Flow::DepthExceeded) => FlatOwnedRecordEvaluationOutcome::CallDepthExceeded,
                    Err(Flow::Cancelled { .. }) => FlatOwnedRecordEvaluationOutcome::GuardError(
                        "unexpected cancellation in flat owned-record evaluation".to_owned(),
                    ),
                    Err(Flow::Utf8MaterializationLimitExceeded { .. }) => {
                        FlatOwnedRecordEvaluationOutcome::GuardError(
                            "unexpected UTF-8 materialization limit in Project-v9 evaluation"
                                .to_owned(),
                        )
                    }
                    Err(Flow::Guard(detail)) => {
                        FlatOwnedRecordEvaluationOutcome::GuardError(detail.to_owned())
                    }
                };
                FlatOwnedRecordEvaluation {
                    function_id: entry.id.clone(),
                    outcome,
                    cleanup_events,
                    steps_used,
                    max_steps,
                }
            })
            .map_err(|error| {
                vec![guard_error(&format!(
                    "flat owned-record evaluation thread failed to start: {error}"
                ))]
            })?;
        worker.join().map_err(|_| {
            vec![guard_error(
                "flat owned-record evaluation thread panicked after HIR validation",
            )]
        })
    })
}

/// Evaluate one exact Project-v10 descriptor export against already-validated
/// HIR. The caller owns descriptor authentication; this seam independently
/// replays every selected signature/identity fact and the complete reachable
/// closure before enabling the fixed UTF-8 materialization meter.
pub(crate) fn evaluate_resolved_owned_utf8_api(
    program: &hir::ResolvedProgram,
    expected: &crate::project::PublicApiExport,
    arguments: &[PublicApiArgument<'_>],
    max_steps: usize,
) -> Result<OwnedUtf8ApiEvaluation, Vec<Diagnostic>> {
    hir::validate(program).map_err(|diagnostic| vec![diagnostic])?;
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "owned UTF-8 API evaluation max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        ))]);
    }
    if arguments.len() > crate::project::MAX_PUBLIC_API_PARAMETERS {
        return Err(vec![argument_error(format!(
            "owned UTF-8 API invocation exceeds {} parameters",
            crate::project::MAX_PUBLIC_API_PARAMETERS
        ))]);
    }
    validate_public_api_borrowed_input_bound(arguments, "owned UTF-8 API")?;

    let entry_id = expected.stable_id().as_str();
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == entry_id)
        .ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("resolved owned UTF-8 export `{entry_id}` is absent"),
            )]
        })?;
    if entry.id == program.entrypoint
        || program
            .declarations
            .declaration(&entry.id)
            .is_none_or(|declaration| declaration.identity_origin != hir::IdentityOrigin::Explicit)
    {
        return Err(vec![selection_error(
            REASON_AUTOMATIC_IDENTITY,
            format!(
                "resolved owned UTF-8 export `{entry_id}` is not an explicit non-entry identity"
            ),
        )]);
    }
    if entry.params.len() != expected.parameters().len() || entry.params.len() != arguments.len() {
        return Err(vec![argument_error(format!(
            "resolved owned UTF-8 export `{entry_id}` argument inventory disagrees with its descriptor"
        ))]);
    }
    for (index, ((parameter, descriptor_parameter), argument)) in entry
        .params
        .iter()
        .zip(expected.parameters())
        .zip(arguments)
        .enumerate()
    {
        if parameter.id != *descriptor_parameter.stable_id()
            || parameter.name != descriptor_parameter.source_name()
            || !public_api_parameter_type_matches(parameter, descriptor_parameter.ty())
            || !public_api_argument_matches(parameter, argument)
        {
            return Err(vec![selection_error(
                REASON_UNSUPPORTED_PARAMETER_TYPE,
                format!(
                    "resolved owned UTF-8 export `{entry_id}` parameter {index} disagrees with its exact descriptor and argument"
                ),
            )]);
        }
    }
    if !owned_utf8_api_result_matches(&entry.return_type, expected.result()) {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!(
                "resolved owned UTF-8 export `{entry_id}` result disagrees with its exact descriptor"
            ),
        )]);
    }

    let admitted = program
        .functions
        .iter()
        .filter(|function| {
            resolved_owned_utf8_signature_is_admitted(function, &program.declarations)
        })
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    if !admitted.contains_key(entry_id) {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("resolved owned UTF-8 export `{entry_id}` is outside the interpreter profile"),
        )]);
    }
    let closure = scan_closure(entry_id, &admitted, &program.declarations, true)?;
    if closure.len() > crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!(
                "owned UTF-8 selected closure exceeds {} functions",
                crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS
            ),
        )]);
    }
    for id in &closure {
        let function = admitted.get(id.as_str()).ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("owned UTF-8 closure function `{id}` is not admitted"),
            )]
        })?;
        crate::project::validate_owned_utf8_closure_function(function).map_err(|detail| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("owned UTF-8 closure function `{id}` failed replay: {detail}"),
            )]
        })?;
    }
    require_acyclic_public_api_closure(entry_id, &admitted)?;

    // Borrowed host carriers are bounded above and snapshotted before the
    // evaluator thread. They are not owned UTF-8 runtime materializations.
    let arguments = entry
        .params
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            let value = match argument {
                PublicApiArgument::I64(value) => ArgumentValue::Int(*value),
                PublicApiArgument::Bool(value) => ArgumentValue::Bool(*value),
                PublicApiArgument::BorrowStr(value) => {
                    ArgumentValue::BorrowedStr((*value).to_owned())
                }
                PublicApiArgument::BorrowSliceU8(value) => {
                    ArgumentValue::BorrowedSlice((*value).to_vec())
                }
            };
            (parameter.name.clone(), value)
        })
        .collect::<Vec<_>>();
    let return_type = entry.return_type.clone();

    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("semaprax-owned-utf8-api-evaluate".to_owned())
            .stack_size(EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, || {
                let (evaluated, steps_used, _, (utf8_materializations_used, utf8_bytes_used)) =
                    evaluate_resolved_entry_with_utf8_budget(
                        entry,
                        &arguments,
                        &admitted,
                        &program.declarations,
                        max_steps,
                        false,
                        Utf8MaterializationBudget::fixed(),
                    );
                let mut settlement_events = Vec::with_capacity(1);
                let outcome = match evaluated {
                    Ok(value) => {
                        match copy_out_owned_utf8_api(value, &return_type, &mut settlement_events) {
                            Ok(value) => OwnedUtf8ApiEvaluationOutcome::Returned(value),
                            Err(detail) => {
                                OwnedUtf8ApiEvaluationOutcome::GuardError(detail.to_owned())
                            }
                        }
                    }
                    Err(Flow::Failure(status)) => {
                        OwnedUtf8ApiEvaluationOutcome::LanguageFailure(status)
                    }
                    Err(Flow::Exhausted) => OwnedUtf8ApiEvaluationOutcome::FuelExhausted,
                    Err(Flow::DepthExceeded) => OwnedUtf8ApiEvaluationOutcome::CallDepthExceeded,
                    Err(Flow::Utf8MaterializationLimitExceeded {
                        attempted_materializations,
                        attempted_bytes,
                    }) => OwnedUtf8ApiEvaluationOutcome::Utf8MaterializationLimitExceeded {
                        attempted_materializations,
                        attempted_bytes,
                    },
                    Err(Flow::Cancelled { .. }) => OwnedUtf8ApiEvaluationOutcome::GuardError(
                        "unexpected cancellation in owned UTF-8 API evaluation".to_owned(),
                    ),
                    Err(Flow::Guard(detail)) => {
                        OwnedUtf8ApiEvaluationOutcome::GuardError(detail.to_owned())
                    }
                };
                OwnedUtf8ApiEvaluation {
                    function_id: entry.id.clone(),
                    outcome,
                    settlement_events,
                    steps_used,
                    max_steps,
                    utf8_materializations_used,
                    utf8_materializations_max: MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS,
                    utf8_bytes_used,
                    utf8_bytes_max: MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES,
                }
            })
            .map_err(|error| {
                vec![guard_error(&format!(
                    "owned UTF-8 API evaluation thread failed to start: {error}"
                ))]
            })?;
        worker.join().map_err(|_| {
            vec![guard_error(
                "owned UTF-8 API evaluation thread panicked after HIR validation",
            )]
        })
    })
}

fn owned_data_result_is_admitted(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Bytes)
        || matches!(
            ty,
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if declaration.as_str() == crate::prelude::OPTION_ID
                && arguments.as_slice() == [ResolvedType::Bytes]
        )
        || matches!(
            ty,
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if declaration.as_str() == crate::prelude::RESULT_ID
                && arguments.as_slice() == [ResolvedType::Bytes, ResolvedType::I64]
        )
}

fn settle_interpreted_bytes(
    value: Value,
    cleanup_events: &mut Vec<OwnedDataCleanupEvent>,
) -> Result<Vec<u8>, &'static str> {
    let Value::Bytes(value) = value else {
        return Err("owned-data result payload is not Bytes");
    };
    if value.bytes.len() as u64 > crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES {
        return Err("owned-data result exceeds the public output bound");
    }
    let bytes = value.bytes.as_ref().to_vec();
    cleanup_events.push(OwnedDataCleanupEvent::CopyOutAndSettleBytes);
    Ok(bytes)
}

fn copy_out_flat_owned_record(
    value: Value,
    expected: &crate::project::FlatOwnedRecordExport,
    cleanup_events: &mut Vec<OwnedDataCleanupEvent>,
) -> Result<FlatOwnedRecordValue, &'static str> {
    let Value::Record(record) = value else {
        return Err("flat owned-record result is not the private record carrier");
    };
    if record.record != *expected.record_id() || record.fields.len() != expected.fields().len() {
        return Err(
            "flat owned-record carrier identity or field inventory disagrees with its descriptor",
        );
    }

    // Authenticate the complete carrier before consuming or publishing any
    // member. This includes the sole owned field and every scalar field.
    for field in expected.fields() {
        let Some(value) = record.fields.get(field.stable_id()) else {
            return Err("flat owned-record carrier is missing an authenticated field");
        };
        let type_matches = matches!(
            (field.ty(), value),
            (crate::project::FlatOwnedRecordFieldType::I64, Value::Int(_))
                | (
                    crate::project::FlatOwnedRecordFieldType::Bool,
                    Value::Bool(_)
                )
                | (
                    crate::project::FlatOwnedRecordFieldType::Usize,
                    Value::Usize(_)
                )
                | (
                    crate::project::FlatOwnedRecordFieldType::OwnedBytes,
                    Value::Bytes(_)
                )
        );
        if !type_matches {
            return Err("flat owned-record carrier field type disagrees with its descriptor");
        }
    }

    let mut record = Arc::try_unwrap(record)
        .map_err(|_| "flat owned-record result still has a live alias at copy-out")?;
    let owned_field = expected
        .fields()
        .iter()
        .find(|field| field.ty() == crate::project::FlatOwnedRecordFieldType::OwnedBytes)
        .ok_or("flat owned-record descriptor has no owned Bytes field")?;
    let owned_value = record
        .fields
        .remove(owned_field.stable_id())
        .ok_or("flat owned-record carrier lost its owned Bytes field")?;
    let settled_bytes = settle_interpreted_bytes(owned_value, cleanup_events)?;

    // Scalar host values are not constructed until byte copy and settlement
    // have both succeeded. Output remains in descriptor order.
    let mut settled_bytes = Some(settled_bytes);
    let mut fields = Vec::with_capacity(expected.fields().len());
    for field in expected.fields() {
        let value = match field.ty() {
            crate::project::FlatOwnedRecordFieldType::OwnedBytes => {
                FlatOwnedRecordMemberValue::Bytes(
                    settled_bytes
                        .take()
                        .ok_or("flat owned-record Bytes field was published more than once")?,
                )
            }
            crate::project::FlatOwnedRecordFieldType::I64 => {
                let Some(Value::Int(value)) = record.fields.remove(field.stable_id()) else {
                    return Err("flat owned-record i64 field changed after authentication");
                };
                FlatOwnedRecordMemberValue::I64(value)
            }
            crate::project::FlatOwnedRecordFieldType::Bool => {
                let Some(Value::Bool(value)) = record.fields.remove(field.stable_id()) else {
                    return Err("flat owned-record bool field changed after authentication");
                };
                FlatOwnedRecordMemberValue::Bool(value)
            }
            crate::project::FlatOwnedRecordFieldType::Usize => {
                let Some(Value::Usize(value)) = record.fields.remove(field.stable_id()) else {
                    return Err("flat owned-record usize field changed after authentication");
                };
                FlatOwnedRecordMemberValue::Usize(value)
            }
        };
        fields.push(FlatOwnedRecordMember {
            field_id: field.stable_id().clone(),
            value,
        });
    }
    if !record.fields.is_empty() || settled_bytes.is_some() {
        return Err("flat owned-record carrier retained an unauthenticated field");
    }
    Ok(FlatOwnedRecordValue {
        record_id: expected.record_id().clone(),
        fields,
    })
}

fn copy_out_owned_data(
    value: Value,
    expected: &ResolvedType,
    cleanup_events: &mut Vec<OwnedDataCleanupEvent>,
) -> Result<OwnedDataValue, &'static str> {
    if expected == &ResolvedType::Bytes {
        return settle_interpreted_bytes(value, cleanup_events).map(OwnedDataValue::Bytes);
    }
    let Value::Variant(value) = value else {
        return Err("owned-data result is not the authenticated variant carrier");
    };
    if &value.ty != expected {
        return Err("owned-data result carrier type disagrees with its signature");
    }
    let mut value = Arc::try_unwrap(value)
        .map_err(|_| "owned-data result still has a live alias at copy-out")?;
    match value.variant.as_str() {
        crate::prelude::OPTION_ID => match value.case.as_str() {
            crate::prelude::OPTION_NONE_ID if value.fields.is_empty() => {
                Ok(OwnedDataValue::OptionBytes(None))
            }
            crate::prelude::OPTION_SOME_ID if value.fields.len() == 1 => {
                let (field, payload) = value
                    .fields
                    .pop_first()
                    .ok_or("Option::Some result has no authenticated value payload")?;
                if field.as_str() != crate::prelude::OPTION_SOME_VALUE_ID {
                    return Err("Option::Some result has an unauthenticated payload identity");
                }
                settle_interpreted_bytes(payload, cleanup_events)
                    .map(Some)
                    .map(OwnedDataValue::OptionBytes)
            }
            _ => Err("Option<Bytes> result has an invalid active case or payload inventory"),
        },
        crate::prelude::RESULT_ID => match value.case.as_str() {
            crate::prelude::RESULT_OK_ID if value.fields.len() == 1 => {
                let (field, payload) = value
                    .fields
                    .pop_first()
                    .ok_or("Result::Ok result has no authenticated value payload")?;
                if field.as_str() != crate::prelude::RESULT_OK_VALUE_ID {
                    return Err("Result::Ok result has an unauthenticated payload identity");
                }
                settle_interpreted_bytes(payload, cleanup_events)
                    .map(Ok)
                    .map(OwnedDataValue::ResultBytesI64)
            }
            crate::prelude::RESULT_ERR_ID if value.fields.len() == 1 => {
                let (field, error) = value
                    .fields
                    .pop_first()
                    .ok_or("Result::Err result has no authenticated error payload")?;
                if field.as_str() != crate::prelude::RESULT_ERR_ERROR_ID {
                    return Err("Result::Err result has an unauthenticated payload identity");
                }
                let Value::Int(error) = error else {
                    return Err("Result::Err payload is not i64");
                };
                Ok(OwnedDataValue::ResultBytesI64(Err(error)))
            }
            _ => Err("Result<Bytes, i64> has an invalid active case or payload inventory"),
        },
        _ => Err("owned-data result uses an unauthenticated variant identity"),
    }
}

fn copy_out_public_api(
    value: Value,
    expected: &ResolvedType,
    cleanup_events: &mut Vec<OwnedDataCleanupEvent>,
) -> Result<PublicApiValue, &'static str> {
    match expected {
        ResolvedType::I64 => match value {
            Value::Int(value) => Ok(PublicApiValue::I64(value)),
            _ => Err("public API i64 result disagrees with its signature"),
        },
        ResolvedType::Bool => match value {
            Value::Bool(value) => Ok(PublicApiValue::Bool(value)),
            _ => Err("public API bool result disagrees with its signature"),
        },
        ResolvedType::Usize => match value {
            Value::Usize(value) => Ok(PublicApiValue::Usize(value)),
            _ => Err("public API usize result disagrees with its signature"),
        },
        _ => match copy_out_owned_data(value, expected, cleanup_events)? {
            OwnedDataValue::Bytes(value) => Ok(PublicApiValue::Bytes(value)),
            OwnedDataValue::OptionBytes(value) => Ok(PublicApiValue::OptionBytes(value)),
            OwnedDataValue::ResultBytesI64(value) => Ok(PublicApiValue::ResultBytesI64(value)),
        },
    }
}

fn copy_out_owned_utf8_api(
    value: Value,
    expected: &ResolvedType,
    settlement_events: &mut Vec<OwnedUtf8SettlementEvent>,
) -> Result<OwnedUtf8ApiValue, &'static str> {
    match expected {
        ResolvedType::I64 => match value {
            Value::Int(value) => Ok(OwnedUtf8ApiValue::I64(value)),
            _ => Err("owned UTF-8 API i64 result disagrees with its signature"),
        },
        ResolvedType::Bool => match value {
            Value::Bool(value) => Ok(OwnedUtf8ApiValue::Bool(value)),
            _ => Err("owned UTF-8 API bool result disagrees with its signature"),
        },
        ResolvedType::Usize => match value {
            Value::Usize(value) => Ok(OwnedUtf8ApiValue::Usize(value)),
            _ => Err("owned UTF-8 API usize result disagrees with its signature"),
        },
        ResolvedType::String => {
            let Value::String(value) = value else {
                return Err("owned UTF-8 API String result disagrees with its signature");
            };
            let output_limit = u64::try_from(crate::project::MAX_PUBLIC_API_OWNED_OUTPUT_BYTES)
                .unwrap_or(u64::MAX);
            if u64::try_from(value.len()).unwrap_or(u64::MAX) > output_limit {
                return Err("owned UTF-8 API String result exceeds the public output bound");
            }
            // Host copy-out has its own fixed public bound and is deliberately
            // outside the private runtime meter. Settle the private carrier
            // only after the host allocation has completed.
            let host_value = value.clone();
            drop(value);
            settlement_events.push(OwnedUtf8SettlementEvent::CopyOutAndSettleUtf8);
            Ok(OwnedUtf8ApiValue::Utf8(host_value))
        }
        _ => {
            let mut byte_events = Vec::with_capacity(1);
            let value = copy_out_owned_data(value, expected, &mut byte_events)?;
            for event in byte_events {
                match event {
                    OwnedDataCleanupEvent::CopyOutAndSettleBytes => {
                        settlement_events.push(OwnedUtf8SettlementEvent::CopyOutAndSettleBytes)
                    }
                }
            }
            Ok(match value {
                OwnedDataValue::Bytes(value) => OwnedUtf8ApiValue::Bytes(value),
                OwnedDataValue::OptionBytes(value) => OwnedUtf8ApiValue::OptionBytes(value),
                OwnedDataValue::ResultBytesI64(value) => OwnedUtf8ApiValue::ResultBytesI64(value),
            })
        }
    }
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
    interpret_with_profile(
        source_path,
        function_token,
        arguments,
        options,
        SourceProfile::Legacy,
    )
}

#[derive(Clone, Copy)]
enum SourceProfile {
    Legacy,
    InternalStrings,
}

impl SourceProfile {
    fn schema(self) -> &'static str {
        match self {
            Self::Legacy => SCHEMA,
            Self::InternalStrings => internal_strings::SCHEMA,
        }
    }

    fn payload_domain(self) -> &'static [u8] {
        match self {
            Self::Legacy => PAYLOAD_DIGEST_DOMAIN,
            Self::InternalStrings => internal_strings::PAYLOAD_DIGEST_DOMAIN,
        }
    }
}

fn interpret_with_profile(
    source_path: &Path,
    function_token: &str,
    arguments: &[String],
    options: &InterpreterOptions,
    profile: SourceProfile,
) -> Result<Interpretation, Vec<Diagnostic>> {
    if matches!(profile, SourceProfile::InternalStrings) {
        InterpreterOptions::new(options.max_bytes, options.max_steps)
            .map_err(|error| vec![error])?;
    }
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
            interpret_on_current_thread(
                &source_path,
                &function_token,
                &arguments,
                &options_owned,
                profile,
            )
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
    profile: SourceProfile,
) -> Result<Interpretation, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = match profile {
        SourceProfile::Legacy => patch::read_source_snapshot(&canonical_source_path)?,
        SourceProfile::InternalStrings => patch::read_source_snapshot_bounded(
            &canonical_source_path,
            internal_strings::MAX_SOURCE_BYTES,
            "SPX-F104",
        )?,
    };
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

    // The selected source boundary remains scalar/borrow-only, while its
    // internal closure may use the verified Useful Data profile. Keeping
    // these gates separate prevents owned buffers or fixed arrays from
    // silently becoming CLI values merely because the evaluator can execute
    // them internally.
    let admitted = admitted_resolved_functions_with_profile(&resolved, profile);

    scan_closure(entry.id.as_str(), &admitted, &resolved.declarations, false)?;

    let (evaluated, steps_used, _) = evaluate_resolved_entry(
        entry,
        &parsed_arguments,
        &admitted,
        &resolved.declarations,
        options.max_steps,
        false,
    );
    let outcome = match evaluated {
        Ok(value) => returned_outcome(&value),
        Err(flow) => match flow {
            Flow::Failure(status) => failed_outcome(&status.to_json()),
            Flow::Exhausted => capacity_outcome(OUTCOME_FUEL_EXHAUSTED),
            Flow::DepthExceeded => capacity_outcome(OUTCOME_CALL_DEPTH_EXCEEDED),
            Flow::Cancelled { .. } => {
                return Err(vec![guard_error(
                    "unexpected cancellation in legacy source evaluation",
                )]);
            }
            Flow::Utf8MaterializationLimitExceeded { .. } => {
                return Err(vec![guard_error(
                    "unexpected UTF-8 materialization limit in legacy source evaluation",
                )]);
            }
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

    // New-profile rendering charges component quotes/joining, then payload,
    // then wrapper. Components plus wrapper digest are disjoint output parts:
    // cumulative work is at most P + 2E <= 3E. Legacy charging stays frozen.
    let render_limit = match profile {
        SourceProfile::Legacy => options.max_bytes,
        SourceProfile::InternalStrings => options.max_bytes.checked_mul(3).ok_or_else(|| {
            vec![option_error(
                "interpret render budget overflowed".to_owned(),
            )]
        })?,
    };
    let (envelope, overflowed) = with_limit(render_limit, || {
        render_with_profile(
            &RenderFacts {
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
            },
            profile,
        )
    });
    if overflowed
        || (matches!(profile, SourceProfile::InternalStrings) && envelope.len() > options.max_bytes)
    {
        return Err(vec![Diagnostic::io(
            "SPX-F104",
            "interpret output exceeds the max-bytes budget; refusing to truncate".to_owned(),
        )]);
    }
    match profile {
        SourceProfile::Legacy => patch::validate_source_unchanged(
            &canonical_source_path,
            source_path,
            &snapshot,
            &revision,
        )?,
        SourceProfile::InternalStrings => {
            // Preserve F104 for observable capacity growth. The subsequent
            // identity/revision recheck remains bounded and fail-closed if
            // the source changes again between these two reads.
            patch::read_source_snapshot_bounded(
                &canonical_source_path,
                internal_strings::MAX_SOURCE_BYTES,
                "SPX-F104",
            )?;
            patch::validate_source_unchanged_bounded(
                &canonical_source_path,
                source_path,
                &snapshot,
                &revision,
                internal_strings::MAX_SOURCE_BYTES,
            )?;
        }
    }
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

/// Closed AST-level admission gate for scalar results and direct scalar or
/// invocation-borrowed UTF-8 inputs.
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
        let admitted = (param.mode == ParamMode::Value && is_admitted_scalar(&param.ty))
            || (param.mode == ParamMode::Borrow && matches!(param.ty, Type::Str | Type::SliceU8));
        if !admitted && param.mode != ParamMode::Value {
            return Some(REASON_UNSUPPORTED_PARAMETER_MODE);
        }
        if !admitted {
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
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::F32
            | Type::F64
            | Type::Char
            | Type::Bool
    )
}

use nested_owned::{
    admitted_owned_record_field, bind_borrowed_pattern, bind_owned_pattern,
    is_admitted_owned_byte_record, record_construction_is_admitted, record_pattern_is_admitted,
    record_update_is_admitted, take_owned_place, update_owned_record,
};

/// Exact non-Copy sum profile admitted by Owned Byte Variant Algebra v1.
/// Authored variants must be flat and monomorphic; generic admission is
/// restricted to the exact compiler-owned prelude instances.
fn is_admitted_owned_byte_variant(declarations: &hir::DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    let Some(item) = declarations.declaration(declaration) else {
        return false;
    };
    if item.kind != hir::DeclarationKind::Variant {
        return false;
    }
    let compiler_owned = item.identity_origin == hir::IdentityOrigin::CompilerOwned
        && hir::admitted_owned_byte_prelude_instance(declaration, arguments);
    if compiler_owned {
        return true;
    }
    if !arguments.is_empty() {
        return false;
    }
    let Some(cases) = declarations.variant_cases(declaration) else {
        return false;
    };
    cases
        .iter()
        .flat_map(|case| &case.fields)
        .any(|field| field.ty == ResolvedType::Bytes)
        && cases
            .iter()
            .flat_map(|case| &case.fields)
            .all(|field| field.ty == ResolvedType::Bytes || is_admitted_resolved_scalar(&field.ty))
}

fn concrete_variant_case_fields(
    declarations: &hir::DeclarationIndex,
    ty: &ResolvedType,
    case: &hir::DeclarationId,
) -> Option<Vec<(hir::DeclarationId, ResolvedType)>> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return None;
    };
    let declared_case = declarations
        .variant_cases(declaration)?
        .iter()
        .find(|candidate| candidate.id == *case)?;
    declared_case
        .fields
        .iter()
        .map(|field| {
            hir::substitute_type(&field.ty, declaration, arguments)
                .ok()
                .map(|ty| (field.id.clone(), ty))
        })
        .collect()
}

fn variant_constructor_is_admitted(
    declarations: &hir::DeclarationIndex,
    expression: &ResolvedExpr,
) -> bool {
    if !is_admitted_owned_byte_variant(declarations, &expression.ty)
        || expression.ownership != hir::OwnershipMode::Own
    {
        return false;
    }
    let ResolvedType::Nominal {
        declaration: concrete_variant,
        ..
    } = &expression.ty
    else {
        return false;
    };
    let ResolvedExprKind::ConstructVariant {
        variant,
        case,
        fields,
    } = &expression.kind
    else {
        return false;
    };
    if variant != concrete_variant {
        return false;
    }
    let Some(declared_fields) = concrete_variant_case_fields(declarations, &expression.ty, case)
    else {
        return false;
    };
    if fields.len() != declared_fields.len() {
        return false;
    }
    let mut seen = BTreeSet::new();
    fields.iter().all(|field| {
        let Some((_, declared_ty)) = declared_fields
            .iter()
            .find(|(field_id, _)| *field_id == field.field)
        else {
            return false;
        };
        seen.insert(field.field.clone())
            && field.value.ty == *declared_ty
            && field.value.ownership
                == if *declared_ty == ResolvedType::Bytes {
                    hir::OwnershipMode::Own
                } else {
                    hir::OwnershipMode::Value
                }
    })
}

fn variant_pattern_is_admitted(
    declarations: &hir::DeclarationIndex,
    mode: hir::ResolvedMatchMode,
    ty: &ResolvedType,
    arms: &[hir::ResolvedMatchArm],
) -> bool {
    if !is_admitted_owned_byte_variant(declarations, ty)
        || !matches!(
            mode,
            hir::ResolvedMatchMode::Own | hir::ResolvedMatchMode::Borrow
        )
        || arms.is_empty()
    {
        return false;
    }
    let ResolvedType::Nominal {
        declaration: expected_variant,
        ..
    } = ty
    else {
        return false;
    };
    let Some(declared_cases) = declarations.variant_cases(expected_variant) else {
        return false;
    };
    let mut seen_cases = BTreeSet::new();
    let mut seen_bindings = BTreeSet::new();
    for arm in arms {
        if arm.guard.is_some() {
            return false;
        }
        let hir::ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } = &arm.pattern
        else {
            return false;
        };
        if variant != expected_variant || !seen_cases.insert(case.clone()) {
            return false;
        }
        let Some(declared_fields) = concrete_variant_case_fields(declarations, ty, case) else {
            return false;
        };
        if fields.len() != declared_fields.len() {
            return false;
        }
        let mut seen_fields = BTreeSet::new();
        for field in fields {
            let Some((_, declared_ty)) = declared_fields
                .iter()
                .find(|(field_id, _)| *field_id == field.field)
            else {
                return false;
            };
            if !seen_fields.insert(field.field.clone())
                || !seen_bindings.insert(field.binding.id.clone())
                || field.binding.ty != *declared_ty
            {
                return false;
            }
            let expected_ownership = if *declared_ty == ResolvedType::Bytes {
                match mode {
                    hir::ResolvedMatchMode::Own => hir::OwnershipMode::Own,
                    hir::ResolvedMatchMode::Borrow => hir::OwnershipMode::Borrow,
                    hir::ResolvedMatchMode::Value => return false,
                }
            } else {
                hir::OwnershipMode::Value
            };
            if field.binding.ownership != expected_ownership {
                return false;
            }
        }
    }
    seen_cases.len() == declared_cases.len()
        && declared_cases
            .iter()
            .all(|case| seen_cases.contains(&case.id))
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
            (&param.ty, &value),
            (Type::I64, ArgumentValue::Int(_))
                | (Type::I32, ArgumentValue::Int32(_))
                | (Type::U8, ArgumentValue::Uint8(_))
                | (Type::Usize, ArgumentValue::Usize(_))
                | (Type::Char, ArgumentValue::Char(_))
                | (Type::F32, ArgumentValue::Float32(_))
                | (Type::F64, ArgumentValue::Float64(_))
                | (Type::Bool, ArgumentValue::Bool(_))
                | (Type::Str, ArgumentValue::BorrowedStr(_))
                | (Type::SliceU8, ArgumentValue::BorrowedSlice(_))
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
#[derive(Clone, Copy)]
struct CopyRecordAdmission {
    contextual: bool,
    project_profile: bool,
}

fn scan_closure(
    entry_id: &str,
    admitted: &BTreeMap<&str, &ResolvedFunction>,
    declarations: &hir::DeclarationIndex,
    allow_copy_records: bool,
) -> Result<BTreeSet<String>, Vec<Diagnostic>> {
    fn scan<'a>(
        expression: &'a ResolvedExpr,
        admitted: &BTreeMap<&'a str, &'a ResolvedFunction>,
        declarations: &hir::DeclarationIndex,
        root_types: &BTreeMap<ValueId, ResolvedType>,
        visited: &mut BTreeMap<&'a str, bool>,
        queue: &mut Vec<(&'a str, bool)>,
        copy_records: CopyRecordAdmission,
    ) -> Result<(), Vec<Diagnostic>> {
        match &expression.kind {
            ResolvedExprKind::ConstructRecord { .. }
                if record_construction_is_admitted(
                    declarations,
                    &expression.ty,
                    copy_records.project_profile || copy_records.contextual,
                ) =>
            {
                Ok(())
            }
            ResolvedExprKind::ConstructRecord { .. } => {
                Err(reject_scan(expression, REASON_RECORD_CONSTRUCTION))
            }
            ResolvedExprKind::ConstructVariant { .. }
                if variant_constructor_is_admitted(declarations, expression) =>
            {
                Ok(())
            }
            ResolvedExprKind::ConstructVariant { .. } => {
                Err(reject_scan(expression, REASON_VARIANT_CONSTRUCTION))
            }
            ResolvedExprKind::UpdateRecord { record, fields, .. }
                if record_update_is_admitted(declarations, &expression.ty, record, fields) =>
            {
                Ok(())
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
            ResolvedExprKind::Match {
                mode,
                scrutinee,
                arms,
            } => {
                // Refutable Match v1: scalar decision chains over admitted
                // Copy scalars with literal/or/binding patterns join the
                // profile; every aggregate match shape stays rejected.
                let scalar = matches!(
                    scrutinee.ty,
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::U8
                        | ResolvedType::Usize
                        | ResolvedType::Char
                        | ResolvedType::Bool
                );
                let patterns_admitted = arms
                    .iter()
                    .all(|arm| arm.pattern_is_literal_or_irrefutable());
                let option_u8 = is_option_u8(&scrutinee.ty)
                    && arms
                        .iter()
                        .all(|arm| option_u8_pattern_is_admitted(&arm.pattern));
                let owned_byte_record = is_admitted_owned_byte_record(declarations, &scrutinee.ty)
                    && matches!(
                        mode,
                        hir::ResolvedMatchMode::Own | hir::ResolvedMatchMode::Borrow
                    )
                    && arms.len() == 1
                    && arms[0].guard.is_none()
                    && is_admitted_resolved_scalar(&expression.ty)
                    && record_pattern_is_admitted(
                        declarations,
                        *mode,
                        &scrutinee.ty,
                        &arms[0].pattern,
                    );
                let owned_byte_variant = is_admitted_resolved_scalar(&expression.ty)
                    && variant_pattern_is_admitted(declarations, *mode, &scrutinee.ty, arms);
                if (!scalar && !option_u8 && !owned_byte_record && !owned_byte_variant)
                    || (scalar && !patterns_admitted)
                    || arms.is_empty()
                {
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
            ResolvedExprKind::Place(place)
                if !place.projections.is_empty()
                    && !root_types.get(&place.root).is_some_and(|root| {
                        admitted_owned_record_field(declarations, root, place, &expression.ty)
                    }) =>
            {
                Err(reject_scan(expression, REASON_PLACE_PROJECTION))
            }
            ResolvedExprKind::Call {
                callee, instance, ..
            } => {
                if instance.is_some() {
                    return Err(reject_scan(expression, REASON_GENERIC_CALL));
                }
                let intrinsic = crate::string_ops::by_id(callee.as_str()).is_some()
                    || crate::str_ops::by_id(callee.as_str()).is_some()
                    || crate::byte_ops::by_id(callee.as_str()).is_some()
                    || crate::host_io_ops::by_id(callee.as_str()).is_some();
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
        let mut children = Vec::new();
        match &expression.kind {
            ResolvedExprKind::ConstructRecord { fields, .. } => {
                children.extend(fields.iter().map(|field| {
                    (
                        &field.value,
                        record_construction_is_admitted(declarations, &field.value.ty, true),
                    )
                }));
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                children.push((base.as_ref(), false));
                children.extend(fields.iter().map(|field| {
                    (
                        &field.value,
                        record_construction_is_admitted(declarations, &field.value.ty, true),
                    )
                }));
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            children.push((child, false));
                        }
                    }
                }
                children.push((tail.as_ref(), copy_records.contextual));
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                children.push((condition.as_ref(), false));
                children.push((then_branch.as_ref(), copy_records.contextual));
                children.push((else_branch.as_ref(), copy_records.contextual));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                children.push((scrutinee.as_ref(), false));
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        children.push((guard.as_ref(), false));
                    }
                    children.push((&arm.value, copy_records.contextual));
                }
            }
            _ => children.extend(
                child_expressions(expression)
                    .into_iter()
                    .map(|child| (child, false)),
            ),
        }
        for (child, child_copy_context) in children {
            scan(
                child,
                admitted,
                declarations,
                root_types,
                visited,
                queue,
                CopyRecordAdmission {
                    contextual: child_copy_context,
                    ..copy_records
                },
            )?;
        }
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            // Callee bodies are enqueued once; the monotone `visited` set
            // makes recursive call cycles terminate.
            if admitted.contains_key(callee.as_str()) {
                let copy_result = copy_records.contextual
                    && record_construction_is_admitted(declarations, &expression.ty, true);
                let prior = visited.get(callee.as_str()).copied();
                if prior.is_none() || (copy_result && prior == Some(false)) {
                    visited.insert(callee.as_str(), copy_result);
                    queue.push((callee.as_str(), copy_result));
                }
            }
        }
        Ok(())
    }

    let mut visited = BTreeMap::new();
    visited.insert(entry_id, false);
    let mut frontier: Vec<(&str, bool)> = vec![(entry_id, false)];
    while let Some((id, copy_result_context)) = frontier.pop() {
        let Some(function) = admitted.get(id) else {
            continue;
        };
        let root_types = resolved_function_value_types(function);
        let mut queue: Vec<(&str, bool)> = Vec::new();
        for clause in function.requires.iter().chain(&function.ensures) {
            scan(
                clause,
                admitted,
                declarations,
                &root_types,
                &mut visited,
                &mut queue,
                CopyRecordAdmission {
                    contextual: false,
                    project_profile: allow_copy_records,
                },
            )?;
        }
        scan(
            &function.body,
            admitted,
            declarations,
            &root_types,
            &mut visited,
            &mut queue,
            CopyRecordAdmission {
                contextual: copy_result_context,
                project_profile: allow_copy_records,
            },
        )?;
        frontier.extend(queue);
    }
    Ok(visited.keys().map(|id| (*id).to_owned()).collect())
}

fn resolved_function_value_types(function: &ResolvedFunction) -> BTreeMap<ValueId, ResolvedType> {
    fn add_pattern(
        pattern: &hir::ResolvedMatchPattern,
        values: &mut BTreeMap<ValueId, ResolvedType>,
    ) {
        match pattern {
            hir::ResolvedMatchPattern::Variant { fields, .. } => {
                for field in fields {
                    values.insert(field.binding.id.clone(), field.binding.ty.clone());
                }
            }
            hir::ResolvedMatchPattern::Binding(binding) => {
                values.insert(binding.id.clone(), binding.ty.clone());
            }
            hir::ResolvedMatchPattern::Record { fields, .. } => {
                let mut pending = fields
                    .iter()
                    .map(|field| &field.pattern)
                    .collect::<Vec<_>>();
                while let Some(pattern) = pending.pop() {
                    match pattern {
                        hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                            values.insert(binding.id.clone(), binding.ty.clone());
                        }
                        hir::ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
                            pending.extend(fields.iter().map(|field| &field.pattern));
                        }
                        hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
                    }
                }
            }
            hir::ResolvedMatchPattern::Wildcard
            | hir::ResolvedMatchPattern::Literal(_)
            | hir::ResolvedMatchPattern::Or(_) => {}
        }
    }

    let mut values = function
        .params
        .iter()
        .map(|parameter| (parameter.id.clone(), parameter.ty.clone()))
        .collect::<BTreeMap<_, _>>();
    values.insert(function.result_id.clone(), function.return_type.clone());
    let mut pending = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::Block { statements, .. } => {
                for statement in statements {
                    if let ResolvedStatement::Let { binding, .. }
                    | ResolvedStatement::Assign { binding, .. } = statement
                    {
                        values.insert(binding.id.clone(), binding.ty.clone());
                    }
                }
            }
            ResolvedExprKind::Match { arms, .. } => {
                for arm in arms {
                    add_pattern(&arm.pattern, &mut values);
                }
            }
            _ => {}
        }
        pending.extend(child_expressions(expression));
    }
    values
}

fn admitted_resolved_functions(
    program: &hir::ResolvedProgram,
) -> BTreeMap<&str, &ResolvedFunction> {
    admitted_resolved_functions_with_profile(program, SourceProfile::Legacy)
}

fn admitted_resolved_functions_with_profile(
    program: &hir::ResolvedProgram,
    profile: SourceProfile,
) -> BTreeMap<&str, &ResolvedFunction> {
    program
        .functions
        .iter()
        .filter(|function| {
            program
                .declarations
                .declaration(&function.id)
                .is_some_and(|declaration| {
                    declaration.identity_origin == hir::IdentityOrigin::Explicit
                })
        })
        .filter(|function| match profile {
            SourceProfile::Legacy => {
                resolved_signature_is_admitted(function, &program.declarations)
            }
            SourceProfile::InternalStrings => {
                function.effects.is_empty()
                    && internal_strings::signature_is_admitted(function, &program.declarations)
            }
        })
        .map(|function| (function.id.as_str(), function))
        .collect()
}

fn resolved_signature_is_admitted(
    function: &ResolvedFunction,
    declarations: &hir::DeclarationIndex,
) -> bool {
    function.effects.is_empty() && resolved_data_signature_is_admitted(function, declarations)
}

fn resolved_data_signature_is_admitted(
    function: &ResolvedFunction,
    declarations: &hir::DeclarationIndex,
) -> bool {
    function.params.iter().all(|parameter| {
        resolved_data_parameter_is_admitted(&parameter.ty, parameter.ownership, declarations)
    }) && resolved_data_result_is_admitted(&function.return_type, declarations)
}

fn resolved_data_parameter_is_admitted(
    ty: &ResolvedType,
    ownership: hir::OwnershipMode,
    declarations: &hir::DeclarationIndex,
) -> bool {
    match (ty, ownership) {
        (ty, hir::OwnershipMode::Value)
            if is_admitted_resolved_scalar(ty) || matches!(ty, ResolvedType::ArrayU8(_)) =>
        {
            true
        }
        (ty @ ResolvedType::Nominal { declaration, .. }, hir::OwnershipMode::Value)
            if declarations
                .declaration(declaration)
                .is_some_and(|item| item.kind == hir::DeclarationKind::Class)
                && record_construction_is_admitted(declarations, ty, true) =>
        {
            true
        }
        (ResolvedType::Bytes, hir::OwnershipMode::Own)
        | (ResolvedType::Bytes, hir::OwnershipMode::Borrow)
        | (ResolvedType::Str, hir::OwnershipMode::Borrow)
        | (ResolvedType::SliceU8, hir::OwnershipMode::Borrow)
        | (ResolvedType::ArrayU8(_), hir::OwnershipMode::Borrow) => true,
        (ty, hir::OwnershipMode::Own | hir::OwnershipMode::Borrow)
            if is_admitted_owned_byte_record(declarations, ty)
                || is_admitted_owned_byte_variant(declarations, ty) =>
        {
            true
        }
        _ => false,
    }
}

fn resolved_data_result_is_admitted(
    ty: &ResolvedType,
    declarations: &hir::DeclarationIndex,
) -> bool {
    is_admitted_resolved_scalar(ty)
        || matches!(ty, ResolvedType::ArrayU8(_) | ResolvedType::Bytes)
        || is_admitted_owned_byte_record(declarations, ty)
        || is_admitted_owned_byte_variant(declarations, ty)
}

pub(crate) fn evaluate_resolved_stdout_transcript(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    max_steps: usize,
) -> Result<(ResolvedEvaluation, Vec<u8>), Vec<Diagnostic>> {
    hir::validate(program).map_err(|diagnostic| vec![diagnostic])?;
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "hosted evaluation max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        ))]);
    }
    if program.entrypoint.as_str() != entry_id
        || !program
            .permits
            .iter()
            .any(|effect| effect == crate::host_io_ops::STDOUT_WRITE_EFFECT)
    {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            "hosted stdout evaluation requires the exact entry point and module permit".to_owned(),
        )]);
    }
    let admitted = program
        .functions
        .iter()
        .filter(|function| {
            program
                .declarations
                .declaration(&function.id)
                .is_some_and(|declaration| {
                    declaration.identity_origin == hir::IdentityOrigin::Explicit
                })
        })
        .filter(|function| {
            resolved_data_signature_is_admitted(function, &program.declarations)
                && function
                    .effects
                    .iter()
                    .all(|effect| effect == crate::host_io_ops::STDOUT_WRITE_EFFECT)
        })
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let entry = admitted.get(entry_id).copied().ok_or_else(|| {
        vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("hosted entry `{entry_id}` is outside the stdout transcript profile"),
        )]
    })?;
    if !entry.params.is_empty() || entry.return_type != ResolvedType::I64 {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!("hosted entry `{entry_id}` must have type `fn main() -> i64`"),
        )]);
    }
    hir::analyze_byte_data_capacity(program).map_err(|diagnostic| vec![diagnostic])?;
    scan_closure(entry_id, &admitted, &program.declarations, true)?;
    let (evaluated, steps_used, mut transcript) = evaluate_resolved_entry(
        entry,
        &[],
        &admitted,
        &program.declarations,
        max_steps,
        true,
    );
    let outcome = match evaluated {
        Ok(Value::Int(value)) => ResolvedEvaluationOutcome::ReturnedI64(value),
        Ok(_) => ResolvedEvaluationOutcome::GuardError(
            "hosted zero-argument i64 entry returned a non-i64 value".to_owned(),
        ),
        Err(Flow::Failure(status)) => ResolvedEvaluationOutcome::LanguageFailure(status),
        Err(Flow::Exhausted) => ResolvedEvaluationOutcome::FuelExhausted,
        Err(Flow::DepthExceeded) => ResolvedEvaluationOutcome::CallDepthExceeded,
        Err(Flow::Cancelled { .. }) => ResolvedEvaluationOutcome::GuardError(
            "unexpected cancellation in legacy hosted evaluation".to_owned(),
        ),
        Err(Flow::Utf8MaterializationLimitExceeded { .. }) => {
            ResolvedEvaluationOutcome::GuardError(
                "unexpected UTF-8 materialization limit in legacy hosted evaluation".to_owned(),
            )
        }
        Err(Flow::Guard(detail)) => ResolvedEvaluationOutcome::GuardError(detail.to_owned()),
    };
    if !matches!(outcome, ResolvedEvaluationOutcome::ReturnedI64(_)) {
        transcript.clear();
    }
    Ok((
        ResolvedEvaluation {
            outcome,
            steps_used,
            max_steps,
            failure: None,
        },
        transcript,
    ))
}

/// Evaluate one selected zero-argument bool command against an immutable,
/// invocation-owned argv/stdin snapshot. Both output channels are published
/// only for a returned bool (including `false`); every other outcome discards
/// both transcripts.
pub(crate) fn evaluate_resolved_language_command(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    arguments: &[String],
    stdin: &[u8],
    max_steps: usize,
) -> Result<(CommandEvaluation, Vec<u8>, Vec<u8>), Vec<Diagnostic>> {
    hir::validate(program).map_err(|diagnostic| vec![diagnostic])?;
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "hosted command max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        ))]);
    }
    if arguments.len() > crate::command_io_ops::MAX_ARGUMENTS as usize {
        return Err(vec![argument_error(format!(
            "hosted command accepts at most {} arguments",
            crate::command_io_ops::MAX_ARGUMENTS
        ))]);
    }
    let mut input_bytes = stdin.len();
    for argument in arguments {
        if argument.as_bytes().contains(&0) {
            return Err(vec![argument_error(
                "hosted command arguments must not contain NUL bytes".to_owned(),
            )]);
        }
        input_bytes = input_bytes.checked_add(argument.len()).ok_or_else(|| {
            vec![argument_error(
                "hosted command input length overflowed".to_owned(),
            )]
        })?;
    }
    if input_bytes > crate::command_io_ops::MAX_INPUT_BYTES as usize {
        return Err(vec![argument_error(format!(
            "hosted command argv plus stdin exceeds {} bytes",
            crate::command_io_ops::MAX_INPUT_BYTES
        ))]);
    }

    let required_effects = [
        crate::command_io_ops::ARGS_READ_EFFECT,
        crate::command_io_ops::STDERR_WRITE_EFFECT,
        crate::command_io_ops::STDIN_READ_EFFECT,
        crate::host_io_ops::STDOUT_WRITE_EFFECT,
    ];
    if program.permits.as_slice() != required_effects {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            "hosted command requires exactly the Language Command I/O v1 permits".to_owned(),
        )]);
    }
    let admitted = program
        .functions
        .iter()
        .filter(|function| {
            program
                .declarations
                .declaration(&function.id)
                .is_some_and(|declaration| {
                    declaration.identity_origin == hir::IdentityOrigin::Explicit
                })
        })
        .filter(|function| {
            resolved_data_signature_is_admitted(function, &program.declarations)
                && function
                    .effects
                    .iter()
                    .all(|effect| required_effects.iter().any(|admitted| effect == admitted))
        })
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let entry = admitted.get(entry_id).copied().ok_or_else(|| {
        vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("hosted command entry `{entry_id}` is outside the command profile"),
        )]
    })?;
    if !entry.params.is_empty() || entry.return_type != ResolvedType::Bool {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!("hosted command entry `{entry_id}` must have type `fn () -> bool`"),
        )]);
    }
    hir::analyze_byte_data_capacity(program).map_err(|diagnostic| vec![diagnostic])?;
    scan_closure(entry_id, &admitted, &program.declarations, true)?;

    let command_input = CommandInputState {
        arguments: arguments
            .iter()
            .map(|value| Arc::<[u8]>::from(value.as_bytes()))
            .collect(),
        stdin: Arc::from(stdin),
        stdin_consumed: false,
        network: None,
    };
    let mut evaluator = Evaluator {
        admitted: FunctionLookup::Borrowed(&admitted),
        declarations: &program.declarations,
        steps: 0,
        budget: max_steps,
        next_byte_allocation: 0,
        allocated_byte_payload: 0,
        utf8_materialization_budget: Utf8MaterializationBudget::UnlimitedLegacy,
        stdout_transcript: Some(Vec::new()),
        stderr_transcript: Some(Vec::new()),
        command_input: Some(command_input),
        cancellation: PreparedCancellation::Never,
        trace_limit: 0,
        trace_events: Vec::new(),
        dropped_trace_events: 0,
        current_function: None,
        trace_identities: BTreeMap::new(),
        trace_phase: ResolvedTracePhase::Body,
        failure_detail: None,
    };
    let evaluated = evaluator.call_frame(entry, Vec::new(), 0);
    let outcome = match evaluated {
        Ok(Value::Bool(value)) => CommandEvaluationOutcome::ReturnedBool(value),
        Ok(_) => CommandEvaluationOutcome::GuardError(
            "hosted zero-argument bool command returned a non-bool value".to_owned(),
        ),
        Err(Flow::Failure(status)) => CommandEvaluationOutcome::LanguageFailure(status),
        Err(Flow::Exhausted) => CommandEvaluationOutcome::FuelExhausted,
        Err(Flow::DepthExceeded) => CommandEvaluationOutcome::CallDepthExceeded,
        Err(Flow::Cancelled { .. }) => CommandEvaluationOutcome::GuardError(
            "unexpected cancellation in legacy hosted command evaluation".to_owned(),
        ),
        Err(Flow::Utf8MaterializationLimitExceeded { .. }) => CommandEvaluationOutcome::GuardError(
            "unexpected UTF-8 materialization limit in hosted command evaluation".to_owned(),
        ),
        Err(Flow::Guard(detail)) => CommandEvaluationOutcome::GuardError(detail.to_owned()),
    };
    let mut stdout = evaluator.stdout_transcript.take().unwrap_or_default();
    let mut stderr = evaluator.stderr_transcript.take().unwrap_or_default();
    if !matches!(outcome, CommandEvaluationOutcome::ReturnedBool(_)) {
        stdout.clear();
        stderr.clear();
    }
    Ok((
        CommandEvaluation {
            outcome,
            steps_used: evaluator.steps,
            max_steps,
        },
        stdout,
        stderr,
    ))
}

fn is_admitted_resolved_scalar(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::U8
            | ResolvedType::Usize
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
        (Value::Usize(actual), crate::hir::PatternValue::Usize(expected)) => *actual == expected,
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

#[derive(Debug, PartialEq)]
enum Value {
    Int(i64),
    Int32(i32),
    Uint8(u8),
    Usize(u64),
    Char(u32),
    Float32(f32),
    Float64(f64),
    Bool(bool),
    ArrayU8(Arc<[u8]>),
    Bytes(OwnedBytesValue),
    String(String),
    BorrowedStr(BorrowedStrValue),
    BorrowedSlice(BorrowedSliceValue),
    OptionU8(Option<u8>),
    /// Private, monomorphic flat-record carrier. Field lookup is exclusively
    /// by authenticated declaration identity; source display names never
    /// participate in runtime selection.
    Record(Arc<OwnedRecordValue>),
    /// Exact authenticated non-Copy variant carrier. The concrete nominal
    /// instance, active case, and payload fields are all stable-ID keyed so a
    /// display-name collision or wrong generic substitution cannot select or
    /// transfer an owned payload.
    Variant(Arc<OwnedVariantValue>),
    /// Runtime tombstone for a verifier-authenticated move from an owned
    /// storage slot. Reaching it again is an impossible post-verify state.
    Moved,
}

fn is_option_u8(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Nominal { declaration, arguments }
            if declaration.as_str() == crate::prelude::OPTION_ID
                && arguments.as_slice() == [ResolvedType::U8]
    )
}

fn option_u8_pattern_is_admitted(pattern: &crate::hir::ResolvedMatchPattern) -> bool {
    let crate::hir::ResolvedMatchPattern::Variant {
        variant,
        case,
        fields,
    } = pattern
    else {
        return false;
    };
    if variant.as_str() != crate::prelude::OPTION_ID {
        return false;
    }
    (case.as_str() == crate::prelude::OPTION_NONE_ID && fields.is_empty())
        || (case.as_str() == crate::prelude::OPTION_SOME_ID
            && fields.len() == 1
            && fields[0].field.as_str() == crate::prelude::OPTION_SOME_VALUE_ID
            && fields[0].binding.ty == ResolvedType::U8)
}

#[derive(Clone, Debug, PartialEq)]
struct BorrowedStrValue {
    invocation_root: ValueId,
    bytes: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq)]
struct BorrowedSliceValue {
    invocation_root: ValueId,
    backing: Arc<[u8]>,
    start: usize,
    end: usize,
}

impl BorrowedSliceValue {
    fn whole(invocation_root: ValueId, backing: Arc<[u8]>) -> Self {
        let end = backing.len();
        Self {
            invocation_root,
            backing,
            start: 0,
            end,
        }
    }

    fn bytes(&self) -> &[u8] {
        &self.backing[self.start..self.end]
    }

    fn range(&self, start: usize, end: usize) -> Self {
        Self {
            invocation_root: self.invocation_root.clone(),
            backing: Arc::clone(&self.backing),
            start: self.start + start,
            end: self.start + end,
        }
    }
}

/// One logical `bytes_copy` allocation. The monotonically assigned identity
/// distinguishes even zero-length copies, for which allocators are permitted
/// to reuse the same dangling physical pointer.
#[derive(Clone, Debug, PartialEq)]
struct OwnedBytesValue {
    allocation: u32,
    bytes: Arc<[u8]>,
}

#[derive(Debug, PartialEq)]
struct OwnedRecordValue {
    record: hir::DeclarationId,
    fields: BTreeMap<hir::DeclarationId, Value>,
}

#[derive(Debug, PartialEq)]
struct OwnedVariantValue {
    ty: ResolvedType,
    variant: hir::DeclarationId,
    case: hir::DeclarationId,
    fields: BTreeMap<hir::DeclarationId, Value>,
}

fn borrowed_text(value: &Value) -> Option<&str> {
    match value {
        Value::BorrowedStr(value) => std::str::from_utf8(value.bytes.as_ref()).ok(),
        _ => None,
    }
}

#[derive(Debug)]
enum Flow {
    Failure(NormalizedStatus),
    Exhausted,
    DepthExceeded,
    Cancelled {
        before_step: usize,
    },
    Utf8MaterializationLimitExceeded {
        attempted_materializations: u64,
        attempted_bytes: u64,
    },
    Guard(&'static str),
}

#[derive(Clone, Copy)]
enum Utf8MaterializationBudget {
    UnlimitedLegacy,
    Fixed {
        used_materializations: u64,
        used_bytes: u64,
    },
}

impl Utf8MaterializationBudget {
    fn fixed() -> Self {
        Self::Fixed {
            used_materializations: 0,
            used_bytes: 0,
        }
    }

    fn usage(self) -> (u64, u64) {
        match self {
            Self::UnlimitedLegacy => (0, 0),
            Self::Fixed {
                used_materializations,
                used_bytes,
            } => (used_materializations, used_bytes),
        }
    }

    fn charge(&mut self, byte_len: usize) -> Result<(), Flow> {
        let Self::Fixed {
            used_materializations,
            used_bytes,
        } = self
        else {
            return Ok(());
        };
        let attempted_materializations = used_materializations.saturating_add(1);
        let byte_len = u64::try_from(byte_len).unwrap_or(u64::MAX);
        let attempted_bytes = used_bytes.saturating_add(byte_len);
        if attempted_materializations > MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS
            || attempted_bytes > MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES
        {
            return Err(Flow::Utf8MaterializationLimitExceeded {
                attempted_materializations,
                attempted_bytes,
            });
        }
        *used_materializations = attempted_materializations;
        *used_bytes = attempted_bytes;
        Ok(())
    }
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
        Value::Usize(value) => ("usize", format!("{value}usize")),
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

fn normalize_command_input(code: u32) -> NormalizedStatus {
    NormalizedStatus::try_new(
        crate::command_io_ops::STATUS_DOMAIN,
        code,
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .expect("compiler-owned command input status table is valid")
}

fn normalize_command_output(code: u32) -> NormalizedStatus {
    NormalizedStatus::try_new(
        crate::command_io_ops::OUTPUT_STATUS_DOMAIN,
        code,
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .expect("compiler-owned command output status table is valid")
}

fn normalize_byte_range(code: u32) -> NormalizedStatus {
    NormalizedStatus::try_new(
        crate::byte_ops::RANGE_STATUS_DOMAIN,
        code,
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .expect("compiler-owned byte range status table is valid")
}

/// One call frame with constant-time binding lookup.
///
/// Resolved value identities are unique within a live frame. Blocks truncate
/// their suffix in structural order, so the index never needs to search or
/// rebuild the retained prefix.
struct Environment {
    bindings: Vec<(ValueId, Value)>,
    slots: HashMap<ValueId, usize>,
}

impl From<Vec<(ValueId, Value)>> for Environment {
    fn from(bindings: Vec<(ValueId, Value)>) -> Self {
        let slots = bindings
            .iter()
            .enumerate()
            .map(|(index, (id, _))| (id.clone(), index))
            .collect();
        Self { bindings, slots }
    }
}

impl Environment {
    fn len(&self) -> usize {
        self.bindings.len()
    }

    fn push(&mut self, binding: (ValueId, Value)) {
        let index = self.bindings.len();
        let previous = self.slots.insert(binding.0.clone(), index);
        debug_assert!(previous.is_none(), "resolved frame identities are unique");
        self.bindings.push(binding);
    }

    fn extend(&mut self, bindings: impl IntoIterator<Item = (ValueId, Value)>) {
        for binding in bindings {
            self.push(binding);
        }
    }

    fn pop(&mut self) -> Option<(ValueId, Value)> {
        let binding = self.bindings.pop()?;
        self.slots.remove(&binding.0);
        Some(binding)
    }

    fn truncate(&mut self, length: usize) {
        while self.bindings.len() > length {
            let _ = self.pop();
        }
    }

    fn get(&self, id: &ValueId) -> Option<&Value> {
        self.slots
            .get(id)
            .and_then(|index| self.bindings.get(*index))
            .map(|(_, value)| value)
    }

    fn binding(&self, id: &ValueId) -> Option<&(ValueId, Value)> {
        self.slots
            .get(id)
            .and_then(|index| self.bindings.get(*index))
    }

    fn get_mut(&mut self, id: &ValueId) -> Option<&mut Value> {
        let index = *self.slots.get(id)?;
        self.bindings.get_mut(index).map(|(_, value)| value)
    }

    fn iter(&self) -> impl DoubleEndedIterator<Item = &(ValueId, Value)> {
        self.bindings.iter()
    }

    fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut (ValueId, Value)> {
        self.bindings.iter_mut()
    }
}

enum FunctionLookup<'a> {
    Borrowed(&'a BTreeMap<&'a str, &'a ResolvedFunction>),
    Prepared {
        functions: &'a [ResolvedFunction],
        indices: &'a BTreeMap<String, usize>,
    },
}

impl<'a> FunctionLookup<'a> {
    fn get(&self, id: &str) -> Option<&'a ResolvedFunction> {
        match self {
            Self::Borrowed(functions) => functions.get(id).copied(),
            Self::Prepared { functions, indices } => {
                indices.get(id).and_then(|index| functions.get(*index))
            }
        }
    }
}

struct Evaluator<'a> {
    admitted: FunctionLookup<'a>,
    declarations: &'a hir::DeclarationIndex,
    steps: usize,
    budget: usize,
    next_byte_allocation: u32,
    allocated_byte_payload: u64,
    utf8_materialization_budget: Utf8MaterializationBudget,
    stdout_transcript: Option<Vec<u8>>,
    stderr_transcript: Option<Vec<u8>>,
    command_input: Option<CommandInputState<'a>>,
    cancellation: PreparedCancellation<'a>,
    trace_limit: usize,
    trace_events: Vec<ResolvedTraceEvent>,
    dropped_trace_events: usize,
    current_function: Option<Arc<str>>,
    trace_identities: BTreeMap<String, Arc<str>>,
    trace_phase: ResolvedTracePhase,
    failure_detail: Option<ContractFailureDetail>,
}

struct CommandInputState<'a> {
    network: Option<network::NetworkState<'a>>,
    arguments: Vec<Arc<[u8]>>,
    stdin: Arc<[u8]>,
    stdin_consumed: bool,
}

fn evaluate_resolved_entry<'a>(
    entry: &'a ResolvedFunction,
    arguments: &[(String, ArgumentValue)],
    admitted: &'a BTreeMap<&'a str, &'a ResolvedFunction>,
    declarations: &'a hir::DeclarationIndex,
    budget: usize,
    host_stdout: bool,
) -> (Result<Value, Flow>, usize, Vec<u8>) {
    let (outcome, steps, transcript, _) = evaluate_resolved_entry_with_utf8_budget(
        entry,
        arguments,
        admitted,
        declarations,
        budget,
        host_stdout,
        Utf8MaterializationBudget::UnlimitedLegacy,
    );
    (outcome, steps, transcript)
}

fn evaluate_resolved_entry_with_utf8_budget<'a>(
    entry: &'a ResolvedFunction,
    arguments: &[(String, ArgumentValue)],
    admitted: &'a BTreeMap<&'a str, &'a ResolvedFunction>,
    declarations: &'a hir::DeclarationIndex,
    budget: usize,
    host_stdout: bool,
    utf8_materialization_budget: Utf8MaterializationBudget,
) -> (Result<Value, Flow>, usize, Vec<u8>, (u64, u64)) {
    let mut evaluator = Evaluator {
        admitted: FunctionLookup::Borrowed(admitted),
        declarations,
        steps: 0,
        budget,
        next_byte_allocation: 0,
        allocated_byte_payload: 0,
        utf8_materialization_budget,
        stdout_transcript: host_stdout.then(Vec::new),
        stderr_transcript: None,
        command_input: None,
        cancellation: PreparedCancellation::Never,
        trace_limit: 0,
        trace_events: Vec::new(),
        dropped_trace_events: 0,
        current_function: None,
        trace_identities: BTreeMap::new(),
        trace_phase: ResolvedTracePhase::Body,
        failure_detail: None,
    };
    let outcome = evaluator.evaluate_entry(entry, arguments);
    let utf8_usage = evaluator.utf8_materialization_budget.usage();
    (
        outcome,
        evaluator.steps,
        evaluator.stdout_transcript.unwrap_or_default(),
        utf8_usage,
    )
}

impl Evaluator<'_> {
    fn new_prepared<'a>(
        admitted: FunctionLookup<'a>,
        declarations: &'a hir::DeclarationIndex,
        budget: usize,
        trace_limit: usize,
        cancellation: PreparedCancellation<'a>,
    ) -> Evaluator<'a> {
        Evaluator {
            admitted,
            declarations,
            steps: 0,
            budget,
            next_byte_allocation: 0,
            allocated_byte_payload: 0,
            utf8_materialization_budget: Utf8MaterializationBudget::UnlimitedLegacy,
            stdout_transcript: None,
            stderr_transcript: None,
            command_input: None,
            cancellation,
            trace_limit,
            trace_events: Vec::with_capacity(trace_limit.min(4096)),
            dropped_trace_events: 0,
            current_function: None,
            trace_identities: BTreeMap::new(),
            trace_phase: ResolvedTracePhase::Body,
            failure_detail: None,
        }
    }

    /// Charges one step before evaluating a node and observes cancellation at
    /// that exact deterministic boundary.
    fn charge(&mut self) -> Result<(), Flow> {
        if self.steps >= self.budget {
            return Err(Flow::Exhausted);
        }
        if self.cancellation.cancelled(self.steps) {
            return Err(Flow::Cancelled {
                before_step: self.steps.saturating_add(1),
            });
        }
        self.steps += 1;
        Ok(())
    }

    fn begin_expression(&mut self, expression: &ResolvedExpr, depth: usize) -> Result<(), Flow> {
        self.charge()?;
        if self.trace_limit == 0 {
            return Ok(());
        }
        if self.trace_events.len() < self.trace_limit {
            let expression_id = self.intern_trace_identity(expression.id.as_str());
            self.trace_events.push(ResolvedTraceEvent {
                step: self.steps,
                depth,
                phase: self.trace_phase,
                function_id: self.current_function.clone().ok_or(Flow::Guard(
                    "trace evaluation has no authenticated function frame",
                ))?,
                expression_id,
                span: expression.span,
            });
        } else {
            self.dropped_trace_events = self
                .dropped_trace_events
                .checked_add(1)
                .ok_or(Flow::Guard("trace event accounting overflowed"))?;
        }
        Ok(())
    }

    fn charge_utf8_materialization(&mut self, byte_len: usize) -> Result<(), Flow> {
        self.utf8_materialization_budget.charge(byte_len)
    }

    fn materialize_utf8_copy(&mut self, value: &str) -> Result<String, Flow> {
        self.charge_utf8_materialization(value.len())?;
        Ok(value.to_owned())
    }

    /// The runtime value carrier deliberately has no `Clone` implementation.
    /// Every semantic copy therefore passes through this evaluator-owned seam,
    /// making owned UTF-8 accounting compiler-enforced at future call sites.
    fn clone_value(&mut self, value: &Value) -> Result<Value, Flow> {
        Ok(match value {
            Value::Int(value) => Value::Int(*value),
            Value::Int32(value) => Value::Int32(*value),
            Value::Uint8(value) => Value::Uint8(*value),
            Value::Usize(value) => Value::Usize(*value),
            Value::Char(value) => Value::Char(*value),
            Value::Float32(value) => Value::Float32(*value),
            Value::Float64(value) => Value::Float64(*value),
            Value::Bool(value) => Value::Bool(*value),
            Value::ArrayU8(value) => Value::ArrayU8(Arc::clone(value)),
            Value::Bytes(value) => Value::Bytes(value.clone()),
            Value::String(value) => Value::String(self.materialize_utf8_copy(value)?),
            Value::BorrowedStr(value) => Value::BorrowedStr(value.clone()),
            Value::BorrowedSlice(value) => Value::BorrowedSlice(value.clone()),
            Value::OptionU8(value) => Value::OptionU8(*value),
            // Aggregate aliases preserve the existing authenticated-borrow
            // semantics; Arc cloning does not duplicate any nested payload.
            Value::Record(value) => Value::Record(Arc::clone(value)),
            Value::Variant(value) => Value::Variant(Arc::clone(value)),
            Value::Moved => Value::Moved,
        })
    }

    fn lookup(&mut self, environment: &Environment, root: &ValueId) -> Result<Option<Value>, Flow> {
        let value = environment.get(root);
        match value {
            Some(Value::Moved) | None => Ok(None),
            Some(value) => self.clone_value(value).map(Some),
        }
    }

    fn lookup_place(
        &mut self,
        environment: &Environment,
        place: &hir::Place,
    ) -> Result<Option<Value>, Flow> {
        let Some(mut value) = self.lookup(environment, &place.root)? else {
            return Ok(None);
        };
        for projection in &place.projections {
            let hir::PlaceProjection::Field(field) = projection else {
                return Ok(None);
            };
            let Value::Record(record) = value else {
                return Ok(None);
            };
            let Some(field) = record.fields.get(field) else {
                return Ok(None);
            };
            value = self.clone_value(field)?;
        }
        Ok(Some(value))
    }

    /// Stage one verifier-authenticated synchronous `borrow Bytes` call
    /// argument without evaluating the owned place as a move. The callee
    /// receives another read-only carrier for the same logical allocation;
    /// neither the named caller slot nor a projected owned-record field is
    /// tombstoned, and the byte allocation counters are unchanged.
    fn borrow_bytes_call_argument(
        &mut self,
        environment: &Environment,
        argument: &ResolvedExpr,
        depth: usize,
    ) -> Result<Value, Flow> {
        // Ordinary argument evaluation would charge this expression before
        // moving it. Borrow admission changes only the storage action, not
        // deterministic fuel or trace accounting at the call boundary.
        self.begin_expression(argument, depth)?;
        let ResolvedExprKind::Place(place) = &argument.kind else {
            return Err(Flow::Guard(
                "borrowed Bytes call argument is not an exact place",
            ));
        };
        if !place.projections.is_empty() {
            let root_ty = environment
                .binding(&place.root)
                .and_then(|(_, value)| match value {
                    Value::Record(record) => Some(ResolvedType::Nominal {
                        declaration: record.record.clone(),
                        arguments: Vec::new(),
                    }),
                    _ => None,
                })
                .ok_or(Flow::Guard(
                    "borrowed Bytes call root is not an owned record carrier",
                ))?;
            if !admitted_owned_record_field(
                self.declarations,
                &root_ty,
                place,
                &ResolvedType::Bytes,
            ) {
                return Err(Flow::Guard(
                    "borrowed Bytes call argument has an unauthenticated field path",
                ));
            }
        }
        let value = self
            .lookup_place(environment, place)?
            .ok_or(Flow::Guard("borrowed Bytes call owner is unavailable"))?;
        let Value::Bytes(value) = value else {
            return Err(Flow::Guard(
                "borrowed Bytes call place does not contain owned bytes",
            ));
        };
        if value.allocation == 0 || value.allocation > self.next_byte_allocation {
            return Err(Flow::Guard(
                "borrowed Bytes call has an invalid logical allocation",
            ));
        }
        Ok(Value::Bytes(value))
    }

    fn value_has_type(&self, value: &Value, ty: &ResolvedType) -> bool {
        match (value, ty) {
            (Value::Int(_), ResolvedType::I64)
            | (Value::Int32(_), ResolvedType::I32)
            | (Value::Uint8(_), ResolvedType::U8)
            | (Value::Usize(_), ResolvedType::Usize)
            | (Value::Char(_), ResolvedType::Char)
            | (Value::Float32(_), ResolvedType::F32)
            | (Value::Float64(_), ResolvedType::F64)
            | (Value::Bool(_), ResolvedType::Bool)
            | (Value::Bytes(_), ResolvedType::Bytes) => true,
            (Value::Variant(carrier), expected) => &carrier.ty == expected,
            (Value::Record(carrier), ResolvedType::Nominal { declaration, .. }) => {
                &carrier.record == declaration
                    && is_admitted_owned_byte_record(self.declarations, ty)
            }
            _ => false,
        }
    }

    fn evaluate_entry(
        &mut self,
        function: &ResolvedFunction,
        arguments: &[(String, ArgumentValue)],
    ) -> Result<Value, Flow> {
        // Every distinct external parameter position is one invocation root,
        // irrespective of whether its source carrier is validated UTF-8 or an
        // arbitrary byte slice. Derived str_as_bytes views preserve that root
        // and therefore never recharge it.
        let has_str_root = arguments
            .iter()
            .any(|(_, argument)| matches!(argument, ArgumentValue::BorrowedStr(_)));
        let has_slice_root = arguments
            .iter()
            .any(|(_, argument)| matches!(argument, ArgumentValue::BorrowedSlice(_)));
        let budget_error = match (has_str_root, has_slice_root) {
            (true, false) => "borrowed string invocation exceeds byte budget",
            (false, true) => "borrowed byte invocation exceeds byte budget",
            _ => "borrowed invocation exceeds byte budget",
        };
        let byte_roots = arguments.iter().filter_map(|(_, argument)| match argument {
            ArgumentValue::BorrowedStr(value) => u64::try_from(value.len()).ok(),
            ArgumentValue::BorrowedSlice(value) => u64::try_from(value.len()).ok(),
            _ => None,
        });
        let mut charged = 0u64;
        for length in byte_roots {
            charged = charged
                .checked_add(length)
                .ok_or(Flow::Guard(budget_error))?;
            if charged > crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES {
                return Err(Flow::Guard(budget_error));
            }
        }
        let mut values = Vec::with_capacity(arguments.len());
        for (param, (_, argument)) in function.params.iter().zip(arguments.iter()) {
            let value = match (&param.ty, argument) {
                (ResolvedType::I64, ArgumentValue::Int(inner)) => Value::Int(*inner),
                (ResolvedType::I32, ArgumentValue::Int32(inner)) => Value::Int32(*inner),
                (ResolvedType::U8, ArgumentValue::Uint8(inner)) => Value::Uint8(*inner),
                (ResolvedType::Usize, ArgumentValue::Usize(inner)) => Value::Usize(*inner),
                (ResolvedType::Char, ArgumentValue::Char(inner)) => Value::Char(*inner),
                (ResolvedType::F32, ArgumentValue::Float32(inner)) => Value::Float32(*inner),
                (ResolvedType::F64, ArgumentValue::Float64(inner)) => Value::Float64(*inner),
                (ResolvedType::Bool, ArgumentValue::Bool(inner)) => Value::Bool(*inner),
                (ResolvedType::Str, ArgumentValue::BorrowedStr(inner)) => {
                    Value::BorrowedStr(BorrowedStrValue {
                        invocation_root: param.id.clone(),
                        bytes: Arc::from(inner.as_bytes()),
                    })
                }
                (ResolvedType::SliceU8, ArgumentValue::BorrowedSlice(inner)) => {
                    Value::BorrowedSlice(BorrowedSliceValue::whole(
                        param.id.clone(),
                        Arc::from(inner.as_slice()),
                    ))
                }
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
        // Legacy evaluators do not record trace events. Keep their call path
        // free of prepared-only identity allocation and trace-context writes.
        if self.trace_limit == 0 {
            return self.call_frame_inner(function, values, depth);
        }
        let function_id = self.intern_trace_identity(function.id.as_str());
        let previous_function = self.current_function.replace(function_id);
        let previous_phase = self.trace_phase;
        let result = self.call_frame_inner(function, values, depth);
        self.current_function = previous_function;
        self.trace_phase = previous_phase;
        result
    }

    fn call_frame_inner(
        &mut self,
        function: &ResolvedFunction,
        values: Vec<(ValueId, Value)>,
        depth: usize,
    ) -> Result<Value, Flow> {
        let mut frame = Environment::from(values);
        self.set_trace_phase(ResolvedTracePhase::Requires);
        for (index, clause) in function.requires.iter().enumerate() {
            self.charge()?;
            match self.evaluate(clause, &mut frame, depth)? {
                Value::Bool(true) => {}
                Value::Bool(false) => {
                    return Err(self.contract_failure(
                        function,
                        &frame,
                        ContractPhase::Requires,
                        index,
                    ))
                }
                _ => return Err(Flow::Guard("non-boolean requires clause")),
            }
        }
        self.set_trace_phase(ResolvedTracePhase::Body);
        let value = self.evaluate(&function.body, &mut frame, depth)?;
        if !function.ensures.is_empty() {
            let result_value = self.clone_value(&value)?;
            frame.push((function.result_id.clone(), result_value));
        }
        self.set_trace_phase(ResolvedTracePhase::Ensures);
        for (index, clause) in function.ensures.iter().enumerate() {
            self.charge()?;
            match self.evaluate(clause, &mut frame, depth)? {
                Value::Bool(true) => {}
                Value::Bool(false) => {
                    return Err(self.contract_failure(
                        function,
                        &frame,
                        ContractPhase::Ensures,
                        index,
                    ))
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
        self.begin_expression(expression, depth)?;
        match &expression.kind {
            ResolvedExprKind::Int(value) => Ok(Value::Int(*value)),
            ResolvedExprKind::Int32(value) => Ok(Value::Int32(*value)),
            ResolvedExprKind::Uint8(value) => Ok(Value::Uint8(*value)),
            ResolvedExprKind::Usize(value) => Ok(Value::Usize(*value)),
            ResolvedExprKind::Char(value) => Ok(Value::Char(*value)),
            ResolvedExprKind::Float32(bits) => Ok(Value::Float32(f32::from_bits(*bits))),
            ResolvedExprKind::Float64(bits) => Ok(Value::Float64(f64::from_bits(*bits))),
            ResolvedExprKind::Bool(value) => Ok(Value::Bool(*value)),
            ResolvedExprKind::ArrayU8(values) => Ok(Value::ArrayU8(Arc::from(values.as_slice()))),
            ResolvedExprKind::RepeatArrayU8 { value, count } => {
                let length = usize::try_from(*count)
                    .map_err(|_| Flow::Guard("fixed byte array length does not fit host usize"))?;
                Ok(Value::ArrayU8(Arc::from(vec![*value; length])))
            }
            ResolvedExprKind::String(value) => {
                Ok(Value::String(self.materialize_utf8_copy(value)?))
            }
            ResolvedExprKind::Place(place) => {
                let moves_storage = expression.ownership == hir::OwnershipMode::Own
                    && matches!(
                        &expression.ty,
                        ResolvedType::Bytes | ResolvedType::Nominal { .. }
                    );
                if moves_storage {
                    take_owned_place(environment, place)
                        .ok_or(Flow::Guard("use of moved owned storage"))
                } else {
                    self.lookup_place(environment, place)?
                        .ok_or(Flow::Guard("unresolved scalar place"))
                }
            }
            ResolvedExprKind::BorrowPlace { operation, place } => {
                let op = crate::byte_ops::by_id(operation.as_str())
                    .ok_or(Flow::Guard("unknown compiler-owned byte view"))?;
                let source = self
                    .lookup_place(environment, place)?
                    .ok_or(Flow::Guard("unresolved byte view storage root"))?;
                match (op, source) {
                    (crate::byte_ops::ByteOp::BytesAsSlice, Value::Bytes(value)) => {
                        if value.allocation == 0 || value.allocation > self.next_byte_allocation {
                            return Err(Flow::Guard(
                                "owned byte view has an invalid logical allocation",
                            ));
                        }
                        Ok(Value::BorrowedSlice(BorrowedSliceValue::whole(
                            place.root.clone(),
                            value.bytes,
                        )))
                    }
                    (crate::byte_ops::ByteOp::ArrayAsSlice, Value::ArrayU8(bytes)) => Ok(
                        Value::BorrowedSlice(BorrowedSliceValue::whole(place.root.clone(), bytes)),
                    ),
                    (crate::byte_ops::ByteOp::StrAsBytes, Value::BorrowedStr(value)) => {
                        Ok(Value::BorrowedSlice(BorrowedSliceValue::whole(
                            value.invocation_root,
                            value.bytes,
                        )))
                    }
                    (crate::byte_ops::ByteOp::StringAsStr, Value::String(value)) => {
                        Ok(Value::BorrowedStr(BorrowedStrValue {
                            invocation_root: place.root.clone(),
                            bytes: Arc::from(value.as_bytes()),
                        }))
                    }
                    _ => Err(Flow::Guard("ill-typed compiler-owned byte view")),
                }
            }
            ResolvedExprKind::ByteRange {
                operation,
                source,
                start,
                end,
            } => {
                if operation.as_str() != crate::byte_ops::RANGE_ID {
                    return Err(Flow::Guard("unknown compiler-owned byte range"));
                }
                let source = self.evaluate(source, environment, depth)?;
                let start = self.evaluate(start, environment, depth)?;
                let end = self.evaluate(end, environment, depth)?;
                let (Value::BorrowedSlice(source), Value::Usize(start), Value::Usize(end)) =
                    (source, start, end)
                else {
                    return Err(Flow::Guard("ill-typed compiler-owned byte range"));
                };
                if start > end {
                    return Err(Flow::Failure(normalize_byte_range(
                        crate::byte_ops::RANGE_START_AFTER_END_CODE,
                    )));
                }
                let Some((start, end)) = usize::try_from(start).ok().zip(usize::try_from(end).ok())
                else {
                    return Err(Flow::Failure(normalize_byte_range(
                        crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
                    )));
                };
                if end > source.bytes().len() {
                    return Err(Flow::Failure(normalize_byte_range(
                        crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
                    )));
                }
                Ok(Value::BorrowedSlice(source.range(start, end)))
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
            ResolvedExprKind::HostCommandCall(call) => {
                use hir::ResolvedHostCommandOperation as Operation;
                match call.operation {
                    Operation::NetConnect
                    | Operation::NetSend
                    | Operation::NetRecv
                    | Operation::NetStreamStdout
                    | Operation::NetWait
                    | Operation::NetClose
                    | Operation::NetTlsConnect
                    | Operation::NetListen
                    | Operation::NetAccept
                    | Operation::NetCloseListener => {
                        self.evaluate_network_operation(call, environment, depth)
                    }
                    Operation::ArgsLen => {
                        if !call.args.is_empty() {
                            return Err(Flow::Guard("invalid args_len arity"));
                        }
                        let input = self.command_input.as_ref().ok_or(Flow::Guard(
                            "args_len reached an evaluator without command input",
                        ))?;
                        Ok(Value::Usize(input.arguments.len() as u64))
                    }
                    Operation::ArgUtf8 => {
                        let [argument] = call.args.as_slice() else {
                            return Err(Flow::Guard("invalid arg_utf8 arity"));
                        };
                        let index = match self.evaluate(argument, environment, depth)? {
                            Value::Usize(value) => usize::try_from(value).ok(),
                            _ => return Err(Flow::Guard("ill-typed arg_utf8 index")),
                        };
                        let bytes = index
                            .and_then(|index| {
                                self.command_input
                                    .as_ref()
                                    .and_then(|input| input.arguments.get(index))
                            })
                            .cloned()
                            .ok_or_else(|| {
                                Flow::Failure(normalize_command_input(
                                    crate::command_io_ops::ARG_INDEX_OUT_OF_BOUNDS,
                                ))
                            })?;
                        if std::str::from_utf8(bytes.as_ref()).is_err() {
                            return Err(Flow::Failure(normalize_command_input(
                                crate::command_io_ops::ARG_INVALID_UTF8,
                            )));
                        }
                        Ok(Value::BorrowedStr(BorrowedStrValue {
                            invocation_root: ValueId::intrinsic_parameter(
                                crate::command_io_ops::ARG_UTF8_ID,
                                usize::MAX,
                            ),
                            bytes,
                        }))
                    }
                    Operation::StdinRead => {
                        if !call.args.is_empty() {
                            return Err(Flow::Guard("invalid stdin_read arity"));
                        }
                        let input = self.command_input.as_ref().ok_or(Flow::Guard(
                            "stdin_read reached an evaluator without command input",
                        ))?;
                        if input.stdin_consumed {
                            return Err(Flow::Failure(normalize_command_input(
                                crate::command_io_ops::STDIN_READ_FAILED,
                            )));
                        }
                        let length = u64::try_from(input.stdin.len()).map_err(|_| {
                            Flow::Failure(normalize_command_input(
                                crate::command_io_ops::INPUT_CAPACITY_EXCEEDED,
                            ))
                        })?;
                        if length > crate::command_io_ops::MAX_INPUT_BYTES {
                            return Err(Flow::Failure(normalize_command_input(
                                crate::command_io_ops::INPUT_CAPACITY_EXCEEDED,
                            )));
                        }
                        let next_count =
                            self.next_byte_allocation.checked_add(1).ok_or_else(|| {
                                Flow::Failure(normalize_command_input(
                                    crate::command_io_ops::INPUT_CAPACITY_EXCEEDED,
                                ))
                            })?;
                        let next_payload = self
                            .allocated_byte_payload
                            .checked_add(length)
                            .ok_or_else(|| {
                                Flow::Failure(normalize_command_input(
                                    crate::command_io_ops::INPUT_CAPACITY_EXCEEDED,
                                ))
                            })?;
                        if next_count > crate::byte_data_capacity::MAX_BYTES_COPY_SITES
                            || next_payload
                                > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES
                        {
                            return Err(Flow::Failure(normalize_command_input(
                                crate::command_io_ops::INPUT_CAPACITY_EXCEEDED,
                            )));
                        }
                        let bytes = Arc::from(input.stdin.as_ref());
                        self.next_byte_allocation = next_count;
                        self.allocated_byte_payload = next_payload;
                        self.command_input
                            .as_mut()
                            .expect("command input presence was checked")
                            .stdin_consumed = true;
                        Ok(Value::Bytes(OwnedBytesValue {
                            allocation: next_count,
                            bytes,
                        }))
                    }
                    Operation::StderrWrite | Operation::StdoutAppend | Operation::StderrAppend => {
                        let [argument] = call.args.as_slice() else {
                            return Err(Flow::Guard("invalid command output arity"));
                        };
                        let value = self.evaluate(argument, environment, depth)?;
                        let Value::BorrowedSlice(value) = value else {
                            return Err(Flow::Guard("ill-typed command output operand"));
                        };
                        let bytes = value.bytes();
                        let stdout_length = self.stdout_transcript.as_ref().map_or(0, Vec::len);
                        let stderr_length = self.stderr_transcript.as_ref().map_or(0, Vec::len);
                        let combined = stdout_length
                            .checked_add(stderr_length)
                            .and_then(|length| length.checked_add(bytes.len()))
                            .ok_or(Flow::Guard("command transcript length overflowed"))?;
                        if combined > crate::command_io_ops::MAX_OUTPUT_BYTES as usize {
                            if matches!(call.operation, Operation::StderrWrite) {
                                return Err(Flow::Guard(
                                    "combined command transcript exceeds verified capacity",
                                ));
                            }
                            return Err(Flow::Failure(normalize_command_output(
                                crate::command_io_ops::OUTPUT_CAPACITY_EXCEEDED,
                            )));
                        }
                        match call.operation {
                            Operation::StdoutAppend => self
                                .stdout_transcript
                                .as_mut()
                                .ok_or(Flow::Guard(
                                    "stdout_append reached an evaluator without command output",
                                ))?
                                .extend_from_slice(bytes),
                            Operation::StderrWrite | Operation::StderrAppend => self
                                .stderr_transcript
                                .as_mut()
                                .ok_or(Flow::Guard(
                                    "stderr output reached an evaluator without command output",
                                ))?
                                .extend_from_slice(bytes),
                            _ => unreachable!("command output operation was matched above"),
                        }
                        Ok(Value::Usize(bytes.len() as u64))
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
            ResolvedExprKind::ConstructRecord { record, fields } => {
                let mut values = BTreeMap::new();
                for field in fields {
                    let value = self.evaluate(&field.value, environment, depth)?;
                    if values.insert(field.field.clone(), value).is_some() {
                        return Err(Flow::Guard(
                            "record construction repeated an authenticated field identity",
                        ));
                    }
                }
                Ok(Value::Record(Arc::new(OwnedRecordValue {
                    record: record.clone(),
                    fields: values,
                })))
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                if !is_admitted_owned_byte_variant(self.declarations, &expression.ty) {
                    return Err(Flow::Guard(
                        "variant construction is outside owned byte variant v1",
                    ));
                }
                let ResolvedType::Nominal {
                    declaration: concrete_variant,
                    ..
                } = &expression.ty
                else {
                    return Err(Flow::Guard("owned byte variant type is not nominal"));
                };
                if concrete_variant != variant {
                    return Err(Flow::Guard(
                        "variant constructor identity disagrees with its concrete type",
                    ));
                }
                let declared_fields =
                    concrete_variant_case_fields(self.declarations, &expression.ty, case).ok_or(
                        Flow::Guard("variant constructor references an unauthenticated case"),
                    )?;
                if fields.len() != declared_fields.len() {
                    return Err(Flow::Guard(
                        "variant constructor field inventory is incomplete",
                    ));
                }
                let mut values = BTreeMap::new();
                for field in fields {
                    let declared_ty = declared_fields
                        .iter()
                        .find_map(|(field_id, ty)| (field_id == &field.field).then_some(ty))
                        .ok_or(Flow::Guard(
                            "variant constructor references an unauthenticated field",
                        ))?;
                    let value = self.evaluate(&field.value, environment, depth)?;
                    if !self.value_has_type(&value, declared_ty) {
                        return Err(Flow::Guard(
                            "variant constructor payload disagrees with its concrete field type",
                        ));
                    }
                    if values.insert(field.field.clone(), value).is_some() {
                        return Err(Flow::Guard(
                            "variant construction repeated an authenticated field identity",
                        ));
                    }
                }
                Ok(Value::Variant(Arc::new(OwnedVariantValue {
                    ty: expression.ty.clone(),
                    variant: variant.clone(),
                    case: case.clone(),
                    fields: values,
                })))
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
                    self.charge()?;
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
                                    let length = left.len().checked_add(right.len()).ok_or(
                                        Flow::Utf8MaterializationLimitExceeded {
                                            attempted_materializations: u64::MAX,
                                            attempted_bytes: u64::MAX,
                                        },
                                    )?;
                                    self.charge_utf8_materialization(length)?;
                                    let mut result = String::with_capacity(length);
                                    result.push_str(left);
                                    result.push_str(right);
                                    Ok(Value::String(result))
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
                                Some(value) => {
                                    let mut bytes = [0u8; 4];
                                    let value = value.encode_utf8(&mut bytes);
                                    Ok(Value::String(self.materialize_utf8_copy(value)?))
                                }
                                None => Err(Flow::Guard("ill-typed string operation operand")),
                            },
                            _ => Err(Flow::Guard("ill-typed string operation operand")),
                        },
                        crate::string_ops::StringOp::FromI64 => match values.first() {
                            Some(Value::Int(value)) => Ok(Value::String(
                                self.materialize_utf8_copy(&value.to_string())?,
                            )),
                            _ => Err(Flow::Guard("ill-typed string operation operand")),
                        },
                        crate::string_ops::StringOp::FromUsize => match values.first() {
                            Some(Value::Usize(value)) => Ok(Value::String(
                                self.materialize_utf8_copy(&value.to_string())?,
                            )),
                            _ => Err(Flow::Guard("ill-typed string operation operand")),
                        },
                    };
                }
                if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                    self.charge()?;
                    let mut values = Vec::with_capacity(args.len());
                    for argument in args {
                        values.push(self.evaluate(argument, environment, depth)?);
                    }
                    return match op {
                        crate::str_ops::StrOp::LenBytes => borrowed_text(&values[0])
                            .map(|value| Value::Int(value.len() as i64))
                            .ok_or(Flow::Guard("ill-typed borrowed string operand")),
                        crate::str_ops::StrOp::IsEmpty => borrowed_text(&values[0])
                            .map(|value| Value::Bool(value.is_empty()))
                            .ok_or(Flow::Guard("ill-typed borrowed string operand")),
                        crate::str_ops::StrOp::StartsWith => {
                            match (borrowed_text(&values[0]), borrowed_text(&values[1])) {
                                (Some(value), Some(prefix)) => {
                                    Ok(Value::Bool(value.starts_with(prefix)))
                                }
                                _ => Err(Flow::Guard("ill-typed borrowed string operand")),
                            }
                        }
                        crate::str_ops::StrOp::Contains => {
                            match (borrowed_text(&values[0]), borrowed_text(&values[1])) {
                                (Some(value), Some(needle)) => {
                                    crate::str_ops::contains(value, needle)
                                        .map(Value::Bool)
                                        .ok_or(Flow::Guard(
                                            "borrowed string operation exceeds byte budget",
                                        ))
                                }
                                _ => Err(Flow::Guard("ill-typed borrowed string operand")),
                            }
                        }
                    };
                }
                if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                    self.charge()?;
                    let mut values = Vec::with_capacity(args.len());
                    for argument in args {
                        values.push(self.evaluate(argument, environment, depth)?);
                    }
                    return match (op, values.as_slice()) {
                        (crate::byte_ops::ByteOp::Len, [Value::BorrowedSlice(value)]) => {
                            Ok(Value::Usize(value.bytes().len() as u64))
                        }
                        (
                            crate::byte_ops::ByteOp::Get,
                            [Value::BorrowedSlice(value), Value::Usize(index)],
                        ) => {
                            let byte = usize::try_from(*index)
                                .ok()
                                .and_then(|index| value.bytes().get(index))
                                .copied();
                            Ok(Value::OptionU8(byte))
                        }
                        (crate::byte_ops::ByteOp::Copy, [Value::BorrowedSlice(value)]) => {
                            let length = u64::try_from(value.bytes().len()).map_err(|_| {
                                Flow::Guard("owned byte payload length does not fit u64")
                            })?;
                            if length > crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES {
                                return Err(Flow::Guard(
                                    "owned byte payload exceeds the verified profile limit",
                                ));
                            }
                            let next_count = self
                                .next_byte_allocation
                                .checked_add(1)
                                .ok_or(Flow::Guard("owned byte allocation count overflowed"))?;
                            if next_count > crate::byte_data_capacity::MAX_BYTES_COPY_SITES {
                                return Err(Flow::Guard(
                                    "owned byte allocation count exceeds verified capacity",
                                ));
                            }
                            let next_payload = self
                                .allocated_byte_payload
                                .checked_add(length)
                                .ok_or(Flow::Guard("owned byte payload accounting overflowed"))?;
                            if next_payload
                                > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES
                            {
                                return Err(Flow::Guard(
                                    "owned byte payload exceeds verified capacity",
                                ));
                            }
                            self.next_byte_allocation = next_count;
                            self.allocated_byte_payload = next_payload;
                            // `Arc::from(&[u8])` copies into a fresh backing
                            // allocation for every non-empty input. The
                            // logical identity above distinguishes empty
                            // allocations as required by the language model.
                            Ok(Value::Bytes(OwnedBytesValue {
                                allocation: next_count,
                                bytes: Arc::from(value.bytes()),
                            }))
                        }
                        (crate::byte_ops::ByteOp::Range, _) => Err(Flow::Guard(
                            "byte_range reached interpreter as an ordinary call",
                        )),
                        _ => Err(Flow::Guard("ill-typed borrowed byte operation operand")),
                    };
                }
                if crate::host_io_ops::by_id(callee.as_str()).is_some() {
                    self.charge()?;
                    let [argument] = args.as_slice() else {
                        return Err(Flow::Guard("invalid stdout_write arity"));
                    };
                    let value = self.evaluate(argument, environment, depth)?;
                    let Value::BorrowedSlice(value) = value else {
                        return Err(Flow::Guard("ill-typed stdout_write operand"));
                    };
                    let stderr_length = self.stderr_transcript.as_ref().map_or(0, Vec::len);
                    let transcript = self
                        .stdout_transcript
                        .as_mut()
                        .ok_or(Flow::Guard("stdout_write reached effect-free interpreter"))?;
                    let next = transcript
                        .len()
                        .checked_add(value.bytes().len())
                        .ok_or(Flow::Guard("stdout transcript length overflowed"))?;
                    if next.checked_add(stderr_length).is_none_or(|combined| {
                        combined > crate::host_io_ops::MAX_STDOUT_TRANSCRIPT_BYTES as usize
                    }) {
                        return Err(Flow::Guard("stdout transcript exceeds verified capacity"));
                    }
                    transcript.extend_from_slice(value.bytes());
                    return Ok(Value::Usize(value.bytes().len() as u64));
                }
                let Some(function) = self.admitted.get(callee.as_str()) else {
                    return Err(Flow::Guard("call outside the admitted closure"));
                };
                let mut values: Vec<(ValueId, Value)> = Vec::with_capacity(args.len());
                for (param, argument) in function.params.iter().zip(args.iter()) {
                    let value = if param.ownership == hir::OwnershipMode::Borrow
                        && param.ty == ResolvedType::Bytes
                    {
                        self.borrow_bytes_call_argument(environment, argument, depth)?
                    } else if param.ownership == hir::OwnershipMode::Borrow
                        && matches!(param.ty, ResolvedType::Nominal { .. })
                    {
                        // The resolved argument expression retains the owned
                        // place's type/mode. Parameter ownership is the call
                        // boundary authority: charge the argument node, then
                        // stage an alias without tombstoning its caller slot.
                        self.charge()?;
                        let ResolvedExprKind::Place(place) = &argument.kind else {
                            return Err(Flow::Guard(
                                "borrowed record call argument is not a named place",
                            ));
                        };
                        if !place.projections.is_empty() {
                            return Err(Flow::Guard("borrowed record call argument is projected"));
                        }
                        self.lookup(environment, &place.root)?
                            .ok_or(Flow::Guard("borrowed record call owner is unavailable"))?
                    } else {
                        self.evaluate(argument, environment, depth)?
                    };
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
                                    let Some(slot) = environment.get_mut(&binding.id) else {
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
                                if let Err(flow) = self.charge() {
                                    interrupted = Some(flow);
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
            ResolvedExprKind::Match {
                mode,
                scrutinee,
                arms,
            } => {
                let staged = if *mode == hir::ResolvedMatchMode::Borrow {
                    // Borrow-mode resolution authenticates an unprojected
                    // named place. Charge that expression node, but retain
                    // the environment's owner and stage only an Arc alias.
                    self.charge()?;
                    let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                        return Err(Flow::Guard(
                            "borrowed record match has a non-place scrutinee",
                        ));
                    };
                    if !place.projections.is_empty() {
                        return Err(Flow::Guard(
                            "borrowed record match has a projected scrutinee",
                        ));
                    }
                    self.lookup(environment, &place.root)?
                        .ok_or(Flow::Guard("borrowed record owner is unavailable"))?
                } else {
                    self.evaluate(scrutinee, environment, depth)?
                };
                if let Value::Record(record) = staged {
                    let [arm] = arms.as_slice() else {
                        return Err(Flow::Guard(
                            "owned-byte record match does not have exactly one arm",
                        ));
                    };
                    if arm.guard.is_some() {
                        return Err(Flow::Guard(
                            "owned-byte record match unexpectedly has a guard",
                        ));
                    }
                    let crate::hir::ResolvedMatchPattern::Record {
                        record: pattern_record,
                        fields,
                        ..
                    } = &arm.pattern
                    else {
                        return Err(Flow::Guard(
                            "owned-byte record match has a non-record pattern",
                        ));
                    };
                    if pattern_record != &record.record || fields.len() != record.fields.len() {
                        return Err(Flow::Guard(
                            "owned-byte record pattern disagrees with its runtime carrier",
                        ));
                    }
                    let mut bindings = Vec::new();
                    match mode {
                        hir::ResolvedMatchMode::Own => {
                            bind_owned_pattern(
                                self.declarations,
                                &scrutinee.ty,
                                record,
                                fields,
                                &mut bindings,
                            )?;
                        }
                        hir::ResolvedMatchMode::Borrow => {
                            bind_borrowed_pattern(
                                self.declarations,
                                &record,
                                fields,
                                &mut bindings,
                            )?;
                        }
                        hir::ResolvedMatchMode::Value => {
                            return Err(Flow::Guard(
                                "owned-byte record reached a plain value match",
                            ));
                        }
                    }
                    let base = environment.len();
                    environment.extend(bindings);
                    let outcome = self.evaluate(&arm.value, environment, depth);
                    environment.truncate(base);
                    return outcome;
                }
                if let Value::Variant(variant) = staged {
                    if !variant_pattern_is_admitted(self.declarations, *mode, &scrutinee.ty, arms) {
                        return Err(Flow::Guard(
                            "owned byte variant match is outside the authenticated profile",
                        ));
                    }
                    let ResolvedType::Nominal {
                        declaration: expected_variant,
                        ..
                    } = &scrutinee.ty
                    else {
                        return Err(Flow::Guard("owned byte variant scrutinee is not nominal"));
                    };
                    if variant.ty != scrutinee.ty || &variant.variant != expected_variant {
                        return Err(Flow::Guard(
                            "owned byte variant runtime carrier disagrees with its scrutinee",
                        ));
                    }
                    let arm = arms
                        .iter()
                        .find(|arm| {
                            matches!(
                                &arm.pattern,
                                hir::ResolvedMatchPattern::Variant { case, .. }
                                    if case == &variant.case
                            )
                        })
                        .ok_or(Flow::Guard(
                            "owned byte variant active case selected no exhaustive arm",
                        ))?;
                    let hir::ResolvedMatchPattern::Variant {
                        variant: pattern_variant,
                        case: pattern_case,
                        fields,
                    } = &arm.pattern
                    else {
                        unreachable!("admitted owned byte variant arm is a variant pattern")
                    };
                    if pattern_variant != &variant.variant || pattern_case != &variant.case {
                        return Err(Flow::Guard(
                            "owned byte variant pattern identity disagrees with its carrier",
                        ));
                    }
                    let declared_fields =
                        concrete_variant_case_fields(self.declarations, &variant.ty, &variant.case)
                            .ok_or(Flow::Guard(
                                "owned byte variant carrier has an unauthenticated active case",
                            ))?;
                    if fields.len() != declared_fields.len()
                        || variant.fields.len() != declared_fields.len()
                    {
                        return Err(Flow::Guard(
                            "owned byte variant active payload inventory is inconsistent",
                        ));
                    }
                    let mut bindings = Vec::with_capacity(fields.len());
                    match mode {
                        hir::ResolvedMatchMode::Own => {
                            let mut variant = Arc::try_unwrap(variant).map_err(|_| {
                                Flow::Guard("owned byte variant still has a live alias at transfer")
                            })?;
                            for field in fields {
                                let declared_ty = declared_fields
                                    .iter()
                                    .find_map(|(field_id, ty)| {
                                        (field_id == &field.field).then_some(ty)
                                    })
                                    .ok_or(Flow::Guard(
                                        "owned byte variant pattern references an unauthenticated field",
                                    ))?;
                                let value =
                                    variant.fields.remove(&field.field).ok_or(Flow::Guard(
                                        "owned byte variant pattern references an absent payload",
                                    ))?;
                                if !self.value_has_type(&value, declared_ty) {
                                    return Err(Flow::Guard(
                                        "owned byte variant payload type changed before transfer",
                                    ));
                                }
                                bindings.push((field.binding.id.clone(), value));
                            }
                            if !variant.fields.is_empty() {
                                return Err(Flow::Guard(
                                    "owned byte variant transfer left unauthenticated payloads",
                                ));
                            }
                        }
                        hir::ResolvedMatchMode::Borrow => {
                            for field in fields {
                                let declared_ty = declared_fields
                                    .iter()
                                    .find_map(|(field_id, ty)| {
                                        (field_id == &field.field).then_some(ty)
                                    })
                                    .ok_or(Flow::Guard(
                                        "borrowed byte variant pattern references an unauthenticated field",
                                    ))?;
                                let value = variant.fields.get(&field.field).ok_or(Flow::Guard(
                                    "borrowed byte variant pattern references an absent payload",
                                ))?;
                                if !self.value_has_type(value, declared_ty) {
                                    return Err(Flow::Guard(
                                        "borrowed byte variant payload type changed before aliasing",
                                    ));
                                }
                                bindings.push((field.binding.id.clone(), self.clone_value(value)?));
                            }
                        }
                        hir::ResolvedMatchMode::Value => {
                            return Err(Flow::Guard(
                                "owned byte variant reached a plain value match",
                            ));
                        }
                    }
                    let base = environment.len();
                    environment.extend(bindings);
                    let outcome = self.evaluate(&arm.value, environment, depth);
                    environment.truncate(base);
                    return outcome;
                }
                for arm in arms {
                    let mut aggregate_bindings = Vec::new();
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
                        crate::hir::ResolvedMatchPattern::Variant {
                            variant,
                            case,
                            fields,
                        } => {
                            if variant.as_str() != crate::prelude::OPTION_ID {
                                return Err(Flow::Guard(
                                    "non-Option variant reached byte evaluation",
                                ));
                            }
                            match (&staged, case.as_str()) {
                                (Value::OptionU8(None), crate::prelude::OPTION_NONE_ID)
                                    if fields.is_empty() =>
                                {
                                    true
                                }
                                (Value::OptionU8(Some(value)), crate::prelude::OPTION_SOME_ID)
                                    if fields.len() == 1
                                        && fields[0].field.as_str()
                                            == crate::prelude::OPTION_SOME_VALUE_ID =>
                                {
                                    aggregate_bindings
                                        .push((fields[0].binding.id.clone(), Value::Uint8(*value)));
                                    true
                                }
                                (Value::OptionU8(_), _) => false,
                                _ => {
                                    return Err(Flow::Guard(
                                        "variant pattern has non-variant value",
                                    ))
                                }
                            }
                        }
                        crate::hir::ResolvedMatchPattern::Record { .. } => {
                            return Err(Flow::Guard(
                                "aggregate match shape reached scalar evaluation",
                            ));
                        }
                    };
                    if !selected {
                        continue;
                    }
                    let aggregate_count = aggregate_bindings.len();
                    environment.extend(aggregate_bindings);
                    // Binding arms capture the staged scrutinee value.
                    let mut has_bound_value = false;
                    if let crate::hir::ResolvedMatchPattern::Binding(binding) = &arm.pattern {
                        let bound_value = self.clone_value(&staged)?;
                        environment.push((binding.id.clone(), bound_value));
                        has_bound_value = true;
                    }
                    let guard_ok = match &arm.guard {
                        Some(guard) => match self.evaluate(guard.as_ref(), environment, depth)? {
                            Value::Bool(flag) => flag,
                            _ => return Err(Flow::Guard("non-boolean match guard")),
                        },
                        None => true,
                    };
                    if !guard_ok {
                        if has_bound_value {
                            environment.pop();
                        }
                        environment.truncate(environment.len().saturating_sub(aggregate_count));
                        continue;
                    }
                    let outcome = self.evaluate(&arm.value, environment, depth);
                    if has_bound_value {
                        environment.pop();
                    }
                    environment.truncate(environment.len().saturating_sub(aggregate_count));
                    return outcome;
                }
                Err(Flow::Guard("refutable match selected no arm"))
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                let base = self.evaluate(base, environment, depth)?;
                let Value::Record(base) = base else {
                    return Err(Flow::Guard("record update base is not a record carrier"));
                };
                if &base.record != record {
                    return Err(Flow::Guard("record update base identity is inconsistent"));
                }
                let mut replacements = Vec::with_capacity(fields.len());
                for field in fields {
                    let value = self.evaluate(&field.value, environment, depth)?;
                    replacements.push((field.field.clone(), value));
                }
                update_owned_record(self.declarations, &expression.ty, base, replacements)
            }
            ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Upcast { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::NativeRustImportCall(_) => Err(Flow::Guard(
                "aggregate/import/match/try shape reached evaluation",
            )),
        }
    }

    fn intern_trace_identity(&mut self, identity: &str) -> Arc<str> {
        if let Some(retained) = self.trace_identities.get(identity) {
            return Arc::clone(retained);
        }
        let retained: Arc<str> = Arc::from(identity);
        self.trace_identities
            .insert(identity.to_owned(), Arc::clone(&retained));
        retained
    }

    fn set_trace_phase(&mut self, phase: ResolvedTracePhase) {
        if self.trace_limit != 0 {
            self.trace_phase = phase;
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
        (Value::Usize(a), Value::Usize(b)) => match op {
            BinaryOp::Add => {
                arithmetic(a.checked_add(b).map(Value::Usize), StatusCase::AddOverflow)
            }
            BinaryOp::Sub => {
                arithmetic(a.checked_sub(b).map(Value::Usize), StatusCase::SubOverflow)
            }
            BinaryOp::Mul => {
                arithmetic(a.checked_mul(b).map(Value::Usize), StatusCase::MulOverflow)
            }
            BinaryOp::Div => a.checked_div(b).map_or_else(
                || arithmetic(None, StatusCase::DivisionByZero),
                |quotient| Some(Ok(Value::Usize(quotient))),
            ),
            BinaryOp::Rem => a.checked_rem(b).map_or_else(
                || arithmetic(None, StatusCase::RemainderByZero),
                |remainder| Some(Ok(Value::Usize(remainder))),
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

fn render_with_profile(facts: &RenderFacts<'_>, profile: SourceProfile) -> String {
    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"function\":{{\"stable_id\":{},\"name\":{}}},\
\"arguments\":[{}],\
\"limits\":{{\"max_bytes\":{},\"max_steps\":{}}},\
\"fuel\":{{\"steps_used\":{},\"budget\":{},\"exhausted\":{}}},\
\"outcome\":{},\"nonclaims\":[{}]}}",
        profile.schema(),
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
        profile.schema(),
        quote_json(&domain_digest(profile.payload_domain(), payload.as_bytes())),
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
    verify_envelope_with_profile(envelope, SourceProfile::Legacy)
}

fn verify_envelope_with_profile(envelope: &str, profile: SourceProfile) -> Result<(), Diagnostic> {
    let schema = profile.schema();
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
    if object["schema"].as_str() != Some(schema) {
        return Err(consistency_error(format!(
            "envelope schema must be {schema}"
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
    if envelope_digest != domain_digest(profile.payload_domain(), payload.as_bytes()) {
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
    if payload_object["schema"].as_str() != Some(schema) {
        return Err(consistency_error(format!(
            "payload schema must be {schema}"
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
            Some(
                type_text @ ("i64" | "i32" | "u8" | "usize" | "char" | "f32" | "f64" | "bool"
                | "str" | "Slice<u8>"),
            ) => type_text,
            _ => {
                return Err(consistency_error(
                    "argument type is outside the canonical interpreter boundary".to_owned(),
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
                    Some(type_text @ ("i64" | "i32" | "u8" | "usize" | "char" | "f32" | "f64" | "bool")) => {
                        type_text
                    }
                    _ => return Err(consistency_error(
                        "returned outcome type must be one of i64, i32, u8, usize, char, f32, f64, bool"
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
    if matches!(profile, SourceProfile::InternalStrings) {
        internal_strings::verify_canonical(envelope, &value, max_bytes)?;
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
        "usize" => matches!(parse_argument(value_text), Ok(ArgumentValue::Usize(_))),
        "bool" => matches!(parse_argument(value_text), Ok(ArgumentValue::Bool(_))),
        "str" => matches!(
            parse_argument(value_text),
            Ok(ArgumentValue::BorrowedStr(value))
                if serde_json::to_string(&value).ok().as_deref() == Some(value_text)
        ),
        "Slice<u8>" => matches!(
            parse_argument(value_text),
            Ok(ArgumentValue::BorrowedSlice(value))
                if serde_json::to_string(&value).ok().as_deref() == Some(value_text)
        ),
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

    #[test]
    fn projected_owned_move_tombstones_only_the_exact_field_without_cloning() {
        let root = ValueId::intrinsic_parameter("test.packet", 0);
        let left = hir::DeclarationId::new("test.packet.left");
        let right = hir::DeclarationId::new("test.packet.right");
        let left_value = OwnedBytesValue {
            allocation: 1,
            bytes: Arc::from([1u8, 2u8].as_slice()),
        };
        let right_value = OwnedBytesValue {
            allocation: 2,
            bytes: Arc::from([3u8].as_slice()),
        };
        let mut environment = Environment::from(vec![(
            root.clone(),
            Value::Record(Arc::new(OwnedRecordValue {
                record: hir::DeclarationId::new("test.packet"),
                fields: BTreeMap::from([
                    (left.clone(), Value::Bytes(left_value.clone())),
                    (right.clone(), Value::Bytes(right_value.clone())),
                ]),
            })),
        )]);
        let moved = take_owned_place(
            &mut environment,
            &hir::Place {
                root: root.clone(),
                projections: vec![hir::PlaceProjection::Field(left.clone())],
            },
        )
        .unwrap();
        assert_eq!(moved, Value::Bytes(left_value));
        let Value::Record(record) = environment.get(&root).unwrap() else {
            panic!("parent record must remain present");
        };
        assert!(matches!(record.fields.get(&left), Some(Value::Moved)));
        assert_eq!(record.fields.get(&right), Some(&Value::Bytes(right_value)));
        assert!(take_owned_place(
            &mut environment,
            &hir::Place {
                root,
                projections: vec![hir::PlaceProjection::Field(left)],
            },
        )
        .is_none());
    }

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
    fn owned_byte_variant_runtime_rejects_hostile_case_and_field_identities() {
        let program = resolved(include_str!("../tests/owned_byte_variant_v1_fixture.spx"));
        for mutate_field in [false, true] {
            let mut hostile = program.clone();
            let body = &mut hostile
                .functions
                .iter_mut()
                .find(|function| function.id.as_str() == "sum.make")
                .expect("fixture make function")
                .body;
            let ResolvedExprKind::Block {
                tail: constructor, ..
            } = &mut body.kind
            else {
                panic!("fixture make body must remain a block")
            };
            let ResolvedExprKind::ConstructVariant { case, fields, .. } = &mut constructor.kind
            else {
                panic!("fixture make body must construct the owned variant")
            };
            if mutate_field {
                fields[0].field = hir::DeclarationId::new("hostile.variant.field");
            } else {
                *case = hir::DeclarationId::new("hostile.variant.case");
            }
            let diagnostics =
                evaluate_resolved_zero_arg_i64(&hostile, "app.main", 10_000).unwrap_err();
            assert_eq!(diagnostics[0].code, "SPX-F102");
            assert!(diagnostics[0].message.contains(REASON_VARIANT_CONSTRUCTION));
        }

        let inspect = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "sum.inspect")
            .expect("fixture inspect function");
        let parameter = &inspect.params[0];
        let ResolvedType::Nominal { declaration, .. } = &parameter.ty else {
            panic!("fixture inspect parameter must be the owned variant")
        };
        let admitted = admitted_resolved_functions(&program);
        for (case, fields) in [
            (
                hir::DeclarationId::new("hostile.variant.case"),
                BTreeMap::new(),
            ),
            (hir::DeclarationId::new("sum.choice.data"), BTreeMap::new()),
        ] {
            let mut evaluator = Evaluator {
                admitted: FunctionLookup::Borrowed(&admitted),
                declarations: &program.declarations,
                steps: 0,
                budget: 10_000,
                next_byte_allocation: 0,
                allocated_byte_payload: 0,
                utf8_materialization_budget: Utf8MaterializationBudget::UnlimitedLegacy,
                stdout_transcript: None,
                stderr_transcript: None,
                command_input: None,
                cancellation: PreparedCancellation::Never,
                trace_limit: 0,
                trace_events: Vec::new(),
                dropped_trace_events: 0,
                current_function: None,
                trace_identities: BTreeMap::new(),
                trace_phase: ResolvedTracePhase::Body,
                failure_detail: None,
            };
            let outcome = evaluator.call_frame(
                inspect,
                vec![(
                    parameter.id.clone(),
                    Value::Variant(Arc::new(OwnedVariantValue {
                        ty: parameter.ty.clone(),
                        variant: declaration.clone(),
                        case,
                        fields,
                    })),
                )],
                0,
            );
            assert!(matches!(outcome, Err(Flow::Guard(_))));
        }
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

    #[test]
    fn borrowed_str_clones_preserve_invocation_root_and_shared_evidence() {
        let root = ValueId::intrinsic_parameter("test.borrowed.root", 0);
        let original = Value::BorrowedStr(BorrowedStrValue {
            invocation_root: root.clone(),
            bytes: Arc::from("aé\0z".as_bytes()),
        });
        let Value::BorrowedStr(value) = &original else {
            unreachable!("constructed borrowed string value")
        };
        let forwarded = Value::BorrowedStr(value.clone());
        let (Value::BorrowedStr(original), Value::BorrowedStr(forwarded)) = (original, forwarded)
        else {
            panic!("borrowed values retain their distinct runtime form")
        };
        assert_eq!(original.invocation_root, root);
        assert_eq!(forwarded.invocation_root, root);
        assert!(Arc::ptr_eq(&original.bytes, &forwarded.bytes));
    }

    #[test]
    fn owned_utf8_budget_is_cumulative_atomic_and_exact_at_both_fixed_caps() {
        let mut bytes = Utf8MaterializationBudget::fixed();
        bytes
            .charge(MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES as usize)
            .unwrap();
        assert_eq!(bytes.usage(), (1, MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES));
        assert!(matches!(
            bytes.charge(1),
            Err(Flow::Utf8MaterializationLimitExceeded {
                attempted_materializations: 2,
                attempted_bytes,
            }) if attempted_bytes == MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES + 1
        ));
        assert_eq!(
            bytes.usage(),
            (1, MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES),
            "a rejected precharge must not mutate either cumulative counter"
        );

        let mut allocations = Utf8MaterializationBudget::fixed();
        for _ in 0..MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS {
            allocations.charge(0).unwrap();
        }
        assert_eq!(allocations.usage(), (MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS, 0));
        assert!(matches!(
            allocations.charge(0),
            Err(Flow::Utf8MaterializationLimitExceeded {
                attempted_materializations,
                attempted_bytes: 0,
            }) if attempted_materializations == MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS + 1
        ));
        assert_eq!(allocations.usage(), (MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS, 0));
    }

    #[test]
    fn owned_utf8_intrinsics_precharge_before_result_allocation_and_legacy_is_unlimited() {
        let concat = resolved(
            "module test.utf8_concat;\n\n@id(\"text.concat\")\nfn concat() -> string { string_concat(\"a\", \"b\") }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
        );
        let entry = concat
            .functions
            .iter()
            .find(|function| function.id.as_str() == "text.concat")
            .unwrap();
        let admitted = concat
            .functions
            .iter()
            .map(|function| (function.id.as_str(), function))
            .collect::<BTreeMap<_, _>>();
        let (outcome, _, _, usage) = evaluate_resolved_entry_with_utf8_budget(
            entry,
            &[],
            &admitted,
            &concat.declarations,
            100,
            false,
            Utf8MaterializationBudget::Fixed {
                used_materializations: 0,
                used_bytes: MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES - 3,
            },
        );
        assert!(matches!(
            outcome,
            Err(Flow::Utf8MaterializationLimitExceeded {
                attempted_materializations: 3,
                attempted_bytes,
            }) if attempted_bytes == MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES + 1
        ));
        assert_eq!(
            usage,
            (2, MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES - 1),
            "the rejected concat result must not mutate the accepted literal charges"
        );

        let from_char = resolved(
            "module test.utf8_char;\n\n@id(\"text.char\")\nfn text() -> string { string_from_char('\\u{1f600}') }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
        );
        let entry = from_char
            .functions
            .iter()
            .find(|function| function.id.as_str() == "text.char")
            .unwrap();
        let admitted = from_char
            .functions
            .iter()
            .map(|function| (function.id.as_str(), function))
            .collect::<BTreeMap<_, _>>();
        let (outcome, _, _, usage) = evaluate_resolved_entry_with_utf8_budget(
            entry,
            &[],
            &admitted,
            &from_char.declarations,
            100,
            false,
            Utf8MaterializationBudget::Fixed {
                used_materializations: 0,
                used_bytes: MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES - 3,
            },
        );
        assert!(matches!(
            outcome,
            Err(Flow::Utf8MaterializationLimitExceeded {
                attempted_materializations: 1,
                attempted_bytes,
            }) if attempted_bytes == MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES + 1
        ));
        assert_eq!(usage, (0, MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES - 3));

        let mut legacy = Utf8MaterializationBudget::UnlimitedLegacy;
        for _ in 0..=MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS {
            legacy.charge(usize::MAX).unwrap();
        }
        assert_eq!(legacy.usage(), (0, 0));
    }

    #[test]
    fn owned_utf8_primitive_replays_descriptor_parameter_and_result_facts() {
        const SHA: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        let original = resolved(
            "module test.utf8_descriptor;\n\n@id(\"api.value\")\nfn value(input: i64) -> i64 { input }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
        );
        let descriptor = crate::project::derive_public_api_descriptor(
            &original,
            &["api.value".to_owned()],
            crate::project::PublicApiSubject {
                project_schema: crate::project::PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
                project_revision: SHA,
                workspace_revision: SHA,
                project_graph_digest: SHA,
            },
        )
        .unwrap();
        let export = &descriptor.exports()[0];

        let parameter_drift = resolved(
            "module test.utf8_descriptor;\n\n@id(\"api.value\")\nfn value(changed: i64) -> i64 { changed }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
        );
        let diagnostics = evaluate_resolved_owned_utf8_api(
            &parameter_drift,
            export,
            &[PublicApiArgument::I64(1)],
            100,
        )
        .unwrap_err();
        assert_eq!(diagnostics[0].code, "SPX-F102");
        assert!(diagnostics[0].message.contains("parameter 0 disagrees"));

        let result_drift = resolved(
            "module test.utf8_descriptor;\n\n@id(\"api.value\")\nfn value(input: i64) -> bool { input == 0 }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
        );
        let diagnostics = evaluate_resolved_owned_utf8_api(
            &result_drift,
            export,
            &[PublicApiArgument::I64(1)],
            100,
        )
        .unwrap_err();
        assert_eq!(diagnostics[0].code, "SPX-F102");
        assert!(diagnostics[0].message.contains("result disagrees"));

        let closure_drift = resolved(
            "module test.utf8_descriptor;\n\n@id(\"api.value\")\nfn value(input: i64) -> i64 { if string_is_empty(\"\") { input } else { input } }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
        );
        let diagnostics = evaluate_resolved_owned_utf8_api(
            &closure_drift,
            export,
            &[PublicApiArgument::I64(1)],
            100,
        )
        .unwrap_err();
        assert_eq!(diagnostics[0].code, "SPX-F102");
        assert!(diagnostics[0].message.contains("failed replay"));
    }

    #[test]
    fn owned_utf8_literal_place_clone_call_transfer_and_host_copy_are_exact() {
        let place = resolved(
            "module test.utf8_place;\n\n@id(\"text.place\")\nfn text() -> string { let value = \"x\"; value }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
        );
        let entry = place
            .functions
            .iter()
            .find(|function| function.id.as_str() == "text.place")
            .unwrap();
        let admitted = place
            .functions
            .iter()
            .map(|function| (function.id.as_str(), function))
            .collect::<BTreeMap<_, _>>();
        let (outcome, _, _, usage) = evaluate_resolved_entry_with_utf8_budget(
            entry,
            &[],
            &admitted,
            &place.declarations,
            100,
            false,
            Utf8MaterializationBudget::fixed(),
        );
        assert_eq!(outcome.unwrap(), Value::String("x".to_owned()));
        assert_eq!(
            usage,
            (2, 2),
            "literal and preserving place clone charge once each"
        );

        let transfer = resolved(
            "module test.utf8_transfer;\n\nfn helper() -> string { \"x\" }\n\n@id(\"text.transfer\")\nfn text() -> string { helper() }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
        );
        let entry = transfer
            .functions
            .iter()
            .find(|function| function.id.as_str() == "text.transfer")
            .unwrap();
        let admitted = transfer
            .functions
            .iter()
            .map(|function| (function.id.as_str(), function))
            .collect::<BTreeMap<_, _>>();
        for _ in 0..2 {
            let (outcome, _, _, usage) = evaluate_resolved_entry_with_utf8_budget(
                entry,
                &[],
                &admitted,
                &transfer.declarations,
                100,
                false,
                Utf8MaterializationBudget::fixed(),
            );
            assert_eq!(outcome.unwrap(), Value::String("x".to_owned()));
            assert_eq!(
                usage,
                (1, 1),
                "callee result transfer and repeated invocations must not clone or retain quota"
            );
        }

        let exact = "x".repeat(crate::project::MAX_PUBLIC_API_OWNED_OUTPUT_BYTES);
        let mut events = Vec::new();
        let output = copy_out_owned_utf8_api(
            Value::String(exact.clone()),
            &ResolvedType::String,
            &mut events,
        )
        .unwrap();
        assert_eq!(output, OwnedUtf8ApiValue::Utf8(exact));
        assert_eq!(events, [OwnedUtf8SettlementEvent::CopyOutAndSettleUtf8]);

        let oversize = "x".repeat(crate::project::MAX_PUBLIC_API_OWNED_OUTPUT_BYTES + 1);
        let mut events = Vec::new();
        assert_eq!(
            copy_out_owned_utf8_api(Value::String(oversize), &ResolvedType::String, &mut events,),
            Err("owned UTF-8 API String result exceeds the public output bound")
        );
        assert!(events.is_empty(), "rejected host copy-out cannot settle");
    }

    #[test]
    fn owned_utf8_primitive_reports_exact_attempted_and_committed_byte_quota() {
        const SHA: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        fn program(length: usize) -> hir::ResolvedProgram {
            let literal = "x".repeat(length);
            resolved(&format!(
                "module test.utf8_quota;\n\n@id(\"api.text\")\nfn text() -> string {{ \"{literal}\" }}\n\n@id(\"app.main\")\nfn main() -> i64 {{ 0 }}\n"
            ))
        }
        fn export(program: &hir::ResolvedProgram) -> crate::project::PublicApiDescriptor {
            crate::project::derive_public_api_descriptor(
                program,
                &["api.text".to_owned()],
                crate::project::PublicApiSubject {
                    project_schema: crate::project::PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
                    project_revision: SHA,
                    workspace_revision: SHA,
                    project_graph_digest: SHA,
                },
            )
            .unwrap()
        }

        let exact = program(MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES as usize);
        let descriptor = export(&exact);
        let evaluation = evaluate_resolved_owned_utf8_api(
            &exact,
            &descriptor.exports()[0],
            &[],
            MAX_STEPS_LIMIT,
        )
        .unwrap();
        assert!(matches!(
            evaluation.outcome,
            OwnedUtf8ApiEvaluationOutcome::Returned(OwnedUtf8ApiValue::Utf8(ref value))
                if value.len() as u64 == MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES
        ));
        assert_eq!(evaluation.utf8_materializations_used, 1);
        assert_eq!(
            evaluation.utf8_bytes_used,
            MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES
        );
        assert_eq!(
            evaluation.settlement_events,
            [OwnedUtf8SettlementEvent::CopyOutAndSettleUtf8]
        );

        let oversize = program((MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES + 1) as usize);
        let descriptor = export(&oversize);
        let evaluation = evaluate_resolved_owned_utf8_api(
            &oversize,
            &descriptor.exports()[0],
            &[],
            MAX_STEPS_LIMIT,
        )
        .unwrap();
        assert_eq!(
            evaluation.outcome,
            OwnedUtf8ApiEvaluationOutcome::Utf8MaterializationLimitExceeded {
                attempted_materializations: 1,
                attempted_bytes: MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES + 1,
            }
        );
        assert_eq!(evaluation.utf8_materializations_used, 0);
        assert_eq!(evaluation.utf8_bytes_used, 0);
        assert!(evaluation.settlement_events.is_empty());
    }
}
