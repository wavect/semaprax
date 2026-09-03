use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::diagnostic::Diagnostic;
use crate::interpreter;

use super::super::ProjectExecutionRole;
use super::trace::{ProjectPreparedExecutionOutcome, ProjectSourceTrace};

pub const MIN_PROJECT_SOURCE_TRACE_BYTES: usize = 64 * 1024;
pub const MAX_PROJECT_SOURCE_TRACE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PROJECT_SOURCE_TRACE_EVENTS: usize = 65_536;
pub const DEFAULT_PROJECT_SOURCE_TRACE_BYTES: usize = 1024 * 1024;
pub const DEFAULT_PROJECT_SOURCE_TRACE_EVENTS: usize = 4096;

/// Monotonic cooperative cancellation for one prepared execution request.
#[derive(Clone, Default)]
pub struct ProjectExecutionCancellation {
    pub(super) cancelled: Arc<AtomicBool>,
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

    pub(crate) fn signal(&self) -> &AtomicBool {
        self.cancelled.as_ref()
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
    pub(super) role: ProjectExecutionRole,
    pub(super) outcome: ProjectPreparedExecutionOutcome,
    pub(super) steps_used: usize,
    pub(super) max_steps: usize,
    pub(super) trace: ProjectSourceTrace,
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

pub(super) fn validate_trace_limits(
    bytes: usize,
    events: usize,
    subject: &str,
) -> Result<(), Diagnostic> {
    if !(MIN_PROJECT_SOURCE_TRACE_BYTES..=MAX_PROJECT_SOURCE_TRACE_BYTES).contains(&bytes)
        || !(1..=MAX_PROJECT_SOURCE_TRACE_EVENTS).contains(&events)
    {
        return Err(request_error(format!("{subject} requires max_trace_bytes {MIN_PROJECT_SOURCE_TRACE_BYTES}..={MAX_PROJECT_SOURCE_TRACE_BYTES} and max_trace_events 1..={MAX_PROJECT_SOURCE_TRACE_EVENTS}")));
    }
    Ok(())
}

pub(super) fn preparation_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| prepare_error(&format!("{}: {}", diagnostic.code, diagnostic.message)))
        .collect()
}
pub(super) fn prepare_error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-F107", message)
}
pub(super) fn request_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-F108", message)
}
pub(super) fn worker_error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-F109", message)
}
