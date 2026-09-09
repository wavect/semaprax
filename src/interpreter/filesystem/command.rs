//! Hosted evaluation of one Bounded Language Filesystem I/O v1 command.
//!
//! Mirrors `evaluate_resolved_language_command`: the same admission, capacity
//! analysis, closure scan, and transcript sealing, plus the injected filesystem
//! provider and the `FilesystemV1` operation profile. The provider settles once,
//! on every outcome, before the result is published.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::diagnostic::Diagnostic;
use crate::filesystem_provider::FileProvider;
use crate::hir::{self, ResolvedType};
use crate::interpreter::{
    option_error, resolved_data_signature_is_admitted, scan_closure, selection_error,
    CommandEvaluation, CommandEvaluationOutcome, CommandInputState, Evaluator, Flow,
    FunctionLookup, PreparedCancellation, ResolvedTracePhase, Utf8MaterializationBudget, Value,
    MAX_STEPS_LIMIT, REASON_UNSUPPORTED_CALLEE, REASON_UNSUPPORTED_RESULT_TYPE,
};

use super::FileState;

/// The closed permit inventory a filesystem command module may declare, in the
/// canonical permit order.
const ADMITTED_EFFECTS: [&str; 2] = [
    crate::filesystem_ops::READ_EFFECT,
    crate::filesystem_ops::WRITE_EFFECT,
];

/// Evaluate one selected zero-argument bool command with filesystem authority
/// supplied by `provider`. Provider invocation state settles on every outcome;
/// only checked successful reads publish owned language values.
pub(crate) fn evaluate_resolved_filesystem_command(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    provider: &mut dyn FileProvider,
    max_steps: usize,
) -> Result<CommandEvaluation, Diagnostic> {
    evaluate_profile(
        program,
        entry_id,
        provider,
        max_steps,
        crate::command_io_ops::CommandOperationProfile::FilesystemV1,
    )
}
pub(crate) fn evaluate_profile(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    provider: &mut dyn FileProvider,
    max_steps: usize,
    profile: crate::command_io_ops::CommandOperationProfile,
) -> Result<CommandEvaluation, Diagnostic> {
    hir::validate(program)?;
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(option_error(format!(
            "hosted filesystem command max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        )));
    }

    if program.permits.is_empty()
        || program
            .permits
            .iter()
            .any(|permit| !ADMITTED_EFFECTS.contains(&permit.as_str()))
    {
        return Err(selection_error(
            REASON_UNSUPPORTED_CALLEE,
            "hosted filesystem command permits must stay within the Language Filesystem I/O v1 \
             inventory"
                .to_owned(),
        ));
    }
    let admitted = program
        .functions
        .iter()
        .filter(|function| {
            program
                .declarations
                .declaration(&function.id)
                .is_some_and(|declaration| {
                    declaration.identity_origin == hir::IdentityOrigin::Explicit
                })
        })
        .filter(|function| {
            resolved_data_signature_is_admitted(function, &program.declarations)
                && function
                    .effects
                    .iter()
                    .all(|effect| ADMITTED_EFFECTS.contains(&effect.as_str()))
        })
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let entry = admitted.get(entry_id).copied().ok_or_else(|| {
        selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("hosted filesystem command entry `{entry_id}` is outside the command profile"),
        )
    })?;
    if !entry.params.is_empty() || entry.return_type != ResolvedType::Bool {
        return Err(selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!("hosted filesystem command entry `{entry_id}` must have type `fn () -> bool`"),
        ));
    }
    crate::command_io_ops::validate_operation_profile(program, &entry.id, profile)?;
    hir::analyze_byte_data_capacity(program)?;
    scan_closure(entry_id, &admitted, program).map_err(first_diagnostic)?;

    let command_input = CommandInputState {
        network: None,
        filesystem: Some(FileState::new(provider)),
        environment: None,
        process: None,
        arguments: Vec::new(),
        stdin: Arc::from([]),
        stdin_consumed: false,
    };
    let mut evaluator = Evaluator {
        admitted: FunctionLookup::Borrowed(&admitted),
        closure_functions: super::super::closures::checked_functions(program)?,
        declarations: &program.declarations,
        steps: 0,
        budget: max_steps,
        next_byte_allocation: 0,
        allocated_byte_payload: 0,
        box_live_allocations: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        utf8_materialization_budget: Utf8MaterializationBudget::UnlimitedLegacy,
        stdout_transcript: Some(Vec::new()),
        stderr_transcript: Some(Vec::new()),
        command_input: Some(command_input),
        cancellation: PreparedCancellation::Never,
        trace_limit: 0,
        trace_events: Vec::new(),
        dropped_trace_events: 0,
        current_function: None,
        trace_identities: BTreeMap::new(),
        trace_phase: ResolvedTracePhase::Body,
        failure_detail: None,
    };
    let evaluated = evaluator.call_frame(entry, Vec::new(), 0);
    // Settlement releases invocation transients before the result is
    // published, on every outcome.
    if let Some(filesystem) = evaluator
        .command_input
        .as_mut()
        .and_then(|input| input.filesystem.take())
    {
        filesystem.settle();
    }
    let outcome = match evaluated {
        Ok(Value::Bool(value)) => CommandEvaluationOutcome::ReturnedBool(value),
        Ok(_) => CommandEvaluationOutcome::GuardError(
            "hosted zero-argument bool filesystem command returned a non-bool value".to_owned(),
        ),
        Err(Flow::Failure(status)) => CommandEvaluationOutcome::LanguageFailure(status),
        Err(Flow::Exhausted) => CommandEvaluationOutcome::FuelExhausted,
        Err(Flow::DepthExceeded) => CommandEvaluationOutcome::CallDepthExceeded,
        Err(Flow::Cancelled { .. }) => CommandEvaluationOutcome::GuardError(
            "unexpected cancellation in hosted filesystem command evaluation".to_owned(),
        ),
        Err(Flow::Utf8MaterializationLimitExceeded { .. }) => CommandEvaluationOutcome::GuardError(
            "unexpected UTF-8 materialization limit in hosted filesystem command evaluation"
                .to_owned(),
        ),
        Err(Flow::Guard(detail)) => CommandEvaluationOutcome::GuardError(detail.to_owned()),
        Err(Flow::Residual(_)) => CommandEvaluationOutcome::GuardError(
            "owned postfix `?` residual escaped its function frame".to_owned(),
        ),
    };
    Ok(CommandEvaluation {
        outcome,
        steps_used: evaluator.steps,
        max_steps,
    })
}

/// The seam publishes one diagnostic; the closure scan reports the first
/// offending expression it meets, which is the actionable one.
fn first_diagnostic(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics.into_iter().next().unwrap_or_else(|| {
        selection_error(
            REASON_UNSUPPORTED_CALLEE,
            "hosted filesystem command closure scan failed without detail".to_owned(),
        )
    })
}
