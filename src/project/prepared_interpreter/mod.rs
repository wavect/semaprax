//! One retained, authority-neutral Project interpreter worker.
//!
//! Preparation validates the exact entry and test closures once and starts one
//! sequential fixed-stack worker. HIR cannot access the filesystem, process,
//! clock, network, backend, or publication authority through this surface.

mod model;
mod origin;
#[cfg(test)]
mod tests;
mod trace;
mod worker;

pub use model::{
    PreparedProjectExecution, PreparedProjectExecutionOptions, PreparedProjectInterpreterOptions,
    ProjectExecutionCancellation, DEFAULT_PROJECT_SOURCE_TRACE_BYTES,
    DEFAULT_PROJECT_SOURCE_TRACE_EVENTS, MAX_PROJECT_SOURCE_TRACE_BYTES,
    MAX_PROJECT_SOURCE_TRACE_EVENTS, MIN_PROJECT_SOURCE_TRACE_BYTES,
};
pub use trace::{
    verify_project_source_trace, verify_project_source_trace_against_revision,
    ProjectPreparedExecutionOutcome, ProjectSourceTrace, ProjectSourceTraceEvent,
    PROJECT_SOURCE_TRACE_SCHEMA,
};
pub use worker::{prepare_project_interpreter, PreparedProjectInterpreter};

pub(super) use origin::FunctionOrigin;

#[cfg(test)]
use origin::insert_origin;
#[cfg(test)]
use worker::{
    ExecutionAdmission, PreparedWorkerPermit, ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS,
    MAX_PREPARED_PROJECT_INTERPRETER_WORKERS,
};
