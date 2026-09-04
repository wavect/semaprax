//! Named Project test cases and the report projection of a contract failure.
//!
//! A test module's `main` remains the closure that decides the command; this
//! module adds the individually executed `fn test_<name>() -> i64` cases of the
//! same manifest-declared module and turns the interpreter's contract-failure
//! frame detail into report text. Neither runs anything the ordinary test
//! closure could not: a case is a function of the already linked test program,
//! selected by name within the declared module, never discovered on disk.

use std::sync::atomic::AtomicBool;

use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::interpreter::{
    self, ContractFailureDetail, PreparedCancellation, PreparedResolvedEvaluationOutcome,
};

use super::{guard_error, ProjectExecutionOptions, ProjectExecutionOutcome};
use crate::project::ProjectRevision;

/// Display-name prefix that selects a function of the test module as a case.
pub const TEST_CASE_PREFIX: &str = "test_";
/// Bound on every contract-failure text field: clause source, argument names,
/// type keys, and rendered values. A longer clause is reported by identity.
pub(super) const MAX_CONTRACT_TEXT_BYTES: usize = 4096;

/// The violated clause of one contract failure, projected for reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectContractFailure {
    /// Stable identity of the function whose clause failed.
    pub function_id: String,
    /// Exactly `requires` or `ensures`.
    pub phase: &'static str,
    /// The clause's source text in the declaring file, or its expression
    /// identity when the retained sources do not cover the span.
    pub clause: String,
    /// The call's parameters in declaration order.
    pub arguments: Vec<ProjectContractArgument>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectContractArgument {
    pub name: String,
    /// Name-independent type key, `i64` or `bool` for the scalar types.
    pub ty: String,
    pub value: String,
}

/// A `test_`-prefixed function of the declared test module that is not a
/// case, with the shape rule it misses, so the report can say so instead of
/// skipping it silently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkippedTestCase {
    pub name: String,
    pub reason: &'static str,
}

/// One executed `test_` case of the declared test module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTestCase {
    pub(super) stable_id: String,
    pub(super) name: String,
    pub(super) outcome: ProjectExecutionOutcome,
    pub(super) steps_used: usize,
    pub(super) max_steps: usize,
    pub(super) failure: Option<ProjectContractFailure>,
}

impl ProjectTestCase {
    pub fn stable_id(&self) -> &str {
        &self.stable_id
    }

    pub fn name(&self) -> &str {
        &self.name
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

    pub const fn failure(&self) -> Option<&ProjectContractFailure> {
        self.failure.as_ref()
    }

    /// A case passes only by returning zero.
    pub const fn passed(&self) -> bool {
        matches!(self.outcome, ProjectExecutionOutcome::Returned(0))
    }
}

pub(super) enum CaseRun {
    Completed(Vec<ProjectTestCase>),
    Cancelled { before_step: usize },
}

/// Project the interpreter's frame detail onto the retained sources.
pub(super) fn contract_failure(
    snapshot: &ProjectRevision,
    detail: &ContractFailureDetail,
) -> ProjectContractFailure {
    let span = detail.clause_span;
    let clause = snapshot
        .semantic
        .rename_function(&detail.function_id)
        .and_then(|function| {
            snapshot
                .sources
                .iter()
                .find(|source| source.path() == function.path)
        })
        .and_then(|source| source.source().get(span.start..span.end))
        .map(|text| text.trim().to_owned())
        .filter(|text| {
            !text.is_empty() && !text.contains('\n') && text.len() <= MAX_CONTRACT_TEXT_BYTES
        })
        .unwrap_or_else(|| detail.clause_id.clone());
    ProjectContractFailure {
        function_id: detail.function_id.clone(),
        phase: detail.phase_text(),
        clause,
        arguments: detail
            .arguments
            .iter()
            .map(|argument| ProjectContractArgument {
                name: argument.name.clone(),
                ty: argument.ty.clone(),
                value: argument.value.clone(),
            })
            .collect(),
    }
}

/// The `test_` cases of `module` in the linked program, in the program's
/// stable-identity order: zero-parameter `i64` functions with explicit
/// identities. A `test_` function
/// of another shape is not a case.
pub(super) fn case_selection<'a>(
    snapshot: &'a ProjectRevision,
    program: &'a hir::ResolvedProgram,
    module: &'a str,
) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
    program.functions.iter().filter_map(move |function| {
        let explicit = program
            .declarations
            .declaration(&function.id)
            .is_some_and(|declaration| {
                declaration.identity_origin == hir::IdentityOrigin::Explicit
            });
        let declared_here = snapshot
            .semantic
            .rename_function(function.id.as_str())
            .is_some_and(|declared| declared.module == module);
        (function.name.starts_with(TEST_CASE_PREFIX)
            && function.params.is_empty()
            && function.return_type == hir::ResolvedType::I64
            && explicit
            && declared_here)
            .then(|| (function.id.as_str(), function.name.as_str()))
    })
}

/// The `test_`-prefixed functions of `module` that are not cases, in the
/// program's stable-identity order, each with the first rule it misses.
pub(super) fn skipped_selection(
    snapshot: &ProjectRevision,
    program: &hir::ResolvedProgram,
    module: &str,
) -> Vec<SkippedTestCase> {
    program
        .functions
        .iter()
        .filter(|function| function.name.starts_with(TEST_CASE_PREFIX))
        .filter(|function| {
            snapshot
                .semantic
                .rename_function(function.id.as_str())
                .is_some_and(|declared| declared.module == module)
        })
        .filter_map(|function| {
            let explicit =
                program
                    .declarations
                    .declaration(&function.id)
                    .is_some_and(|declaration| {
                        declaration.identity_origin == hir::IdentityOrigin::Explicit
                    });
            let reason = if !function.params.is_empty() {
                "it takes parameters"
            } else if function.return_type != hir::ResolvedType::I64 {
                "it does not return `i64`"
            } else if !explicit {
                "it has no explicit `@id`"
            } else {
                return None;
            };
            Some(SkippedTestCase {
                name: function.name.clone(),
                reason,
            })
        })
        .collect()
}

/// Execute every case with its own full step budget. A cancellation observed
/// inside any case cancels the whole test execution.
pub(super) fn run_cases(
    snapshot: &ProjectRevision,
    program: &hir::ResolvedProgram,
    module: &str,
    options: &ProjectExecutionOptions,
    cancellation: Option<&AtomicBool>,
) -> Result<CaseRun, Vec<Diagnostic>> {
    let mut cases = Vec::new();
    for (stable_id, name) in case_selection(snapshot, program, module) {
        let cancellation = match cancellation {
            Some(flag) => PreparedCancellation::Atomic(flag),
            None => PreparedCancellation::Never,
        };
        let evaluated = interpreter::evaluate_resolved_zero_arg_i64_function(
            program,
            stable_id,
            options.max_steps,
            false,
            cancellation,
        )?;
        let outcome = match evaluated.outcome {
            PreparedResolvedEvaluationOutcome::ReturnedI64(value) => {
                ProjectExecutionOutcome::Returned(value)
            }
            PreparedResolvedEvaluationOutcome::LanguageFailure(status) => {
                ProjectExecutionOutcome::LanguageFailure(status)
            }
            PreparedResolvedEvaluationOutcome::FuelExhausted => {
                ProjectExecutionOutcome::FuelExhausted
            }
            PreparedResolvedEvaluationOutcome::CallDepthExceeded => {
                ProjectExecutionOutcome::CallDepthExceeded
            }
            PreparedResolvedEvaluationOutcome::Cancelled { before_step } => {
                return Ok(CaseRun::Cancelled { before_step });
            }
            PreparedResolvedEvaluationOutcome::GuardError(detail) => {
                return Err(vec![guard_error(format!(
                    "test case `{stable_id}` reached an impossible post-validation state: {detail}"
                ))]);
            }
        };
        let failure = evaluated
            .failure
            .as_ref()
            .filter(|_| matches!(outcome, ProjectExecutionOutcome::LanguageFailure(_)))
            .map(|detail| contract_failure(snapshot, detail));
        cases.push(ProjectTestCase {
            stable_id: stable_id.to_owned(),
            name: name.to_owned(),
            outcome,
            steps_used: evaluated.steps_used,
            max_steps: evaluated.max_steps,
            failure,
        });
    }
    Ok(CaseRun::Completed(cases))
}
