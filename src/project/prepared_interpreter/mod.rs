//! One retained, authority-neutral Project interpreter worker.
//!
//! Preparation validates the exact entry and test closures once and starts one
//! sequential fixed-stack worker. HIR cannot access the filesystem, process,
//! clock, network, backend, or publication authority through this surface.

#[cfg(test)]
mod tests;
mod trace;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::diagnostic::Diagnostic;
use crate::interpreter::{
    self, PreparedCancellation, PreparedResolvedEvaluation, PreparedResolvedI64,
};

use super::{ProjectExecutionRole, ProjectRevision, ProjectSnapshot};

pub use trace::{
    verify_project_source_trace, verify_project_source_trace_against_revision,
    ProjectPreparedExecutionOutcome, ProjectSourceTrace, ProjectSourceTraceEvent,
    PROJECT_SOURCE_TRACE_SCHEMA,
};

pub const MIN_PROJECT_SOURCE_TRACE_BYTES: usize = 64 * 1024;
pub const MAX_PROJECT_SOURCE_TRACE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PROJECT_SOURCE_TRACE_EVENTS: usize = 65_536;
pub const DEFAULT_PROJECT_SOURCE_TRACE_BYTES: usize = 1024 * 1024;
pub const DEFAULT_PROJECT_SOURCE_TRACE_EVENTS: usize = 4096;
/// Process-wide ceiling for retained 64 MiB prepared-interpreter workers.
pub(crate) const MAX_PREPARED_PROJECT_INTERPRETER_WORKERS: usize = 8;

static ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS: AtomicUsize = AtomicUsize::new(0);

/// Monotonic cooperative cancellation for one prepared execution request.
#[derive(Clone, Default)]
pub struct ProjectExecutionCancellation {
    cancelled: Arc<AtomicBool>,
}

impl ProjectExecutionCancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Per-worker ceilings. Every request is independently checked against them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedProjectInterpreterOptions {
    pub max_trace_bytes: usize,
    pub max_trace_events: usize,
}

impl PreparedProjectInterpreterOptions {
    pub fn new(max_trace_bytes: usize, max_trace_events: usize) -> Result<Self, Diagnostic> {
        validate_trace_limits(max_trace_bytes, max_trace_events, "prepared interpreter")?;
        Ok(Self {
            max_trace_bytes,
            max_trace_events,
        })
    }
}

impl Default for PreparedProjectInterpreterOptions {
    fn default() -> Self {
        Self {
            max_trace_bytes: DEFAULT_PROJECT_SOURCE_TRACE_BYTES,
            max_trace_events: DEFAULT_PROJECT_SOURCE_TRACE_EVENTS,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedProjectExecutionOptions {
    pub max_steps: usize,
    pub max_trace_bytes: usize,
    pub max_trace_events: usize,
}

impl PreparedProjectExecutionOptions {
    pub fn new(
        max_steps: usize,
        max_trace_bytes: usize,
        max_trace_events: usize,
    ) -> Result<Self, Diagnostic> {
        if !(1..=interpreter::MAX_STEPS_LIMIT).contains(&max_steps) {
            return Err(request_error(format!(
                "prepared max_steps must be between 1 and {}",
                interpreter::MAX_STEPS_LIMIT
            )));
        }
        validate_trace_limits(max_trace_bytes, max_trace_events, "prepared execution")?;
        Ok(Self {
            max_steps,
            max_trace_bytes,
            max_trace_events,
        })
    }
}

impl Default for PreparedProjectExecutionOptions {
    fn default() -> Self {
        Self {
            max_steps: interpreter::DEFAULT_MAX_STEPS,
            max_trace_bytes: DEFAULT_PROJECT_SOURCE_TRACE_BYTES,
            max_trace_events: DEFAULT_PROJECT_SOURCE_TRACE_EVENTS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedProjectExecution {
    role: ProjectExecutionRole,
    outcome: ProjectPreparedExecutionOutcome,
    steps_used: usize,
    max_steps: usize,
    trace: ProjectSourceTrace,
}

impl PreparedProjectExecution {
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

    pub const fn trace(&self) -> &ProjectSourceTrace {
        &self.trace
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FunctionOrigin {
    pub(super) path: String,
    pub(super) source_revision: String,
    pub(super) source_digest: String,
    pub(super) source_bytes: usize,
}

struct PreparedClosures {
    entry: PreparedResolvedI64,
    test: PreparedResolvedI64,
    origins: BTreeMap<String, FunctionOrigin>,
}

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
struct ExecutionAdmission<'a> {
    executing: &'a AtomicBool,
}

impl<'a> ExecutionAdmission<'a> {
    fn acquire(executing: &'a AtomicBool) -> Result<Self, Vec<Diagnostic>> {
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
struct PreparedWorkerPermit {
    active: &'static AtomicUsize,
}

impl PreparedWorkerPermit {
    fn acquire(active: &'static AtomicUsize, limit: usize) -> Result<Self, Vec<Diagnostic>> {
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
    let entry = interpreter::prepare_resolved_zero_arg_i64(
        revision.entry_program(),
        revision.entry_program().entrypoint.as_str(),
    )
    .map_err(preparation_diagnostics)?;
    let test = interpreter::prepare_resolved_zero_arg_i64(
        revision.test_program(),
        revision.test_program().entrypoint.as_str(),
    )
    .map_err(preparation_diagnostics)?;
    let nodes = entry
        .origin_nodes()
        .checked_add(test.origin_nodes())
        .ok_or_else(|| vec![prepare_error("prepared node accounting overflowed")])?;
    let mut bytes = entry
        .index_bytes()
        .checked_add(test.index_bytes())
        .ok_or_else(|| vec![prepare_error("prepared index accounting overflowed")])?;
    if nodes > interpreter::MAX_PREPARED_ORIGIN_NODES {
        return Err(vec![prepare_error(
            "combined entry/test origin-node bound exceeded",
        )]);
    }
    let mut origins: BTreeMap<String, FunctionOrigin> = BTreeMap::new();
    for id in entry.function_ids().chain(test.function_ids()) {
        let semantic = revision.semantic.rename_function(id).ok_or_else(|| {
            vec![prepare_error(
                "prepared function has no Phase-A source identity",
            )]
        })?;
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == semantic.path)
            .ok_or_else(|| vec![prepare_error("prepared source path is absent")])?;
        let origin = FunctionOrigin {
            path: source.path().to_owned(),
            source_revision: source.source_revision().to_owned(),
            source_digest: source.source_digest().to_owned(),
            source_bytes: source.source().len(),
        };
        insert_origin(&mut origins, id, origin, &mut bytes)?;
    }
    if bytes > interpreter::MAX_PREPARED_INDEX_BYTES {
        return Err(vec![prepare_error(
            "combined prepared index byte bound exceeded",
        )]);
    }
    validate_origin_spans(revision.entry_program(), &entry, &origins)?;
    validate_origin_spans(revision.test_program(), &test, &origins)?;
    let closures = PreparedClosures {
        entry,
        test,
        origins,
    };
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

fn insert_origin(
    origins: &mut BTreeMap<String, FunctionOrigin>,
    id: &str,
    origin: FunctionOrigin,
    bytes: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(previous) = origins.get(id) {
        if previous != &origin {
            return Err(vec![prepare_error(
                "duplicate prepared function source-origin facts disagree",
            )]);
        }
        return Ok(());
    }
    *bytes = bytes
        .checked_add(id.len())
        .and_then(|value| value.checked_add(origin.path.len()))
        .and_then(|value| value.checked_add(origin.source_revision.len()))
        .and_then(|value| value.checked_add(origin.source_digest.len()))
        .ok_or_else(|| vec![prepare_error("prepared source index accounting overflowed")])?;
    origins.insert(id.to_owned(), origin);
    Ok(())
}

fn validate_origin_spans(
    program: &crate::hir::ResolvedProgram,
    prepared: &PreparedResolvedI64,
    origins: &BTreeMap<String, FunctionOrigin>,
) -> Result<(), Vec<Diagnostic>> {
    for id in prepared.function_ids() {
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .ok_or_else(|| vec![prepare_error("prepared function index drifted")])?;
        let origin = origins
            .get(id)
            .ok_or_else(|| vec![prepare_error("prepared source origin is absent")])?;
        let mut expressions = function
            .requires
            .iter()
            .chain(&function.ensures)
            .chain(std::iter::once(&function.body))
            .collect::<Vec<_>>();
        while let Some(expression) = expressions.pop() {
            if expression.span.start > expression.span.end
                || expression.span.end > origin.source_bytes
            {
                return Err(vec![prepare_error(
                    "prepared expression span is outside its authenticated source",
                )]);
            }
            let fact_bytes = id
                .len()
                .checked_add(expression.id.as_str().len())
                .and_then(|value| value.checked_add(origin.path.len()))
                .and_then(|value| value.checked_add(origin.source_revision.len()))
                .and_then(|value| value.checked_add(origin.source_digest.len()))
                .ok_or_else(|| vec![prepare_error("prepared origin fact overflowed")])?;
            if fact_bytes > MIN_PROJECT_SOURCE_TRACE_BYTES / 2 {
                return Err(vec![prepare_error(
                    "one prepared source-origin fact cannot fit the minimum trace envelope",
                )]);
            }
            expressions.extend(interpreter::trace_child_expressions(expression));
        }
    }
    Ok(())
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
    origins: &BTreeMap<String, FunctionOrigin>,
) -> Result<PreparedProjectExecution, Vec<Diagnostic>> {
    let (outcome, trace) = trace::render(revision, role, options, evaluated, origins)?;
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

fn validate_trace_limits(bytes: usize, events: usize, subject: &str) -> Result<(), Diagnostic> {
    if !(MIN_PROJECT_SOURCE_TRACE_BYTES..=MAX_PROJECT_SOURCE_TRACE_BYTES).contains(&bytes)
        || !(1..=MAX_PROJECT_SOURCE_TRACE_EVENTS).contains(&events)
    {
        return Err(request_error(format!(
            "{subject} requires max_trace_bytes {MIN_PROJECT_SOURCE_TRACE_BYTES}..={MAX_PROJECT_SOURCE_TRACE_BYTES} and max_trace_events 1..={MAX_PROJECT_SOURCE_TRACE_EVENTS}"
        )));
    }
    Ok(())
}

fn preparation_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| prepare_error(&format!("{}: {}", diagnostic.code, diagnostic.message)))
        .collect()
}

fn prepare_error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-F107", message)
}

fn request_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-F108", message)
}

fn worker_error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-F109", message)
}
