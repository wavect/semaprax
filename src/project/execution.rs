//! Bounded, deterministic execution over one authenticated Project v1 snapshot.
//!
//! This module receives only the already-linked entry or test HIR retained by
//! [`super::ProjectRevision`]. It never parses, resolves, links, reads, writes,
//! spawns, or invokes a backend. The enclosing authenticated-project operation
//! retains ownership of the final held-input recheck.

mod report;

use crate::conformance::NormalizedStatus;
use crate::diagnostic::Diagnostic;
use crate::interpreter::{self, ResolvedEvaluation, ResolvedEvaluationOutcome, DEFAULT_MAX_STEPS};

use super::ProjectRevision;
use report::render;
pub use report::{verify_execution_envelope, PROJECT_EXECUTION_SCHEMA};

#[cfg(test)]
use super::{MAX_MODULE_BYTES, MAX_NAME_BYTES, MAX_STABLE_ID_BYTES, PROJECT_SCHEMA};
#[cfg(test)]
use crate::cleanup_plan::StatusCase;
#[cfg(test)]
use crate::{graph, runtime_status};
#[cfg(test)]
use report::{domain_digest, PAYLOAD_DIGEST_DOMAIN};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectExecutionRole {
    Entry,
    Test,
}

impl ProjectExecutionRole {
    const fn text(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Test => "test",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectExecutionOptions {
    pub max_bytes: usize,
    pub max_steps: usize,
}

impl ProjectExecutionOptions {
    pub fn new(max_bytes: usize, max_steps: usize) -> Result<Self, Diagnostic> {
        interpreter::InterpreterOptions::new(max_bytes, max_steps)?;
        Ok(Self {
            max_bytes,
            max_steps,
        })
    }
}

impl Default for ProjectExecutionOptions {
    fn default() -> Self {
        let defaults = interpreter::InterpreterOptions::default();
        Self {
            max_bytes: defaults.max_bytes,
            max_steps: DEFAULT_MAX_STEPS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectExecutionOutcome {
    Returned(i64),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectExecution {
    role: ProjectExecutionRole,
    module: String,
    stable_id: String,
    outcome: ProjectExecutionOutcome,
    steps_used: usize,
    max_steps: usize,
    envelope: String,
}

impl ProjectExecution {
    pub const fn role(&self) -> ProjectExecutionRole {
        self.role
    }

    pub fn module(&self) -> &str {
        &self.module
    }

    pub fn stable_id(&self) -> &str {
        &self.stable_id
    }

    pub const fn outcome(&self) -> &ProjectExecutionOutcome {
        &self.outcome
    }

    pub const fn steps_used(&self) -> usize {
        self.steps_used
    }

    pub const fn max_steps(&self) -> usize {
        self.max_steps
    }

    pub fn envelope(&self) -> &str {
        &self.envelope
    }

    /// Command-level success: any returned entry value is a successful run;
    /// the exact declared test closure passes only by returning zero.
    pub const fn command_succeeded(&self) -> bool {
        matches!(
            (&self.role, &self.outcome),
            (
                ProjectExecutionRole::Entry,
                ProjectExecutionOutcome::Returned(_)
            ) | (
                ProjectExecutionRole::Test,
                ProjectExecutionOutcome::Returned(0)
            )
        )
    }
}

pub(super) fn execute(
    snapshot: &ProjectRevision,
    role: ProjectExecutionRole,
    options: &ProjectExecutionOptions,
) -> Result<ProjectExecution, Vec<Diagnostic>> {
    // Revalidate public option construction even if a caller assembled the
    // public fields directly.
    interpreter::InterpreterOptions::new(options.max_bytes, options.max_steps)
        .map_err(|error| vec![error])?;

    let (program, module) = match role {
        ProjectExecutionRole::Entry => (&snapshot.entry_program, snapshot.manifest.entry()),
        ProjectExecutionRole::Test => (&snapshot.test_program, snapshot.manifest.test_module()),
    };
    if program.module != module {
        return Err(vec![guard_error(format!(
            "authenticated {role:?} closure module `{}` disagrees with manifest module `{module}`",
            program.module
        ))]);
    }
    let entry_id = program.entrypoint.as_str();
    let evaluated =
        interpreter::evaluate_resolved_zero_arg_i64(program, entry_id, options.max_steps)?;
    finish(snapshot, role, module, entry_id, evaluated, options)
}

fn finish(
    snapshot: &ProjectRevision,
    role: ProjectExecutionRole,
    module: &str,
    entry_id: &str,
    evaluated: ResolvedEvaluation,
    options: &ProjectExecutionOptions,
) -> Result<ProjectExecution, Vec<Diagnostic>> {
    let outcome = match evaluated.outcome {
        ResolvedEvaluationOutcome::ReturnedI64(value) => ProjectExecutionOutcome::Returned(value),
        ResolvedEvaluationOutcome::LanguageFailure(status) => {
            ProjectExecutionOutcome::LanguageFailure(status)
        }
        ResolvedEvaluationOutcome::FuelExhausted => ProjectExecutionOutcome::FuelExhausted,
        ResolvedEvaluationOutcome::CallDepthExceeded => ProjectExecutionOutcome::CallDepthExceeded,
        ResolvedEvaluationOutcome::GuardError(detail) => {
            return Err(vec![guard_error(format!(
                "authenticated project execution reached an impossible post-validation state: {detail}"
            ))]);
        }
    };
    let envelope = render(
        snapshot.manifest.schema(),
        snapshot.project_revision(),
        snapshot.workspace_revision(),
        snapshot.manifest.name(),
        role,
        module,
        entry_id,
        evaluated.steps_used,
        evaluated.max_steps,
        options.max_bytes,
        &outcome,
    )?;
    Ok(ProjectExecution {
        role,
        module: module.to_owned(),
        stable_id: entry_id.to_owned(),
        outcome,
        steps_used: evaluated.steps_used,
        max_steps: evaluated.max_steps,
        envelope,
    })
}

fn guard_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-F105", message)
}

#[cfg(test)]
#[path = "execution/tests.rs"]
mod tests;
