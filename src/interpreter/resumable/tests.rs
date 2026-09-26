//! Resumable Effects v1 (issue #204): the interpreter suspend/resume lane.
//!
//! Every test here drives real `.spx` source through parse -> resolve ->
//! evaluate, so a regression in any of those layers reddens these, not only
//! a regression in this module.

use super::*;
use crate::hir;
use std::path::Path;

const MAX_STEPS: usize = 10_000;

/// The canonical slice program: one scalar parameter, one top-level
/// `yield`, and a suffix that uses the resumed value.
const ASK: &str = r#"
module test.resumable_effects_interpreter;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let answer = yield seed + 1;
    answer * 2
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

/// A direct mutable assignment receives the declared response type, not the
/// request placeholder type used while the expression first resolves.
const ASSIGNED_BOOL_ANSWER: &str = r#"
module test.resumable_effects_interpreter_assignment;
@id("app.ask")
fn ask(seed: i64) -> bool
    yields i64 -> bool
{
    let mut answer = false;
    answer = yield seed + 1;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

const TWO_YIELDS: &str = r#"
module test.sequential_resumable_effects_interpreter;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let first = yield seed + 1;
    let second = yield first + 2;
    first + second
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

const THREE_YIELDS: &str = r#"
module test.three_sequential_resumable_effects_interpreter;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let first = yield seed;
    let second = yield first + 1;
    let third = yield second + 1;
    first + second + third
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

const TWO_FLOAT_YIELDS: &str = r#"
module test.float_sequential_resumable_effects_interpreter;
@id("app.ask")
fn ask(seed: f64) -> f64
    yields f64 -> f64
{
    let first = yield seed;
    yield first
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str) -> hir::ResolvedProgram {
    let program = crate::parse(source, Path::new("resumable-effects-interpreter.spx"))
        .expect("the slice source parses");
    hir::resolve(&program).expect("the slice source resolves")
}

fn suspension(
    program: &hir::ResolvedProgram,
    arguments: &[ArgumentValue],
) -> (ResumableStateId, ResumableSuspensionBinding, ArgumentValue) {
    match run_resumable_effect(program, "app.ask", arguments, MAX_STEPS)
        .unwrap()
        .step
    {
        ResumableStep::Suspended {
            state,
            binding,
            request,
        } => (state, binding, request),
        other => panic!("expected suspension, got {other:?}"),
    }
}

fn sequential_suspension(
    program: &hir::ResolvedProgram,
    arguments: &[ArgumentValue],
) -> ResumableContinuation {
    let evaluated =
        run_sequential_resumable_effect(program, "app.ask", arguments, MAX_STEPS).unwrap();
    let SequentialResumableStep::Suspended { continuation } = evaluated.step else {
        panic!("expected sequential suspension")
    };
    continuation
}

#[test]
fn a_fresh_invocation_suspends_at_its_yield_with_the_computed_request() {
    let program = resolved(ASK);
    let evaluated =
        run_resumable_effect(&program, "app.ask", &[ArgumentValue::Int(41)], MAX_STEPS).unwrap();
    // `seed + 1` is the request: the prefix really ran, it was not skipped.
    let ResumableStep::Suspended {
        state,
        binding,
        request,
    } = evaluated.step
    else {
        panic!("fresh invocation did not suspend")
    };
    assert_eq!(request, ArgumentValue::Int(42));
    assert!(state.as_str().contains("|suspended|"));
    assert_ne!(binding.as_bytes(), &[0; 32]);
    assert!(evaluated.steps_used > 0);
    assert_eq!(evaluated.max_steps, MAX_STEPS);
}

#[test]
fn resuming_substitutes_the_answer_at_the_yield_and_runs_the_suffix() {
    let program = resolved(ASK);
    let (state, binding, request) = suspension(&program, &[ArgumentValue::Int(41)]);
    let evaluated = resume_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(41)],
        &state,
        &binding,
        &request,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap();
    // 10 * 2 -- the suffix ran with the supplied answer, not with the
    // request and not with the seed.
    let ResumableStep::Completed {
        state: completed,
        result,
    } = &evaluated.step
    else {
        panic!("resume did not complete")
    };
    assert_eq!(*result, ArgumentValue::Int(20));
    assert!(completed.as_str().contains("|complete"));
    // Negative control: the suffix is genuinely a function of the answer.
    let other = resume_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(41)],
        &state,
        &binding,
        &request,
        &ArgumentValue::Int(11),
        MAX_STEPS,
    )
    .unwrap();
    assert!(matches!(
        other.step,
        ResumableStep::Completed {
            result: ArgumentValue::Int(22),
            ..
        }
    ));
    assert_ne!(other.step, evaluated.step);
}

#[test]
fn a_distinct_response_type_can_resume_through_a_direct_assignment() {
    let program = resolved(ASSIGNED_BOOL_ANSWER);
    let (state, binding, request) = suspension(&program, &[ArgumentValue::Int(41)]);
    assert_eq!(request, ArgumentValue::Int(42));
    let evaluation = resume_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(41)],
        &state,
        &binding,
        &request,
        &ArgumentValue::Bool(true),
        MAX_STEPS,
    )
    .unwrap();
    assert!(matches!(
        evaluation.step,
        ResumableStep::Completed {
            result: ArgumentValue::Bool(true),
            ..
        }
    ));
    let error = resume_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(41)],
        &state,
        &binding,
        &request,
        &ArgumentValue::Int(42),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F113");
}

#[test]
fn two_yields_replay_in_order_and_complete_through_the_opaque_continuation() {
    let program = resolved(TWO_YIELDS);
    let first =
        run_sequential_resumable_effect(&program, "app.ask", &[ArgumentValue::Int(4)], MAX_STEPS)
            .unwrap();
    let SequentialResumableStep::Suspended {
        continuation: first,
    } = first.step
    else {
        panic!("two-site start did not produce a sequential continuation")
    };
    assert_eq!(first.request(), &ArgumentValue::Int(5));

    let second = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &first,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialResumableStep::Suspended {
        continuation: second,
    } = second.step
    else {
        panic!("first answer did not park the second request")
    };
    assert_eq!(second.request(), &ArgumentValue::Int(12));
    assert_ne!(second.state(), first.state());
    assert_ne!(second.binding(), first.binding());

    let complete = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &second,
        &ArgumentValue::Int(20),
        MAX_STEPS,
    )
    .unwrap();
    assert!(matches!(
        complete.step,
        SequentialResumableStep::Completed {
            result: ArgumentValue::Int(30),
            ..
        }
    ));
}

#[test]
fn sequential_checkpoint_recovers_only_the_exact_plan_arguments_and_history() {
    use super::checkpoint::{decode, encode};

    let program = resolved(TWO_YIELDS);
    let arguments = [ArgumentValue::Int(4)];
    let first = sequential_suspension(&program, &arguments);
    let bytes = encode("app.ask", &first).unwrap();
    let first = decode(&program, "app.ask", &arguments, &bytes).unwrap();
    assert_eq!(first.request(), &ArgumentValue::Int(5));

    let second = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &first,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialResumableStep::Suspended {
        continuation: second,
    } = second.step
    else {
        panic!("recovered first continuation did not reach the second site")
    };
    let bytes = encode("app.ask", &second).unwrap();
    let second = decode(&program, "app.ask", &arguments, &bytes).unwrap();
    assert_eq!(second.request(), &ArgumentValue::Int(12));

    let mut wrong_shape = first.clone();
    wrong_shape.state = second.state.clone();
    assert_eq!(
        decode(
            &program,
            "app.ask",
            &arguments,
            &encode("app.ask", &wrong_shape).unwrap(),
        ),
        Err(super::checkpoint::CheckpointError::SuspensionMismatch)
    );

    let complete = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &second,
        &ArgumentValue::Int(20),
        MAX_STEPS,
    )
    .unwrap();
    assert!(matches!(
        complete.step,
        SequentialResumableStep::Completed {
            result: ArgumentValue::Int(30),
            ..
        }
    ));

    assert!(decode(&program, "app.ask", &[ArgumentValue::Int(5)], &bytes).is_err());
    assert!(decode(&program, "app.other", &arguments, &bytes).is_err());
}

#[test]
fn sequential_checkpoint_keeps_float_bits_and_refuses_hostile_bytes() {
    use super::checkpoint::{decode, encode, CheckpointError};

    let program = resolved(TWO_FLOAT_YIELDS);
    let arguments = [ArgumentValue::Float64(-0.0)];
    let first = sequential_suspension(&program, &arguments);
    let bytes = encode("app.ask", &first).unwrap();
    let recovered = decode(&program, "app.ask", &arguments, &bytes).unwrap();
    assert!(
        matches!(recovered.request(), ArgumentValue::Float64(value) if value.to_bits() == (-0.0f64).to_bits())
    );

    let mut tampered = bytes.clone();
    let digit = tampered
        .iter()
        .position(|byte| *byte == b'0')
        .expect("canonical checkpoint has a digit");
    tampered[digit] = b'1';
    assert_eq!(
        decode(&program, "app.ask", &arguments, &tampered),
        Err(CheckpointError::DigestMismatch)
    );
    assert!(matches!(
        decode(&program, "app.ask", &arguments, &bytes[..bytes.len() - 1]),
        Err(CheckpointError::NonCanonical)
    ));
    let oversized = vec![b' '; 16 * 1024 + 1];
    assert_eq!(
        decode(&program, "app.ask", &arguments, &oversized),
        Err(CheckpointError::TooLarge)
    );

    let noncanonical = [b" ".as_slice(), bytes.as_slice()].concat();
    assert_eq!(
        decode(&program, "app.ask", &arguments, &noncanonical),
        Err(CheckpointError::NonCanonical)
    );
}

#[test]
fn recomputed_checkpoint_digest_cannot_bypass_request_replay() {
    use super::checkpoint::{decode, encode};

    let program = resolved(TWO_YIELDS);
    let arguments = [ArgumentValue::Int(4)];
    let first = sequential_suspension(&program, &arguments);
    let second = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &first,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialResumableStep::Suspended {
        continuation: second,
    } = second.step
    else {
        panic!("expected the second suspension")
    };

    // Model a writer that can deliberately create fresh, internally
    // consistent bytes instead of merely corrupting an existing document.
    let mut forged = second.clone();
    forged.history[0].request = ArgumentValue::Int(999);
    let forged = decode(
        &program,
        "app.ask",
        &arguments,
        &encode("app.ask", &forged).unwrap(),
    )
    .expect("decode is structural and authority-free");
    let diagnostics = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &forged,
        &ArgumentValue::Int(20),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(diagnostics[0].code, REQUEST_DRIFT);

    let mut forged = second;
    forged.request = ArgumentValue::Int(998);
    let forged = decode(
        &program,
        "app.ask",
        &arguments,
        &encode("app.ask", &forged).unwrap(),
    )
    .expect("current request is also a replay-checked proof claim");
    let diagnostics = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &forged,
        &ArgumentValue::Int(20),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(diagnostics[0].code, REQUEST_DRIFT);
}

#[test]
fn sequential_checkpoint_encoding_is_bounded_and_closed() {
    use super::checkpoint::{decode, encode, CheckpointError};

    let program = resolved(TWO_YIELDS);
    let arguments = [ArgumentValue::Int(4)];
    let continuation = sequential_suspension(&program, &arguments);
    assert_eq!(
        encode(&"x".repeat(1025), &continuation),
        Err(CheckpointError::TooLarge)
    );

    let bytes = encode("app.ask", &continuation).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let duplicate = text.replacen(
        "{\"binding\":",
        "{\"schema\":\"semaprax.source-resumable-sequential-checkpoint.v1\",\"binding\":",
        1,
    );
    assert_eq!(
        decode(&program, "app.ask", &arguments, duplicate.as_bytes()),
        Err(CheckpointError::NonCanonical)
    );
}

#[test]
fn legacy_one_site_surface_stays_exhaustive_and_refuses_multi_site_programs() {
    fn exhaustively_match_legacy(step: ResumableStep) {
        match step {
            ResumableStep::Suspended { .. }
            | ResumableStep::Completed { .. }
            | ResumableStep::LanguageFailure(_)
            | ResumableStep::FuelExhausted
            | ResumableStep::CallDepthExceeded
            | ResumableStep::GuardError(_) => {}
        }
    }

    exhaustively_match_legacy(ResumableStep::FuelExhausted);
    let multi = resolved(TWO_YIELDS);
    let error =
        run_resumable_effect(&multi, "app.ask", &[ArgumentValue::Int(4)], MAX_STEPS).unwrap_err();
    assert_eq!(error[0].code, "SPX-F115");
    assert!(error[0].message.contains("explicit sequential API"));

    let single = resolved(ASK);
    let error =
        run_sequential_resumable_effect(&single, "app.ask", &[ArgumentValue::Int(4)], MAX_STEPS)
            .unwrap_err();
    assert_eq!(error[0].code, "SPX-F115");
    assert!(error[0].message.contains("legacy one-site API"));
}

#[test]
fn three_yields_replay_every_request_and_complete_in_authored_order() {
    let program = resolved(THREE_YIELDS);
    let first = sequential_suspension(&program, &[ArgumentValue::Int(4)]);
    assert_eq!(first.request(), &ArgumentValue::Int(4));

    let second = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &first,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialResumableStep::Suspended {
        continuation: second,
    } = second.step
    else {
        panic!("first answer did not reach the second site")
    };
    assert_eq!(second.request(), &ArgumentValue::Int(11));

    let third = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &second,
        &ArgumentValue::Int(20),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialResumableStep::Suspended {
        continuation: third,
    } = third.step
    else {
        panic!("second answer did not reach the third site")
    };
    assert_eq!(third.request(), &ArgumentValue::Int(21));

    let complete = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(4)],
        &third,
        &ArgumentValue::Int(30),
        MAX_STEPS,
    )
    .unwrap();
    assert!(matches!(
        complete.step,
        SequentialResumableStep::Completed {
            result: ArgumentValue::Int(60),
            ..
        }
    ));
}

#[test]
fn sequential_state_history_binding_and_current_answer_type_fail_closed() {
    let program = resolved(TWO_YIELDS);
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.ask")
        .unwrap();
    let plan = lowering::lower_sequential(&program, function).unwrap();
    let arguments = [ArgumentValue::Int(4)];
    let first = sequential_suspension(&program, &arguments);

    let mut wrong_state = first.clone();
    wrong_state.state = plan.complete.id.clone();
    let error = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &wrong_state,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F115");

    let mut wrong_history_length = first.clone();
    wrong_history_length.state = plan.suspensions[1].state.id.clone();
    let error = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &wrong_history_length,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F115");

    let mut wrong_binding = first.clone();
    wrong_binding.binding = plan.suspension_binding(&[ResumableScalar::I64(99)]);
    let error = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &wrong_binding,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F115");

    let error = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &first,
        &ArgumentValue::Bool(true),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F113");
}

#[test]
fn sequential_current_and_historical_request_drift_are_both_refused() {
    let program = resolved(TWO_YIELDS);
    let arguments = [ArgumentValue::Int(4)];
    let first = sequential_suspension(&program, &arguments);

    let mut changed_current = first.clone();
    changed_current.request = ArgumentValue::Int(99);
    let error = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &changed_current,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F114");

    let second = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &first,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialResumableStep::Suspended {
        continuation: mut second,
    } = second.step
    else {
        panic!("first answer did not reach the second site")
    };
    second.history[0].request = ArgumentValue::Int(99);
    let error = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &second,
        &ArgumentValue::Int(20),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F114");
}

#[test]
fn sequential_binding_commits_prior_float_answer_bits() {
    let program = resolved(TWO_FLOAT_YIELDS);
    let arguments = [ArgumentValue::Float64(1.0)];
    let first = sequential_suspension(&program, &arguments);
    let second = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &first,
        &ArgumentValue::Float64(-0.0),
        MAX_STEPS,
    )
    .unwrap();
    let SequentialResumableStep::Suspended {
        continuation: mut second,
    } = second.step
    else {
        panic!("first float answer did not reach the second site")
    };
    second.history[0].answer = ArgumentValue::Float64(0.0);
    let error = resume_sequential_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &second,
        &ArgumentValue::Float64(2.0),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F115");
}

#[test]
fn a_resume_presenting_a_request_the_replayed_prefix_does_not_recompute_is_refused() {
    let program = resolved(ASK);
    let (state, binding, _) = suspension(&program, &[ArgumentValue::Int(41)]);
    // The genuine suspension for seed 41 recorded request 42. Presenting 99
    // is drift: the prefix recomputes 42 and the resume fails closed.
    let refused = resume_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(41)],
        &state,
        &binding,
        &ArgumentValue::Int(99),
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].code, "SPX-F114");
    assert!(refused[0].message.contains("different request"));
}

#[test]
fn a_resume_presenting_a_suspension_under_different_arguments_is_refused_before_replay() {
    let program = resolved(ASK);
    let (state, binding, request) = suspension(&program, &[ArgumentValue::Int(41)]);
    // The binding rejects the other invocation before request replay.
    let refused = resume_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(7)],
        &state,
        &binding,
        &request,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(refused[0].code, "SPX-F115");
}

#[test]
fn an_answer_of_the_wrong_type_is_refused_before_the_program_is_entered() {
    let program = resolved(ASK);
    let (state, binding, request) = suspension(&program, &[ArgumentValue::Int(41)]);
    for wrong in [
        ArgumentValue::Bool(true),
        ArgumentValue::Int32(10),
        ArgumentValue::Usize(10),
        ArgumentValue::BorrowedStr("ten".to_owned()),
    ] {
        let refused = resume_resumable_effect(
            &program,
            "app.ask",
            &[ArgumentValue::Int(41)],
            &state,
            &binding,
            &request,
            &wrong,
            MAX_STEPS,
        )
        .unwrap_err();
        assert_eq!(refused[0].code, "SPX-F113", "{wrong:?}");
        assert!(refused[0].message.contains("answer"), "{wrong:?}");
    }
}

#[test]
fn a_recorded_request_of_the_wrong_type_is_refused_the_same_way() {
    let program = resolved(ASK);
    let (state, binding, _) = suspension(&program, &[ArgumentValue::Int(41)]);
    let refused = resume_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(41)],
        &state,
        &binding,
        &ArgumentValue::Bool(true),
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(refused[0].code, "SPX-F113");
    assert!(refused[0].message.contains("request"));
}

#[test]
fn an_argument_of_the_wrong_type_is_refused_before_anything_runs() {
    let program = resolved(ASK);
    let refused =
        run_resumable_effect(&program, "app.ask", &[ArgumentValue::Bool(true)], MAX_STEPS)
            .unwrap_err();
    assert_eq!(refused[0].code, "SPX-F103");
    let arity = run_resumable_effect(&program, "app.ask", &[], MAX_STEPS).unwrap_err();
    assert_eq!(arity[0].code, "SPX-F103");
    assert!(arity[0].message.contains("1 argument(s)"));
}

#[test]
fn a_function_that_cannot_suspend_is_refused_by_this_lane() {
    let program = resolved(ASK);
    let refused = run_resumable_effect(&program, "app.main", &[], MAX_STEPS).unwrap_err();
    assert_eq!(refused[0].code, "SPX-F102");
    assert!(refused[0].message.contains("declares no `yields` clause"));
    let absent = run_resumable_effect(&program, "app.absent", &[], MAX_STEPS).unwrap_err();
    assert_eq!(absent[0].code, "SPX-F102");
}

#[test]
fn max_steps_is_bounded_and_exhaustion_is_reported_rather_than_looping() {
    let program = resolved(ASK);
    assert_eq!(
        run_resumable_effect(&program, "app.ask", &[ArgumentValue::Int(41)], 0).unwrap_err()[0]
            .code,
        "SPX-F101"
    );
    let exhausted =
        run_resumable_effect(&program, "app.ask", &[ArgumentValue::Int(41)], 2).unwrap();
    assert_eq!(exhausted.step, ResumableStep::FuelExhausted);
}

#[test]
fn every_ordinary_interpreter_lane_still_refuses_a_yield_outright() {
    // `Resumption::Refused` is the default every other evaluator carries, so
    // a `yield` reaching one of them is a guard refusal, never a suspension.
    let mut state = Resumption::Refused;
    let site = ExpressionId::new(
        &hir::FunctionExecutionId::Monomorphic(hir::DeclarationId::new("app.ask")),
        "body.tail",
    );
    let refused = settle_yield(&mut state, &site, Value::Int(1)).unwrap_err();
    let Flow::Guard(detail) = refused else {
        panic!("an ordinary lane must refuse a yield");
    };
    assert_eq!(detail, "`yield` is not yet evaluated by the interpreter");
}

#[test]
fn a_suspension_grants_no_authority_and_dispatches_nothing() {
    // The slice refuses `uses` effects (`SPX-T302`), so a suspending
    // function has no host boundary at all to cross; this pins that the
    // refusal is real rather than merely documented.
    let source = r#"
module test.resumable_effects_effectful;

permit { clock.read }

@id("app.ask")
fn ask(seed: i64) -> i64
    uses { clock.read }
    yields i64 -> i64
{
    let answer = yield seed + 1;
    answer * 2
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program =
        crate::parse(source, Path::new("resumable-effects-effectful.spx")).expect("it parses");
    let refused = hir::resolve(&program).unwrap_err();
    assert!(
        refused.iter().any(|error| error.code == "SPX-T302"),
        "{refused:?}"
    );
}

#[test]
fn the_backends_that_cannot_lower_a_suspension_still_refuse_the_exact_program_this_lane_runs() {
    // Backend equivalence: exactly one engine claims `yield` today, and the
    // two that cannot lower it refuse the very same source with their own
    // stable codes rather than silently differing from the interpreter.
    let program = resolved(ASK);
    assert!(matches!(
        run_resumable_effect(&program, "app.ask", &[ArgumentValue::Int(41)], MAX_STEPS)
            .unwrap()
            .step,
        ResumableStep::Suspended { .. }
    ));
    let native = crate::codegen::emit_hir_c(&program).unwrap_err();
    assert_eq!(native.code, "SPX-B116");
    let wasm = crate::wasm::emit_resolved_module(&program).unwrap_err();
    assert_eq!(wasm.code, "SPX-W126");
}

#[test]
fn a_float_request_replays_by_bits_so_drift_detection_is_exact() {
    let source = r#"
module test.resumable_effects_float;
@id("app.ask")
fn ask(seed: f64) -> f64
    yields f64 -> f64
{
    let answer = yield seed;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = hir::resolve(
        &crate::parse(source, Path::new("resumable-effects-float.spx")).expect("it parses"),
    )
    .expect("it resolves");
    let negative_zero = ArgumentValue::Float64(-0.0);
    let (state, binding, request) = suspension(&program, &[negative_zero.clone()]);
    assert_eq!(request, negative_zero);
    // `-0.0 == 0.0` in IEEE, but they are distinct requests: presenting the
    // positive zero as the recorded request is drift, not a match.
    let refused = resume_resumable_effect(
        &program,
        "app.ask",
        &[negative_zero.clone()],
        &state,
        &binding,
        &ArgumentValue::Float64(0.0),
        &ArgumentValue::Float64(1.0),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(refused[0].code, "SPX-F114");
    // The genuine recorded request replays.
    assert!(matches!(
        resume_resumable_effect(
            &program,
            "app.ask",
            &[negative_zero.clone()],
            &state,
            &binding,
            &negative_zero,
            &ArgumentValue::Float64(1.5),
            MAX_STEPS,
        )
        .unwrap()
        .step,
        ResumableStep::Completed {
            result: ArgumentValue::Float64(1.5),
            ..
        }
    ));
}

#[test]
fn equal_requests_from_different_arguments_cannot_alias_one_suspension() {
    let source = r#"
module test.resumable_effects_argument_binding;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let answer = yield 0;
    answer + seed
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = resolved(source);
    let (state, binding, request) = suspension(&program, &[ArgumentValue::Int(1)]);
    assert_eq!(request, ArgumentValue::Int(0));
    assert_eq!(
        suspension(&program, &[ArgumentValue::Int(2)]).2,
        ArgumentValue::Int(0),
        "negative control: both invocations genuinely request the same value"
    );
    let refused = resume_resumable_effect(
        &program,
        "app.ask",
        &[ArgumentValue::Int(2)],
        &state,
        &binding,
        &request,
        &ArgumentValue::Int(10),
        MAX_STEPS,
    )
    .unwrap_err();
    assert_eq!(refused[0].code, "SPX-F115");
}
