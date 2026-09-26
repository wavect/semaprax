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
    )
    .unwrap();
    assert_eq!(rebuilt, genuine);
}

#[test]
fn loops_are_bounded_by_the_suspension_limit() {
    let program = program(CONTROL_SOURCE);
    let (step, sites) = drive(&program, 40);
    assert_eq!(step, ControlResumableStep::SuspensionBoundExceeded);
    assert_eq!(sites.len(), MAX_CONTROL_SUSPENSIONS);
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
