use super::*;
use crate::hir::DeclarationId;
use crate::interpreter::resumable::{
    channel::{
        resume_sequential_channel_resumable_effect_with_arguments,
        run_sequential_channel_resumable_effect_with_arguments, SequentialChannelArgumentsStep,
    },
    ResumableChannelValue,
};

const SOURCE: &str = r#"
module test.checkpoint_arguments;
@id("app.input")
record Input {
    @id("app.input.seed") seed: i64,
    @id("app.input.urgent") urgent: bool,
}
@id("app.output")
record Output {
    @id("app.output.value") value: i64,
    @id("app.output.urgent") urgent: bool,
}
@id("app.ask")
fn ask(input: Input) -> Output yields i64 -> i64 {
    let first = yield input.seed;
    let second = yield first;
    Output { value: second, urgent: input.urgent }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn fixture() -> (
    ResolvedProgram,
    SourceCheckpointKey,
    SourceCheckpointScope,
    Vec<ResumableChannelValue>,
) {
    let parsed = crate::parse(SOURCE, std::path::Path::new("checkpoint-arguments.spx")).unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    (
        program,
        SourceCheckpointKey::new([7; 32]),
        SourceCheckpointScope::new("sha256:program", "aggregate-invocation", 3).unwrap(),
        vec![ResumableChannelValue::Record {
            declaration: DeclarationId::new("app.input"),
            fields: vec![ArgumentValue::Int(4), ArgumentValue::Bool(true)],
        }],
    )
}

fn suspended(step: SequentialChannelArgumentsStep) -> ResumableChannelContinuation {
    let SequentialChannelArgumentsStep::Suspended { continuation } = step else {
        panic!("expected suspension")
    };
    continuation
}

#[test]
fn v7_round_trips_both_sites_and_publishes_only_the_checked_aggregate_result() {
    let (program, key, scope, arguments) = fixture();
    let mut continuation = suspended(
        run_sequential_channel_resumable_effect_with_arguments(
            &program, "app.ask", &arguments, 10_000,
        )
        .unwrap()
        .step,
    );
    for (site, answer) in [9, 12].into_iter().enumerate() {
        let bytes = encode_source_checkpoint_v7(
            &program,
            &key,
            &scope,
            "app.ask",
            &arguments,
            &continuation,
        )
        .unwrap();
        let wire: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(wire["schema"], SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V7);
        assert_eq!(
            wire["continuation"]["schema"],
            checkpoint::SEQUENTIAL_CHANNEL_ARGUMENTS_CHECKPOINT_SCHEMA
        );
        assert_eq!(
            wire["continuation"]["history"].as_array().unwrap().len(),
            site
        );
        assert!(wire.get("result").is_none());
        continuation =
            decode_source_checkpoint_v7(&program, &key, &scope, "app.ask", &arguments, &bytes)
                .unwrap();
        assert_eq!(
            bytes,
            encode_source_checkpoint_v7(
                &program,
                &key,
                &scope,
                "app.ask",
                &arguments,
                &continuation
            )
            .unwrap()
        );
        let resumed = resume_sequential_channel_resumable_effect_with_arguments(
            &program,
            "app.ask",
            &arguments,
            &continuation,
            &ResumableChannelValue::Scalar(ArgumentValue::Int(answer)),
            10_000,
        )
        .unwrap();
        if site == 0 {
            continuation = suspended(resumed.step);
        } else {
            let SequentialChannelArgumentsStep::Completed { result, .. } = resumed.step else {
                panic!("expected completion")
            };
            assert_eq!(
                result,
                ResumableChannelValue::Record {
                    declaration: DeclarationId::new("app.output"),
                    fields: vec![ArgumentValue::Int(12), ArgumentValue::Bool(true)]
                }
            );
        }
    }
}

#[test]
fn v7_rejects_changed_arguments_scope_noncanonical_bytes_and_schema_substitution() {
    let (program, key, scope, arguments) = fixture();
    let continuation = suspended(
        run_sequential_channel_resumable_effect_with_arguments(
            &program, "app.ask", &arguments, 10_000,
        )
        .unwrap()
        .step,
    );
    let bytes =
        encode_source_checkpoint_v7(&program, &key, &scope, "app.ask", &arguments, &continuation)
            .unwrap();
    let decode = |args: &[ResumableChannelValue], data: &[u8]| {
        decode_source_checkpoint_v7(&program, &key, &scope, "app.ask", args, data).unwrap_err()
    };
    let mut changed = arguments.clone();
    let ResumableChannelValue::Record { fields, .. } = &mut changed[0] else {
        unreachable!()
    };
    fields[0] = ArgumentValue::Int(5);
    assert_eq!(
        decode(&changed, &bytes),
        SourceCheckpointError::ArgumentsMismatch
    );
    let wrong_scope = SourceCheckpointScope::new("sha256:program", "other", 3).unwrap();
    assert_eq!(
        decode_source_checkpoint_v7(&program, &key, &wrong_scope, "app.ask", &arguments, &bytes)
            .unwrap_err(),
        SourceCheckpointError::ScopeMismatch
    );
    let mut padded = bytes.clone();
    padded.push(b' ');
    assert_eq!(
        decode(&arguments, &padded),
        SourceCheckpointError::NonCanonical
    );
    for schema in [
        SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V5,
        SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V6,
    ] {
        let mut document: Value = serde_json::from_slice(&bytes).unwrap();
        document["schema"] = json!(schema);
        assert_eq!(
            decode(&arguments, format!("{document}\n").as_bytes()),
            SourceCheckpointError::SchemaMismatch
        );
    }
    assert_eq!(
        decode_source_checkpoint_v5(&program, &key, &scope, "app.ask", &[], &bytes).unwrap_err(),
        SourceCheckpointError::SchemaMismatch
    );
    assert_eq!(
        decode_source_checkpoint_v6(&program, &key, &scope, "app.ask", &[], &bytes).unwrap_err(),
        SourceCheckpointError::SchemaMismatch
    );
    for schema in [
        checkpoint::SEQUENTIAL_CHANNEL_CHECKPOINT_SCHEMA,
        checkpoint::SEQUENTIAL_CHANNEL_BYTES_CHECKPOINT_SCHEMA,
    ] {
        let mut document: Value = serde_json::from_slice(&bytes).unwrap();
        document.as_object_mut().unwrap().remove("authentication");
        document["continuation"]["schema"] = json!(schema);
        let forged = render(&key, document, AUTHENTICATION_DOMAIN_V7).unwrap();
        assert_eq!(
            decode(&arguments, &forged),
            SourceCheckpointError::SchemaMismatch
        );
    }
}

#[test]
fn v7_checks_nominal_field_and_leaf_types_and_exact_argument_framing() {
    let (program, key, scope, arguments) = fixture();
    let continuation = suspended(
        run_sequential_channel_resumable_effect_with_arguments(
            &program, "app.ask", &arguments, 10_000,
        )
        .unwrap()
        .step,
    );
    let bytes =
        encode_source_checkpoint_v7(&program, &key, &scope, "app.ask", &arguments, &continuation)
            .unwrap();
    for bad in [
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("wrong"),
            fields: vec![ArgumentValue::Int(4), ArgumentValue::Bool(true)],
        },
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("app.input"),
            fields: vec![ArgumentValue::Int(4)],
        },
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("app.input"),
            fields: vec![ArgumentValue::Bool(false), ArgumentValue::Bool(true)],
        },
    ] {
        assert_eq!(
            decode_source_checkpoint_v7(&program, &key, &scope, "app.ask", &[bad], &bytes)
                .unwrap_err(),
            SourceCheckpointError::ProgramMismatch
        );
    }
    use sha2::Digest;
    let scalar = ArgumentValue::Int(4);
    let framed = format!("{}\n", checkpoint::scalar_json(&scalar));
    assert_eq!(
        channel_arguments_digest(&[ResumableChannelValue::Scalar(scalar)]),
        format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(framed.as_bytes()))
        )
    );
    let mut document: Value = serde_json::from_slice(&bytes).unwrap();
    document["arguments_digest"] = json!("sha256:changed");
    assert_eq!(
        decode_source_checkpoint_v7(
            &program,
            &key,
            &scope,
            "app.ask",
            &arguments,
            format!("{document}\n").as_bytes()
        )
        .unwrap_err(),
        SourceCheckpointError::AuthenticationMismatch
    );
    document.as_object_mut().unwrap().remove("authentication");
    let authenticated = render(&key, document, AUTHENTICATION_DOMAIN_V7).unwrap();
    assert_eq!(
        decode_source_checkpoint_v7(
            &program,
            &key,
            &scope,
            "app.ask",
            &arguments,
            &authenticated
        )
        .unwrap_err(),
        SourceCheckpointError::ArgumentsMismatch
    );
}

#[test]
fn v7_variant_arguments_bind_the_selected_case_and_return_variant_values() {
    let source = r#"
module test.checkpoint_variant;
@id("app.input")
variant Input {
    @id("app.input.first") First { @id("app.input.first.value") value: i64, },
    @id("app.input.second") Second { @id("app.input.second.value") value: i64, },
}
@id("app.ask")
fn ask(input: Input) -> Input yields i64 -> i64 {
    let first = yield 1;
    let second = yield first;
    input
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(source, std::path::Path::new("checkpoint-variant.spx")).unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    let (_, key, scope, _) = fixture();
    let argument = ResumableChannelValue::Variant {
        declaration: DeclarationId::new("app.input"),
        case: DeclarationId::new("app.input.first"),
        fields: vec![ArgumentValue::Int(7)],
    };
    let arguments = [argument.clone()];
    let continuation = suspended(
        run_sequential_channel_resumable_effect_with_arguments(
            &program, "app.ask", &arguments, 10_000,
        )
        .unwrap()
        .step,
    );
    let bytes =
        encode_source_checkpoint_v7(&program, &key, &scope, "app.ask", &arguments, &continuation)
            .unwrap();
    for (case, expected) in [
        ("app.input.second", SourceCheckpointError::ArgumentsMismatch),
        ("unknown.case", SourceCheckpointError::ProgramMismatch),
    ] {
        let bad = ResumableChannelValue::Variant {
            declaration: DeclarationId::new("app.input"),
            case: DeclarationId::new(case),
            fields: vec![ArgumentValue::Int(7)],
        };
        assert_eq!(
            decode_source_checkpoint_v7(&program, &key, &scope, "app.ask", &[bad], &bytes)
                .unwrap_err(),
            expected
        );
    }
    let restored =
        decode_source_checkpoint_v7(&program, &key, &scope, "app.ask", &arguments, &bytes).unwrap();
    let answer = ResumableChannelValue::Scalar(ArgumentValue::Int(9));
    let second = suspended(
        resume_sequential_channel_resumable_effect_with_arguments(
            &program, "app.ask", &arguments, &restored, &answer, 10_000,
        )
        .unwrap()
        .step,
    );
    let bytes = encode_source_checkpoint_v7(&program, &key, &scope, "app.ask", &arguments, &second)
        .unwrap();
    let restored =
        decode_source_checkpoint_v7(&program, &key, &scope, "app.ask", &arguments, &bytes).unwrap();
    let completed = resume_sequential_channel_resumable_effect_with_arguments(
        &program, "app.ask", &arguments, &restored, &answer, 10_000,
    )
    .unwrap();
    let SequentialChannelArgumentsStep::Completed { result, .. } = completed.step else {
        panic!("expected completion")
    };
    assert_eq!(result, argument);
}
