//! Hosted evaluation over one caller-supplied immutable environment snapshot.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::diagnostic::Diagnostic;
use crate::environment_snapshot::EnvironmentSnapshot;
use crate::hir::{
    self, ResolvedHostCommandCall, ResolvedHostCommandOperation as Operation, ResolvedType,
};

use super::{
    argument_error, option_error, resolved_data_signature_is_admitted, scan_closure,
    selection_error, CommandEvaluation, CommandEvaluationOutcome, CommandInputState, Evaluator,
    Flow, FunctionLookup, PreparedCancellation, ResolvedTracePhase, Utf8MaterializationBudget,
    Value, MAX_STEPS_LIMIT, REASON_UNSUPPORTED_CALLEE, REASON_UNSUPPORTED_RESULT_TYPE,
};

const ADMITTED_EFFECTS: [&str; 5] = [
    crate::command_io_ops::ARGS_READ_EFFECT,
    crate::environment_ops::EFFECT,
    crate::command_io_ops::STDIN_READ_EFFECT,
    crate::host_io_ops::STDOUT_WRITE_EFFECT,
    crate::command_io_ops::STDERR_WRITE_EFFECT,
];

pub(super) struct EnvironmentState {
    snapshot: Option<EnvironmentSnapshot>,
}

impl EnvironmentState {
    pub(super) fn new(snapshot: Option<EnvironmentSnapshot>) -> Self {
        Self { snapshot }
    }

    fn len(&self) -> Result<usize, Flow> {
        let snapshot = self.snapshot.as_ref().ok_or_else(authority_denied)?;
        if snapshot.len() > crate::environment_ops::MAX_ENTRIES as usize
            || snapshot.byte_len() > crate::environment_ops::MAX_INPUT_BYTES as usize
        {
            return Err(capacity_exceeded());
        }
        for index in 0..snapshot.len() {
            let (name, value) = snapshot.raw_entry(index).ok_or_else(invalid_input)?;
            if name.is_empty()
                || name.contains(&b'=')
                || name.contains(&0)
                || value.contains(&0)
                || std::str::from_utf8(&name).is_err()
                || std::str::from_utf8(&value).is_err()
            {
                return Err(invalid_input());
            }
        }
        Ok(snapshot.len())
    }

    fn entry(&self, index: usize, value: bool) -> Result<Arc<[u8]>, Flow> {
        self.len()?;
        let snapshot = self
            .snapshot
            .as_ref()
            .expect("environment authority was checked");
        let (name, entry_value) = snapshot.raw_entry(index).ok_or_else(index_out_of_bounds)?;
        Ok(if value { entry_value } else { name })
    }
}

fn failure(code: u32) -> Flow {
    Flow::Failure(
        crate::conformance::NormalizedStatus::try_new(
            crate::environment_ops::STATUS_DOMAIN,
            code,
            crate::conformance::StatusClass::Adapter,
            crate::conformance::Retryability::Known(false),
        )
        .expect("closed environment status table is valid"),
    )
}
fn authority_denied() -> Flow {
    failure(crate::environment_ops::AUTHORITY_DENIED)
}
fn index_out_of_bounds() -> Flow {
    failure(crate::environment_ops::INDEX_OUT_OF_BOUNDS)
}
fn invalid_input() -> Flow {
    failure(crate::environment_ops::INVALID_INPUT)
}
fn capacity_exceeded() -> Flow {
    failure(crate::environment_ops::CAPACITY_EXCEEDED)
}

pub(crate) fn evaluate_resolved_environment_command(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    arguments: &[String],
    stdin: &[u8],
    snapshot: Option<EnvironmentSnapshot>,
    max_steps: usize,
) -> Result<(CommandEvaluation, Vec<u8>, Vec<u8>), Vec<Diagnostic>> {
    evaluate_profile(
        program,
        entry_id,
        arguments,
        stdin,
        snapshot,
        None,
        max_steps,
        crate::command_io_ops::CommandOperationProfile::EnvironmentV1,
    )
}

pub(crate) fn evaluate_profile(
    program: &hir::ResolvedProgram,
    entry_id: &str,
    arguments: &[String],
    stdin: &[u8],
    snapshot: Option<EnvironmentSnapshot>,
    mut process_provider: Option<&mut dyn crate::process_provider::ProcessProvider>,
    max_steps: usize,
    profile: crate::command_io_ops::CommandOperationProfile,
) -> Result<(CommandEvaluation, Vec<u8>, Vec<u8>), Vec<Diagnostic>> {
    let process = profile == crate::command_io_ops::CommandOperationProfile::ProcessV1;
    let admitted_effects: &[&str] = if process {
        &super::process::ADMITTED_EFFECTS
    } else {
        &ADMITTED_EFFECTS
    };
    let required_effect = if process {
        crate::process_ops::EFFECT
    } else {
        crate::environment_ops::EFFECT
    };
    hir::validate(program).map_err(|diagnostic| vec![diagnostic])?;
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "hosted environment command max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        ))]);
    }
    let input_bytes = validate_input(arguments, stdin, snapshot.as_ref())?;
    if input_bytes > crate::environment_ops::MAX_INPUT_BYTES as usize {
        return Err(vec![argument_error(format!(
            "hosted environment command argv plus stdin plus environment exceeds {} bytes",
            crate::environment_ops::MAX_INPUT_BYTES
        ))]);
    }
    if program.permits.is_empty()
        || program
            .permits
            .iter()
            .any(|permit| !admitted_effects.contains(&permit.as_str()))
        || !program
            .permits
            .iter()
            .any(|permit| permit == required_effect)
    {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            "hosted environment command permits must include process.environment.read and stay within Environment I/O v1".to_owned(),
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
        .filter(|function| {
            resolved_data_signature_is_admitted(function, &program.declarations)
                && function
                    .effects
                    .iter()
                    .all(|effect| admitted_effects.contains(&effect.as_str()))
        })
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let entry = admitted.get(entry_id).copied().ok_or_else(|| {
        vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!("hosted environment command entry `{entry_id}` is outside the command profile"),
        )]
    })?;
    if !entry.params.is_empty() || entry.return_type != ResolvedType::Bool {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!("hosted environment command entry `{entry_id}` must have type `fn () -> bool`"),
        )]);
    }
    crate::command_io_ops::validate_operation_profile(program, &entry.id, profile)
        .map_err(|diagnostic| vec![diagnostic])?;
    hir::analyze_byte_data_capacity(program).map_err(|diagnostic| vec![diagnostic])?;
    scan_closure(entry_id, &admitted, program)?;
    let command_input = CommandInputState {
        network: None,
        filesystem: None,
        environment: Some(EnvironmentState::new(snapshot)),
        process: process_provider
            .as_mut()
            .map(|provider| super::process::ProcessState::new(&mut **provider)),
        arguments: arguments
            .iter()
            .map(|value| Arc::<[u8]>::from(value.as_bytes()))
            .collect(),
        stdin: Arc::from(stdin),
        stdin_consumed: false,
    };
    let mut evaluator = Evaluator {
        admitted: FunctionLookup::Borrowed(&admitted),
        closure_functions: super::closures::checked_functions(program)
            .map_err(|error| vec![error])?,
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
    let mut evaluated = evaluator.call_frame(entry, Vec::new(), 0);
    if let Some(process) = evaluator
        .command_input
        .as_mut()
        .and_then(|state| state.process.take())
    {
        if let Err(failure) = process.settle() {
            if evaluated.is_ok() {
                evaluated = Err(super::process::failure(failure));
            }
        }
    }
    let outcome = match evaluated {
        Ok(Value::Bool(value)) => CommandEvaluationOutcome::ReturnedBool(value),
        Ok(_) => CommandEvaluationOutcome::GuardError(
            "hosted zero-argument bool environment command returned a non-bool value".to_owned(),
        ),
        Err(Flow::Failure(status)) => CommandEvaluationOutcome::LanguageFailure(status),
        Err(Flow::Exhausted) => CommandEvaluationOutcome::FuelExhausted,
        Err(Flow::DepthExceeded) => CommandEvaluationOutcome::CallDepthExceeded,
        Err(Flow::Cancelled { .. }) => CommandEvaluationOutcome::GuardError(
            "unexpected cancellation in hosted environment command evaluation".to_owned(),
        ),
        Err(Flow::Utf8MaterializationLimitExceeded { .. }) => CommandEvaluationOutcome::GuardError(
            "unexpected UTF-8 materialization limit in hosted environment command evaluation"
                .to_owned(),
        ),
        Err(Flow::Guard(detail)) => CommandEvaluationOutcome::GuardError(detail.to_owned()),
        Err(Flow::Residual(_)) => CommandEvaluationOutcome::GuardError(
            "owned postfix `?` residual escaped its function frame".to_owned(),
        ),
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

fn validate_input(
    arguments: &[String],
    stdin: &[u8],
    snapshot: Option<&EnvironmentSnapshot>,
) -> Result<usize, Vec<Diagnostic>> {
    if arguments.len() > crate::command_io_ops::MAX_ARGUMENTS as usize {
        return Err(vec![argument_error(format!(
            "hosted environment command accepts at most {} arguments",
            crate::command_io_ops::MAX_ARGUMENTS
        ))]);
    }
    let mut total = stdin.len();
    for argument in arguments {
        if argument.as_bytes().contains(&0) {
            return Err(vec![argument_error(
                "hosted environment command arguments must not contain NUL bytes".to_owned(),
            )]);
        }
        total = total.checked_add(argument.len()).ok_or_else(|| {
            vec![argument_error(
                "hosted environment command input length overflowed".to_owned(),
            )]
        })?;
    }
    if let Some(snapshot) = snapshot {
        total = total.checked_add(snapshot.byte_len()).ok_or_else(|| {
            vec![argument_error(
                "hosted environment command input length overflowed".to_owned(),
            )]
        })?;
    }
    Ok(total)
}

impl Evaluator<'_> {
    pub(super) fn evaluate_environment_operation(
        &mut self,
        call: &ResolvedHostCommandCall,
        environment: &mut super::Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        if crate::process_ops::is_process(call.operation) {
            return self.evaluate_process_operation(call, environment, depth);
        }
        if call.args.len() != crate::environment_ops::arity(call.operation) {
            return Err(Flow::Guard("invalid environment operation arity"));
        }
        let index = if crate::environment_ops::is_lookup(call.operation) {
            match self.evaluate(&call.args[0], environment, depth)? {
                Value::Usize(value) => usize::try_from(value).map_err(|_| index_out_of_bounds())?,
                _ => return Err(Flow::Guard("ill-typed environment index")),
            }
        } else {
            0
        };
        let state = self
            .command_input
            .as_ref()
            .and_then(|input| input.environment.as_ref())
            .ok_or_else(authority_denied)?;
        match call.operation {
            Operation::EnvLen => Ok(Value::Usize(state.len()? as u64)),
            Operation::EnvNameUtf8 | Operation::EnvValueUtf8 => {
                Ok(Value::BorrowedStr(super::BorrowedStrValue {
                    invocation_root: crate::hir::ValueId::intrinsic_parameter(
                        crate::environment_ops::ARENA_ID,
                        usize::MAX,
                    ),
                    bytes: state.entry(index, call.operation == Operation::EnvValueUtf8)?,
                }))
            }
            _ => Err(Flow::Guard("unknown environment operation")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_is_charged_once_alongside_argv_and_stdin() {
        let snapshot =
            EnvironmentSnapshot::from_entries(vec![("A".to_owned(), "b".to_owned())]).unwrap();
        let args = vec!["x".repeat(31)];
        let stdin = vec![0; 65_503];
        assert_eq!(
            validate_input(&args, &stdin, Some(&snapshot)).unwrap(),
            65_536
        );
        assert_eq!(validate_input(&args, &stdin, None).unwrap(), 65_534);
        let state = EnvironmentState::new(Some(snapshot.clone()));
        for _ in 0..300 {
            assert_eq!(state.entry(0, true).unwrap().as_ref(), b"b");
        }
        assert_eq!(
            validate_input(&args, &stdin, Some(&snapshot)).unwrap(),
            65_536
        );
        assert_eq!(
            validate_input(&args, &[0; 65_504], Some(&snapshot)).unwrap(),
            65_537
        );
        assert!(validate_input(&vec![String::new(); 17], &[], None).is_err());
        assert!(validate_input(&["nul\0".to_owned()], &[], None).is_err());
    }

    #[test]
    fn absent_snapshot_fails_with_the_environment_authority_status() {
        let state = EnvironmentState::new(None);
        let Err(Flow::Failure(status)) = state.len() else {
            panic!("missing environment authority must fail");
        };
        assert_eq!(status.domain_id(), crate::environment_ops::STATUS_DOMAIN);
        assert_eq!(status.code(), crate::environment_ops::AUTHORITY_DENIED);
    }

    #[test]
    fn entries_keep_one_immutable_arc_root_per_snapshot_value() {
        let snapshot = EnvironmentSnapshot::from_entries(vec![
            ("B".to_owned(), "two".to_owned()),
            ("A".to_owned(), "one".to_owned()),
        ])
        .unwrap();
        let state = EnvironmentState::new(Some(snapshot));
        assert_eq!(state.len().unwrap(), 2);
        let first = state.entry(0, false).unwrap();
        let again = state.entry(0, false).unwrap();
        assert_eq!(first.as_ref(), b"A");
        assert!(Arc::ptr_eq(&first, &again));
        assert_eq!(state.entry(0, true).unwrap().as_ref(), b"one");
        let Err(Flow::Failure(status)) = state.entry(2, false) else {
            panic!("out-of-range environment lookup must fail");
        };
        assert_eq!(status.code(), crate::environment_ops::INDEX_OUT_OF_BOUNDS);
    }
}

pub(super) fn handles(operation: Operation) -> bool {
    crate::environment_ops::is_environment(operation) || crate::process_ops::is_process(operation)
}
