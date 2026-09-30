//! Version 3: the authenticated envelope for control-dependent continuations
//! (issue #296). It is a separate wire: v1 and v2 bytes are never
//! reinterpreted as v3 and v3 bytes are refused by the v1/v2 decoders by
//! schema. Besides the v2 facts it records each settled suspension's static
//! site; the binding over sites and answers is re-derived at decode, so the
//! bytes cannot choose a branch or loop count.

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

/// Schema of the control-dependent continuation envelope.
pub const SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V3: &str = "semaprax.source-resumable-checkpoint.v3";
const AUTHENTICATION_DOMAIN_V3: &[u8] = b"semaprax.source-resumable-checkpoint-authentication.v3\0";

fn hex(bytes: &[u8; 32]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(bytes))
}

fn signature_json(signature: &SourceEffectSignature) -> Value {
    json!({
        "plan": "control",
        "request_shape": signature.request_shape(),
        "answer_shape": signature.answer_shape(),
        "plan_identity": format!("sha256:{}", hex(signature.plan_identity())),
        "yield_count": signature.yield_count(),
    })
}

fn control_signature(
    program: &ResolvedProgram,
    function_id: &str,
) -> Result<SourceEffectSignature, SourceCheckpointError> {
    let signature = derive_source_effect_signature(program, function_id)
        .map_err(|_| SourceCheckpointError::ProgramMismatch)?;
    if !signature.is_control_dependent() {
        return Err(SourceCheckpointError::ProgramMismatch);
    }
    Ok(signature)
}

fn continuation_json(continuation: &ControlContinuation) -> Value {
    json!({
        "state": continuation.state().as_str(),
        "binding": hex(continuation.binding().as_bytes()),
        "request": scalar_json(continuation.request()),
        "history": continuation.history().iter().map(|record| json!({
            "site": record.site().as_str(),
            "request": scalar_json(record.request()),
            "answer": scalar_json(record.answer()),
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
        "schema": SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V3,
        "scope": scope_json(scope),
        "function": function_id,
        "signature": signature,
        "continuation": continuation,
    })
}

fn render(key: &SourceCheckpointKey, mut payload: Value) -> Result<Vec<u8>, SourceCheckpointError> {
    let tag = key.authenticate(AUTHENTICATION_DOMAIN_V3, payload.to_string().as_bytes());
    payload["authentication"] = Value::String(format!("hmac-sha256:{}", hex(&tag)));
    let bytes = format!("{payload}\n").into_bytes();
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(SourceCheckpointError::TooLarge);
    }
    Ok(bytes)
}

/// Authenticate a control continuation under the caller's exact scope. The
/// continuation is first rebuilt from the current program and arguments, so
/// a continuation of other source or arguments cannot be signed as current.
pub fn encode_source_checkpoint_v3(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    continuation: &ControlContinuation,
) -> Result<Vec<u8>, SourceCheckpointError> {
    validate_scope(scope)?;
    validate_scope_field(function_id)?;
    let signature = control_signature(program, function_id)?;
    let encoded = continuation_json(continuation);
    decode_continuation(program, function_id, arguments, &encoded)?;
    render(
        key,
        payload(scope, function_id, signature_json(&signature), encoded),
    )
}

/// Recover a control continuation from untrusted bytes under independently
/// supplied facts. The result is inert until explicitly resumed.
pub fn decode_source_checkpoint_v3(
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
    if required_str(&document, "schema")? != SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V3 {
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
    let tag = (0..32)
        .map(|index| u8::from_str_radix(&tag[index * 2..index * 2 + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| SourceCheckpointError::AuthenticationMismatch)?;
    if !key.verify(
        AUTHENTICATION_DOMAIN_V3,
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
    let expected_signature = control_signature(program, function_id)?;
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
    keys(value, &["state", "binding", "request", "history"])?;
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
    let continuation = rebuild_control_continuation(
        program,
        function_id,
        arguments,
        required_str(value, "state")?,
        binding,
        scalar(&value["request"])?,
        records,
        Vec::new(),
    )
    .map_err(|_| SourceCheckpointError::SuspensionMismatch)?;
    if continuation_json(&continuation) != *value {
        return Err(SourceCheckpointError::NonCanonical);
    }
    Ok(continuation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::resumable::control::{
        run_control_resumable_effect, ControlResumableStep,
    };
    use crate::resumable_effects::source_checkpoint::{
        decode_source_checkpoint_v2, SourceCheckpointKey,
    };

    const SOURCE: &str = r#"
module test.control_checkpoint;
@id("app.ask")
fn ask(limit: i64) -> i64
    yields i64 -> i64
{
    let mut total = 0;
    let mut round = 0;
    while round < limit {
        let answer = yield round;
        total = total + answer;
        round = round + 1;
        round > 0
    }
    total
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program() -> ResolvedProgram {
        crate::hir::resolve(&crate::parse(SOURCE, "control-checkpoint.spx").unwrap()).unwrap()
    }

    fn scope() -> SourceCheckpointScope {
        SourceCheckpointScope::new("root", "invocation-3", 1).unwrap()
    }

    fn first(program: &ResolvedProgram) -> ControlContinuation {
        let ControlResumableStep::Suspended { continuation } =
            run_control_resumable_effect(program, "app.ask", &[ArgumentValue::Int(2)], 100_000)
                .unwrap()
                .step
        else {
            panic!("suspends")
        };
        continuation
    }

    #[test]
    fn v3_round_trips_and_no_other_version_reads_it() {
        let program = program();
        let key = SourceCheckpointKey::new([7; 32]);
        let arguments = [ArgumentValue::Int(2)];
        let continuation = first(&program);
        let bytes = encode_source_checkpoint_v3(
            &program,
            &key,
            &scope(),
            "app.ask",
            &arguments,
            &continuation,
        )
        .unwrap();
        assert_eq!(
            decode_source_checkpoint_v3(&program, &key, &scope(), "app.ask", &arguments, &bytes)
                .unwrap(),
            continuation
        );
        assert_eq!(
            decode_source_checkpoint_v2(&program, &key, &scope(), "app.ask", &arguments, &bytes),
            Err(SourceCheckpointError::SchemaMismatch)
        );
        let other_scope = SourceCheckpointScope::new("root", "invocation-4", 1).unwrap();
        assert_eq!(
            decode_source_checkpoint_v3(
                &program,
                &key,
                &other_scope,
                "app.ask",
                &arguments,
                &bytes
            ),
            Err(SourceCheckpointError::ScopeMismatch)
        );
        let tampered =
            String::from_utf8(bytes.clone())
                .unwrap()
                .replacen("invocation-3", "invocation-4", 1);
        assert_eq!(
            decode_source_checkpoint_v3(
                &program,
                &key,
                &other_scope,
                "app.ask",
                &arguments,
                tampered.as_bytes()
            ),
            Err(SourceCheckpointError::AuthenticationMismatch)
        );
        // Other arguments cannot adopt the continuation.
        assert_eq!(
            decode_source_checkpoint_v3(
                &program,
                &key,
                &scope(),
                "app.ask",
                &[ArgumentValue::Int(3)],
                &bytes
            ),
            Err(SourceCheckpointError::SuspensionMismatch)
        );
    }
}
