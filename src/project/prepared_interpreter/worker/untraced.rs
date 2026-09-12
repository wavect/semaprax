//! Explicit no-trace requests on the existing sequential prepared worker.

use super::super::ProjectPreparedExecutionOutcome;
use super::*;

/// Outcome and fuel facts only. This operation constructs no source trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UntracedPreparedProjectExecution {
    role: ProjectExecutionRole,
    outcome: ProjectPreparedExecutionOutcome,
    steps_used: usize,
    max_steps: usize,
}

impl UntracedPreparedProjectExecution {
    pub const fn role(&self) -> ProjectExecutionRole {
        self.role
    }
    pub const fn outcome(&self) -> &ProjectPreparedExecutionOutcome {
        &self.outcome
    }
    pub const fn steps_used(&self) -> usize {
        self.steps_used
    }
    pub const fn max_steps(&self) -> usize {
        self.max_steps
    }
}

pub(super) struct Request {
    role: ProjectExecutionRole,
    max_steps: usize,
    cancellation: Arc<AtomicBool>,
    reply: SyncSender<Result<UntracedPreparedProjectExecution, Vec<Diagnostic>>>,
}

impl PreparedProjectInterpreter {
    /// Execute without collecting expression events or rendering evidence.
    /// Admission, worker ownership, evaluation and cancellation are unchanged.
    pub fn execute_untraced(
        &self,
        role: ProjectExecutionRole,
        max_steps: usize,
        cancellation: &ProjectExecutionCancellation,
    ) -> Result<UntracedPreparedProjectExecution, Vec<Diagnostic>> {
        let _admission = ExecutionAdmission::acquire(&self.executing)?;
        if !(1..=interpreter::MAX_STEPS_LIMIT).contains(&max_steps) {
            return Err(vec![request_error(format!(
                "prepared max_steps must be between 1 and {}",
                interpreter::MAX_STEPS_LIMIT
            ))]);
        }
        let (reply, response) = mpsc::sync_channel(0);
        self.sender
            .send(WorkerMessage::ExecuteUntraced(Request {
                role,
                max_steps,
                cancellation: Arc::clone(&cancellation.cancelled),
                reply,
            }))
            .map_err(|_| vec![worker_error("prepared interpreter worker is closed")])?;
        response.recv().map_err(|_| {
            vec![worker_error(
                "prepared interpreter worker terminated without a response",
            )]
        })?
    }

    pub fn execute_entry_untraced(
        &self,
        max_steps: usize,
        cancellation: &ProjectExecutionCancellation,
    ) -> Result<UntracedPreparedProjectExecution, Vec<Diagnostic>> {
        self.execute_untraced(ProjectExecutionRole::Entry, max_steps, cancellation)
    }

    pub fn execute_test_untraced(
        &self,
        max_steps: usize,
        cancellation: &ProjectExecutionCancellation,
    ) -> Result<UntracedPreparedProjectExecution, Vec<Diagnostic>> {
        self.execute_untraced(ProjectExecutionRole::Test, max_steps, cancellation)
    }
}

pub(super) fn process(state: &WorkerState, request: Request) -> bool {
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| evaluate(state, &request)));
    match result {
        Ok(result) => {
            let _ = request.reply.send(result);
            true
        }
        Err(_) => {
            let _ = request.reply.send(Err(vec![worker_error(
                "prepared interpreter worker panicked and is now terminal",
            )]));
            false
        }
    }
}

fn evaluate(
    state: &WorkerState,
    request: &Request,
) -> Result<UntracedPreparedProjectExecution, Vec<Diagnostic>> {
    let (program, prepared) = match request.role {
        ProjectExecutionRole::Entry => (state.revision.entry_program(), &state.closures.entry),
        ProjectExecutionRole::Test => (state.revision.test_program(), &state.closures.test),
    };
    // Zero selects the existing evaluator's no-trace branch: no event storage,
    // identity interning, or per-call trace-context writes. Admission and fuel
    // checks are identical to traced evaluation. No renderer is called here.
    let evaluated = interpreter::evaluate_prepared_resolved_zero_arg_i64(
        program,
        prepared,
        request.max_steps,
        0,
        PreparedCancellation::Atomic(&request.cancellation),
    )?;
    debug_assert!(evaluated.events.is_empty());
    debug_assert_eq!(evaluated.dropped_events, 0);
    Ok(UntracedPreparedProjectExecution {
        role: request.role,
        outcome: super::super::trace::map_outcome(evaluated.outcome)?,
        steps_used: evaluated.steps_used,
        max_steps: evaluated.max_steps,
    })
}
