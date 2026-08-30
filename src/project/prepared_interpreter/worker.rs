use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::diagnostic::Diagnostic;
use crate::interpreter::{self, PreparedCancellation, PreparedResolvedEvaluation};

use super::super::{ProjectExecutionRole, ProjectRevision, ProjectSnapshot};
use super::model::{
    prepare_error, request_error, worker_error, PreparedProjectExecution,
    PreparedProjectExecutionOptions, PreparedProjectInterpreterOptions,
    ProjectExecutionCancellation,
};
use super::origin::{prepare_closures, FunctionOrigin, PreparedClosures};

pub(crate) const MAX_PREPARED_PROJECT_INTERPRETER_WORKERS: usize = 8;
pub(crate) static ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS: AtomicUsize = AtomicUsize::new(0);

struct ExecutionRequest {
    role: ProjectExecutionRole,
    options: PreparedProjectExecutionOptions,
    cancellation: Arc<AtomicBool>,
    reply: mpsc::SyncSender<Result<PreparedProjectExecution, Vec<Diagnostic>>>,
}

enum WorkerMessage {
    Execute(ExecutionRequest),
    Shutdown,
}

/// One non-cloneable, sequential prepared evaluator over an immutable Project
/// revision. Creating it owns exactly one local worker thread.
pub struct PreparedProjectInterpreter {
    sender: SyncSender<WorkerMessage>,
    worker: Option<JoinHandle<()>>,
    ceilings: PreparedProjectInterpreterOptions,
    executing: AtomicBool,
    _worker_permit: PreparedWorkerPermit,
}

impl PreparedProjectInterpreter {
    pub fn execute(
        &self,
        role: ProjectExecutionRole,
        options: &PreparedProjectExecutionOptions,
        cancellation: &ProjectExecutionCancellation,
    ) -> Result<PreparedProjectExecution, Vec<Diagnostic>> {
        let _admission = ExecutionAdmission::acquire(&self.executing)?;
        PreparedProjectExecutionOptions::new(
            options.max_steps,
            options.max_trace_bytes,
            options.max_trace_events,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        if options.max_trace_bytes > self.ceilings.max_trace_bytes
            || options.max_trace_events > self.ceilings.max_trace_events
        {
            return Err(vec![request_error(
                "prepared execution exceeds its worker's trace ceilings".to_owned(),
            )]);
        }
        let (reply, response) = mpsc::sync_channel(0);
        self.sender
            .send(WorkerMessage::Execute(ExecutionRequest {
                role,
                options: *options,
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
    pub fn execute_entry(
        &self,
        options: &PreparedProjectExecutionOptions,
        cancellation: &ProjectExecutionCancellation,
    ) -> Result<PreparedProjectExecution, Vec<Diagnostic>> {
        self.execute(ProjectExecutionRole::Entry, options, cancellation)
    }
    pub fn execute_test(
        &self,
        options: &PreparedProjectExecutionOptions,
        cancellation: &ProjectExecutionCancellation,
    ) -> Result<PreparedProjectExecution, Vec<Diagnostic>> {
        self.execute(ProjectExecutionRole::Test, options, cancellation)
    }
}

impl Drop for PreparedProjectInterpreter {
    fn drop(&mut self) {
        let _ = self.sender.send(WorkerMessage::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug)]
pub(crate) struct ExecutionAdmission<'a> {
    executing: &'a AtomicBool,
}
impl<'a> ExecutionAdmission<'a> {
    pub(crate) fn acquire(executing: &'a AtomicBool) -> Result<Self, Vec<Diagnostic>> {
        executing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                vec![worker_error(
                    "prepared interpreter already has one outstanding execution",
                )]
            })?;
        Ok(Self { executing })
    }
}
impl Drop for ExecutionAdmission<'_> {
    fn drop(&mut self) {
        self.executing.store(false, Ordering::Release);
    }
}

#[derive(Debug)]
pub(crate) struct PreparedWorkerPermit {
    active: &'static AtomicUsize,
}
impl PreparedWorkerPermit {
    pub(crate) fn acquire(
        active: &'static AtomicUsize,
        limit: usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut observed = active.load(Ordering::Acquire);
        loop {
            if observed >= limit {
                return Err(vec![prepare_error(&format!(
                    "process-wide prepared interpreter worker bound of {limit} exceeded"
                ))]);
            }
            match active.compare_exchange_weak(
                observed,
                observed + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(Self { active }),
                Err(current) => observed = current,
            }
        }
    }
}
impl Drop for PreparedWorkerPermit {
    fn drop(&mut self) {
        let previous = self.active.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous != 0, "prepared worker accounting underflowed");
    }
}

pub fn prepare_project_interpreter(
    revision: Arc<ProjectRevision>,
    options: PreparedProjectInterpreterOptions,
) -> Result<PreparedProjectInterpreter, Vec<Diagnostic>> {
    PreparedProjectInterpreterOptions::new(options.max_trace_bytes, options.max_trace_events)
        .map_err(|diagnostic| vec![diagnostic])?;
    let worker_permit = PreparedWorkerPermit::acquire(
        &ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS,
        MAX_PREPARED_PROJECT_INTERPRETER_WORKERS,
    )?;
    let closures = prepare_closures(&revision)?;
    let (sender, receiver) = mpsc::sync_channel(0);
    let worker_revision = Arc::clone(&revision);
    let worker = std::thread::Builder::new()
        .name("semaprax-project-prepared".to_owned())
        .stack_size(interpreter::EVALUATION_STACK_BYTES)
        .spawn(move || worker_loop(worker_revision, closures, receiver))
        .map_err(|error| {
            vec![worker_error(&format!(
                "cannot start prepared interpreter worker: {error}"
            ))]
        })?;
    Ok(PreparedProjectInterpreter {
        sender,
        worker: Some(worker),
        ceilings: options,
        executing: AtomicBool::new(false),
        _worker_permit: worker_permit,
    })
}

fn worker_loop(
    revision: Arc<ProjectRevision>,
    closures: PreparedClosures,
    receiver: Receiver<WorkerMessage>,
) {
    while let Ok(message) = receiver.recv() {
        let WorkerMessage::Execute(request) = message else {
            break;
        };
        let evaluated = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            execute_request(&revision, &closures, &request)
        }));
        match evaluated {
            Ok(result) => {
                if request.reply.send(result).is_err() {
                    continue;
                }
            }
            Err(_) => {
                let _ = request.reply.send(Err(vec![worker_error(
                    "prepared interpreter worker panicked and is now terminal",
                )]));
                break;
            }
        }
    }
}

fn execute_request(
    revision: &ProjectRevision,
    closures: &PreparedClosures,
    request: &ExecutionRequest,
) -> Result<PreparedProjectExecution, Vec<Diagnostic>> {
    let (program, prepared) = match request.role {
        ProjectExecutionRole::Entry => (revision.entry_program(), &closures.entry),
        ProjectExecutionRole::Test => (revision.test_program(), &closures.test),
    };
    let evaluated = interpreter::evaluate_prepared_resolved_zero_arg_i64(
        program,
        prepared,
        request.options.max_steps,
        request.options.max_trace_events,
        PreparedCancellation::Atomic(&request.cancellation),
    )?;
    finish_execution(
        revision,
        request.role,
        request.options,
        evaluated,
        &closures.origins,
    )
}

fn finish_execution(
    revision: &ProjectRevision,
    role: ProjectExecutionRole,
    options: PreparedProjectExecutionOptions,
    evaluated: PreparedResolvedEvaluation,
    origins: &std::collections::BTreeMap<String, FunctionOrigin>,
) -> Result<PreparedProjectExecution, Vec<Diagnostic>> {
    let (outcome, trace) = super::trace::render(revision, role, options, evaluated, origins)?;
    Ok(PreparedProjectExecution {
        role,
        steps_used: trace.steps_used(),
        max_steps: options.max_steps,
        outcome,
        trace,
    })
}

impl ProjectRevision {
    pub fn prepare_interpreter(
        self: &Arc<Self>,
        options: PreparedProjectInterpreterOptions,
    ) -> Result<PreparedProjectInterpreter, Vec<Diagnostic>> {
        prepare_project_interpreter(Arc::clone(self), options)
    }
}
impl ProjectSnapshot {
    pub fn prepare_interpreter(
        &self,
        options: PreparedProjectInterpreterOptions,
    ) -> Result<PreparedProjectInterpreter, Vec<Diagnostic>> {
        prepare_project_interpreter(self.retain_revision(), options)
    }
}
