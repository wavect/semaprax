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

/// The retained Project v25 selector; legacy callers retain their closed map.
#[derive(Clone, Copy)]
pub(crate) enum ResolvedFunctionProfile {
    Legacy,
    StreamText,
}
impl ResolvedFunctionProfile {
    pub(crate) fn for_project(profile: crate::project::ProjectProfile) -> Self {
        if matches!(
            profile,
            crate::project::ProjectProfile::StdinStreamTextCommandIoV1
                | crate::project::ProjectProfile::StdinStreamDataCommandIoV1
        ) {
            Self::StreamText
        } else {
            Self::Legacy
        }
    }
    pub(super) fn admitted(
        self,
        program: &hir::ResolvedProgram,
    ) -> std::collections::BTreeMap<&str, &hir::ResolvedFunction> {
        let mut admitted = admitted_resolved_functions(program);
        if matches!(self, Self::StreamText) {
            admitted.extend(
                program
                    .functions
                    .iter()
                    .filter(|function| {
                        function.effects.is_empty()
                            && (function.return_type == ResolvedType::String
                                || crate::map_ops::is_collection(&function.return_type)
                                || hir::owned_text_record::admitted(
                                    &function.return_type,
                                    &program.declarations,
                                )
                                || function.params.iter().any(|p| {
                                    p.ty == ResolvedType::String
                                        || crate::map_ops::is_collection(&p.ty)
                                        || hir::owned_text_record::admitted(
                                            &p.ty,
                                            &program.declarations,
                                        )
                                }))
                            && hir::stream_text_return_with_index(
                                &function.return_type,
                                &program.declarations,
                            )
                            && (!crate::stdin_stream_ops::is_reader(&function.return_type)
                                || crate::stdin_stream_ops::resolved_forward_signature(function))
                            && function.params.iter().all(|p| {
                                hir::stream_text_parameter_with_index(p, &program.declarations)
                            })
                            && program
                                .declarations
                                .declaration(&function.id)
                                .is_some_and(|d| d.identity_origin == hir::IdentityOrigin::Explicit)
                    })
                    .map(|function| (function.id.as_str(), function)),
            );
        }
        admitted
    }
}

pub(crate) fn evaluate_resolved_profile_i64_entry(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    max_steps: usize,
    profile: ResolvedFunctionProfile,
) -> Result<super::ResolvedEvaluation, Vec<Diagnostic>> {
    let evaluated = evaluate_resolved_i64_function_with_profile(
        program,
        entry_id,
        max_steps,
        true,
        PreparedCancellation::Never,
        profile,
    )?;
    let outcome = match evaluated.outcome {
        PreparedResolvedEvaluationOutcome::ReturnedI64(v) => {
            super::ResolvedEvaluationOutcome::ReturnedI64(v)
        }
        PreparedResolvedEvaluationOutcome::LanguageFailure(s) => {
            super::ResolvedEvaluationOutcome::LanguageFailure(s)
        }
        PreparedResolvedEvaluationOutcome::FuelExhausted => {
            super::ResolvedEvaluationOutcome::FuelExhausted
        }
        PreparedResolvedEvaluationOutcome::CallDepthExceeded => {
            super::ResolvedEvaluationOutcome::CallDepthExceeded
        }
        PreparedResolvedEvaluationOutcome::GuardError(detail) => {
            super::ResolvedEvaluationOutcome::GuardError(detail)
        }
        PreparedResolvedEvaluationOutcome::Cancelled { .. } => {
            return Err(vec![guard_error(
                "unexpected cancellation in profile entry evaluation",
            )])
        }
    };
    Ok(super::ResolvedEvaluation {
        outcome,
        steps_used: evaluated.steps_used,
        max_steps: evaluated.max_steps,
        failure: evaluated.failure,
    })
}

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
    evaluate_resolved_i64_function_with_profile(
        program,
        function_id,
        max_steps,
        entrypoint_only,
        cancellation,
        ResolvedFunctionProfile::Legacy,
    )
}

pub(crate) fn evaluate_resolved_i64_function_with_profile(
    program: &hir::ResolvedProgram,
    function_id: &str,
    max_steps: usize,
    entrypoint_only: bool,
    cancellation: PreparedCancellation<'_>,
    profile: ResolvedFunctionProfile,
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

    let admitted = profile.admitted(program);
    scan_closure(function_id, &admitted, program)?;
    // Even uncalled attached instances must authenticate before execution.
    hir::validate(program).map_err(|error| vec![error])?;

    let closure_functions =
        super::closures::checked_functions(program).map_err(|error| vec![error])?;
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("semaprax-resolved-evaluate".to_owned())
            .stack_size(EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, || {
                let mut evaluator = Evaluator::new_prepared(
                    FunctionLookup::Borrowed(&admitted),
                    closure_functions,
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
                    Err(Flow::Residual(_)) => PreparedResolvedEvaluationOutcome::GuardError(
                        "owned postfix `?` residual escaped its function frame".to_owned(),
                    ),
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

#[cfg(test)]
mod tests {
    use super::*;
    fn string_program(body: &str) -> hir::ResolvedProgram {
        let source = format!("module text.calls; @id(\"text.helper\") fn helper(text:string)->string {{ {body} }} @id(\"app.main\") fn main()->i64 {{ let text=helper(\" ready \"); string_len(text) }}");
        hir::resolve(&crate::parse(&source, "text-calls.spx").unwrap()).unwrap()
    }
    #[test]
    fn stream_text_owned_calls_execute_and_legacy_stays_closed() {
        let program = string_program("string_trim(text)");
        let refused = evaluate_resolved_zero_arg_i64_function(
            &program,
            "app.main",
            1000,
            true,
            PreparedCancellation::Never,
        )
        .unwrap_err();
        assert!(refused[0].message.contains("unsupported_callee"));
        let evaluated = evaluate_resolved_profile_i64_entry(
            &program,
            "app.main",
            1000,
            ResolvedFunctionProfile::StreamText,
        )
        .unwrap();
        assert!(matches!(
            evaluated.outcome,
            super::super::ResolvedEvaluationOutcome::ReturnedI64(5)
        ));
        let prepared = super::super::prepared::prepare_resolved_i64_with_profile(
            &program,
            "app.main",
            ResolvedFunctionProfile::StreamText,
        )
        .unwrap();
        assert!(prepared.function_ids().any(|id| id == "text.helper"));
        let helper_index = program
            .functions
            .iter()
            .position(|function| function.id.as_str() == "text.helper")
            .unwrap();
        let helper = &program.functions[helper_index];
        let mut hostile = program.clone();
        hostile.functions[helper_index].params[0].ownership = hir::OwnershipMode::Borrow;
        assert!(!ResolvedFunctionProfile::StreamText
            .admitted(&hostile)
            .contains_key(helper.id.as_str()));
        hostile = program.clone();
        hostile.functions[helper_index]
            .effects
            .push("fs.read".into());
        assert!(!ResolvedFunctionProfile::StreamText
            .admitted(&hostile)
            .contains_key(helper.id.as_str()));
        assert!(matches!(
            ResolvedFunctionProfile::for_project(
                crate::project::ProjectProfile::StdinStreamCommandIoV2
            ),
            ResolvedFunctionProfile::Legacy
        ));
    }
    #[test]
    fn stream_text_owned_call_failure_keeps_selected_status() {
        let program = string_program("string_slice(text,0,99)");
        let evaluated = evaluate_resolved_profile_i64_entry(
            &program,
            "app.main",
            1000,
            ResolvedFunctionProfile::StreamText,
        )
        .unwrap();
        match evaluated.outcome {
            super::super::ResolvedEvaluationOutcome::LanguageFailure(status) => {
                assert_eq!(status.domain_id(), "semaprax.text.v1");
                assert_eq!(status.code(), 1);
            }
            other => panic!("expected selected Text failure, got {other:?}"),
        }
    }
}
