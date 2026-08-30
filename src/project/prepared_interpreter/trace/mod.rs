//! Canonical bounded Source Trace v1 rendering and replay.

mod model;
mod render;
mod verify;

pub(super) use model::parse_status;
pub use model::{
    ProjectPreparedExecutionOutcome, ProjectSourceTrace, ProjectSourceTraceEvent,
    PROJECT_SOURCE_TRACE_SCHEMA,
};
pub(super) use render::render;
pub use verify::{verify_project_source_trace, verify_project_source_trace_against_revision};
