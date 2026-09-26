//! Closed recovery bytes for the admitted sequential source-resumable lane.
//!
//! This is intentionally narrower than the reference `resumable_effects`
//! journal codec. It preserves only the pure Copy-scalar continuation the
//! interpreter already admits; it is neither an external-await ABI nor a
//! durable-runtime promise. Decoding receives the checked program, selected
//! function and original arguments anew, lowers that program again, and
//! derives the state/binding rather than trusting either from the bytes. The
//! self-digest detects accidental corruption; exact request values are
//! independently replay-checked by the existing resume path.
//!
//! This driver excludes the control-dependent lane (issue #296: a `yield`
//! inside an `if`/`else` branch or `while` body). [`decode`]'s
//! [`checked_plan`] calls [`lowering::lower_sequential`], which itself
//! refuses any function whose yields are not *all* direct top-level slots
//! (`"resumable yield is nested instead of occupying a direct top-level
//! slot"`); that refusal is folded into the generic
//! [`CheckpointError::ProgramMismatch`] like every other lowering failure,
//! rather than reported as its own class. A control-dependent function's
//! non-durable checkpoint is
//! `resumable_effects::source_checkpoint::control`'s separate v3 envelope;
//! its durable one is `resumable_effects::continuation`'s journal, which
//! drives both lanes.

use super::{
    bind_scalar_arguments, resumable_scalars, typed_resume_value, ArgumentValue,
    ResumableContinuation, ResumableYieldRecord,
};
use crate::hir::{self, IdentityOrigin};
use crate::resumable_effects::lowering::{
    self, ResumableScalar, SequentialResumablePlan, MAX_RESUMABLE_YIELDS,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// A breaking change to this wire shape always gets a new schema name.
pub(crate) const SEQUENTIAL_CHECKPOINT_SCHEMA: &str =
    "semaprax.source-resumable-sequential-checkpoint.v1";

/// Eight sites mean at most seven settled request/answer records. The byte
/// bound is deliberately much smaller than a general journal: this carrier
/// contains no source, HIR, effects, or owned state.
const MAX_CHECKPOINT_BYTES: usize = 16 * 1024;
const MAX_CHECKPOINT_FIELD_BYTES: usize = 1024;
const MAX_HISTORY: usize = MAX_RESUMABLE_YIELDS - 1;

/// Why a source-resumable continuation was not recovered. The bytes remain
/// proof data throughout: none of these paths dispatches an effect or runs a
/// source function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CheckpointError {
    TooLarge,
    Malformed(String),
    SchemaMismatch,
    DigestMismatch,
    NonCanonical,
    FunctionMismatch,
    ProgramMismatch,
    SuspensionMismatch,
}

/// Encode an already-created continuation into closed canonical JSON bytes.
/// The caller must supply the selected function ID independently again to
/// recover it; original invocation arguments deliberately stay out of this
/// portable proof carrier and are re-bound at decode time.
pub(crate) fn encode(
    function_id: &str,
    continuation: &ResumableContinuation,
) -> Result<Vec<u8>, CheckpointError> {
    // Bound variable text before JSON escaping or formatting allocates. JSON
    // can expand each input byte by at most six bytes; these field caps plus
    // the fixed scalar/history limits stay within the document byte ceiling.
    if function_id.len() > MAX_CHECKPOINT_FIELD_BYTES
        || continuation.state.as_str().len() > MAX_CHECKPOINT_FIELD_BYTES
    {
        return Err(CheckpointError::TooLarge);
    }
    if continuation.history.len() > MAX_HISTORY {
        return Err(CheckpointError::SuspensionMismatch);
    }
    let history = Value::Array(
        continuation
            .history
            .iter()
            .map(|record| {
                json!({
                    "request": scalar_json(&record.request),
                    "answer": scalar_json(&record.answer),
                })
            })
            .collect(),
    );
    let payload = payload_json(
        SEQUENTIAL_CHECKPOINT_SCHEMA,
        function_id,
        continuation.state.as_str(),
        binding_hex(continuation.binding.as_bytes()),
        scalar_json(&continuation.request),
        history,
    );
    let digest = checkpoint_digest(&payload);
    let bytes = format!(
        "{}\n",
        json!({
            "schema": SEQUENTIAL_CHECKPOINT_SCHEMA,
            "function": function_id,
            "state": continuation.state.as_str(),
            "binding": binding_hex(continuation.binding.as_bytes()),
            "request": scalar_json(&continuation.request),
            "history": continuation.history.iter().map(|record| json!({
                "request": scalar_json(&record.request),
                "answer": scalar_json(&record.answer),
            })).collect::<Vec<_>>(),
            "digest": digest,
        })
    )
    .into_bytes();
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(CheckpointError::TooLarge);
    }
    Ok(bytes)
}

/// Recover a sequential continuation from untrusted bytes. `program`,
/// `function_id`, and `arguments` are caller-supplied current facts; the
/// document cannot choose them. Recovery re-lowers the program and derives
/// the expected state/binding from those facts plus the decoded answer bits.
/// It performs no source evaluation and grants no ability to answer a yield.
/// Request values remain proof claims until the normal resume replay checks
/// them against actual execution.
pub(crate) fn decode(
    program: &hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    bytes: &[u8],
) -> Result<ResumableContinuation, CheckpointError> {
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(CheckpointError::TooLarge);
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|error| CheckpointError::Malformed(error.to_string()))?;
    let document: Value = serde_json::from_str(text)
        .map_err(|error| CheckpointError::Malformed(error.to_string()))?;
    keys(
        &document,
        &[
            "schema", "function", "state", "binding", "request", "history", "digest",
        ],
    )?;
    let schema = required_str(&document, "schema")?;
    if schema != SEQUENTIAL_CHECKPOINT_SCHEMA {
        return Err(CheckpointError::SchemaMismatch);
    }
    let encoded_function = required_str(&document, "function")?;
    if encoded_function != function_id {
        return Err(CheckpointError::FunctionMismatch);
    }
    let state = required_str(&document, "state")?;
    let claimed_binding = binding_from_hex(required_str(&document, "binding")?)?;
    let request_value = document["request"].clone();
    let history_value = document["history"].clone();
    let history = history_value
        .as_array()
        .ok_or_else(|| CheckpointError::Malformed("history".to_owned()))?;
    if history.len() > MAX_HISTORY {
        return Err(CheckpointError::SuspensionMismatch);
    }
    let payload = payload_json(
        schema,
        encoded_function,
        state,
        binding_hex(&claimed_binding),
        request_value.clone(),
        history_value.clone(),
    );
    let claimed_digest = required_str(&document, "digest")?;
    if claimed_digest != checkpoint_digest(&payload) {
        return Err(CheckpointError::DigestMismatch);
    }

    let (plan, scalar_arguments) = checked_plan(program, function_id, arguments)?;
    let index = plan
        .suspensions
        .iter()
        .position(|site| site.state.id.as_str() == state)
        .ok_or(CheckpointError::SuspensionMismatch)?;
    if history.len() != index {
        return Err(CheckpointError::SuspensionMismatch);
    }

    let mut records = Vec::with_capacity(history.len());
    let mut answer_scalars = Vec::with_capacity(history.len());
    for (index, raw) in history.iter().enumerate() {
        keys(raw, &["request", "answer"])?;
        let request = scalar_from_json(&raw["request"])?;
        let answer = scalar_from_json(&raw["answer"])?;
        let site = &plan.suspensions[index];
        typed_resume_value(&site.request_type, &request, "historical request")
            .map_err(|_| CheckpointError::SuspensionMismatch)?;
        typed_resume_value(&site.response_type, &answer, "historical answer")
            .map_err(|_| CheckpointError::SuspensionMismatch)?;
        answer_scalars.push(
            resumable_scalars(&[answer.clone()])
                .expect("decoded scalar answer")
                .pop()
                .expect("one decoded scalar answer"),
        );
        records.push(ResumableYieldRecord { request, answer });
    }
    let request = scalar_from_json(&request_value)?;
    let current = &plan.suspensions[index];
    typed_resume_value(&current.request_type, &request, "request")
        .map_err(|_| CheckpointError::SuspensionMismatch)?;
    let binding = plan
        .suspension_binding_at(index, &scalar_arguments, &answer_scalars)
        .map_err(|_| CheckpointError::ProgramMismatch)?;
    if binding.as_bytes() != &claimed_binding {
        return Err(CheckpointError::SuspensionMismatch);
    }
    let continuation = ResumableContinuation {
        state: current.state.id.clone(),
        binding,
        request,
        history: records,
    };

    // Parsing JSON alone accepts duplicate keys and insignificant whitespace.
    // Exact re-encoding keeps this wire closed and deterministic, rejecting
    // both rather than letting two byte sequences mean one continuation.
    if encode(function_id, &continuation)?.as_slice() != bytes {
        return Err(CheckpointError::NonCanonical);
    }
    Ok(continuation)
}

fn checked_plan(
    program: &hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
) -> Result<(SequentialResumablePlan, Vec<ResumableScalar>), CheckpointError> {
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == function_id)
        .ok_or(CheckpointError::ProgramMismatch)?;
    if !program
        .declarations
        .declaration(&entry.id)
        .is_some_and(|declaration| declaration.identity_origin == IdentityOrigin::Explicit)
        || entry.yields.is_none()
        || !super::super::resolved_signature_is_admitted(entry, &program.declarations)
    {
        return Err(CheckpointError::ProgramMismatch);
    }
    bind_scalar_arguments(entry, arguments).map_err(|_| CheckpointError::ProgramMismatch)?;
    let admitted = super::super::admitted_resolved_functions(program);
    super::super::scan_closure(function_id, &admitted, program)
        .map_err(|_| CheckpointError::ProgramMismatch)?;
    hir::validate(program).map_err(|_| CheckpointError::ProgramMismatch)?;
    let plan =
        lowering::lower_sequential(program, entry).map_err(|_| CheckpointError::ProgramMismatch)?;
    if plan.suspensions.len() <= 1 {
        return Err(CheckpointError::ProgramMismatch);
    }
    let scalars = resumable_scalars(arguments).ok_or(CheckpointError::ProgramMismatch)?;
    Ok((plan, scalars))
}

pub(crate) fn scalar_json(value: &ArgumentValue) -> Value {
    match value {
        ArgumentValue::Int(value) => json!({"tag": "i64", "value": value}),
        ArgumentValue::Int32(value) => json!({"tag": "i32", "value": value}),
        ArgumentValue::Uint8(value) => json!({"tag": "u8", "value": value}),
        ArgumentValue::Usize(value) => json!({"tag": "usize", "value": value}),
        ArgumentValue::Char(value) => json!({"tag": "char", "value": *value as u32}),
        ArgumentValue::Float32(value) => {
            json!({"tag": "f32", "bits": format!("{:08x}", value.to_bits())})
        }
        ArgumentValue::Float64(value) => {
            json!({"tag": "f64", "bits": format!("{:016x}", value.to_bits())})
        }
        ArgumentValue::Bool(value) => json!({"tag": "bool", "value": value}),
        ArgumentValue::BorrowedStr(_) | ArgumentValue::BorrowedSlice(_) => {
            unreachable!("only admitted resumable scalars are checkpointed")
        }
    }
}

pub(crate) fn scalar_from_json(value: &Value) -> Result<ArgumentValue, CheckpointError> {
    let tag = required_str(value, "tag")?;
    match tag {
        "i64" => {
            keys(value, &["tag", "value"])?;
            value["value"]
                .as_i64()
                .map(ArgumentValue::Int)
                .ok_or_else(|| CheckpointError::Malformed("i64.value".to_owned()))
        }
        "i32" => {
            keys(value, &["tag", "value"])?;
            value["value"]
                .as_i64()
                .and_then(|value| i32::try_from(value).ok())
                .map(ArgumentValue::Int32)
                .ok_or_else(|| CheckpointError::Malformed("i32.value".to_owned()))
        }
        "u8" => {
            keys(value, &["tag", "value"])?;
            value["value"]
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .map(ArgumentValue::Uint8)
                .ok_or_else(|| CheckpointError::Malformed("u8.value".to_owned()))
        }
        "usize" => {
            keys(value, &["tag", "value"])?;
            value["value"]
                .as_u64()
                .map(ArgumentValue::Usize)
                .ok_or_else(|| CheckpointError::Malformed("usize.value".to_owned()))
        }
        "char" => {
            keys(value, &["tag", "value"])?;
            value["value"]
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| char::from_u32(*value).is_some())
                .map(ArgumentValue::Char)
                .ok_or_else(|| CheckpointError::Malformed("char.value".to_owned()))
        }
        "f32" => {
            keys(value, &["tag", "bits"])?;
            let bits = parse_bits(required_str(value, "bits")?, 8)?;
            Ok(ArgumentValue::Float32(f32::from_bits(bits as u32)))
        }
        "f64" => {
            keys(value, &["tag", "bits"])?;
            let bits = parse_bits(required_str(value, "bits")?, 16)?;
            Ok(ArgumentValue::Float64(f64::from_bits(bits)))
        }
        "bool" => {
            keys(value, &["tag", "value"])?;
            value["value"]
                .as_bool()
                .map(ArgumentValue::Bool)
                .ok_or_else(|| CheckpointError::Malformed("bool.value".to_owned()))
        }
        _ => Err(CheckpointError::Malformed("scalar.tag".to_owned())),
    }
}

fn parse_bits(text: &str, width: usize) -> Result<u64, CheckpointError> {
    if text.len() != width || !text.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(CheckpointError::Malformed("scalar.bits".to_owned()));
    }
    u64::from_str_radix(text, 16).map_err(|_| CheckpointError::Malformed("scalar.bits".to_owned()))
}

fn payload_json(
    schema: &str,
    function: &str,
    state: &str,
    binding: String,
    request: Value,
    history: Value,
) -> Value {
    json!({
        "schema": schema,
        "function": function,
        "state": state,
        "binding": binding,
        "request": request,
        "history": history,
    })
}

fn binding_hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn binding_from_hex(text: &str) -> Result<[u8; 32], CheckpointError> {
    if text.len() != 64 || !text.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(CheckpointError::Malformed("binding".to_owned()));
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| CheckpointError::Malformed("binding".to_owned()))?;
    }
    Ok(bytes)
}

fn checkpoint_digest(payload: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.source-resumable-sequential-checkpoint-digest.v1\0");
    hasher.update(payload.to_string().as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, CheckpointError> {
    value[field]
        .as_str()
        .ok_or_else(|| CheckpointError::Malformed(field.to_owned()))
}

fn keys(value: &Value, expected: &[&str]) -> Result<(), CheckpointError> {
    let map = value
        .as_object()
        .ok_or_else(|| CheckpointError::Malformed("object".to_owned()))?;
    if map.len() != expected.len() || !expected.iter().all(|key| map.contains_key(*key)) {
        return Err(CheckpointError::Malformed(
            "unexpected or missing object keys".to_owned(),
        ));
    }
    Ok(())
}
