//! Gated skill-evolution host capability (HN-15, `skill.evolve/v1`).
//!
//! An isolated experiment ingests consented task traces, lets an external
//! evolution implementation (reached through an adapter) consolidate a wiki and
//! propose at most one derived skill, and evaluates it against held-out tasks
//! with host-owned graders. The result is a promotion *proposal*; promotion is a
//! separate explicit action. See `docs/HARNESS-EVOLUTION-V1.md`. Diagnostics use
//! letter `W`.

pub mod adapter;
pub mod cli;
pub mod derived;
pub mod gate;
pub mod protected;
pub mod run;
pub mod spec;
pub mod trace;

pub use adapter::{Adapter, AdapterError, Cancel, ProcessAdapter};
pub use run::{promote, run_experiment, Outcome, Report};
pub use spec::Spec;

use crate::diag::HarnessDiagnostic;

pub(crate) fn w(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

pub const RESULT_SCHEMA: &str = "semaprax.evolution-result.v1";
