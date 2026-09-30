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
pub const SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V6: &str = "semaprax.source-resumable-checkpoint.v6";
const AUTHENTICATION_DOMAIN_V6: &[u8] = b"semaprax.source-resumable-checkpoint-authentication.v6\0";
// One v6 suspension can carry eight bounded 1 KiB Bytes leaves. Their JSON
// decimal representation needs more than the scalar/v5 envelope's 32 KiB cap.
const MAX_V6_CHECKPOINT_BYTES: usize = 64 * 1024;

pub const SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V7: &str = "semaprax.source-resumable-checkpoint.v7";
const AUTHENTICATION_DOMAIN_V7: &[u8] = b"semaprax.source-resumable-checkpoint-authentication.v7\0";

/// Canonical invocation binding. Scalar channel values retain exactly the
/// existing scalar JSON plus LF framing; nominal identities remain included.
pub(crate) fn channel_arguments_digest(
    arguments: &[crate::interpreter::resumable::ResumableChannelValue],
) -> String {
    use sha2::Digest;
    let mut digest = Sha256::new();
    for argument in arguments {
        digest.update(channel_json(argument).to_string().as_bytes());
        digest.update(b"\n");
    }
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn arguments_signature(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[crate::interpreter::resumable::ResumableChannelValue],
) -> Result<Value, SourceCheckpointError> {
    let (plan, _) = checkpoint::checked_channel_arguments_plan(program, function_id, arguments)
        .map_err(map_inner_error)?;
    let yields = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == function_id)
        .and_then(|f| f.yields.as_ref())
        .ok_or(SourceCheckpointError::ProgramMismatch)?;
    let prefix = crate::resumable_effects::source_signature::SOURCE_TYPE_SHAPE_PREFIX;
    Ok(json!({
        "request_shape": format!("{prefix}{}", yields.request_type.identity_key()),
        "answer_shape": format!("{prefix}{}", yields.response_type.identity_key()),
        "plan_identity": format!("sha256:{:x}", crate::digest_hex::LowerHex(plan.identity.as_bytes())),
        "yield_count": plan.suspensions.len(),
    }))
}

fn arguments_payload(
    scope: &SourceCheckpointScope,
    function_id: &str,
    signature: Value,
    arguments: Value,
    arguments_digest: Value,
    continuation: Value,
) -> Value {
    let mut value = payload(
        SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V7,
        scope,
        function_id,
        signature,
        continuation,
    );
    value["arguments"] = arguments;
    value["arguments_digest"] = arguments_digest;
    value
}

/// Authenticate only a suspended Copy-aggregate invocation. This never runs
/// source, dispatches host work, or grants answer or storage authority.
pub fn encode_source_checkpoint_v7(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[crate::interpreter::resumable::ResumableChannelValue],
    continuation: &ResumableChannelContinuation,
) -> Result<Vec<u8>, SourceCheckpointError> {
    validate_scope(scope)?;
    validate_scope_field(function_id)?;
    let signature = arguments_signature(program, function_id, arguments)?;
    let inner =
        checkpoint::encode_channel_arguments(function_id, continuation).map_err(map_inner_error)?;
    checkpoint::decode_channel_arguments(program, function_id, arguments, &inner)
        .map_err(map_inner_error)?;
    let continuation =
        serde_json::from_slice(&inner).map_err(|_| SourceCheckpointError::Malformed)?;
    render(
        key,
        arguments_payload(
            scope,
            function_id,
            signature,
            Value::Array(arguments.iter().map(channel_json).collect()),
            Value::String(channel_arguments_digest(arguments)),
            continuation,
        ),
        AUTHENTICATION_DOMAIN_V7,
    )
}

/// Decode inert proof data under independently supplied checked invocation
/// facts. V7 is selected from the checked boundary before interpreting bytes.
pub fn decode_source_checkpoint_v7(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    expected_scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[crate::interpreter::resumable::ResumableChannelValue],
    bytes: &[u8],
) -> Result<ResumableChannelContinuation, SourceCheckpointError> {
    validate_scope(expected_scope)?;
    validate_scope_field(function_id)?;
    let expected_signature = arguments_signature(program, function_id, arguments)?;
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(SourceCheckpointError::TooLarge);
    }
    let document: Value =
        serde_json::from_slice(bytes).map_err(|_| SourceCheckpointError::Malformed)?;
    if required_str(&document, "schema")? != SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V7 {
        return Err(SourceCheckpointError::SchemaMismatch);
    }
    keys(
        &document,
        &[
            "schema",
            "scope",
            "function",
            "signature",
            "arguments",
            "arguments_digest",
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
    let unsigned = arguments_payload(
        &observed_scope,
        encoded_function,
        document["signature"].clone(),
        document["arguments"].clone(),
        document["arguments_digest"].clone(),
        document["continuation"].clone(),
    );
    verify_authentication(
        key,
        &unsigned,
        required_str(&document, "authentication")?,
        AUTHENTICATION_DOMAIN_V7,
    )?;
    if render(key, unsigned, AUTHENTICATION_DOMAIN_V7)?.as_slice() != bytes {
        return Err(SourceCheckpointError::NonCanonical);
    }
    if observed_scope != *expected_scope {
        return Err(SourceCheckpointError::ScopeMismatch);
    }
    if encoded_function != function_id {
        return Err(SourceCheckpointError::FunctionMismatch);
    }
    if document["signature"] != expected_signature {
        return Err(SourceCheckpointError::ProgramMismatch);
    }
    if document["arguments"] != Value::Array(arguments.iter().map(channel_json).collect())
        || document["arguments_digest"] != Value::String(channel_arguments_digest(arguments))
    {
        return Err(SourceCheckpointError::ArgumentsMismatch);
    }
    let inner = format!("{}\n", document["continuation"]);
    checkpoint::decode_channel_arguments(program, function_id, arguments, inner.as_bytes())
        .map_err(map_inner_error)
}

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
    encode_source_checkpoint(
        program,
        key,
        scope,
        function_id,
        arguments,
        continuation,
        SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V5,
        AUTHENTICATION_DOMAIN_V5,
        false,
    )
}

pub fn encode_source_checkpoint_v6(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    continuation: &ResumableChannelContinuation,
) -> Result<Vec<u8>, SourceCheckpointError> {
    encode_source_checkpoint(
        program,
        key,
        scope,
        function_id,
        arguments,
        continuation,
        SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V6,
        AUTHENTICATION_DOMAIN_V6,
        true,
    )
}

fn encode_source_checkpoint(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    continuation: &ResumableChannelContinuation,
    schema: &str,
    domain: &[u8],
    requires_bytes: bool,
) -> Result<Vec<u8>, SourceCheckpointError> {
    validate_scope(scope)?;
    validate_scope_field(function_id)?;
    let signature = derive_signature(program, function_id)?;
    if !signature.is_aggregate_channel() || signature.has_aggregate_bytes() != requires_bytes {
        // A scalar-channel function has no representation gap this envelope
        // exists to close; callers select v2 for it instead.
        return Err(SourceCheckpointError::ProgramMismatch);
    }
    let inner = if requires_bytes {
        checkpoint::encode_channel_bytes(function_id, continuation)
    } else {
        checkpoint::encode_channel(function_id, continuation)
    }
    .map_err(map_inner_error)?;
    if requires_bytes {
        checkpoint::decode_channel_bytes(program, function_id, arguments, &inner)
    } else {
        checkpoint::decode_channel(program, function_id, arguments, &inner)
    }
    .map_err(map_inner_error)?;
    let continuation: Value =
        serde_json::from_slice(&inner).map_err(|_| SourceCheckpointError::Malformed)?;
    render(
        key,
        payload(
            schema,
            scope,
            function_id,
            signature_json(&signature),
            continuation,
        ),
        domain,
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
    decode_source_checkpoint(
        program,
        key,
        expected_scope,
        function_id,
        arguments,
        bytes,
        SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V5,
        AUTHENTICATION_DOMAIN_V5,
        false,
    )
}

pub fn decode_source_checkpoint_v6(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    expected_scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    bytes: &[u8],
) -> Result<ResumableChannelContinuation, SourceCheckpointError> {
    decode_source_checkpoint(
        program,
        key,
        expected_scope,
        function_id,
        arguments,
        bytes,
        SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V6,
        AUTHENTICATION_DOMAIN_V6,
        true,
    )
}

fn decode_source_checkpoint(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    expected_scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    bytes: &[u8],
    schema: &str,
    domain: &[u8],
    requires_bytes: bool,
) -> Result<ResumableChannelContinuation, SourceCheckpointError> {
    validate_scope(expected_scope)?;
    validate_scope_field(function_id)?;
    let limit = if requires_bytes {
        MAX_V6_CHECKPOINT_BYTES
    } else {
        MAX_CHECKPOINT_BYTES
    };
    if bytes.len() > limit {
        return Err(SourceCheckpointError::TooLarge);
    }
    let document: Value =
        serde_json::from_slice(bytes).map_err(|_| SourceCheckpointError::Malformed)?;
    if required_str(&document, "schema")? != schema {
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
        schema,
        &observed_scope,
        encoded_function,
        document["signature"].clone(),
        document["continuation"].clone(),
    );
    verify_authentication(
        key,
        &unsigned,
        required_str(&document, "authentication")?,
        domain,
    )?;
    if render(key, unsigned, domain)?.as_slice() != bytes {
        return Err(SourceCheckpointError::NonCanonical);
    }
    if observed_scope != *expected_scope {
        return Err(SourceCheckpointError::ScopeMismatch);
    }
    if encoded_function != function_id {
        return Err(SourceCheckpointError::FunctionMismatch);
    }
    let expected_signature = derive_signature(program, function_id)?;
    if !expected_signature.is_aggregate_channel()
        || expected_signature.has_aggregate_bytes() != requires_bytes
    {
        return Err(SourceCheckpointError::ProgramMismatch);
    }
    if document["signature"] != signature_json(&expected_signature) {
        return Err(SourceCheckpointError::ProgramMismatch);
    }
    let inner = format!("{}\n", document["continuation"]).into_bytes();
    if requires_bytes {
        checkpoint::decode_channel_bytes(program, function_id, arguments, &inner)
    } else {
        checkpoint::decode_channel(program, function_id, arguments, &inner)
    }
    .map_err(map_inner_error)
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
    schema: &str,
    scope: &SourceCheckpointScope,
    function_id: &str,
    signature: Value,
    continuation: Value,
) -> Value {
    json!({
        "schema": schema,
        "scope": scope_json(scope),
        "function": function_id,
        "signature": signature,
        "continuation": continuation,
    })
}

fn mac(key: &SourceCheckpointKey, payload: &Value, domain: &[u8]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(&key.0).expect("HMAC accepts a 32-byte key");
    mac.update(domain);
    mac.update(payload.to_string().as_bytes());
    mac
}

fn render(
    key: &SourceCheckpointKey,
    mut payload: Value,
    domain: &[u8],
) -> Result<Vec<u8>, SourceCheckpointError> {
    let authentication = format!(
        "hmac-sha256:{:x}",
        crate::digest_hex::LowerHex(mac(key, &payload, domain).finalize().into_bytes())
    );
    payload["authentication"] = Value::String(authentication);
    let bytes = format!("{payload}\n").into_bytes();
    let limit = if domain == AUTHENTICATION_DOMAIN_V6 {
        MAX_V6_CHECKPOINT_BYTES
    } else {
        MAX_CHECKPOINT_BYTES
    };
    if bytes.len() > limit {
        return Err(SourceCheckpointError::TooLarge);
    }
    Ok(bytes)
}

fn verify_authentication(
    key: &SourceCheckpointKey,
    payload: &Value,
    claimed: &str,
    domain: &[u8],
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
    mac(key, payload, domain)
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

    const BYTES_SOURCE: &str = r#"
module test.channel_checkpoint_bytes;
@id("bytes.make") fn make_buf() -> Bytes { let raw = [1u8, 2u8]; bytes_copy(array_as_slice(raw)) }
@id("app.prompt") record Prompt { @id("app.prompt.payload") payload: Bytes, }
@id("app.ask") fn ask(seed: i64) -> i64 yields Prompt -> i64 {
    let answer = yield Prompt { payload: make_buf() };
    answer + seed
}
@id("app.main") fn main() -> i64 { 0 }
"#;

    #[test]
    fn v6_round_trips_a_bytes_request_and_refuses_v5_cross_decode() {
        let program = program(BYTES_SOURCE);
        let continuation = first(&program);
        crate::resumable_effects::source_signature::derive_source_effect_signature(
            &program, "app.ask",
        )
        .unwrap();
        let bytes = encode_source_checkpoint_v6(
            &program,
            &key(),
            &scope(),
            "app.ask",
            &[ArgumentValue::Int(4)],
            &continuation,
        )
        .unwrap();
        let recovered = decode_source_checkpoint_v6(
            &program,
            &key(),
            &scope(),
            "app.ask",
            &[ArgumentValue::Int(4)],
            &bytes,
        )
        .unwrap();
        assert_eq!(recovered, continuation);
        assert_eq!(
            decode_source_checkpoint_v5(
                &program,
                &key(),
                &scope(),
                "app.ask",
                &[ArgumentValue::Int(4)],
                &bytes
            ),
            Err(SourceCheckpointError::SchemaMismatch)
        );
    }

    #[test]
    fn v6_checkpoints_the_largest_admitted_bytes_request() {
        let raw = vec!["255u8"; 1024].join(", ");
        let fields = (0..8)
            .map(|index| format!("@id(\"app.prompt.f{index}\") f{index}: Bytes"))
            .collect::<Vec<_>>()
            .join(", ");
        let values = (0..8)
            .map(|index| format!("f{index}: make_buf()"))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            "module test.channel_checkpoint_max_bytes;\n\
             @id(\"bytes.make\") fn make_buf() -> Bytes {{ let raw = [{raw}]; bytes_copy(array_as_slice(raw)) }}\n\
             @id(\"app.prompt\") record Prompt {{ {fields}, }}\n\
             @id(\"app.ask\") fn ask(seed: i64) -> i64 yields Prompt -> i64 {{\n\
                 let answer = yield Prompt {{ {values} }}; answer + seed\n\
             }}\n\
             @id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
        );
        let program = program(&source);
        let continuation = first(&program);
        let bytes = encode_source_checkpoint_v6(
            &program,
            &key(),
            &scope(),
            "app.ask",
            &[ArgumentValue::Int(4)],
            &continuation,
        )
        .unwrap();
        assert!(bytes.len() > MAX_CHECKPOINT_BYTES);
        assert!(bytes.len() < MAX_V6_CHECKPOINT_BYTES);
        assert_eq!(
            decode_source_checkpoint_v6(
                &program,
                &key(),
                &scope(),
                "app.ask",
                &[ArgumentValue::Int(4)],
                &bytes,
            )
            .unwrap(),
            continuation
        );
    }

    const VARIANT_BYTES_SOURCE: &str = r#"
module test.channel_checkpoint_variant_bytes;
@id("app.step") variant Step {
 @id("app.step.scalar") Scalar { @id("app.step.scalar.value") value: i64, },
 @id("app.step.bytes") Bytes { @id("app.step.bytes.payload") payload: Bytes, },
}
@id("app.ask") fn ask(seed: i64) -> i64 yields Step -> i64 { yield Step::Scalar { value: seed } }
@id("app.main") fn main() -> i64 { 0 }
"#;

    #[test]
    fn v6_uses_checked_variant_shape_when_the_current_case_is_scalar() {
        let program = program(VARIANT_BYTES_SOURCE);
        let continuation = first(&program);
        assert!(matches!(
            continuation.request(),
            ResumableChannelValue::Variant { .. }
        ));
        let bytes = encode_source_checkpoint_v6(
            &program,
            &key(),
            &scope(),
            "app.ask",
            &[ArgumentValue::Int(4)],
            &continuation,
        )
        .unwrap();
        let document: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(document["schema"], SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V6);
        assert_eq!(
            decode_source_checkpoint_v6(
                &program,
                &key(),
                &scope(),
                "app.ask",
                &[ArgumentValue::Int(4)],
                &bytes
            )
            .unwrap(),
            continuation
        );
    }

    #[test]
    fn authenticated_v6_bytes_request_corruption_is_refused() {
        let program = program(BYTES_SOURCE);
        let continuation = first(&program);
        let bytes = encode_source_checkpoint_v6(
            &program,
            &key(),
            &scope(),
            "app.ask",
            &[ArgumentValue::Int(4)],
            &continuation,
        )
        .unwrap();
        let document: Value = serde_json::from_slice(&bytes).unwrap();
        let mut corrupted = document["continuation"].clone();
        corrupted["request"]["fields"]
            .as_array_mut()
            .unwrap()
            .clear();
        let forged = render(
            &key(),
            payload(
                SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V6,
                &scope(),
                "app.ask",
                document["signature"].clone(),
                corrupted,
            ),
            AUTHENTICATION_DOMAIN_V6,
        )
        .unwrap();
        assert!(decode_source_checkpoint_v6(
            &program,
            &key(),
            &scope(),
            "app.ask",
            &[ArgumentValue::Int(4)],
            &forged
        )
        .is_err());
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

    #[test]
    fn v5_rejects_a_v6_schema_before_interpreting_its_payload() {
        let program = program(SOURCE);
        let continuation = first(&program);
        let bytes = encode(&program, &continuation);
        let mut document: Value = serde_json::from_slice(&bytes).unwrap();
        document["schema"] = Value::String(SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V6.to_owned());
        let crossed = format!("{document}\n").into_bytes();
        assert_eq!(
            decode(&program, &crossed),
            Err(SourceCheckpointError::SchemaMismatch)
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
                SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V5,
                &scope(),
                "app.ask",
                document["signature"].clone(),
                corrupted,
            ),
            AUTHENTICATION_DOMAIN_V5,
        )
        .unwrap();
        // A freshly, validly signed corrupted document still fails: the
        // outer HMAC alone is not what is catching this.
        assert!(decode(&program, &forged).is_err());
    }
}

#[cfg(test)]
mod arguments_tests;
