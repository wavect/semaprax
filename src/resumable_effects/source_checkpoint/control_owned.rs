//! Version 4: the authenticated envelope for a control-dependent continuation
//! that carries owned `Bytes` locals across its suspension (issue #296, spec
//! section 11.6). It is a separate wire from v1/v2/v3: those decoders refuse
//! v4 bytes by schema and this decoder refuses theirs; v2/v3 bytes are never
//! reinterpreted as v4 and v4 bytes are never reinterpreted as v2/v3. Besides
//! v3's facts it carries each carried owned value's exact bytes, in the
//! plan's own cleanup-inventory order for the awaited site; the binding is
//! re-derived at decode over the exact carried bytes (as well as the site,
//! arguments, and settled history), so a tampered carried value is refused
//! rather than silently adopted, and the bytes cannot choose a branch or loop
//! count.

use super::{
    keys, required_str, scope_json, validate_scope, validate_scope_field, ArgumentValue,
    ResolvedProgram, SourceCheckpointError, SourceCheckpointKey, SourceCheckpointScope,
    MAX_CHECKPOINT_BYTES,
};
use crate::interpreter::resumable::checkpoint::{scalar_from_json, scalar_json};
use crate::interpreter::resumable::control::{rebuild_control_continuation, ControlContinuation};
use crate::resumable_effects::lowering::control::MAX_CONTROL_SUSPENSIONS;
use crate::resumable_effects::source_signature::{
    derive_source_effect_signature, SourceEffectSignature,
};
use serde_json::{json, Value};

/// Schema of the owned-Bytes-carrying control-dependent continuation
/// envelope.
pub const SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V4: &str = "semaprax.source-resumable-checkpoint.v4";
const AUTHENTICATION_DOMAIN_V4: &[u8] = b"semaprax.source-resumable-checkpoint-authentication.v4\0";

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn unhex(text: &str) -> Result<Vec<u8>, SourceCheckpointError> {
    if text.len() % 2 != 0 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(SourceCheckpointError::Malformed);
    }
    (0..text.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&text[index..index + 2], 16)
                .map_err(|_| SourceCheckpointError::Malformed)
        })
        .collect()
}

fn hex32(bytes: &[u8; 32]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(bytes))
}

fn signature_json(signature: &SourceEffectSignature) -> Value {
    json!({
        "plan": "control-owned",
        "request_shape": signature.request_shape(),
        "answer_shape": signature.answer_shape(),
        "plan_identity": format!("sha256:{}", hex32(signature.plan_identity())),
        "yield_count": signature.yield_count(),
    })
}

fn control_owned_signature(
    program: &ResolvedProgram,
    function_id: &str,
) -> Result<SourceEffectSignature, SourceCheckpointError> {
    let signature = derive_source_effect_signature(program, function_id)
        .map_err(|_| SourceCheckpointError::ProgramMismatch)?;
    if !signature.is_control_dependent() || !signature.carries_owned_bytes() {
        return Err(SourceCheckpointError::ProgramMismatch);
    }
    Ok(signature)
}

fn continuation_json(continuation: &ControlContinuation) -> Value {
    json!({
        "state": continuation.state().as_str(),
        "binding": hex32(continuation.binding().as_bytes()),
        "request": scalar_json(continuation.request()),
        "history": continuation.history().iter().map(|record| json!({
            "site": record.site().as_str(),
            "request": scalar_json(record.request()),
            "answer": scalar_json(record.answer()),
        })).collect::<Vec<_>>(),
        "carried": continuation.carried().iter().map(|(binding, bytes)| json!({
            "binding": binding.as_str(),
            "bytes": hex(bytes),
        })).collect::<Vec<_>>(),
    })
}

fn payload(
    scope: &SourceCheckpointScope,
    function_id: &str,
    signature: Value,
    continuation: Value,
) -> Value {
    json!({
        "schema": SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V4,
        "scope": scope_json(scope),
        "function": function_id,
        "signature": signature,
        "continuation": continuation,
    })
}

fn render(key: &SourceCheckpointKey, mut payload: Value) -> Result<Vec<u8>, SourceCheckpointError> {
    let tag = key.authenticate(AUTHENTICATION_DOMAIN_V4, payload.to_string().as_bytes());
    payload["authentication"] = Value::String(format!("hmac-sha256:{}", hex(&tag)));
    let bytes = format!("{payload}\n").into_bytes();
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(SourceCheckpointError::TooLarge);
    }
    Ok(bytes)
}

/// Authenticate an owned-Bytes-carrying control continuation under the
/// caller's exact scope. The continuation is first rebuilt from the current
/// program and arguments -- carried bytes included -- so a continuation of
/// other source, arguments, or carried values cannot be signed as current.
pub fn encode_source_checkpoint_v4(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    continuation: &ControlContinuation,
) -> Result<Vec<u8>, SourceCheckpointError> {
    validate_scope(scope)?;
    validate_scope_field(function_id)?;
    let signature = control_owned_signature(program, function_id)?;
    let encoded = continuation_json(continuation);
    decode_continuation(program, function_id, arguments, &encoded)?;
    render(
        key,
        payload(scope, function_id, signature_json(&signature), encoded),
    )
}

/// Recover an owned-Bytes-carrying control continuation from untrusted
/// bytes under independently supplied facts. The result is inert until
/// explicitly resumed.
pub fn decode_source_checkpoint_v4(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    expected_scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    bytes: &[u8],
) -> Result<ControlContinuation, SourceCheckpointError> {
    validate_scope(expected_scope)?;
    validate_scope_field(function_id)?;
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(SourceCheckpointError::TooLarge);
    }
    let document: Value =
        serde_json::from_slice(bytes).map_err(|_| SourceCheckpointError::Malformed)?;
    if required_str(&document, "schema")? != SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V4 {
        return Err(SourceCheckpointError::SchemaMismatch);
    }
    keys(
        &document,
        &[
            "schema",
            "scope",
            "function",
            "signature",
            "continuation",
            "authentication",
        ],
    )?;
    let encoded_scope = &document["scope"];
    keys(
        encoded_scope,
        &["program_root", "invocation_id", "policy_epoch"],
    )?;
    let observed_scope = SourceCheckpointScope::new(
        required_str(encoded_scope, "program_root")?,
        required_str(encoded_scope, "invocation_id")?,
        encoded_scope["policy_epoch"]
            .as_u64()
            .ok_or(SourceCheckpointError::Malformed)?,
    )?;
    let encoded_function = required_str(&document, "function")?;
    validate_scope_field(encoded_function)?;
    let unsigned = payload(
        &observed_scope,
        encoded_function,
        document["signature"].clone(),
        document["continuation"].clone(),
    );
    let tag = required_str(&document, "authentication")?
        .strip_prefix("hmac-sha256:")
        .filter(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(SourceCheckpointError::AuthenticationMismatch)?;
    let tag = unhex(tag).map_err(|_| SourceCheckpointError::AuthenticationMismatch)?;
    if !key.verify(
        AUTHENTICATION_DOMAIN_V4,
        unsigned.to_string().as_bytes(),
        &tag,
    ) {
        return Err(SourceCheckpointError::AuthenticationMismatch);
    }
    if render(key, unsigned)?.as_slice() != bytes {
        return Err(SourceCheckpointError::NonCanonical);
    }
    if observed_scope != *expected_scope {
        return Err(SourceCheckpointError::ScopeMismatch);
    }
    if encoded_function != function_id {
        return Err(SourceCheckpointError::FunctionMismatch);
    }
    let expected_signature = control_owned_signature(program, function_id)?;
    if document["signature"] != signature_json(&expected_signature) {
        return Err(SourceCheckpointError::ProgramMismatch);
    }
    decode_continuation(program, function_id, arguments, &document["continuation"])
}

fn scalar(value: &Value) -> Result<ArgumentValue, SourceCheckpointError> {
    scalar_from_json(value).map_err(|_| SourceCheckpointError::Malformed)
}

fn decode_continuation(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    value: &Value,
) -> Result<ControlContinuation, SourceCheckpointError> {
    keys(
        value,
        &["state", "binding", "request", "history", "carried"],
    )?;
    let binding_text = required_str(value, "binding")?;
    if binding_text.len() != 64 || !binding_text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(SourceCheckpointError::Malformed);
    }
    let mut binding = [0_u8; 32];
    for (index, byte) in binding.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&binding_text[index * 2..index * 2 + 2], 16)
            .map_err(|_| SourceCheckpointError::Malformed)?;
    }
    let history = value["history"]
        .as_array()
        .ok_or(SourceCheckpointError::Malformed)?;
    if history.len() >= MAX_CONTROL_SUSPENSIONS {
        return Err(SourceCheckpointError::SuspensionMismatch);
    }
    let mut records = Vec::with_capacity(history.len());
    for record in history {
        keys(record, &["site", "request", "answer"])?;
        records.push((
            required_str(record, "site")?.to_owned(),
            scalar(&record["request"])?,
            scalar(&record["answer"])?,
        ));
    }
    let carried_json = value["carried"]
        .as_array()
        .ok_or(SourceCheckpointError::Malformed)?;
    let mut carried = Vec::with_capacity(carried_json.len());
    for entry in carried_json {
        keys(entry, &["binding", "bytes"])?;
        let bytes = unhex(required_str(entry, "bytes")?)?;
        carried.push((required_str(entry, "binding")?.to_owned(), bytes));
    }
    let continuation = rebuild_control_continuation(
        program,
        function_id,
        arguments,
        required_str(value, "state")?,
        binding,
        scalar(&value["request"])?,
        records,
        carried,
    )
    .map_err(|_| SourceCheckpointError::SuspensionMismatch)?;
    if continuation_json(&continuation) != *value {
        return Err(SourceCheckpointError::NonCanonical);
    }
    Ok(continuation)
}

#[cfg(test)]
mod tests;
