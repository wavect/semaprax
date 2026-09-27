//! Issue #296 R20: the bounded Copy-scalar record/variant `yields` channel,
//! driven through real `.spx` source end to end on the interpreter.

use super::*;
use crate::hir::{self, DeclarationId};
use std::path::Path;

const MAX_STEPS: usize = 10_000;

const BYTES_REQUEST_ASK: &str = r#"
module test.resumable_channel_bytes;
@id("bytes.make")
fn make_buf() -> Bytes {
    let bytes = [1u8, 2u8, 3u8];
    bytes_copy(array_as_slice(bytes))
}
@id("app.prompt")
record Prompt { @id("app.prompt.payload") payload: Bytes, }
@id("app.ask")
fn ask(seed: i64) -> i64 yields Prompt -> i64 {
    let answer = yield Prompt { payload: make_buf() };
    answer + seed
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Two sequential record-channel yields: the second request is built from
/// the first answer's own field, and the final scalar result combines both
/// answers. The function's own parameters and return type stay Copy-scalar
/// (this increment widens only the `yields` channel).
const RECORD_ASK: &str = r#"
module test.resumable_channel_record;
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

/// A variant response channel: the answer selects which case the suffix
/// branches on.
const VARIANT_ASK: &str = r#"
module test.resumable_channel_variant;
@id("app.step")
variant Step {
    @id("app.step.continue")
    Continue { @id("app.step.continue.round") round: i64, },
    @id("app.step.done")
    Done { @id("app.step.done.total") total: i64, },
}
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> Step
{
    let answer = yield seed;
    match answer {
        Step::Continue { round: round } => round,
        Step::Done { total: total } => total,
    }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

/// A variant request channel: the request is built inline
/// (`ConstructVariant`), exercising the same bounded Copy-scalar admission
/// on the construction side that `VARIANT_ASK` exercises on the match side.
const VARIANT_REQUEST_ASK: &str = r#"
module test.resumable_channel_variant_request;
@id("app.step")
variant Step {
    @id("app.step.continue")
    Continue { @id("app.step.continue.round") round: i64, },
    @id("app.step.done")
    Done { @id("app.step.done.ok") ok: bool, },
}
@id("app.ask")
fn ask(seed: i64) -> i64
    yields Step -> i64
{
    yield Step::Continue { round: seed }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

const WHOLE_FUNCTION_AGGREGATE: &str = r#"
module test.resumable_channel_whole_function;
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

fn resolved(source: &str) -> hir::ResolvedProgram {
    let program = crate::parse(source, Path::new("resumable-channel.spx"))
        .expect("the fixture source parses");
    hir::resolve(&program).expect("the fixture source resolves")
}

fn prompt(seed: i64, urgent: bool) -> ResumableChannelValue {
    ResumableChannelValue::Record {
        declaration: DeclarationId::new("app.prompt"),
        fields: vec![ArgumentValue::Int(seed), ArgumentValue::Bool(urgent)],
    }
}

fn answer(value: i64, ok: bool) -> ResumableChannelValue {
    ResumableChannelValue::Record {
        declaration: DeclarationId::new("app.answer"),
        fields: vec![ArgumentValue::Int(value), ArgumentValue::Bool(ok)],
    }
}

#[test]
fn a_direct_sequential_bytes_request_suspends_and_resumes() {
    let program = resolved(BYTES_REQUEST_ASK);
    let started = run_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        MAX_STEPS,
    )
    .unwrap();
    let SequentialChannelResumableStep::Suspended { continuation } = started.step else {
        panic!("byte request did not suspend")
    };
    let ResumableChannelValue::RecordBytes { fields, .. } = continuation.request() else {
        panic!("request did not preserve its owned Bytes leaf")
    };
    assert_eq!(
        fields,
        &[super::super::channel_bytes::ChannelField::Bytes(vec![
            1, 2, 3
        ])]
    );
    let resumed = resume_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &continuation,
        &ResumableChannelValue::Scalar(ArgumentValue::Int(8)),
        MAX_STEPS,
    )
    .unwrap();
    assert!(matches!(
        resumed.step,
        SequentialChannelResumableStep::Completed {
            result: ResumableChannelValue::Scalar(ArgumentValue::Int(12)),
            ..
        }
    ));
}

#[test]
fn a_record_channel_suspends_twice_and_completes_with_the_combined_scalar_result() {
    let program = resolved(RECORD_ASK);
    let started = run_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        MAX_STEPS,
    )
    .unwrap();
    let SequentialChannelResumableStep::Suspended { continuation } = started.step else {
        panic!("fresh invocation did not suspend at its first record yield")
    };
    assert_eq!(continuation.request(), &prompt(4, false));

    let resumed = resume_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &continuation,
        &answer(10, true),
        MAX_STEPS,
    )
    .unwrap();
    let debug_step = resumed.step.clone();
    let SequentialChannelResumableStep::Suspended { continuation } = resumed.step else {
        panic!("first answer did not reach the second record yield: {debug_step:?}")
    };
    // The second request is built from the first answer's own field
    // (`first.value`), so a wrong first answer would produce a different
    // second request -- this is the record-channel counterpart of the
    // scalar sequential lane's own suffix-is-a-function-of-the-answer test.
    assert_eq!(continuation.request(), &prompt(10, true));

    let finished = resume_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &continuation,
        &answer(20, false),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialChannelResumableStep::Completed { result, .. } = finished.step else {
        panic!("the suffix did not complete")
    };
    assert_eq!(
        result,
        ResumableChannelValue::Scalar(ArgumentValue::Int(30))
    );
}

#[test]
fn a_variant_channel_answer_selects_the_branch_the_suffix_takes() {
    let program = resolved(VARIANT_ASK);
    for (case, value, expected) in [("app.step.continue", 7, 7), ("app.step.done", 41, 41)] {
        let started = run_sequential_channel_resumable_effect(
            &program,
            "app.ask",
            &[ArgumentValue::Int(2)],
            MAX_STEPS,
        )
        .unwrap();
        let SequentialChannelResumableStep::Suspended { continuation } = started.step else {
            panic!("fresh invocation did not suspend")
        };
        assert_eq!(
            continuation.request(),
            &ResumableChannelValue::Scalar(ArgumentValue::Int(2))
        );
        let finished = resume_sequential_channel_resumable_effect(
            &program,
            "app.ask",
            &[ArgumentValue::Int(2)],
            &continuation,
            &ResumableChannelValue::Variant {
                declaration: DeclarationId::new("app.step"),
                case: DeclarationId::new(case),
                fields: vec![ArgumentValue::Int(value)],
            },
            MAX_STEPS,
        )
        .unwrap();
        let debug_step = finished.step.clone();
        let SequentialChannelResumableStep::Completed { result, .. } = finished.step else {
            panic!("the suffix did not complete for case {case}: {debug_step:?}")
        };
        assert_eq!(
            result,
            ResumableChannelValue::Scalar(ArgumentValue::Int(expected))
        );
    }
}

#[test]
fn a_variant_request_is_constructed_inline_and_replays_identically() {
    let program = resolved(VARIANT_REQUEST_ASK);
    let started = run_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(9)],
        MAX_STEPS,
    )
    .unwrap();
    let SequentialChannelResumableStep::Suspended { continuation } = started.step else {
        panic!("fresh invocation did not suspend at its constructed variant request")
    };
    assert_eq!(
        continuation.request(),
        &ResumableChannelValue::Variant {
            declaration: DeclarationId::new("app.step"),
            case: DeclarationId::new("app.step.continue"),
            fields: vec![ArgumentValue::Int(9)],
        }
    );
    let finished = resume_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(9)],
        &continuation,
        &ResumableChannelValue::Scalar(ArgumentValue::Int(42)),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialChannelResumableStep::Completed { result, .. } = finished.step else {
        panic!("the suffix did not complete: {:?}", finished.step)
    };
    assert_eq!(
        result,
        ResumableChannelValue::Scalar(ArgumentValue::Int(42))
    );
}

#[test]
fn whole_function_copy_aggregate_arguments_and_results_are_replay_bound() {
    let program = resolved(WHOLE_FUNCTION_AGGREGATE);
    let input = ResumableChannelValue::Record {
        declaration: DeclarationId::new("app.input"),
        fields: vec![ArgumentValue::Int(4), ArgumentValue::Bool(true)],
    };
    let started = run_sequential_channel_resumable_effect_with_arguments(
        &program,
        "app.ask",
        std::slice::from_ref(&input),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialChannelResumableStep::Suspended { continuation } = started.step else {
        panic!("whole-function aggregate invocation did not suspend");
    };
    let resumed = resume_sequential_channel_resumable_effect_with_arguments(
        &program,
        "app.ask",
        std::slice::from_ref(&input),
        &continuation,
        &ResumableChannelValue::Scalar(ArgumentValue::Int(9)),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialChannelResumableStep::Suspended { continuation } = resumed.step else {
        panic!("first answer did not reach the second yield");
    };
    let completed = resume_sequential_channel_resumable_effect_with_arguments(
        &program,
        "app.ask",
        std::slice::from_ref(&input),
        &continuation,
        &ResumableChannelValue::Scalar(ArgumentValue::Int(12)),
        MAX_STEPS,
    )
    .unwrap();
    assert!(
        matches!(completed.step, SequentialChannelResumableStep::Completed {
        result: ResumableChannelValue::Record { ref declaration, ref fields }, ..
    } if *declaration == DeclarationId::new("app.output")
        && fields == &vec![ArgumentValue::Int(12), ArgumentValue::Bool(true)])
    );

    let changed = ResumableChannelValue::Record {
        declaration: DeclarationId::new("app.input"),
        fields: vec![ArgumentValue::Int(5), ArgumentValue::Bool(true)],
    };
    let error = resume_sequential_channel_resumable_effect_with_arguments(
        &program,
        "app.ask",
        &[changed],
        &continuation,
        &ResumableChannelValue::Scalar(ArgumentValue::Int(12)),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F115");
}

#[test]
fn whole_function_argument_with_the_wrong_nominal_identity_is_refused_before_running() {
    let program = resolved(WHOLE_FUNCTION_AGGREGATE);
    let error = run_sequential_channel_resumable_effect_with_arguments(
        &program,
        "app.ask",
        &[ResumableChannelValue::Record {
            declaration: DeclarationId::new("app.output"),
            fields: vec![ArgumentValue::Int(4), ArgumentValue::Bool(true)],
        }],
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F103");
}

#[test]
fn an_answer_of_the_wrong_record_type_is_refused_before_the_suffix_runs() {
    let program = resolved(RECORD_ASK);
    let started = run_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        MAX_STEPS,
    )
    .unwrap();
    let SequentialChannelResumableStep::Suspended { continuation } = started.step else {
        panic!("fresh invocation did not suspend")
    };
    // A `Prompt` (the request type) presented as the answer (the declared
    // response type is `Answer`) is refused, not silently coerced.
    let wrong = prompt(1, true);
    let error = resume_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &continuation,
        &wrong,
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F113");

    // A scalar answer is refused the same way.
    let error = resume_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &continuation,
        &ResumableChannelValue::Scalar(ArgumentValue::Int(10)),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F113");
}

#[test]
fn a_binding_recorded_for_a_different_site_is_refused() {
    let program = resolved(RECORD_ASK);
    let started = run_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        MAX_STEPS,
    )
    .unwrap();
    let SequentialChannelResumableStep::Suspended {
        continuation: first,
    } = started.step
    else {
        panic!("fresh invocation did not suspend")
    };
    let resumed = resume_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &first,
        &answer(10, true),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialChannelResumableStep::Suspended {
        continuation: second,
    } = resumed.step
    else {
        panic!("first answer did not reach the second record yield")
    };

    // `first`'s own state paired with `second`'s binding (recorded for a
    // different site, history, and answer) must not be accepted: the
    // binding commits to the exact site, not merely to a state name.
    let mut mismatched = first.clone();
    mismatched.binding = second.binding.clone();
    let error = resume_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &mismatched,
        &answer(10, true),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F115");
}

/// A resume under a different argument vector cannot reuse a suspension
/// bound to the original one, even though both requests happen to have the
/// same record shape (same fields, same case): the binding hash commits to
/// the exact original scalar argument bits.
#[test]
fn a_resume_under_different_arguments_is_refused_before_replay() {
    let program = resolved(RECORD_ASK);
    let started = run_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        MAX_STEPS,
    )
    .unwrap();
    let SequentialChannelResumableStep::Suspended { continuation } = started.step else {
        panic!("fresh invocation did not suspend")
    };
    let error = resume_sequential_channel_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(5)],
        &continuation,
        &answer(10, true),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F115");
}

const BYTES_NAMED_LOCAL_REFUSAL: &str = r#"
module test.resumable_channel_bytes_named;
@id("bytes.make") fn make_buf() -> Bytes {
    let bytes = [1u8, 2u8, 3u8];
    bytes_copy(array_as_slice(bytes))
}
@id("app.prompt") record Prompt { @id("app.prompt.payload") payload: Bytes, }
@id("app.ask") fn ask() -> i64 yields Prompt -> i64 {
    let spare = make_buf();
    yield Prompt { payload: make_buf() }
}
@id("app.main") fn main() -> i64 { 0 }
"#;

#[test]
fn a_named_bytes_local_before_an_aggregate_yield_stays_t303_refused() {
    let program = crate::parse(
        BYTES_NAMED_LOCAL_REFUSAL,
        Path::new("bytes-named-local.spx"),
    )
    .unwrap();
    let error = hir::resolve(&program).unwrap_err();
    assert_eq!(error[0].code, "SPX-T303");
}

const TWO_BYTES_REQUESTS: &str = r#"
module test.resumable_channel_bytes_replay;
@id("bytes.make") fn make_buf() -> Bytes { let raw = [4u8, 5u8]; bytes_copy(array_as_slice(raw)) }
@id("app.prompt") record Prompt { @id("app.prompt.payload") payload: Bytes, }
@id("app.ask") fn ask() -> i64 yields Prompt -> i64 {
    let first = yield Prompt { payload: make_buf() };
    let second = yield Prompt { payload: make_buf() };
    first + second
}
@id("app.main") fn main() -> i64 { 0 }
"#;

#[test]
fn two_bytes_requests_refuse_before_unproved_owned_replay() {
    let program = crate::parse(TWO_BYTES_REQUESTS, Path::new("two-bytes-requests.spx")).unwrap();
    let errors = hir::resolve(&program).unwrap_err();
    assert_eq!(errors[0].code, "SPX-T307");
}
