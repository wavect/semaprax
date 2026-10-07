//! Single-file command-line programs on the reference interpreter
//! (`docs/TEXT-TOOLKIT-V1.md`): an ordinary `fn main() -> i64` with argv, two
//! staged output channels, and an optional injected read-only file provider.
//! The provider settles once, on every outcome, before anything is published.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::*;
use crate::filesystem_provider::FileProvider;

/// The settled result of one command-line invocation. Both transcripts are
/// empty unless `main` returned.
pub(crate) struct SourceCommandEvaluation {
    pub(crate) evaluation: ResolvedEvaluation,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

pub(crate) fn evaluate_resolved_source_command(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    arguments: &[String],
    files: Option<&mut dyn FileProvider>,
    max_steps: usize,
) -> Result<SourceCommandEvaluation, Vec<Diagnostic>> {
    hir::validate(program).map_err(|diagnostic| vec![diagnostic])?;
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "hosted evaluation max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        ))]);
    }
    if arguments.len() > crate::command_io_ops::MAX_ARGUMENTS as usize {
        return Err(vec![argument_error(format!(
            "a command-line program accepts at most {} arguments",
            crate::command_io_ops::MAX_ARGUMENTS
        ))]);
    }
    let mut input_bytes = 0usize;
    for argument in arguments {
        if argument.as_bytes().contains(&0) {
            return Err(vec![argument_error(
                "command-line program arguments must not contain NUL bytes".to_owned(),
            )]);
        }
        input_bytes = input_bytes.saturating_add(argument.len());
    }
    if input_bytes > crate::command_io_ops::MAX_INPUT_BYTES as usize {
        return Err(vec![argument_error(format!(
            "command-line program arguments exceed {} bytes",
            crate::command_io_ops::MAX_INPUT_BYTES
        ))]);
    }
    crate::source_command::validate_authority(program).map_err(|diagnostic| vec![diagnostic])?;
    if program.entrypoint.as_str() != entry_id {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("command-line entry `{entry_id}` is not the program entry point"),
        )]);
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
        .filter(|function| resolved_data_signature_is_admitted(function, &program.declarations))
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let entry = admitted.get(entry_id).copied().ok_or_else(|| {
        vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("command-line entry `{entry_id}` is outside the interpreter profile"),
        )]
    })?;
    if !entry.params.is_empty() || entry.return_type != ResolvedType::I64 {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!("command-line entry `{entry_id}` must have type `fn main() -> i64`"),
        )]);
    }
    hir::analyze_byte_data_capacity(program).map_err(|diagnostic| vec![diagnostic])?;
    scan_closure(entry_id, &admitted, program)?;

    let command_input = CommandInputState {
        network: None,
        // A closure, not the bare constructor path: the provider's trait-object
        // lifetime shortens to the evaluator's only at this call.
        #[allow(clippy::redundant_closure)]
        filesystem: files.map(|provider| filesystem::FileState::new(provider)),
        environment: None,
        process: None,
        arguments: arguments
            .iter()
            .map(|value| Arc::<[u8]>::from(value.as_bytes()))
            .collect(),
        stdin: Arc::from([]),
        stdin_consumed: false,
    };
    let mut evaluator = Evaluator {
        admitted: FunctionLookup::Borrowed(&admitted),
        closure_functions: closures::checked_functions(program).map_err(|error| vec![error])?,
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
        resumption: resumable::Resumption::Refused,
        semantic: Default::default(),
    };
    let evaluated = evaluator.call_frame(entry, Vec::new(), 0);
    if let Some(files) = evaluator
        .command_input
        .as_mut()
        .and_then(|input| input.filesystem.take())
    {
        files.settle();
    }
    let outcome = match evaluated {
        Ok(Value::Int(value)) => ResolvedEvaluationOutcome::ReturnedI64(value),
        Ok(_) => ResolvedEvaluationOutcome::GuardError(
            "command-line entry returned a non-i64 value".to_owned(),
        ),
        Err(Flow::Failure(status)) => ResolvedEvaluationOutcome::LanguageFailure(status),
        Err(Flow::Exhausted) => ResolvedEvaluationOutcome::FuelExhausted,
        Err(Flow::DepthExceeded) => ResolvedEvaluationOutcome::CallDepthExceeded,
        Err(Flow::Cancelled { .. }) => ResolvedEvaluationOutcome::GuardError(
            "unexpected cancellation in command-line evaluation".to_owned(),
        ),
        Err(Flow::Utf8MaterializationLimitExceeded { .. }) => {
            ResolvedEvaluationOutcome::GuardError(
                "unexpected UTF-8 materialization limit in command-line evaluation".to_owned(),
            )
        }
        Err(Flow::Guard(detail)) => ResolvedEvaluationOutcome::GuardError(detail.to_owned()),
        Err(Flow::Residual(_)) => {
            ResolvedEvaluationOutcome::GuardError(owned_try::ESCAPED_RESIDUAL_GUARD.to_owned())
        }
    };
    let mut stdout = evaluator.stdout_transcript.take().unwrap_or_default();
    let mut stderr = evaluator.stderr_transcript.take().unwrap_or_default();
    if !matches!(outcome, ResolvedEvaluationOutcome::ReturnedI64(_)) {
        stdout.clear();
        stderr.clear();
    }
    Ok(SourceCommandEvaluation {
        evaluation: ResolvedEvaluation {
            outcome,
            steps_used: evaluator.steps,
            max_steps,
            failure: None,
        },
        stdout,
        stderr,
    })
}
