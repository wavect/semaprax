//! Decision-task registry. Tasks are compile-time constants: a provider or
//! model can neither add a task nor activate a reserved one.

use super::diag::{DecisionResult, Diagnostic};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionTask {
    ModelRoute,
    /// MR-01: negotiated `decision.evaluate` v2 routing task. v1 is unchanged.
    ModelRouteV2,
    ToolSelect,
    ContextPlan,
    /// MR-11: finite runtime choice over caller-admitted tools or agents
    /// (`decision.evaluate` v3). Supersedes the reserved `tool-select/v1`.
    ChoiceSelect,
}

pub struct TaskEntry {
    pub task: DecisionTask,
    pub id: &'static str,
    /// Reserved tasks are named but refuse every request.
    pub active: bool,
}

pub const TASKS: &[TaskEntry] = &[
    TaskEntry {
        task: DecisionTask::ModelRoute,
        id: "model-route/v1",
        active: true,
    },
    TaskEntry {
        task: DecisionTask::ModelRouteV2,
        id: "model-route/v2",
        active: true,
    },
    TaskEntry {
        task: DecisionTask::ToolSelect,
        id: "tool-select/v1",
        active: false,
    },
    TaskEntry {
        task: DecisionTask::ContextPlan,
        id: "context-plan/v1",
        active: false,
    },
    TaskEntry {
        task: DecisionTask::ChoiceSelect,
        id: "choice-select/v1",
        active: true,
    },
];

impl DecisionTask {
    pub fn id(self) -> &'static str {
        TASKS
            .iter()
            .find(|t| t.task == self)
            .map(|t| t.id)
            .unwrap_or("")
    }
}

/// Resolve a task id to an active registered task.
pub fn resolve(id: &str) -> DecisionResult<DecisionTask> {
    match TASKS.iter().find(|t| t.id == id) {
        None => Err(Diagnostic::new(
            "SPX-HPJ001",
            format!("unregistered decision task `{id}`"),
        )),
        Some(t) if !t.active => Err(Diagnostic::new(
            "SPX-HPJ002",
            format!("decision task `{id}` is reserved and not active"),
        )),
        Some(t) => Ok(t.task),
    }
}

/// Resolve a task id to an active model-route task. A choice task is not a
/// route request and its records never replay as one (`SPX-HPJ025`).
pub fn resolve_route(id: &str) -> DecisionResult<DecisionTask> {
    match resolve(id)? {
        t @ (DecisionTask::ModelRoute | DecisionTask::ModelRouteV2) => Ok(t),
        _ => Err(Diagnostic::new(
            "SPX-HPJ025",
            format!("decision task `{id}` is not a model-route task"),
        )),
    }
}
