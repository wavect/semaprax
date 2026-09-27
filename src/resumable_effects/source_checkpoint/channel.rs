//! Issue #296 R20: version 5 binds the compiler-derived effect signature into
//! the authenticated envelope for a bounded record/variant `yields`
//! request/response channel, exactly like version 2 does for the
//! Copy-scalar sequential lane. It is a distinct schema and type, never a
//! variant of v1/v2: those two wrap `interpreter::resumable::checkpoint`'s
//! scalar-only inner codec, which has no representation for an aggregate
//! channel value, so every existing v1/v2 (and v3/v4, the control-dependent
//! lane's own separate envelopes) program stays byte-identical -- this
//! module changes none of their code.

use super::{
    keys, map_inner_error, required_str, scope_json, validate_scope, validate_scope_field,
    ArgumentValue, Hmac, KeyInit, Mac, ResolvedProgram, Sha256, SourceCheckpointError,
    SourceCheckpointKey, SourceCheckpointScope, MAX_CHECKPOINT_BYTES,
};
use crate::interpreter::resumable::checkpoint::{self, channel_json};
use crate::interpreter::resumable::ResumableChannelContinuation;
use crate::resumable_effects::source_signature::{
    derive_source_effect_signature, SourceEffectSignature,
};
use serde_json::{json, Value};

/// Explicit opt-in schema binding checked source effect shapes and lowering,
/// for the bounded record/variant channel only.
pub const SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V5: &str = "semaprax.source-resumable-checkpoint.v5";
const AUTHENTICATION_DOMAIN_V5: &[u8] = b"semaprax.source-resumable-checkpoint-authentication.v5\0";

/// Authenticate a structurally valid bounded-aggregate-channel continuation
/// and its compiler-derived signature under the caller's exact scope. No
/// source is executed; a signed checkpoint remains inert proof data.
pub fn encode_source_checkpoint_v5(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    continuation: &ResumableChannelContinuation,
) -> Result<Vec<u8>, SourceCheckpointError> {
    validate_scope(scope)?;
    validate_scope_field(function_id)?;
    let signature = derive_signature(program, function_id)?;
    if !signature.is_aggregate_channel() {
        // A scalar-channel function has no representation gap this envelope
        // exists to close; callers select v2 for it instead.
        return Err(SourceCheckpointError::ProgramMismatch);
    }
    let inner = checkpoint::encode_channel(function_id, continuation).map_err(map_inner_error)?;
    checkpoint::decode_channel(program, function_id, arguments, &inner).map_err(map_inner_error)?;
    let continuation: Value =
        serde_json::from_slice(&inner).map_err(|_| SourceCheckpointError::Malformed)?;
    render(
        key,
        payload(scope, function_id, signature_json(&signature), continuation),
    )
}

/// Recover only when authenticated scope, selected function, checked effect
/// signature, lowering plan, and the structural continuation all match the
/// independently supplied current program and arguments.
pub fn decode_source_checkpoint_v5(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    expected_scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    bytes: &[u8],
) -> Result<ResumableChannelContinuation, SourceCheckpointError> {
    validate_scope(expected_scope)?;
    validate_scope_field(function_id)?;
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(SourceCheckpointError::TooLarge);
    }
    let document: Value =
        serde_json::from_slice(bytes).map_err(|_| SourceCheckpointError::Malformed)?;
    if required_str(&document, "schema")? != SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V5 {
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
    keys(
        &document["signature"],
        &[
            "request_shape",
            "answer_shape",
            "plan_identity",
            "yield_count",
        ],
    )?;
    let unsigned = payload(
        &observed_scope,
        encoded_function,
        document["signature"].clone(),
        document["continuation"].clone(),
    );
    verify_authentication(key, &unsigned, required_str(&document, "authentication")?)?;
    if render(key, unsigned)?.as_slice() != bytes {
        return Err(SourceCheckpointError::NonCanonical);
    }
    if observed_scope != *expected_scope {
        return Err(SourceCheckpointError::ScopeMismatch);
    }
    if encoded_function != function_id {
        return Err(SourceCheckpointError::FunctionMismatch);
    }
    let expected_signature = derive_signature(program, function_id)?;
    if !expected_signature.is_aggregate_channel() {
        return Err(SourceCheckpointError::ProgramMismatch);
    }
    if document["signature"] != signature_json(&expected_signature) {
        return Err(SourceCheckpointError::ProgramMismatch);
    }
    let inner = format!("{}\n", document["continuation"]).into_bytes();
    checkpoint::decode_channel(program, function_id, arguments, &inner).map_err(map_inner_error)
}

fn derive_signature(
    program: &ResolvedProgram,
    function_id: &str,
) -> Result<SourceEffectSignature, SourceCheckpointError> {
    derive_source_effect_signature(program, function_id)
        .map_err(|_| SourceCheckpointError::ProgramMismatch)
}

fn signature_json(signature: &SourceEffectSignature) -> Value {
    json!({
        "request_shape": signature.request_shape(),
        "answer_shape": signature.answer_shape(),
        "plan_identity": format!("sha256:{:x}", crate::digest_hex::LowerHex(signature.plan_identity())),
        "yield_count": signature.yield_count(),
    })
}

fn payload(
    scope: &SourceCheckpointScope,
    function_id: &str,
    signature: Value,
    continuation: Value,
) -> Value {
    json!({
        "schema": SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V5,
        "scope": scope_json(scope),
        "function": function_id,
        "signature": signature,
        "continuation": continuation,
    })
}

fn mac(key: &SourceCheckpointKey, payload: &Value) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(&key.0).expect("HMAC accepts a 32-byte key");
    mac.update(AUTHENTICATION_DOMAIN_V5);
    mac.update(payload.to_string().as_bytes());
    mac
}

fn render(key: &SourceCheckpointKey, mut payload: Value) -> Result<Vec<u8>, SourceCheckpointError> {
    let authentication = format!(
        "hmac-sha256:{:x}",
        crate::digest_hex::LowerHex(mac(key, &payload).finalize().into_bytes())
    );
    payload["authentication"] = Value::String(authentication);
    let bytes = format!("{payload}\n").into_bytes();
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(SourceCheckpointError::TooLarge);
    }
    Ok(bytes)
}

fn verify_authentication(
    key: &SourceCheckpointKey,
    payload: &Value,
    claimed: &str,
) -> Result<(), SourceCheckpointError> {
    let hex = claimed
        .strip_prefix("hmac-sha256:")
        .ok_or(SourceCheckpointError::AuthenticationMismatch)?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(SourceCheckpointError::AuthenticationMismatch);
    }
    let mut tag = [0; 32];
    for (index, byte) in tag.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| SourceCheckpointError::AuthenticationMismatch)?;
    }
    mac(key, payload)
        .verify_slice(&tag)
        .map_err(|_| SourceCheckpointError::AuthenticationMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::DeclarationId;
    use crate::interpreter::resumable::channel::{
        resume_sequential_channel_resumable_effect, run_sequential_channel_resumable_effect,
        SequentialChannelResumableStep,
    };
    use crate::interpreter::resumable::ResumableChannelValue;
    use std::path::Path;

    const SOURCE: &str = r#"
module test.channel_checkpoint;
@id("app.prompt")
record Prompt {
    @id("app.prompt.seed") seed: i64,
    @id("app.prompt.urgent") urgent: bool,
}
@id("app.answer")
record Answer {
    @id("app.answer.value") value: i64,
    @id("app.answer.ok") ok: bool,
}
@id("app.ask")
fn ask(seed: i64) -> i64
    yields Prompt -> Answer
{
    let first = yield Prompt { seed: seed, urgent: false };
    let second = yield Prompt { seed: first.value, urgent: true };
    first.value + second.value
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program(source: &str) -> ResolvedProgram {
        let ast = crate::parse(source, Path::new("channel-checkpoint.spx")).unwrap();
        crate::hir::resolve(&ast).unwrap()
    }

    fn scope() -> SourceCheckpointScope {
        SourceCheckpointScope::new("sha256:program", "invocation-7", 11).unwrap()
    }

    fn key() -> SourceCheckpointKey {
        SourceCheckpointKey::new([0x5a; 32])
    }

    fn first(program: &ResolvedProgram) -> ResumableChannelContinuation {
        let evaluation = run_sequential_channel_resumable_effect(
            program,
            "app.ask",
            &[ArgumentValue::Int(4)],
            10_000,
        )
        .unwrap();
        let SequentialChannelResumableStep::Suspended { continuation } = evaluation.step else {
            panic!("fixture did not suspend")
        };
        continuation
    }

    fn answer(value: i64, ok: bool) -> ResumableChannelValue {
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("app.answer"),
            fields: vec![ArgumentValue::Int(value), ArgumentValue::Bool(ok)],
        }
    }

    fn encode(program: &ResolvedProgram, continuation: &ResumableChannelContinuation) -> Vec<u8> {
        encode_source_checkpoint_v5(
            program,
            &key(),
            &scope(),
            "app.ask",
            &[ArgumentValue::Int(4)],
            continuation,
        )
        .unwrap()
    }

    fn decode(
        program: &ResolvedProgram,
        bytes: &[u8],
    ) -> Result<ResumableChannelContinuation, SourceCheckpointError> {
        decode_source_checkpoint_v5(
            program,
            &key(),
            &scope(),
            "app.ask",
            &[ArgumentValue::Int(4)],
            bytes,
        )
    }

    #[test]
    fn v5_round_trips_and_recovers_each_aggregate_site() {
        let program = program(SOURCE);
        let continuation = first(&program);
        let bytes = encode(&program, &continuation);
        assert_eq!(bytes, encode(&program, &continuation));
        let document: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(document["schema"], SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V5);
        let recovered = decode(&program, &bytes).unwrap();
        assert_eq!(recovered, continuation);
        let resumed = resume_sequential_channel_resumable_effect(
            &program,
            "app.ask",
            &[ArgumentValue::Int(4)],
            &recovered,
            &answer(10, true),
            10_000,
        )
        .unwrap();
        let SequentialChannelResumableStep::Suspended { continuation } = resumed.step else {
            panic!("second site was not reached")
        };
        let recovered = decode(&program, &encode(&program, &continuation)).unwrap();
        assert_eq!(
            recovered.request(),
            &ResumableChannelValue::Record {
                declaration: DeclarationId::new("app.prompt"),
                fields: vec![ArgumentValue::Int(10), ArgumentValue::Bool(true)],
            }
        );
    }

    #[test]
    fn v5_rejects_corruption_and_downgrade_and_a_scalar_channel_function() {
        let checked = program(SOURCE);
        let continuation = first(&checked);
        let bytes = encode(&checked, &continuation);
        assert_eq!(
            decode(&checked, &[b" ".as_slice(), bytes.as_slice()].concat()),
            Err(SourceCheckpointError::NonCanonical)
        );
        assert_eq!(
            decode(&checked, &vec![b' '; MAX_CHECKPOINT_BYTES + 1]),
            Err(SourceCheckpointError::TooLarge)
        );

        let scalar_source = r#"
module test.channel_checkpoint_scalar;
@id("app.ask")
fn ask(seed: i64) -> i64 yields i64 -> i64 {
    let first = yield seed;
    yield first
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let scalar_program = program(scalar_source);
        assert_eq!(
            encode_source_checkpoint_v5(
                &scalar_program,
                &key(),
                &scope(),
                "app.ask",
                &[ArgumentValue::Int(4)],
                &continuation,
            ),
            Err(SourceCheckpointError::ProgramMismatch)
        );
    }

    /// Negative control (issue #296 R20): dropping one field from the
    /// canonical v5 encoding must fail the round trip -- even when the
    /// corrupted bytes are freshly, validly re-signed, so this isolates
    /// `decode_channel`'s own structural field-count check rather than
    /// merely the outer HMAC catching an arbitrary byte change.
    #[test]
    fn dropping_one_encoded_field_fails_the_round_trip() {
        let program = program(SOURCE);
        let continuation = first(&program);
        let bytes = encode(&program, &continuation);
        let document: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(document["continuation"]["request"]["tag"], "record");
        let mut corrupted = document["continuation"].clone();
        // Drop `Prompt`'s first encoded field (`seed`) from the request's
        // `"fields"` array: the carrier now claims to be a `Prompt` with
        // only one field where the checked declaration requires two.
        corrupted["request"]["fields"]
            .as_array_mut()
            .unwrap()
            .remove(0);
        let forged = render(
            &key(),
            payload(
                &scope(),
                "app.ask",
                document["signature"].clone(),
                corrupted,
            ),
        )
        .unwrap();
        // A freshly, validly signed corrupted document still fails: the
        // outer HMAC alone is not what is catching this.
        assert!(decode(&program, &forged).is_err());
    }
}
