//! Checked scaffold-decision invocation for the reference service.
//!
//! Every admission question the host must ask -- is this request line
//! admitted, is this identifier valid, does this session own this row, is
//! this enqueue new or a duplicate -- is answered by evaluating the
//! operator-loaded project's own checked `.spx` decision functions through
//! [`ProjectRevision::evaluate_service_decision_v1`]. Selection authority
//! stays with the retained authenticated revision: only explicit stable
//! identities already linked into its entry closure can run, and only with
//! the frozen public-invocation vocabulary (`i64`, `bool`, borrowed bytes).
//!
//! Only the scaffold decisions whose signatures that vocabulary admits,
//! and whose entire call closure is effect- and contract-free, are
//! invocable here: `request_is_admitted`, `identifier_is_valid`,
//! `method_is_rejected`, and `task_owner_authorized`. `enqueue_outcome`
//! admits the vocabulary but its closure reaches the contract-bearing
//! `std.bytes.byte_to_i64`, so the host mirrors its documented 0/1/2 truth
//! table instead of invoking it (see `mapping`). The remaining scaffold
//! decisions (registration bounds, session ticks, job terminality,
//! migration/transaction, log/trace/metric/export/webhook policies) take
//! `u8`/`usize` parameters the frozen vocabulary does not carry, so they
//! keep their existing fixture-mode coverage and are not invoked by this
//! host. That vocabulary is deliberately not widened here.
//!
//! [`ProjectRevision::evaluate_service_decision_v1`]: semaprax::project::ProjectRevision::evaluate_service_decision_v1

use semaprax::interpreter::{PublicApiArgument, PublicApiEvaluationOutcome, PublicApiValue};
use semaprax::project::ProjectRevision;

/// The per-decision interpreter step budget. Well under the
/// `MAX_STEPS_LIMIT` ceiling; pure admission predicates settle far below it.
pub const DECISION_MAX_STEPS: usize = 1_000_000;

/// Stable refusal categories for decision resolution and evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionRefusal {
    /// The retained revision does not carry exactly one unambiguous
    /// decision set (wrong project, or an ambiguous module layout).
    Unresolved,
    /// A resolved decision failed to evaluate (fuel, guard, or language
    /// failure). The outcome is unusable; the caller must fail closed.
    EvaluationFailed,
    /// A resolved decision returned a value outside its checked result
    /// shape. This cannot happen for the admitted result types and fails
    /// closed if it ever does.
    UnexpectedResult,
}

/// The scaffold decision identities resolved from one revision. The module
/// prefix is discovered, never assumed: a scaffolded project carries
/// `<module>.core.<decision>` for its own module name. All five must
/// resolve (proving the exact decision set), but only four are invoked
/// (see the module documentation).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionIdentities {
    prefix: String,
    request_is_admitted: String,
    identifier_is_valid: String,
    method_is_rejected: String,
    task_owner_authorized: String,
    enqueue_outcome: String,
}

impl DecisionIdentities {
    /// Resolve the decision set from the retained entry closure. Exactly one
    /// unambiguous `<prefix>.core.*` family must exist.
    pub fn resolve(revision: &ProjectRevision) -> Result<Self, DecisionRefusal> {
        let program = revision.entry_program();
        let request_is_admitted = sole(program, "request_is_admitted")?;
        let identifier_is_valid = sole(program, "identifier_is_valid")?;
        let method_is_rejected = sole(program, "method_is_rejected")?;
        let task_owner_authorized = sole(program, "task_owner_authorized")?;
        let enqueue_outcome = sole(program, "enqueue_outcome")?;
        let prefix = prefix_of(&request_is_admitted).ok_or(DecisionRefusal::Unresolved)?;
        for identity in [
            &identifier_is_valid,
            &method_is_rejected,
            &task_owner_authorized,
            &enqueue_outcome,
        ] {
            if prefix_of(identity) != Some(prefix) {
                return Err(DecisionRefusal::Unresolved);
            }
        }
        Ok(Self {
            prefix: prefix.to_owned(),
            request_is_admitted,
            identifier_is_valid,
            method_is_rejected,
            task_owner_authorized,
            enqueue_outcome,
        })
    }

    /// The discovered decision-family prefix (for example `task_service`).
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// The resolved but deliberately uninvoked enqueue identity. Its closure
    /// reaches the contract-bearing `std.bytes.byte_to_i64`, so invocation
    /// is refused (`SPX-F102`) and the host mirrors its truth table.
    pub fn enqueue_outcome_id(&self) -> &str {
        &self.enqueue_outcome
    }
}

fn sole(
    program: &semaprax::hir::ResolvedProgram,
    decision: &str,
) -> Result<String, DecisionRefusal> {
    let suffix = format!(".core.{decision}");
    let mut found: Option<String> = None;
    for function in &program.functions {
        let id = function.id.as_str();
        if id.len() > suffix.len()
            && id.ends_with(suffix.as_str())
            && !id[..id.len() - suffix.len()].contains('.')
        {
            if found.is_some() {
                return Err(DecisionRefusal::Unresolved);
            }
            found = Some(id.to_owned());
        }
    }
    found.ok_or(DecisionRefusal::Unresolved)
}

fn prefix_of(identity: &str) -> Option<&str> {
    identity
        .strip_suffix(".core.request_is_admitted")
        .or_else(|| identity.strip_suffix(".core.identifier_is_valid"))
        .or_else(|| identity.strip_suffix(".core.method_is_rejected"))
        .or_else(|| identity.strip_suffix(".core.task_owner_authorized"))
        .or_else(|| identity.strip_suffix(".core.enqueue_outcome"))
}

/// One bound decision engine over an operator-retained revision.
pub struct DecisionEngine<'revision> {
    revision: &'revision ProjectRevision,
    identities: DecisionIdentities,
    max_steps: usize,
}

impl<'revision> DecisionEngine<'revision> {
    /// Bind the decision set of one retained revision.
    pub fn bind(
        revision: &'revision ProjectRevision,
        max_steps: usize,
    ) -> Result<Self, DecisionRefusal> {
        if max_steps == 0 {
            return Err(DecisionRefusal::EvaluationFailed);
        }
        Ok(Self {
            revision,
            identities: DecisionIdentities::resolve(revision)?,
            max_steps,
        })
    }

    pub fn identities(&self) -> &DecisionIdentities {
        &self.identities
    }

    fn invoke_bool(
        &self,
        identity: &str,
        arguments: &[PublicApiArgument<'_>],
    ) -> Result<bool, DecisionRefusal> {
        let evaluation = self
            .revision
            .evaluate_service_decision_v1(identity, arguments, self.max_steps)
            .map_err(|_| DecisionRefusal::EvaluationFailed)?;
        match evaluation.outcome {
            PublicApiEvaluationOutcome::Returned(PublicApiValue::Bool(value)) => Ok(value),
            PublicApiEvaluationOutcome::Returned(_) => Err(DecisionRefusal::UnexpectedResult),
            _ => Err(DecisionRefusal::EvaluationFailed),
        }
    }

    /// Evaluate the scaffold's request-line admission decision.
    pub fn request_is_admitted(
        &self,
        method: &[u8],
        target: &[u8],
    ) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.request_is_admitted,
            &[
                PublicApiArgument::BorrowSliceU8(method),
                PublicApiArgument::BorrowSliceU8(target),
            ],
        )
    }

    /// Evaluate the scaffold's identifier-grammar decision.
    pub fn identifier_is_valid(&self, name: &[u8]) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.identifier_is_valid,
            &[PublicApiArgument::BorrowSliceU8(name)],
        )
    }

    /// Evaluate the scaffold's method-rejection decision.
    pub fn method_is_rejected(&self, method: &[u8]) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.method_is_rejected,
            &[PublicApiArgument::BorrowSliceU8(method)],
        )
    }

    /// Evaluate the scaffold's row-level authorization decision.
    pub fn task_owner_authorized(
        &self,
        task_owner_account_id: i64,
        session_account_id: i64,
        session_usable: bool,
    ) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.task_owner_authorized,
            &[
                PublicApiArgument::I64(task_owner_account_id),
                PublicApiArgument::I64(session_account_id),
                PublicApiArgument::Bool(session_usable),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semaprax::project::with_authenticated_project;

    fn example_revision() -> std::sync::Arc<semaprax::project::ProjectRevision> {
        // The project loader rejects `.`/`..` components, so the fixture
        // path is canonicalized before loading.
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("task-service-project")
            .join("semaprax.toml")
            .canonicalize()
            .expect("canonicalize task-service-project fixture");
        with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
            .expect("load task-service-project fixture")
    }

    #[test]
    fn scaffold_decisions_resolve_and_answer() {
        let revision = example_revision();
        let engine = DecisionEngine::bind(&revision, DECISION_MAX_STEPS).unwrap();
        assert_eq!(engine.identities().prefix(), "task_service");
        assert!(engine.request_is_admitted(b"GET", b"/tasks").unwrap());
        assert!(!engine.request_is_admitted(b"get", b"/tasks").unwrap());
        assert!(engine.identifier_is_valid(b"alice").unwrap());
        assert!(!engine.identifier_is_valid(b"1task").unwrap());
        assert!(engine.method_is_rejected(b"get").unwrap());
        assert!(!engine.method_is_rejected(b"GET").unwrap());
        assert!(engine.task_owner_authorized(1, 1, true).unwrap());
        assert!(!engine.task_owner_authorized(1, 2, true).unwrap());
        assert!(!engine.task_owner_authorized(1, 1, false).unwrap());
        // The enqueue identity resolves (proving the exact decision set)
        // but is deliberately never invoked (see below).
        assert_eq!(
            engine.identities().enqueue_outcome_id(),
            "task_service.core.enqueue_outcome"
        );
    }

    #[test]
    fn hostile_selection_fails_closed() {
        let revision = example_revision();
        // An unknown identity is not invocable.
        assert!(revision
            .evaluate_service_decision_v1("task_service.core.absent", &[], DECISION_MAX_STEPS)
            .is_err());
        // A wrong arity is not invocable.
        assert!(revision
            .evaluate_service_decision_v1(
                "task_service.core.identifier_is_valid",
                &[],
                DECISION_MAX_STEPS
            )
            .is_err());
        // A decision outside the frozen vocabulary (usize parameter) is not
        // invocable, even though it is linked and checked.
        assert!(revision
            .evaluate_service_decision_v1(
                "task_service.core.job_status_is_complete",
                &[PublicApiArgument::I64(0)],
                DECISION_MAX_STEPS
            )
            .is_err());
        // A vocabulary-admitting decision whose closure reaches a
        // contract-bearing callee (`std.bytes.byte_to_i64`) is refused as
        // well; the host mirrors its truth table instead of invoking it.
        let enqueue = revision.evaluate_service_decision_v1(
            "task_service.core.enqueue_outcome",
            &[
                PublicApiArgument::Bool(false),
                PublicApiArgument::BorrowSliceU8(b"job-1"),
                PublicApiArgument::BorrowSliceU8(b"job-1"),
            ],
            DECISION_MAX_STEPS,
        );
        assert!(enqueue.is_err());
        let diagnostics = enqueue.err().unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-F102");
        assert!(diagnostics[0].message.contains("std.bytes.byte_to_i64"));
        // The entry closure root itself is not invocable: its closure
        // reaches contract-bearing callees, so the host never selects it
        // and resolves only the admission decisions above.
        assert!(revision
            .evaluate_service_decision_v1("task_service.app.main", &[], DECISION_MAX_STEPS)
            .is_err());
        // A zero step budget binds nothing.
        assert!(matches!(
            DecisionEngine::bind(&revision, 0),
            Err(DecisionRefusal::EvaluationFailed)
        ));
    }
}
