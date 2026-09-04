//! Bounded, deterministic execution over one authenticated Project v1 snapshot.
//!
//! This module receives only the already-linked entry or test HIR retained by
//! [`super::ProjectRevision`]. It never parses, resolves, links, reads, writes,
//! spawns, or invokes a backend. The enclosing authenticated-project operation
//! retains ownership of the final held-input recheck.

mod cases;
mod report;

use crate::conformance::NormalizedStatus;
use crate::diagnostic::Diagnostic;
use crate::interpreter::{self, ResolvedEvaluation, ResolvedEvaluationOutcome, DEFAULT_MAX_STEPS};

use super::{ProjectExecutionCancellation, ProjectRevision};
pub use cases::{
    ProjectContractArgument, ProjectContractFailure, ProjectTestCase, SkippedTestCase,
    TEST_CASE_PREFIX,
};
#[cfg(test)]
use report::render;
use report::render_full;
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
    failure: Option<ProjectContractFailure>,
    cases: Vec<ProjectTestCase>,
    skipped_cases: Vec<SkippedTestCase>,
    envelope: String,
}

pub(super) enum CancellableProjectExecution {
    Completed(Box<ProjectExecution>),
    Cancelled {
        before_step: usize,
        steps_used: usize,
        max_steps: usize,
    },
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

    /// The violated clause and call frame when the outcome is a contract
    /// failure the evaluator could attribute.
    pub const fn failure(&self) -> Option<&ProjectContractFailure> {
        self.failure.as_ref()
    }

    /// The individually executed `test_` cases of the test module, ordered by
    /// stable identity; always empty for the entry role.
    pub fn cases(&self) -> &[ProjectTestCase] {
        &self.cases
    }

    /// `test_`-prefixed functions of the test module that are not cases, with
    /// the shape rule each misses; always empty for the entry role. Reports
    /// name them so a mis-shaped case is never skipped silently.
    pub fn skipped_cases(&self) -> &[SkippedTestCase] {
        &self.skipped_cases
    }

    /// Command-level success: any returned entry value is a successful run;
    /// the declared test closure passes only when `main` and every named case
    /// return zero.
    pub fn command_succeeded(&self) -> bool {
        match (&self.role, &self.outcome) {
            (ProjectExecutionRole::Entry, ProjectExecutionOutcome::Returned(_)) => true,
            (ProjectExecutionRole::Test, ProjectExecutionOutcome::Returned(0)) => {
                self.cases.iter().all(ProjectTestCase::passed)
            }
            _ => false,
        }
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
    let cases = match role {
        ProjectExecutionRole::Entry => Vec::new(),
        ProjectExecutionRole::Test => {
            match cases::run_cases(snapshot, program, module, options, None)? {
                cases::CaseRun::Completed(cases) => cases,
                cases::CaseRun::Cancelled { .. } => {
                    return Err(vec![guard_error(
                        "test cases observed a cancellation without a cancellation signal"
                            .to_owned(),
                    )])
                }
            }
        }
    };
    let cases = TestCases {
        cases,
        skipped: skipped(snapshot, program, module, role),
    };
    finish(snapshot, role, module, entry_id, evaluated, cases, options)
}

/// The executed named cases of one test run and the `test_` functions that
/// were not admitted as cases.
struct TestCases {
    cases: Vec<ProjectTestCase>,
    skipped: Vec<SkippedTestCase>,
}

fn skipped(
    snapshot: &ProjectRevision,
    program: &crate::hir::ResolvedProgram,
    module: &str,
    role: ProjectExecutionRole,
) -> Vec<SkippedTestCase> {
    match role {
        ProjectExecutionRole::Entry => Vec::new(),
        ProjectExecutionRole::Test => cases::skipped_selection(snapshot, program, module),
    }
}

/// Execute through the cancellation-aware prepared evaluator without retaining
/// or rendering a source trace. Non-cancelled results are finished by the same
/// Project execution renderer as the legacy path, preserving its exact bytes.
pub(super) fn execute_cancellable(
    snapshot: &ProjectRevision,
    role: ProjectExecutionRole,
    options: &ProjectExecutionOptions,
    cancellation: &ProjectExecutionCancellation,
) -> Result<CancellableProjectExecution, Vec<Diagnostic>> {
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
    let prepared = interpreter::prepare_resolved_zero_arg_i64(program, entry_id)?;
    let evaluated = std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("semaprax-resolved-cancellable".to_owned())
            .stack_size(interpreter::EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, || {
                interpreter::evaluate_prepared_resolved_zero_arg_i64(
                    program,
                    &prepared,
                    options.max_steps,
                    0,
                    interpreter::PreparedCancellation::Atomic(cancellation.signal()),
                )
            })
            .map_err(|error| {
                vec![guard_error(format!(
                    "cancellable resolved evaluation thread failed to start: {error}"
                ))]
            })?;
        worker.join().map_err(|_| {
            vec![guard_error(
                "cancellable resolved evaluation thread panicked after HIR validation".to_owned(),
            )]
        })?
    })?;
    let outcome = match evaluated.outcome {
        interpreter::PreparedResolvedEvaluationOutcome::ReturnedI64(value) => {
            ResolvedEvaluationOutcome::ReturnedI64(value)
        }
        interpreter::PreparedResolvedEvaluationOutcome::LanguageFailure(status) => {
            ResolvedEvaluationOutcome::LanguageFailure(status)
        }
        interpreter::PreparedResolvedEvaluationOutcome::FuelExhausted => {
            ResolvedEvaluationOutcome::FuelExhausted
        }
        interpreter::PreparedResolvedEvaluationOutcome::CallDepthExceeded => {
            ResolvedEvaluationOutcome::CallDepthExceeded
        }
        interpreter::PreparedResolvedEvaluationOutcome::Cancelled { before_step } => {
            return Ok(CancellableProjectExecution::Cancelled {
                before_step,
                steps_used: evaluated.steps_used,
                max_steps: evaluated.max_steps,
            });
        }
        interpreter::PreparedResolvedEvaluationOutcome::GuardError(detail) => {
            ResolvedEvaluationOutcome::GuardError(detail)
        }
    };
    let cases = match role {
        ProjectExecutionRole::Entry => Vec::new(),
        ProjectExecutionRole::Test => {
            match cases::run_cases(
                snapshot,
                program,
                module,
                options,
                Some(cancellation.signal()),
            )? {
                cases::CaseRun::Completed(cases) => cases,
                cases::CaseRun::Cancelled { before_step } => {
                    return Ok(CancellableProjectExecution::Cancelled {
                        before_step,
                        steps_used: evaluated.steps_used,
                        max_steps: evaluated.max_steps,
                    });
                }
            }
        }
    };
    let cases = TestCases {
        cases,
        skipped: skipped(snapshot, program, module, role),
    };
    let execution = finish(
        snapshot,
        role,
        module,
        entry_id,
        ResolvedEvaluation {
            outcome,
            steps_used: evaluated.steps_used,
            max_steps: evaluated.max_steps,
            failure: evaluated.failure,
        },
        cases,
        options,
    )?;
    Ok(CancellableProjectExecution::Completed(Box::new(execution)))
}

fn finish(
    snapshot: &ProjectRevision,
    role: ProjectExecutionRole,
    module: &str,
    entry_id: &str,
    evaluated: ResolvedEvaluation,
    cases: TestCases,
    options: &ProjectExecutionOptions,
) -> Result<ProjectExecution, Vec<Diagnostic>> {
    let TestCases {
        cases,
        skipped: skipped_cases,
    } = cases;
    let failure = evaluated
        .failure
        .as_ref()
        .filter(|_| {
            matches!(
                evaluated.outcome,
                ResolvedEvaluationOutcome::LanguageFailure(_)
            )
        })
        .map(|detail| cases::contract_failure(snapshot, detail));
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
    let envelope = render_full(
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
        failure.as_ref(),
        &cases,
    )?;
    Ok(ProjectExecution {
        role,
        module: module.to_owned(),
        stable_id: entry_id.to_owned(),
        outcome,
        steps_used: evaluated.steps_used,
        max_steps: evaluated.max_steps,
        failure,
        cases,
        skipped_cases,
        envelope,
    })
}

fn guard_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-F105", message)
}

#[cfg(test)]
#[path = "execution/tests.rs"]
mod tests;
