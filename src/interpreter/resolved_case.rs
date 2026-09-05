//! Zero-argument `i64` evaluation of one named function in a resolved program.
//!
//! This is the shared seam behind the legacy resolved entry evaluator and the
//! Project test-case runner. Both admit exactly the `fn name() -> i64` profile
//! with an explicit stable identity and an admitted transitive closure; the
//! entry evaluator additionally requires the selection to be the program's
//! entrypoint, while a test case is any admitted function the caller names.
//! Every evaluation runs on its own fixed 64 MiB stack and returns the same
//! closed outcome vocabulary as the prepared evaluator, including the retained
//! contract-failure detail.

use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedType};

use super::prepared::{
    PreparedCancellation, PreparedResolvedEvaluation, PreparedResolvedEvaluationOutcome,
};
use super::{
    admitted_resolved_functions, guard_error, option_error, resolved_signature_is_admitted,
    scan_closure, selection_error, Evaluator, Flow, FunctionLookup, Value, EVALUATION_STACK_BYTES,
    MAX_STEPS_LIMIT, REASON_AUTOMATIC_IDENTITY, REASON_UNSUPPORTED_CALLEE,
    REASON_UNSUPPORTED_RESULT_TYPE,
};

/// Evaluate `function_id` as a zero-argument `i64` function of `program`.
///
/// With `entrypoint_only`, the selection must be the resolved entrypoint, which
/// is the legacy `evaluate_resolved_zero_arg_i64` contract. Without it, any
/// function of the program that has an explicit identity, the exact signature,
/// and an admitted closure is evaluated; the caller owns the choice of which
/// functions those are.
pub(crate) fn evaluate_resolved_zero_arg_i64_function(
    program: &hir::ResolvedProgram,
    function_id: &str,
    max_steps: usize,
    entrypoint_only: bool,
    cancellation: PreparedCancellation<'_>,
) -> Result<PreparedResolvedEvaluation, Vec<Diagnostic>> {
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "resolved evaluation max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        ))]);
    }
    if entrypoint_only && program.entrypoint.as_str() != function_id {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!(
                "selection `{function_id}` is not the resolved entry point `{}`",
                program.entrypoint
            ),
        )]);
    }
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == function_id)
        .ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("resolved entry `{function_id}` is absent from the function index"),
            )]
        })?;
    let explicit_entry = program
        .declarations
        .declaration(&entry.id)
        .is_some_and(|declaration| declaration.identity_origin == hir::IdentityOrigin::Explicit);
    if !explicit_entry {
        return Err(vec![selection_error(
            REASON_AUTOMATIC_IDENTITY,
            format!("resolved entry `{function_id}` does not have an explicit stable identity"),
        )]);
    }
    if !entry.params.is_empty() || entry.return_type != ResolvedType::I64 {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!(
                "resolved entry `{function_id}` must have type `fn {}() -> i64`",
                entry.name
            ),
        )]);
    }
    if !resolved_signature_is_admitted(entry, &program.declarations) {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("resolved entry `{function_id}` is outside the interpreter profile"),
        )]);
    }

    let admitted = admitted_resolved_functions(program);
    scan_closure(function_id, &admitted, &program.declarations, true)?;

    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("semaprax-resolved-evaluate".to_owned())
            .stack_size(EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, || {
                let mut evaluator = Evaluator::new_prepared(
                    FunctionLookup::Borrowed(&admitted),
                    &program.declarations,
                    max_steps,
                    0,
                    cancellation,
                );
                let evaluated = evaluator.call_frame(entry, Vec::new(), 0);
                let outcome = match evaluated {
                    Ok(Value::Int(value)) => PreparedResolvedEvaluationOutcome::ReturnedI64(value),
                    Ok(_) => PreparedResolvedEvaluationOutcome::GuardError(
                        "zero-argument i64 entry returned a non-i64 value".to_owned(),
                    ),
                    Err(Flow::Failure(status)) => {
                        PreparedResolvedEvaluationOutcome::LanguageFailure(status)
                    }
                    Err(Flow::Exhausted) => PreparedResolvedEvaluationOutcome::FuelExhausted,
                    Err(Flow::DepthExceeded) => {
                        PreparedResolvedEvaluationOutcome::CallDepthExceeded
                    }
                    Err(Flow::Cancelled { before_step }) => {
                        PreparedResolvedEvaluationOutcome::Cancelled { before_step }
                    }
                    Err(Flow::Utf8MaterializationLimitExceeded { .. }) => {
                        PreparedResolvedEvaluationOutcome::GuardError(
                            "unexpected UTF-8 materialization limit in legacy resolved evaluation"
                                .to_owned(),
                        )
                    }
                    Err(Flow::Guard(detail)) => {
                        PreparedResolvedEvaluationOutcome::GuardError(detail.to_owned())
                    }
                };
                PreparedResolvedEvaluation {
                    outcome,
                    steps_used: evaluator.steps,
                    max_steps,
                    events: Vec::new(),
                    dropped_events: 0,
                    failure: evaluator.failure_detail.take(),
                }
            })
            .map_err(|error| {
                vec![guard_error(&format!(
                    "resolved evaluation thread failed to start: {error}"
                ))]
            })?;
        worker.join().map_err(|_| {
            vec![guard_error(
                "resolved evaluation thread panicked after HIR validation",
            )]
        })
    })
}
