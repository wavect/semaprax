//! Control-dependent lane: branch and loop suspension, exact site replay,
//! and the dynamic suspension bound.

use super::*;
use std::path::Path;

pub(crate) const CONTROL_SOURCE: &str = r#"
module test.control_interpreter;
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
    let bonus = if total > 10 {
        let extra = yield total;
        extra
    } else {
        0
    };
    total + bonus
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

const STEPS: usize = 100_000;

fn program(source: &str) -> hir::ResolvedProgram {
    hir::resolve(&crate::parse(source, Path::new("control-interpreter.spx")).unwrap()).unwrap()
}

/// Drive to completion answering `request * 10 + 5`; returns the result and
/// the static site index of every suspension.
fn drive(program: &hir::ResolvedProgram, limit: i64) -> (ControlResumableStep, Vec<usize>) {
    let plan = lower_control(
        program,
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap(),
    )
    .unwrap();
    let arguments = [ArgumentValue::Int(limit)];
    let mut step = run_control_resumable_effect(program, "app.ask", &arguments, STEPS)
        .unwrap()
        .step;
    let mut sites = Vec::new();
    while let ControlResumableStep::Suspended { continuation } = &step {
        sites.push(plan.site_of_state(continuation.state()).unwrap());
        let ArgumentValue::Int(request) = continuation.request() else {
            panic!("scalar request")
        };
        let answer = ArgumentValue::Int(request * 10 + 5);
        step = resume_control_resumable_effect(
            program,
            "app.ask",
            &arguments,
            continuation,
            &answer,
            STEPS,
        )
        .unwrap()
        .step;
    }
    (step, sites)
}

fn result(step: &ControlResumableStep) -> Option<&ArgumentValue> {
    match step {
        ControlResumableStep::Completed { result, .. } => Some(result),
        _ => None,
    }
}

#[test]
fn loop_and_branch_suspensions_follow_the_answers() {
    let program = program(CONTROL_SOURCE);
    // Three loop suspensions (answers 5, 15, 25), total 45 > 10, so the
    // branch suspends once more with 45 and receives 455.
    let (step, sites) = drive(&program, 3);
    assert_eq!(result(&step), Some(&ArgumentValue::Int(45 + 455)));
    assert_eq!(sites, [0, 0, 0, 1]);
    // One loop suspension (answer 5) keeps the total at 5: no branch yield.
    let (step, sites) = drive(&program, 1);
    assert_eq!(result(&step), Some(&ArgumentValue::Int(5)));
    assert_eq!(sites, [0]);
    // No iterations and no branch: completes without suspending.
    let (step, sites) = drive(&program, 0);
    assert_eq!(result(&step), Some(&ArgumentValue::Int(0)));
    assert!(sites.is_empty());
}

fn first_two(program: &hir::ResolvedProgram) -> ControlContinuation {
    let arguments = [ArgumentValue::Int(3)];
    let ControlResumableStep::Suspended { continuation } =
        run_control_resumable_effect(program, "app.ask", &arguments, STEPS)
            .unwrap()
            .step
    else {
        panic!("first suspension")
    };
    let ControlResumableStep::Suspended { continuation } = resume_control_resumable_effect(
        program,
        "app.ask",
        &arguments,
        &continuation,
        &ArgumentValue::Int(5),
        STEPS,
    )
    .unwrap()
    .step
    else {
        panic!("second suspension")
    };
    continuation
}

#[test]
fn forged_binding_or_recorded_site_is_refused() {
    let program = program(CONTROL_SOURCE);
    let plan = lower_control(
        &program,
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap(),
    )
    .unwrap();
    let arguments = [ArgumentValue::Int(3)];
    let genuine = first_two(&program);
    assert_eq!(genuine.history().len(), 1);

    let mut forged = genuine.clone();
    forged.binding = plan
        .binding(
            0,
            &[ResumableScalar::I64(4)],
            &[(0, ResumableScalar::I64(5))],
            &[],
        )
        .unwrap();
    let error = resume_control_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &forged,
        &ArgumentValue::Int(15),
        STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, SUSPENSION_MISMATCH);

    // Claim the settled suspension happened at the branch site instead of the
    // loop site, with a binding consistent with that claim: replay reaches
    // the loop site first and refuses.
    let mut steered = genuine.clone();
    steered.history[0].site = plan.sites[1].state.id.clone();
    steered.binding = plan
        .binding(
            0,
            &[ResumableScalar::I64(3)],
            &[(1, ResumableScalar::I64(5))],
            &[],
        )
        .unwrap();
    let error = resume_control_resumable_effect(
        &program,
        "app.ask",
        &arguments,
        &steered,
        &ArgumentValue::Int(15),
        STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, REQUEST_DRIFT);

    // Rebuilding from checkpoint fields re-derives the same continuation.
    let rebuilt = rebuild_control_continuation(
        &program,
        "app.ask",
        &arguments,
        genuine.state().as_str(),
        *genuine.binding().as_bytes(),
        genuine.request().clone(),
        genuine
            .history()
            .iter()
            .map(|record| {
                (
                    record.site().as_str().to_owned(),
                    record.request().clone(),
                    record.answer().clone(),
                )
            })
            .collect(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(rebuilt, genuine);
}

#[test]
fn completion_or_failure_with_unconsumed_replay_history_is_drift() {
    use crate::cleanup_plan::StatusCase;
    use crate::conformance::NormalizedStatus;

    let program = program(CONTROL_SOURCE);
    let plan = lower_control(
        &program,
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap(),
    )
    .unwrap();
    let arguments = [ResumableScalar::I64(3)];
    let replay = |observed| Resumption::Replay {
        expected: vec![Value::Int(0), Value::Int(1)],
        answers: vec![Value::Int(5), Value::Int(15)],
        observed,
        parked: None,
        history: Vec::new(),
        sites: None,
        parked_site: None,
        parked_environment: None,
        carried: std::collections::BTreeMap::new(),
    };
    // One of two expected records consumed: completion and failure are both
    // refused as drift, exactly like a per-site request mismatch.
    for settled in [
        Ok(Value::Int(0)),
        Err(Flow::Failure(NormalizedStatus::arithmetic(
            StatusCase::AddOverflow,
        ))),
    ] {
        let mut resumption = replay(1);
        assert_eq!(
            settle(settled, &mut resumption, &plan, &arguments, Vec::new()),
            ControlResumableStep::GuardError(REQUEST_DRIFT.to_owned()),
        );
    }
    // Fully consumed history still settles honestly.
    let mut resumption = replay(2);
    assert!(matches!(
        settle(
            Ok(Value::Int(0)),
            &mut resumption,
            &plan,
            &arguments,
            Vec::new()
        ),
        ControlResumableStep::Completed { .. }
    ));
    let mut resumption = replay(2);
    assert!(matches!(
        settle(
            Err(Flow::Failure(NormalizedStatus::arithmetic(
                StatusCase::AddOverflow
            ))),
            &mut resumption,
            &plan,
            &arguments,
            Vec::new()
        ),
        ControlResumableStep::LanguageFailure(_)
    ));
}

#[test]
fn loops_are_bounded_by_the_suspension_limit() {
    let program = program(CONTROL_SOURCE);
    let (step, sites) = drive(&program, 40);
    assert_eq!(step, ControlResumableStep::SuspensionBoundExceeded);
    assert_eq!(sites.len(), MAX_CONTROL_SUSPENSIONS);
}

/// Mixed if/else and loop carrying (issue #296): one owned `Bytes` local
/// (`buf`) defined before the `while` loop and carried across its
/// loop-embedded site, and a *second*, unrelated owned `Bytes` local
/// (`extra_buf`) carried across a separate if/else-nested site reached
/// afterward. Both admit (the loop-embedded one because `buf`'s own storage
/// is never touched inside the loop, the if/else one under the existing,
/// unchanged rule), and driving the function through every suspension of
/// both sites to completion, resuming from real, independently constructed
/// continuations exactly as a caller would, produces the same result as a
/// direct (non-suspending) computation of the same arithmetic.
const MIXED_SOURCE: &str = r#"
module test.control_owned_mixed;
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
fn ask(limit: i64, flag: bool) -> i64
    yields i64 -> i64
{
    let buf = make_buf();
    let mut total = 0;
    let mut round = 0;
    while round < limit {
        let answer = yield round;
        total = total + answer;
        round = round + 1;
        round > 0
    }
    let branch_extra = if flag {
        let extra_buf = make_buf();
        let answer2 = yield total;
        let extra = consume(extra_buf);
        answer2 + extra
    } else {
        0
    };
    let used = consume(buf);
    total + used + branch_extra
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn drive_mixed(limit: i64, flag: bool, answers: &[i64]) -> i64 {
    let program = program(MIXED_SOURCE);
    let arguments = [ArgumentValue::Int(limit), ArgumentValue::Bool(flag)];
    let mut step = run_control_resumable_effect(&program, "app.ask", &arguments, STEPS)
        .unwrap()
        .step;
    let mut next_answer = answers.iter();
    while let ControlResumableStep::Suspended { continuation } = &step {
        let answer = ArgumentValue::Int(*next_answer.next().expect("enough answers"));
        step = resume_control_resumable_effect(
            &program,
            "app.ask",
            &arguments,
            continuation,
            &answer,
            STEPS,
        )
        .unwrap()
        .step;
    }
    let ControlResumableStep::Completed { result, .. } = step else {
        panic!("expected completion, got {step:?}")
    };
    let ArgumentValue::Int(result) = result else {
        panic!("scalar result")
    };
    result
}

#[test]
fn a_loop_embedded_and_an_if_else_nested_carried_local_coexist_and_settle_correctly() {
    // Two loop suspensions (answers 5, 15; total = 20), then the branch
    // suspension (answer 100): branch_extra = 100 + consume(extra_buf) =
    // 100 + 100 = 200. used = consume(buf) = 100.
    // Result = total + used + branch_extra = 20 + 100 + 200 = 320.
    assert_eq!(drive_mixed(2, true, &[5, 15, 100]), 320);
    // With the branch untaken, only the loop-embedded carried local is ever
    // exercised: result = total + used + 0 = 20 + 100 = 120.
    assert_eq!(drive_mixed(2, false, &[5, 15]), 120);
}

#[test]
fn each_lane_refuses_the_other_profile() {
    let control = program(CONTROL_SOURCE);
    let error = super::super::run_sequential_resumable_effect(
        &control,
        "app.ask",
        &[ArgumentValue::Int(1)],
        STEPS,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-H006");
    let sequential = program(
        "module test.sequential;\n@id(\"app.ask\")\nfn ask(seed: i64) -> i64 yields i64 -> i64 {\n    let first = yield seed;\n    yield first\n}\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let error =
        run_control_resumable_effect(&sequential, "app.ask", &[ArgumentValue::Int(1)], STEPS)
            .unwrap_err();
    assert_eq!(error[0].code, "SPX-H006");
}

/// Bug #296 (R20): a purely scalar control-dependent function (no owned
/// `Bytes` local, an empty cleanup plan) reaching a top-level `yield` only
/// past a preceding statement that itself branches (`first`'s own
/// `if`/`else`, each arm holding its own `yield`) must run to completion
/// exactly like any other admitted control-dependent plan, carrying nothing
/// (`resumable_effects::lowering::control::carried_locals_at` is only ever
/// asked about owned `Bytes` locals, and there are none here). Both branches
/// are driven: the request each records, and the final scalar result, must
/// be exactly the values this replay computes, not merely "some" result.
const JOIN_SOURCE: &str = r#"
module test.control_interpreter_join;
@id("app.ask_join")
fn ask_join(seed: i64) -> i64
    yields i64 -> i64
{
    let first = if seed > 0 {
        let a = yield seed;
        a
    } else {
        let b = yield 0;
        b
    };
    let second = yield first + 1;
    second
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn drive_join(seed: i64, answers: &[i64]) -> (ArgumentValue, Vec<ArgumentValue>) {
    let program = program(JOIN_SOURCE);
    let arguments = [ArgumentValue::Int(seed)];
    let mut step = run_control_resumable_effect(&program, "app.ask_join", &arguments, STEPS)
        .unwrap()
        .step;
    let mut requests = Vec::new();
    let mut next_answer = answers.iter();
    while let ControlResumableStep::Suspended { continuation } = &step {
        requests.push(continuation.request().clone());
        let answer = ArgumentValue::Int(*next_answer.next().expect("enough answers"));
        step = resume_control_resumable_effect(
            &program,
            "app.ask_join",
            &arguments,
            continuation,
            &answer,
            STEPS,
        )
        .unwrap()
        .step;
    }
    let ControlResumableStep::Completed { result, .. } = step else {
        panic!("expected completion, got {step:?}")
    };
    (result, requests)
}

#[test]
fn a_top_level_yield_past_a_scalar_branching_predecessor_runs_to_completion() {
    // `seed = 5` takes the `then` branch: first suspension requests `seed`
    // (5), answered 100 so `first = 100`; the top-level site then requests
    // `first + 1` (101), answered 7 so the final result is `second = 7`.
    let (result, requests) = drive_join(5, &[100, 7]);
    assert_eq!(requests, [ArgumentValue::Int(5), ArgumentValue::Int(101)]);
    assert_eq!(result, ArgumentValue::Int(7));

    // `seed = -1` takes the `else` branch instead: first suspension requests
    // the literal `0`, answered 50 so `first = 50`; the same top-level site
    // then requests `51`, answered 9 for a final result of `9`. The branch
    // taken genuinely differs at runtime while the once-refused top-level
    // site after it still replays correctly either way.
    let (result, requests) = drive_join(-1, &[50, 9]);
    assert_eq!(requests, [ArgumentValue::Int(0), ArgumentValue::Int(51)]);
    assert_eq!(result, ArgumentValue::Int(9));
}
