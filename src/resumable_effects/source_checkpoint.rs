//! Public, ambient-authority-free checkpoint bytes for the bounded sequential source
//! resumable profile.
//!
//! A checkpoint is proof data. Encoding and decoding do not dispatch an
//! effect, answer a suspension, access storage, or grant resume authority.
//! Recovery requires the checked program, selected function, original scalar
//! arguments, exact caller-owned scope, and the caller's 256-bit HMAC key
//! again; normal resume replay still recomputes every recorded request before
//! accepting an answer.

use crate::hir::ResolvedProgram;
use crate::interpreter::resumable::{checkpoint, ResumableContinuation};
use crate::interpreter::ArgumentValue;
use hmac::{Hmac, KeyInit, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use zeroize::Zeroize;

mod migration;
mod signature_bound;
pub use migration::{
    migrate_source_checkpoint_v2, SourceCheckpointMigration, SourceCheckpointMigrationBudget,
    SourceCheckpointMigrationError, SourceCheckpointMigrationInput,
};
pub use signature_bound::{
    decode_source_checkpoint_v2, encode_source_checkpoint_v2, SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V2,
};

/// Breaking changes to the public envelope require a new schema identity.
pub const SOURCE_RESUMABLE_CHECKPOINT_SCHEMA: &str = "semaprax.source-resumable-checkpoint.v1";

const AUTHENTICATION_DOMAIN: &[u8] = b"semaprax.source-resumable-checkpoint-authentication.v1\0";
const MAX_CHECKPOINT_BYTES: usize = 32 * 1024;
const MAX_SCOPE_FIELD_BYTES: usize = 1024;

/// Exact external facts under which a checkpoint may be considered for
/// recovery. These values carry no authority and are compared byte for byte.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCheckpointScope {
    program_root: String,
    invocation_id: String,
    policy_epoch: u64,
}

/// Caller-owned authentication capability for an untrusted checkpoint store.
/// The key is never serialized, cloned, formatted, or derived from checkpoint
/// bytes, and is cleared when this value is dropped.
pub struct SourceCheckpointKey([u8; 32]);

impl SourceCheckpointKey {
    pub fn new(key: [u8; 32]) -> Self {
        Self(key)
    }
}

impl SourceCheckpointKey {
    /// Domain-separated HMAC-SHA256 under this key for sibling authenticated
    /// carriers (the continuation journal). The key itself never leaves.
    pub(crate) fn authenticate(&self, domain: &[u8], payload: &[u8]) -> [u8; 32] {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.0).expect("HMAC accepts a 32-byte key");
        mac.update(domain);
        mac.update(payload);
        mac.finalize().into_bytes().into()
    }

    /// Constant-time verification of [`Self::authenticate`].
    pub(crate) fn verify(&self, domain: &[u8], payload: &[u8], tag: &[u8]) -> bool {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.0).expect("HMAC accepts a 32-byte key");
        mac.update(domain);
        mac.update(payload);
        mac.verify_slice(tag).is_ok()
    }
}

impl Drop for SourceCheckpointKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl SourceCheckpointScope {
    pub fn new(
        program_root: impl Into<String>,
        invocation_id: impl Into<String>,
        policy_epoch: u64,
    ) -> Result<Self, SourceCheckpointError> {
        let program_root = program_root.into();
        let invocation_id = invocation_id.into();
        validate_scope_field(&program_root)?;
        validate_scope_field(&invocation_id)?;
        Ok(Self {
            program_root,
            invocation_id,
            policy_epoch,
        })
    }

    pub fn program_root(&self) -> &str {
        &self.program_root
    }

    pub fn invocation_id(&self) -> &str {
        &self.invocation_id
    }

    pub fn policy_epoch(&self) -> u64 {
        self.policy_epoch
    }
}

/// Stable refusal classes for public source-continuation recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCheckpointError {
    TooLarge,
    InvalidScope,
    Malformed,
    SchemaMismatch,
    AuthenticationMismatch,
    DigestMismatch,
    NonCanonical,
    ScopeMismatch,
    FunctionMismatch,
    ProgramMismatch,
    SuspensionMismatch,
}

/// Encode a continuation under exact recovery facts authenticated by the
/// caller-owned key. The key grants only the ability to authenticate these
/// bytes; it grants no effect, storage, scheduling, or resume authority.
pub fn encode_source_checkpoint(
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    function_id: &str,
    continuation: &ResumableContinuation,
) -> Result<Vec<u8>, SourceCheckpointError> {
    validate_scope(scope)?;
    validate_scope_field(function_id)?;
    let inner = checkpoint::encode(function_id, continuation).map_err(map_inner_error)?;
    let continuation: Value =
        serde_json::from_slice(&inner).map_err(|_| SourceCheckpointError::Malformed)?;
    render(key, scope, function_id, continuation)
}

/// Decode untrusted bytes only under independently supplied current facts and
/// a caller-owned authentication key.
/// The returned continuation remains inert until the caller explicitly passes
/// it to the ordinary replaying resume API with a separately supplied answer.
pub fn decode_source_checkpoint(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    expected_scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    bytes: &[u8],
) -> Result<ResumableContinuation, SourceCheckpointError> {
    validate_scope(expected_scope)?;
    validate_scope_field(function_id)?;
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(SourceCheckpointError::TooLarge);
    }
    let document: Value =
        serde_json::from_slice(bytes).map_err(|_| SourceCheckpointError::Malformed)?;
    keys(
        &document,
        &[
            "schema",
            "scope",
            "function",
            "continuation",
            "authentication",
        ],
    )?;
    if required_str(&document, "schema")? != SOURCE_RESUMABLE_CHECKPOINT_SCHEMA {
        return Err(SourceCheckpointError::SchemaMismatch);
    }
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
    let continuation_value = document["continuation"].clone();
    let payload = payload(
        &observed_scope,
        encoded_function,
        continuation_value.clone(),
    );
    verify_authentication(key, &payload, required_str(&document, "authentication")?)?;
    if render(
        key,
        &observed_scope,
        encoded_function,
        continuation_value.clone(),
    )?
    .as_slice()
        != bytes
    {
        return Err(SourceCheckpointError::NonCanonical);
    }
    if observed_scope != *expected_scope {
        return Err(SourceCheckpointError::ScopeMismatch);
    }
    if encoded_function != function_id {
        return Err(SourceCheckpointError::FunctionMismatch);
    }
    let inner = format!("{continuation_value}\n").into_bytes();
    checkpoint::decode(program, function_id, arguments, &inner).map_err(map_inner_error)
}

fn render(
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    function_id: &str,
    continuation: Value,
) -> Result<Vec<u8>, SourceCheckpointError> {
    let payload = payload(scope, function_id, continuation.clone());
    let bytes = format!(
        "{}\n",
        json!({
            "schema": SOURCE_RESUMABLE_CHECKPOINT_SCHEMA,
            "scope": scope_json(scope),
            "function": function_id,
            "continuation": continuation,
            "authentication": authentication(key, &payload),
        })
    )
    .into_bytes();
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(SourceCheckpointError::TooLarge);
    }
    Ok(bytes)
}

fn payload(scope: &SourceCheckpointScope, function_id: &str, continuation: Value) -> Value {
    json!({
        "schema": SOURCE_RESUMABLE_CHECKPOINT_SCHEMA,
        "scope": scope_json(scope),
        "function": function_id,
        "continuation": continuation,
    })
}

fn scope_json(scope: &SourceCheckpointScope) -> Value {
    json!({
        "program_root": scope.program_root,
        "invocation_id": scope.invocation_id,
        "policy_epoch": scope.policy_epoch,
    })
}

fn authentication(key: &SourceCheckpointKey, payload: &Value) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(&key.0).expect("HMAC accepts a 32-byte key");
    mac.update(AUTHENTICATION_DOMAIN);
    mac.update(payload.to_string().as_bytes());
    format!(
        "hmac-sha256:{:x}",
        crate::digest_hex::LowerHex(mac.finalize().into_bytes())
    )
}

fn verify_authentication(
    key: &SourceCheckpointKey,
    payload: &Value,
    claimed: &str,
) -> Result<(), SourceCheckpointError> {
    let Some(hex) = claimed.strip_prefix("hmac-sha256:") else {
        return Err(SourceCheckpointError::AuthenticationMismatch);
    };
    if hex.len() != 64 || !hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(SourceCheckpointError::AuthenticationMismatch);
    }
    let mut tag = [0_u8; 32];
    for (index, byte) in tag.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| SourceCheckpointError::AuthenticationMismatch)?;
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(&key.0).expect("HMAC accepts a 32-byte key");
    mac.update(AUTHENTICATION_DOMAIN);
    mac.update(payload.to_string().as_bytes());
    mac.verify_slice(&tag)
        .map_err(|_| SourceCheckpointError::AuthenticationMismatch)
}

fn validate_scope(scope: &SourceCheckpointScope) -> Result<(), SourceCheckpointError> {
    validate_scope_field(&scope.program_root)?;
    validate_scope_field(&scope.invocation_id)
}

fn validate_scope_field(value: &str) -> Result<(), SourceCheckpointError> {
    if value.is_empty()
        || value.len() > MAX_SCOPE_FIELD_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(SourceCheckpointError::InvalidScope);
    }
    Ok(())
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, SourceCheckpointError> {
    value[field]
        .as_str()
        .ok_or(SourceCheckpointError::Malformed)
}

fn keys(value: &Value, expected: &[&str]) -> Result<(), SourceCheckpointError> {
    let object = value.as_object().ok_or(SourceCheckpointError::Malformed)?;
    if object.len() != expected.len() || !expected.iter().all(|key| object.contains_key(*key)) {
        return Err(SourceCheckpointError::Malformed);
    }
    Ok(())
}

fn map_inner_error(error: checkpoint::CheckpointError) -> SourceCheckpointError {
    match error {
        checkpoint::CheckpointError::TooLarge => SourceCheckpointError::TooLarge,
        checkpoint::CheckpointError::Malformed(_) => SourceCheckpointError::Malformed,
        checkpoint::CheckpointError::SchemaMismatch => SourceCheckpointError::SchemaMismatch,
        checkpoint::CheckpointError::DigestMismatch => SourceCheckpointError::DigestMismatch,
        checkpoint::CheckpointError::NonCanonical => SourceCheckpointError::NonCanonical,
        checkpoint::CheckpointError::FunctionMismatch => SourceCheckpointError::FunctionMismatch,
        checkpoint::CheckpointError::ProgramMismatch => SourceCheckpointError::ProgramMismatch,
        checkpoint::CheckpointError::SuspensionMismatch => {
            SourceCheckpointError::SuspensionMismatch
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::resumable::{
        resume_sequential_resumable_effect, run_sequential_resumable_effect,
        SequentialResumableStep,
    };
    use std::path::Path;

    const SOURCE: &str = r#"
module test.source_checkpoint;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let first = yield seed + 1;
    yield first + 2
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program(source: &str) -> ResolvedProgram {
        let ast = crate::parse(source, Path::new("source-checkpoint.spx")).unwrap();
        crate::hir::resolve(&ast).unwrap()
    }

    fn scope() -> SourceCheckpointScope {
        SourceCheckpointScope::new("sha256:program", "invocation-7", 11).unwrap()
    }

    fn key() -> SourceCheckpointKey {
        SourceCheckpointKey::new([0x5a; 32])
    }

    fn first_continuation(program: &ResolvedProgram) -> ResumableContinuation {
        let evaluation =
            run_sequential_resumable_effect(program, "app.ask", &[ArgumentValue::Int(4)], 10_000)
                .unwrap();
        let SequentialResumableStep::Suspended { continuation } = evaluation.step else {
            panic!("fixture did not suspend")
        };
        continuation
    }

    #[test]
    fn scoped_checkpoint_round_trips_and_resumes_through_normal_replay() {
        let program = program(SOURCE);
        let scope = scope();
        let key = key();
        let bytes =
            encode_source_checkpoint(&key, &scope, "app.ask", &first_continuation(&program))
                .unwrap();
        let recovered = decode_source_checkpoint(
            &program,
            &key,
            &scope,
            "app.ask",
            &[ArgumentValue::Int(4)],
            &bytes,
        )
        .unwrap();
        let resumed = resume_sequential_resumable_effect(
            &program,
            "app.ask",
            &[ArgumentValue::Int(4)],
            &recovered,
            &ArgumentValue::Int(10),
            10_000,
        )
        .unwrap();
        let SequentialResumableStep::Suspended { continuation } = resumed.step else {
            panic!("first answer did not reach the second suspension")
        };
        assert_eq!(continuation.request(), &ArgumentValue::Int(12));
    }

    #[test]
    fn every_external_scope_dimension_is_required_exactly() {
        let program = program(SOURCE);
        let key = key();
        let bytes =
            encode_source_checkpoint(&key, &scope(), "app.ask", &first_continuation(&program))
                .unwrap();
        for wrong in [
            SourceCheckpointScope::new("sha256:other", "invocation-7", 11).unwrap(),
            SourceCheckpointScope::new("sha256:program", "invocation-8", 11).unwrap(),
            SourceCheckpointScope::new("sha256:program", "invocation-7", 12).unwrap(),
        ] {
            assert_eq!(
                decode_source_checkpoint(
                    &program,
                    &key,
                    &wrong,
                    "app.ask",
                    &[ArgumentValue::Int(4)],
                    &bytes,
                ),
                Err(SourceCheckpointError::ScopeMismatch)
            );
        }
    }

    #[test]
    fn corruption_noncanonical_bytes_and_program_drift_fail_closed() {
        let original = program(SOURCE);
        let key = key();
        let bytes =
            encode_source_checkpoint(&key, &scope(), "app.ask", &first_continuation(&original))
                .unwrap();
        let mut corrupted = bytes.clone();
        let position = corrupted
            .windows("invocation-7".len())
            .position(|window| window == b"invocation-7")
            .unwrap();
        corrupted[position] = b'I';
        assert_eq!(
            decode_source_checkpoint(
                &original,
                &key,
                &scope(),
                "app.ask",
                &[ArgumentValue::Int(4)],
                &corrupted,
            ),
            Err(SourceCheckpointError::AuthenticationMismatch)
        );

        let padded = [b" ".as_slice(), bytes.as_slice()].concat();
        assert_eq!(
            decode_source_checkpoint(
                &original,
                &key,
                &scope(),
                "app.ask",
                &[ArgumentValue::Int(4)],
                &padded,
            ),
            Err(SourceCheckpointError::NonCanonical)
        );

        let drifted = program(&SOURCE.replace("seed + 1", "seed + 9"));
        assert!(matches!(
            decode_source_checkpoint(
                &drifted,
                &key,
                &scope(),
                "app.ask",
                &[ArgumentValue::Int(4)],
                &bytes,
            ),
            Err(SourceCheckpointError::ProgramMismatch)
                | Err(SourceCheckpointError::SuspensionMismatch)
        ));
    }

    #[test]
    fn untrusted_store_cannot_rebind_scope_or_use_another_key() {
        let program = program(SOURCE);
        let key = key();
        let bytes =
            encode_source_checkpoint(&key, &scope(), "app.ask", &first_continuation(&program))
                .unwrap();
        assert_eq!(
            decode_source_checkpoint(
                &program,
                &SourceCheckpointKey::new([0xa5; 32]),
                &scope(),
                "app.ask",
                &[ArgumentValue::Int(4)],
                &bytes,
            ),
            Err(SourceCheckpointError::AuthenticationMismatch)
        );

        let rebound = SourceCheckpointScope::new("sha256:program", "invocation-8", 12).unwrap();
        let mut document: Value = serde_json::from_slice(&bytes).unwrap();
        document["scope"] = scope_json(&rebound);
        let forged = format!("{document}\n").into_bytes();
        assert_eq!(
            decode_source_checkpoint(
                &program,
                &key,
                &rebound,
                "app.ask",
                &[ArgumentValue::Int(4)],
                &forged,
            ),
            Err(SourceCheckpointError::AuthenticationMismatch)
        );
    }

    #[test]
    fn scope_and_document_bounds_are_explicit() {
        assert_eq!(
            SourceCheckpointScope::new("", "invocation", 0),
            Err(SourceCheckpointError::InvalidScope)
        );
        assert_eq!(
            SourceCheckpointScope::new("x".repeat(MAX_SCOPE_FIELD_BYTES + 1), "invocation", 0),
            Err(SourceCheckpointError::InvalidScope)
        );
        assert_eq!(
            decode_source_checkpoint(
                &program(SOURCE),
                &key(),
                &scope(),
                "app.ask",
                &[ArgumentValue::Int(4)],
                &vec![b'x'; MAX_CHECKPOINT_BYTES + 1],
            ),
            Err(SourceCheckpointError::TooLarge)
        );
    }
}
