//! One bounded, session-scoped candidate test task.
//!
//! The worker receives only an immutable candidate, fixed host test policy and
//! a cooperative cancellation flag. Result release remains behind live source
//! authentication in `VNextSession`.

use super::*;
use crate::project::{
    CandidateTestPolicy, ProjectCandidateTestTaskOutcome, ProjectExecutionCancellation,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

const TASK_DOMAIN: &[u8] = b"semaprax.image-candidate-test-task.v1\0";
const MAX_RESULT_CHUNK_BYTES: usize = 512 * 1024;
const MAX_ACTIVE_TASKS: usize = 8;
static ACTIVE_TASKS: AtomicUsize = AtomicUsize::new(0);

const CANDIDATE: Parameter = Parameter {
    name: "candidate_revision",
    kind: ParameterKind::Digest,
    required: true,
};
const TASK: Parameter = Parameter {
    name: "task_revision",
    kind: ParameterKind::Digest,
    required: true,
};
const OFFSET: Parameter = Parameter {
    name: "offset",
    kind: ParameterKind::Integer(0, crate::project::MAX_PROJECT_CANDIDATE_TEST_REPORT_BYTES),
    required: true,
};
const CHUNK_BYTES: Parameter = Parameter {
    name: "max_bytes",
    kind: ParameterKind::Integer(4096, MAX_RESULT_CHUNK_BYTES),
    required: true,
};

const METHODS: [Method; 4] = [
    Method {
        name: "candidate/test-task-start",
        operation: Operation::VNext(Action::CandidateTestTaskStart),
        parameters: &[REVISION, CANDIDATE],
        query: false,
        payload_schema: "semaprax.image-candidate-test-task-start.v1",
    },
    Method {
        name: "candidate/test-task-status",
        operation: Operation::VNext(Action::CandidateTestTaskStatus),
        parameters: &[REVISION, TASK],
        query: true,
        payload_schema: "semaprax.image-candidate-test-task-status.v1",
    },
    Method {
        name: "candidate/test-task-cancel",
        operation: Operation::VNext(Action::CandidateTestTaskCancel),
        parameters: &[REVISION, TASK],
        query: false,
        payload_schema: "semaprax.image-candidate-test-task-cancel.v1",
    },
    Method {
        name: "candidate/test-task-result",
        operation: Operation::VNext(Action::CandidateTestTaskResult),
        parameters: &[REVISION, TASK, OFFSET, CHUNK_BYTES],
        query: true,
        payload_schema: "semaprax.image-candidate-test-task-result-chunk.v1",
    },
];

pub(super) fn methods() -> impl Iterator<Item = &'static Method> {
    METHODS.iter()
}

pub(super) struct Registry {
    task: Option<Task>,
}

impl Registry {
    pub(super) const fn new() -> Self {
        Self { task: None }
    }

    pub(super) const fn is_active(&self) -> bool {
        self.task.is_some()
    }

    pub(super) fn request(
        &mut self,
        action: Action,
        params: &Map<String, Value>,
        image: &ProjectSemanticImage,
        candidates: &candidates::Registry,
        policy: &CandidateTestPolicy,
    ) -> Result<Value, Vec<Diagnostic>> {
        match action {
            Action::CandidateTestTaskStart => self.start(params, image, candidates, *policy),
            Action::CandidateTestTaskStatus => {
                let task = self.task(params)?;
                task.release();
                task.poll();
                Ok(task.status("semaprax.image-candidate-test-task-status.v1", false))
            }
            Action::CandidateTestTaskCancel => {
                let task = self.task(params)?;
                task.cancellation.cancel();
                task.release();
                task.await_terminal();
                Ok(task.status("semaprax.image-candidate-test-task-cancel.v1", true))
            }
            Action::CandidateTestTaskResult => {
                let task = self.task(params)?;
                task.release();
                task.poll();
                task.result(number(params, "offset", 0), number(params, "max_bytes", 0))
            }
            _ => Err(task_error("unknown candidate test task operation")),
        }
    }

    fn start(
        &mut self,
        params: &Map<String, Value>,
        image: &ProjectSemanticImage,
        candidates: &candidates::Registry,
        policy: CandidateTestPolicy,
    ) -> Result<Value, Vec<Diagnostic>> {
        if self.task.is_some() {
            return Err(task_error(
                "this session already scheduled its one bounded candidate test task",
            ));
        }
        let candidate = Arc::clone(candidates.candidate(text(params, "candidate_revision"))?);
        let permit = TaskPermit::acquire(&ACTIVE_TASKS, MAX_ACTIVE_TASKS)?;
        let task_revision = task_revision(image.image_digest(), candidate.candidate_digest());
        let cancellation = ProjectExecutionCancellation::new();
        let worker_cancellation = cancellation.clone();
        let worker_candidate = Arc::clone(&candidate);
        let (start, ready) = mpsc::sync_channel(0);
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("semaprax-candidate-test-task".to_owned())
            .spawn(move || {
                if ready.recv().is_err() {
                    return;
                }
                let result = worker_candidate.execute_tests_cancellable(
                    worker_candidate.candidate_digest(),
                    &policy,
                    &worker_cancellation,
                );
                let _ = sender.send(result);
            })
            .map_err(|_| worker_error("cannot start candidate test task worker"))?;
        let task = Task {
            task_revision,
            image_revision: image.image_digest().to_owned(),
            project_revision: image.revision().project_revision().to_owned(),
            candidate_revision: candidate.candidate_digest().to_owned(),
            max_steps: policy.max_steps(),
            cancellation,
            start: Some(start),
            running: false,
            receiver,
            worker: Some(worker),
            terminal: None,
            permit: Some(permit),
        };
        let payload = task.status("semaprax.image-candidate-test-task-start.v1", false);
        self.task = Some(task);
        Ok(payload)
    }

    fn task(&mut self, params: &Map<String, Value>) -> Result<&mut Task, Vec<Diagnostic>> {
        let expected = text(params, "task_revision");
        self.task
            .as_mut()
            .filter(|task| task.task_revision == expected)
            .ok_or_else(|| {
                task_error("candidate test task handle is stale, invalidated, or unknown")
            })
    }

    pub(super) fn invalidate(&mut self) {
        if let Some(mut task) = self.task.take() {
            task.cancellation.cancel();
            task.release();
            task.join();
        }
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        self.invalidate();
    }
}

struct Task {
    task_revision: String,
    image_revision: String,
    project_revision: String,
    candidate_revision: String,
    max_steps: usize,
    cancellation: ProjectExecutionCancellation,
    start: Option<mpsc::SyncSender<()>>,
    running: bool,
    receiver: mpsc::Receiver<Result<ProjectCandidateTestTaskOutcome, Vec<Diagnostic>>>,
    worker: Option<JoinHandle<()>>,
    terminal: Option<Terminal>,
    permit: Option<TaskPermit>,
}

#[derive(Debug)]
struct TaskPermit {
    active: &'static AtomicUsize,
}

impl TaskPermit {
    fn acquire(active: &'static AtomicUsize, limit: usize) -> Result<Self, Vec<Diagnostic>> {
        let mut observed = active.load(Ordering::Acquire);
        loop {
            if observed >= limit {
                return Err(task_error(
                    "process-wide candidate test task bound is exhausted",
                ));
            }
            match active.compare_exchange_weak(
                observed,
                observed + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(Self { active }),
                Err(actual) => observed = actual,
            }
        }
    }
}

impl Drop for TaskPermit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

enum Terminal {
    Completed {
        report: String,
        digest: String,
        passed: bool,
        steps_used: usize,
    },
    Cancelled {
        before_step: usize,
        steps_used: usize,
    },
    Failed(Vec<Value>),
}

impl Task {
    fn release(&mut self) {
        if let Some(start) = self.start.take() {
            let _ = start.send(());
            self.running = true;
        }
    }

    fn await_terminal(&mut self) {
        if self.terminal.is_some() {
            return;
        }
        match self.receiver.recv() {
            Ok(result) => self.admit(result),
            Err(_) => {
                self.terminal = Some(Terminal::Failed(diagnostic_values(&worker_error(
                    "candidate test task worker terminated without an outcome",
                ))));
            }
        }
        self.join();
    }

    fn poll(&mut self) {
        if self.terminal.is_some() {
            return;
        }
        match self.receiver.try_recv() {
            Ok(result) => {
                self.admit(result);
                self.join();
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.terminal = Some(Terminal::Failed(diagnostic_values(&worker_error(
                    "candidate test task worker terminated without an outcome",
                ))));
                self.join();
            }
        }
    }

    fn admit(&mut self, result: Result<ProjectCandidateTestTaskOutcome, Vec<Diagnostic>>) {
        match result {
            Ok(ProjectCandidateTestTaskOutcome::Completed(report)) => {
                let steps_used = report.execution().steps_used();
                self.terminal = Some(Terminal::Completed {
                    report: report.to_json().to_owned(),
                    digest: report.report_digest().to_owned(),
                    passed: report.passed(),
                    steps_used,
                });
            }
            Ok(ProjectCandidateTestTaskOutcome::Cancelled {
                before_step,
                steps_used,
                max_steps: _,
            }) => {
                self.terminal = Some(Terminal::Cancelled {
                    before_step,
                    steps_used,
                });
            }
            Err(errors) => {
                self.terminal = Some(Terminal::Failed(diagnostic_values(&errors)));
            }
        }
    }

    fn join(&mut self) {
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() && self.terminal.is_none() {
                self.terminal = Some(Terminal::Failed(diagnostic_values(&worker_error(
                    "candidate test task worker panicked",
                ))));
            }
            self.permit.take();
        }
    }

    fn status(&self, schema: &str, cancel: bool) -> Value {
        let (state, report_digest, passed, before_step, steps_used, diagnostics) =
            match &self.terminal {
                None => (
                    if self.running { "running" } else { "queued" },
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    vec![],
                ),
                Some(Terminal::Completed {
                    digest,
                    passed,
                    steps_used,
                    ..
                }) => (
                    "completed",
                    json!(digest),
                    json!(passed),
                    Value::Null,
                    json!(steps_used),
                    vec![],
                ),
                Some(Terminal::Cancelled {
                    before_step,
                    steps_used,
                }) => (
                    "cancelled",
                    Value::Null,
                    Value::Null,
                    json!(before_step),
                    json!(steps_used),
                    vec![],
                ),
                Some(Terminal::Failed(diagnostics)) => (
                    "failed",
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    diagnostics.clone(),
                ),
            };
        let mut value = common(
            schema,
            &self.image_revision,
            &self.project_revision,
            &self.candidate_revision,
            &self.task_revision,
        );
        value["state"] = json!(state);
        value["terminal"] = json!(self.terminal.is_some());
        value["cancellation_requested"] = json!(self.cancellation.is_cancelled());
        value["report_digest"] = report_digest;
        value["passed"] = passed;
        value["before_step"] = before_step;
        value["steps_used"] = steps_used;
        value["max_steps"] = json!(self.max_steps);
        value["diagnostics"] = json!(diagnostics);
        if cancel {
            value["cancel_observed"] = json!(true);
        }
        value
    }

    fn result(&self, offset: usize, max_bytes: usize) -> Result<Value, Vec<Diagnostic>> {
        let Terminal::Completed { report, digest, .. } =
            self.terminal.as_ref().ok_or_else(|| {
                task_error("candidate test task result is unavailable before successful completion")
            })?
        else {
            return Err(task_error(
                "candidate test task has no completed report result",
            ));
        };
        if offset > report.len() || !report.is_char_boundary(offset) {
            return Err(capacity_error(
                "candidate test task result offset is invalid",
            ));
        }
        let mut end = offset.saturating_add(max_bytes).min(report.len());
        while end > offset && !report.is_char_boundary(end) {
            end -= 1;
        }
        let next = (end < report.len()).then_some(end);
        let mut value = common(
            "semaprax.image-candidate-test-task-result-chunk.v1",
            &self.image_revision,
            &self.project_revision,
            &self.candidate_revision,
            &self.task_revision,
        );
        value["report_schema"] = json!(crate::project::PROJECT_CANDIDATE_TEST_REPORT_SCHEMA);
        value["report_digest"] = json!(digest);
        value["offset"] = json!(offset);
        value["total_bytes"] = json!(report.len());
        value["chunk"] = json!(&report[offset..end]);
        value["next_offset"] = json!(next);
        value["complete"] = json!(next.is_none());
        Ok(value)
    }
}

fn common(schema: &str, image: &str, project: &str, candidate: &str, task: &str) -> Value {
    json!({
        "schema":schema,"image_revision":image,"project_revision":project,
        "candidate_revision":candidate,"task_revision":task,"source_authority":false,
        "authority":{"source_write":false,"process":false,"network":false,"target_runtime":false,"publication":false},
        "blind_spots":["native_and_wasm_runtime","deployment_configuration","generated_artifacts",
            "external_api_behavior","runtime_environment","external_consumers"]
    })
}

fn task_revision(image: &str, candidate: &str) -> String {
    crate::protocol_check::domain_digest(TASK_DOMAIN, format!("{image}\0{candidate}").as_bytes())
}

fn diagnostic_values(errors: &[Diagnostic]) -> Vec<Value> {
    errors
        .iter()
        .filter_map(|error| serde_json::from_str(&error.json()).ok())
        .collect()
}

fn task_error(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G365", message)]
}
fn worker_error(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G366", message)]
}
fn capacity_error(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G367", message)]
}

#[cfg(test)]
mod tests {
    use super::*;

    static ACTIVE: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn task_permit_is_bounded_and_reusable() {
        let first = TaskPermit::acquire(&ACTIVE, 2).unwrap();
        let second = TaskPermit::acquire(&ACTIVE, 2).unwrap();
        assert_eq!(
            TaskPermit::acquire(&ACTIVE, 2).unwrap_err()[0].code,
            "SPX-G365"
        );
        drop(first);
        let replacement = TaskPermit::acquire(&ACTIVE, 2).unwrap();
        drop((second, replacement));
        assert_eq!(ACTIVE.load(Ordering::Acquire), 0);
    }
}
