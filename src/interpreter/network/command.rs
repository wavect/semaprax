//! Hosted evaluation of one Bounded Language Network I/O v1 command.
//!
//! Mirrors `evaluate_resolved_language_command`: the same admission, capacity
//! analysis, closure scan, and transcript sealing, plus the injected network
//! provider and the `NetworkV1` operation profile. The provider settles once,
//! on every outcome, before the result is published.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedType};
use crate::interpreter::{
    argument_error, option_error, resolved_data_signature_is_admitted, scan_closure,
    selection_error, CommandEvaluation, CommandEvaluationOutcome, CommandInputState, Evaluator,
    Flow, FunctionLookup, PreparedCancellation, ResolvedTracePhase, Utf8MaterializationBudget,
    Value, MAX_STEPS_LIMIT, REASON_UNSUPPORTED_CALLEE, REASON_UNSUPPORTED_RESULT_TYPE,
};
use crate::network_provider::NetworkProvider;

use super::NetworkState;

/// The closed permit inventory a network command module may declare, in the
/// canonical permit order.
const ADMITTED_EFFECTS: [&str; 11] = [
    crate::network_io_ops::NETWORK_CONNECT_EFFECT,
    crate::network_io_ops::NETWORK_READ_EFFECT,
    crate::network_io_ops::NETWORK_WRITE_EFFECT,
    crate::network_io_ops::NETWORK_TLS_EFFECT,
    crate::network_io_ops::NETWORK_LISTEN_EFFECT,
    crate::network_io_ops::NETWORK_ACCEPT_EFFECT,
    crate::network_io_ops::NETWORK_HTTP_EFFECT,
    crate::command_io_ops::ARGS_READ_EFFECT,
    crate::command_io_ops::STDERR_WRITE_EFFECT,
    crate::command_io_ops::STDIN_READ_EFFECT,
    crate::host_io_ops::STDOUT_WRITE_EFFECT,
];

/// Evaluate one selected zero-argument bool command with network authority
/// supplied by `provider`. Both output channels are published only for a
/// returned bool (including `false`); every other outcome discards them. The
/// provider's connections are released on every outcome.
pub(crate) fn evaluate_resolved_network_command(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    arguments: &[String],
    stdin: &[u8],
    provider: &mut dyn NetworkProvider,
    max_steps: usize,
) -> Result<(CommandEvaluation, Vec<u8>, Vec<u8>), Diagnostic> {
    hir::validate(program)?;
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(option_error(format!(
            "hosted network command max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        )));
    }
    validate_input(arguments, stdin)?;

    if program.permits.is_empty()
        || program
            .permits
            .iter()
            .any(|permit| !ADMITTED_EFFECTS.contains(&permit.as_str()))
    {
        return Err(selection_error(
            REASON_UNSUPPORTED_CALLEE,
            "hosted network command permits must stay within the Language Network I/O v1 \
             inventory"
                .to_owned(),
        ));
    }
    if !program
        .permits
        .iter()
        .any(|permit| crate::network_io_ops::NETWORK_EFFECTS.contains(&permit.as_str()))
    {
        return Err(selection_error(
            REASON_UNSUPPORTED_CALLEE,
            "hosted network command must permit at least one network effect".to_owned(),
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
            format!("hosted network command entry `{entry_id}` is outside the command profile"),
        )
    })?;
    if !entry.params.is_empty() || entry.return_type != ResolvedType::Bool {
        return Err(selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!("hosted network command entry `{entry_id}` must have type `fn () -> bool`"),
        ));
    }
    let profile = if program
        .permits
        .iter()
        .any(|permit| permit == crate::network_io_ops::NETWORK_HTTP_EFFECT)
    {
        crate::command_io_ops::CommandOperationProfile::HttpV1
    } else if program.permits.iter().any(|permit| {
        [
            crate::network_io_ops::NETWORK_TLS_EFFECT,
            crate::network_io_ops::NETWORK_LISTEN_EFFECT,
            crate::network_io_ops::NETWORK_ACCEPT_EFFECT,
        ]
        .contains(&permit.as_str())
    }) {
        crate::command_io_ops::CommandOperationProfile::ServiceV1
    } else {
        crate::command_io_ops::CommandOperationProfile::NetworkV1
    };
    crate::command_io_ops::validate_operation_profile(program, &entry.id, profile)?;
    hir::analyze_byte_data_capacity(program)?;
    scan_closure(entry_id, &admitted, &program.declarations, true).map_err(first_diagnostic)?;

    let command_input = CommandInputState {
        network: Some(NetworkState::new(provider)),
        arguments: arguments
            .iter()
            .map(|value| Arc::<[u8]>::from(value.as_bytes()))
            .collect(),
        stdin: Arc::from(stdin),
        stdin_consumed: false,
    };
    let mut evaluator = Evaluator {
        admitted: FunctionLookup::Borrowed(&admitted),
        declarations: &program.declarations,
        steps: 0,
        budget: max_steps,
        next_byte_allocation: 0,
        allocated_byte_payload: 0,
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
    // Settlement releases every provider connection before anything is
    // published, on every outcome.
    if let Some(network) = evaluator
        .command_input
        .as_mut()
        .and_then(|input| input.network.take())
    {
        network.settle();
    }
    let outcome = match evaluated {
        Ok(Value::Bool(value)) => CommandEvaluationOutcome::ReturnedBool(value),
        Ok(_) => CommandEvaluationOutcome::GuardError(
            "hosted zero-argument bool network command returned a non-bool value".to_owned(),
        ),
        Err(Flow::Failure(status)) => CommandEvaluationOutcome::LanguageFailure(status),
        Err(Flow::Exhausted) => CommandEvaluationOutcome::FuelExhausted,
        Err(Flow::DepthExceeded) => CommandEvaluationOutcome::CallDepthExceeded,
        Err(Flow::Cancelled { .. }) => CommandEvaluationOutcome::GuardError(
            "unexpected cancellation in hosted network command evaluation".to_owned(),
        ),
        Err(Flow::Utf8MaterializationLimitExceeded { .. }) => CommandEvaluationOutcome::GuardError(
            "unexpected UTF-8 materialization limit in hosted network command evaluation"
                .to_owned(),
        ),
        Err(Flow::Guard(detail)) => CommandEvaluationOutcome::GuardError(detail.to_owned()),
    };
    let mut stdout = evaluator.stdout_transcript.take().unwrap_or_default();
    let mut stderr = evaluator.stderr_transcript.take().unwrap_or_default();
    if !matches!(outcome, CommandEvaluationOutcome::ReturnedBool(_)) {
        stdout.clear();
        stderr.clear();
    }
    Ok((
        CommandEvaluation {
            outcome,
            steps_used: evaluator.steps,
            max_steps,
        },
        stdout,
        stderr,
    ))
}

fn validate_input(arguments: &[String], stdin: &[u8]) -> Result<(), Diagnostic> {
    if arguments.len() > crate::command_io_ops::MAX_ARGUMENTS as usize {
        return Err(argument_error(format!(
            "hosted network command accepts at most {} arguments",
            crate::command_io_ops::MAX_ARGUMENTS
        )));
    }
    let mut input_bytes = stdin.len();
    for argument in arguments {
        if argument.as_bytes().contains(&0) {
            return Err(argument_error(
                "hosted network command arguments must not contain NUL bytes".to_owned(),
            ));
        }
        input_bytes = input_bytes.checked_add(argument.len()).ok_or_else(|| {
            argument_error("hosted network command input length overflowed".to_owned())
        })?;
    }
    if input_bytes > crate::command_io_ops::MAX_INPUT_BYTES as usize {
        return Err(argument_error(format!(
            "hosted network command argv plus stdin exceeds {} bytes",
            crate::command_io_ops::MAX_INPUT_BYTES
        )));
    }
    Ok(())
}

/// The seam publishes one diagnostic; the closure scan reports the first
/// offending expression it meets, which is the actionable one.
fn first_diagnostic(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics.into_iter().next().unwrap_or_else(|| {
        selection_error(
            REASON_UNSUPPORTED_CALLEE,
            "hosted network command closure scan failed without detail".to_owned(),
        )
    })
}
