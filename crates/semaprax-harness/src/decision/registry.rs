//! Decision-task registry. Tasks are compile-time constants: a provider or
//! model can neither add a task nor activate a reserved one.

use crate::diag::{HarnessDiagnostic, HarnessResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionTask {
    ModelRoute,
    /// MR-01: negotiated `decision.evaluate` v2 routing task. v1 is unchanged.
    ModelRouteV2,
    ToolSelect,
    ContextPlan,
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
pub fn resolve(id: &str) -> HarnessResult<DecisionTask> {
    match TASKS.iter().find(|t| t.id == id) {
        None => Err(HarnessDiagnostic::new(
            "SPX-HPJ001",
            format!("unregistered decision task `{id}`"),
        )),
        Some(t) if !t.active => Err(HarnessDiagnostic::new(
            "SPX-HPJ002",
            format!("decision task `{id}` is reserved and not active"),
        )),
        Some(t) => Ok(t.task),
    }
}
