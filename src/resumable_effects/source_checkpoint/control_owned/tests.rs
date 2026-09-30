use super::*;
use crate::hir;
use crate::interpreter::resumable::control::{run_control_resumable_effect, ControlResumableStep};
use crate::resumable_effects::source_checkpoint::{
    decode_source_checkpoint_v2, decode_source_checkpoint_v3, SourceCheckpointKey,
};

pub(crate) const OWNED_SOURCE: &str = r#"
module test.control_owned_checkpoint;
@id("bytes.make")
fn make_buf() -> Bytes {
    let bytes = [1u8, 2u8, 3u8];
    bytes_copy(array_as_slice(bytes))
}
@id("bytes.consume")
fn consume(value: own Bytes) -> i64 {
    let _ = bytes_as_slice(value);
    100
}
@id("app.ask")
fn ask(seed: i64, flag: bool) -> i64
    yields i64 -> i64
{
    let outcome = if flag {
        let buf = make_buf();
        let answer = yield seed;
        let used = consume(buf);
        answer + used
    } else {
        0
    };
    outcome
}
@id("app.main") fn main() -> i64 { 0 }
"#;

pub(crate) fn program() -> ResolvedProgram {
    hir::resolve(
        &crate::parse(
            OWNED_SOURCE,
            std::path::Path::new("control-owned-checkpoint.spx"),
        )
        .unwrap(),
    )
    .unwrap()
}

fn scope() -> SourceCheckpointScope {
    SourceCheckpointScope::new("root", "invocation-owned-1", 1).unwrap()
}

pub(crate) fn first(program: &ResolvedProgram) -> ControlContinuation {
    let ControlResumableStep::Suspended { continuation } = run_control_resumable_effect(
        program,
        "app.ask",
        &[ArgumentValue::Int(7), ArgumentValue::Bool(true)],
        100_000,
    )
    .unwrap()
    .step
    else {
        panic!("suspends")
    };
    continuation
}

#[test]
fn v4_round_trips_carries_the_owned_bytes_value_and_no_other_version_reads_it() {
    let program = program();
    let key = SourceCheckpointKey::new([9; 32]);
    let arguments = [ArgumentValue::Int(7), ArgumentValue::Bool(true)];
    let continuation = first(&program);
    assert_eq!(continuation.carried().len(), 1);
    assert_eq!(continuation.carried()[0].1, vec![1u8, 2u8, 3u8]);

    let bytes = encode_source_checkpoint_v4(
        &program,
        &key,
        &scope(),
        "app.ask",
        &arguments,
        &continuation,
    )
    .unwrap();
    assert_eq!(
        decode_source_checkpoint_v4(&program, &key, &scope(), "app.ask", &arguments, &bytes)
            .unwrap(),
        continuation
    );
    // v2 and v3 both refuse v4 bytes by schema.
    assert_eq!(
        decode_source_checkpoint_v2(&program, &key, &scope(), "app.ask", &arguments, &bytes),
        Err(SourceCheckpointError::SchemaMismatch)
    );
    assert_eq!(
        decode_source_checkpoint_v3(&program, &key, &scope(), "app.ask", &arguments, &bytes),
        Err(SourceCheckpointError::SchemaMismatch)
    );
}

#[test]
fn tampered_carried_bytes_are_refused() {
    let program = program();
    let key = SourceCheckpointKey::new([9; 32]);
    let arguments = [ArgumentValue::Int(7), ArgumentValue::Bool(true)];
    let continuation = first(&program);
    let bytes = encode_source_checkpoint_v4(
        &program,
        &key,
        &scope(),
        "app.ask",
        &arguments,
        &continuation,
    )
    .unwrap();
    let text = String::from_utf8(bytes).unwrap();
    // The carried payload is the hex of `[1, 2, 3]`.
    let tampered = text.replacen("010203", "040506", 1);
    assert_ne!(tampered, text);
    let error = decode_source_checkpoint_v4(
        &program,
        &key,
        &scope(),
        "app.ask",
        &arguments,
        tampered.as_bytes(),
    )
    .unwrap_err();
    assert_eq!(error, SourceCheckpointError::AuthenticationMismatch);
}

#[test]
fn a_non_carrying_control_plan_keeps_its_v3_identity() {
    // `test.control_checkpoint` (the v3 fixture) never carries an owned
    // value: its own signature must still report v3, not v4, so the two
    // schemas never accidentally trade places.
    let source = r#"
module test.control_checkpoint_v3_still;
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
    let program =
        hir::resolve(&crate::parse(source, std::path::Path::new("v3-still.spx")).unwrap()).unwrap();
    let signature = crate::resumable_effects::source_signature::derive_source_effect_signature(
        &program, "app.ask",
    )
    .unwrap();
    assert!(signature.is_control_dependent());
    assert!(!signature.carries_owned_bytes());
}
