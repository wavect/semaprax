//! Checked scaffold-decision invocation for the reference service.
//!
//! Every admission question the host must ask -- is this request line
//! admitted, is this identifier valid, does this session own this row, is
//! this enqueue new or a duplicate -- is answered by evaluating the
//! operator-loaded project's own checked `.spx` decision functions through
//! [`ProjectRevision::evaluate_service_decision_v1`]. Selection authority
//! stays with the retained authenticated revision: only explicit stable
//! identities already linked into its entry closure can run, and only with
//! the bounded public-invocation vocabulary (`i64`, `u8`, `usize`, `bool`,
//! borrowed bytes).
//!
//! Only the scaffold decisions whose signatures that vocabulary admits,
//! and whose entire call closure is effect- and contract-free, are
//! invocable here: `request_is_admitted`, `identifier_is_valid`,
//! `method_is_rejected`, `task_owner_authorized`, and the three session
//! predicates/transitions.
//! `registration_admitted`, `enqueue_is_legal`, `enqueue_outcome`, the three
//! task transaction decisions, `mark_job_succeeded`,
//! `completed_job_log_is_admitted`, `completed_job_metric_is_admitted`,
//! `completed_job_export_is_admitted`, and the opt-in v2 webhook decision are invoked through the checked
//! public-API seam before their corresponding host work. The remaining
//! scaffold decisions (migration and trace policies)
//! retain fixture-mode coverage until a host route needs them.
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
/// `<module>.core.<decision>` for its own module name. Every identity used by
/// the host resolves from that one family before any route is served.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionIdentities {
    prefix: String,
    request_is_admitted: String,
    identifier_is_valid: String,
    registration_admitted: String,
    method_is_rejected: String,
    task_owner_authorized: String,
    session_is_usable: String,
    session_next_state_on_access: String,
    session_next_state_on_logout: String,
    enqueue_outcome: String,
    enqueue_is_legal: String,
    create_is_committed: String,
    update_is_committed: String,
    delete_is_committed: String,
    mark_job_succeeded: String,
    job_status_is_complete: String,
    completed_job_log_is_admitted: String,
    completed_job_metric_is_admitted: String,
    completed_job_export_is_admitted: String,
    completed_job_webhook_is_admitted: Option<String>,
}

impl DecisionIdentities {
    /// Resolve the decision set from the retained entry closure. Exactly one
    /// unambiguous `<prefix>.core.*` family must exist.
    pub fn resolve(revision: &ProjectRevision) -> Result<Self, DecisionRefusal> {
        let program = revision.entry_program();
        let request_is_admitted = sole(program, "request_is_admitted")?;
        let identifier_is_valid = sole(program, "identifier_is_valid")?;
        let registration_admitted = sole(program, "registration_admitted")?;
        let method_is_rejected = sole(program, "method_is_rejected")?;
        let task_owner_authorized = sole(program, "task_owner_authorized")?;
        let session_is_usable = sole(program, "session_is_usable")?;
        let session_next_state_on_access = sole(program, "session_next_state_on_access")?;
        let session_next_state_on_logout = sole(program, "session_next_state_on_logout")?;
        let enqueue_outcome = sole(program, "enqueue_outcome")?;
        let enqueue_is_legal = sole(program, "enqueue_is_legal")?;
        let create_is_committed = sole(program, "create_is_committed")?;
        let update_is_committed = sole(program, "update_is_committed")?;
        let delete_is_committed = sole(program, "delete_is_committed")?;
        let mark_job_succeeded = sole(program, "mark_job_succeeded")?;
        let job_status_is_complete = sole(program, "job_status_is_complete")?;
        let completed_job_log_is_admitted = sole(program, "completed_job_log_is_admitted")?;
        let completed_job_metric_is_admitted = sole(program, "completed_job_metric_is_admitted")?;
        let completed_job_export_is_admitted = sole(program, "completed_job_export_is_admitted")?;
        let completed_job_webhook_is_admitted =
            optional_sole(program, "completed_job_webhook_is_admitted")?;
        let prefix = prefix_of(&request_is_admitted).ok_or(DecisionRefusal::Unresolved)?;
        for identity in [
            &identifier_is_valid,
            &registration_admitted,
            &method_is_rejected,
            &task_owner_authorized,
            &session_is_usable,
            &session_next_state_on_access,
            &session_next_state_on_logout,
            &enqueue_outcome,
            &enqueue_is_legal,
            &create_is_committed,
            &update_is_committed,
            &delete_is_committed,
            &mark_job_succeeded,
            &job_status_is_complete,
            &completed_job_log_is_admitted,
            &completed_job_metric_is_admitted,
            &completed_job_export_is_admitted,
        ] {
            if prefix_of(identity) != Some(prefix) {
                return Err(DecisionRefusal::Unresolved);
            }
        }
        if completed_job_webhook_is_admitted
            .as_deref()
            .is_some_and(|id| prefix_of(id) != Some(prefix))
        {
            return Err(DecisionRefusal::Unresolved);
        }
        Ok(Self {
            prefix: prefix.to_owned(),
            request_is_admitted,
            identifier_is_valid,
            registration_admitted,
            method_is_rejected,
            task_owner_authorized,
            session_is_usable,
            session_next_state_on_access,
            session_next_state_on_logout,
            enqueue_outcome,
            enqueue_is_legal,
            create_is_committed,
            update_is_committed,
            delete_is_committed,
            mark_job_succeeded,
            job_status_is_complete,
            completed_job_log_is_admitted,
            completed_job_metric_is_admitted,
            completed_job_export_is_admitted,
            completed_job_webhook_is_admitted,
        })
    }

    /// The discovered decision-family prefix (for example `task_service`).
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
}

fn sole(
    program: &semaprax::hir::ResolvedProgram,
    decision: &str,
) -> Result<String, DecisionRefusal> {
    optional_sole(program, decision)?.ok_or(DecisionRefusal::Unresolved)
}

fn optional_sole(
    program: &semaprax::hir::ResolvedProgram,
    decision: &str,
) -> Result<Option<String>, DecisionRefusal> {
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
    Ok(found)
}

fn prefix_of(identity: &str) -> Option<&str> {
    identity
        .strip_suffix(".core.request_is_admitted")
        .or_else(|| identity.strip_suffix(".core.identifier_is_valid"))
        .or_else(|| identity.strip_suffix(".core.registration_admitted"))
        .or_else(|| identity.strip_suffix(".core.method_is_rejected"))
        .or_else(|| identity.strip_suffix(".core.task_owner_authorized"))
        .or_else(|| identity.strip_suffix(".core.session_is_usable"))
        .or_else(|| identity.strip_suffix(".core.session_next_state_on_access"))
        .or_else(|| identity.strip_suffix(".core.session_next_state_on_logout"))
        .or_else(|| identity.strip_suffix(".core.enqueue_outcome"))
        .or_else(|| identity.strip_suffix(".core.enqueue_is_legal"))
        .or_else(|| identity.strip_suffix(".core.create_is_committed"))
        .or_else(|| identity.strip_suffix(".core.update_is_committed"))
        .or_else(|| identity.strip_suffix(".core.delete_is_committed"))
        .or_else(|| identity.strip_suffix(".core.mark_job_succeeded"))
        .or_else(|| identity.strip_suffix(".core.job_status_is_complete"))
        .or_else(|| identity.strip_suffix(".core.completed_job_log_is_admitted"))
        .or_else(|| identity.strip_suffix(".core.completed_job_metric_is_admitted"))
        .or_else(|| identity.strip_suffix(".core.completed_job_export_is_admitted"))
        .or_else(|| identity.strip_suffix(".core.completed_job_webhook_is_admitted"))
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

    fn invoke_usize(
        &self,
        identity: &str,
        arguments: &[PublicApiArgument<'_>],
    ) -> Result<u64, DecisionRefusal> {
        let evaluation = self
            .revision
            .evaluate_service_decision_v1(identity, arguments, self.max_steps)
            .map_err(|_| DecisionRefusal::EvaluationFailed)?;
        match evaluation.outcome {
            PublicApiEvaluationOutcome::Returned(PublicApiValue::Usize(value)) => Ok(value),
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

    /// Evaluate the checked account-capacity and password-policy admission
    /// decision before deriving or hashing a registration secret.
    pub fn registration_admitted(
        &self,
        username: &[u8],
        active_count: u64,
        max_accounts: u64,
        password_memory_cost_kib: u64,
        password_time_cost: u64,
        password_parallelism: u64,
    ) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.registration_admitted,
            &[
                PublicApiArgument::BorrowSliceU8(username),
                PublicApiArgument::Usize(active_count),
                PublicApiArgument::Usize(max_accounts),
                PublicApiArgument::Usize(password_memory_cost_kib),
                PublicApiArgument::Usize(password_time_cost),
                PublicApiArgument::Usize(password_parallelism),
            ],
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

    /// Evaluate the scaffold's session deadline predicate. The host supplies
    /// persisted deadline facts; the checked source selects usability.
    pub fn session_is_usable(
        &self,
        state: u64,
        now_tick: u64,
        idle_deadline_tick: u64,
        absolute_deadline_tick: u64,
    ) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.session_is_usable,
            &[
                PublicApiArgument::Usize(state),
                PublicApiArgument::Usize(now_tick),
                PublicApiArgument::Usize(idle_deadline_tick),
                PublicApiArgument::Usize(absolute_deadline_tick),
            ],
        )
    }

    /// Evaluate the source transition selected for an access attempt. The
    /// returned state is persisted by the host, preserving terminality across
    /// restart instead of treating expiry as a transient authorization check.
    pub fn session_next_state_on_access(
        &self,
        state: u64,
        now_tick: u64,
        idle_deadline_tick: u64,
        absolute_deadline_tick: u64,
    ) -> Result<u64, DecisionRefusal> {
        self.invoke_usize(
            &self.identities.session_next_state_on_access,
            &[
                PublicApiArgument::Usize(state),
                PublicApiArgument::Usize(now_tick),
                PublicApiArgument::Usize(idle_deadline_tick),
                PublicApiArgument::Usize(absolute_deadline_tick),
            ],
        )
    }

    /// Evaluate the source transition selected for an explicit logout.
    pub fn session_next_state_on_logout(&self, state: u64) -> Result<u64, DecisionRefusal> {
        self.invoke_usize(
            &self.identities.session_next_state_on_logout,
            &[PublicApiArgument::Usize(state)],
        )
    }

    /// Evaluate source admission using explicit scheduling facts. This method
    /// does not read a clock or create scheduling authority.
    pub fn enqueue_is_legal(
        &self,
        state: u64,
        now_tick: u64,
        next_run_tick: u64,
    ) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.enqueue_is_legal,
            &[
                PublicApiArgument::Usize(state),
                PublicApiArgument::Usize(now_tick),
                PublicApiArgument::Usize(next_run_tick),
            ],
        )
    }

    /// Evaluate the scaffold's contract-free idempotent-enqueue decision.
    pub fn enqueue_outcome(
        &self,
        key_exists: bool,
        existing_descriptor: &[u8],
        candidate_descriptor: &[u8],
    ) -> Result<u64, DecisionRefusal> {
        self.invoke_usize(
            &self.identities.enqueue_outcome,
            &[
                PublicApiArgument::Bool(key_exists),
                PublicApiArgument::BorrowSliceU8(existing_descriptor),
                PublicApiArgument::BorrowSliceU8(candidate_descriptor),
            ],
        )
    }

    /// Evaluate the scaffold's creation decision before the host constructs
    /// or commits a new task. The host supplies the fixed idle transaction
    /// fact, so source refusal leaves the authoritative state untouched.
    pub fn create_is_committed(&self, transaction_state: u64) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.create_is_committed,
            &[PublicApiArgument::Usize(transaction_state)],
        )
    }

    /// Evaluate the scaffold's transaction decision before publishing one
    /// task-status update. The host supplies the fixed idle transaction fact;
    /// it does not synthesize a source-selected commit after mutation.
    pub fn update_is_committed(&self, transaction_state: u64) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.update_is_committed,
            &[PublicApiArgument::Usize(transaction_state)],
        )
    }

    /// Evaluate the scaffold's deletion decision before publishing one task
    /// removal. The host supplies the fixed idle transaction fact before it
    /// constructs a candidate state.
    pub fn delete_is_committed(&self, transaction_state: u64) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.delete_is_committed,
            &[PublicApiArgument::Usize(transaction_state)],
        )
    }

    /// Evaluate the source-selected terminal code before the host publishes
    /// its one completed-job state. The reference profile binds its only
    /// completion attempt; this decision does not create retry authority.
    pub fn mark_job_succeeded(
        &self,
        attempt: u8,
        max_attempts: u8,
    ) -> Result<u64, DecisionRefusal> {
        self.invoke_usize(
            &self.identities.mark_job_succeeded,
            &[
                PublicApiArgument::U8(attempt),
                PublicApiArgument::U8(max_attempts),
            ],
        )
    }

    /// Evaluate whether the persisted job-status representation is terminal
    /// before the host attempts any completion delivery.
    pub fn job_status_is_complete(&self, state: u64) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.job_status_is_complete,
            &[PublicApiArgument::Usize(state)],
        )
    }

    /// Evaluate the checked structured-log policy before serializing an OTLP
    /// completion. Bits 0..=5 identify password, API key, bearer token, session
    /// token, webhook signing secret, and SMTP credential. The source adapter
    /// rejects unknown bits and calls its nine-argument structured-log policy.
    pub fn completed_job_log_is_admitted(
        &self,
        level: u8,
        threshold: u8,
        field_count: u64,
        secret_flags: u8,
    ) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.completed_job_log_is_admitted,
            &[
                PublicApiArgument::U8(level),
                PublicApiArgument::U8(threshold),
                PublicApiArgument::Usize(field_count),
                PublicApiArgument::U8(secret_flags),
            ],
        )
    }

    /// Evaluate the checked metric policy before the completion route advances
    /// to outbound delivery. This selects source semantics only; it emits no
    /// metric by itself.
    pub fn completed_job_metric_is_admitted(
        &self,
        label: &[u8],
        value: &[u8],
        carries_secret: bool,
    ) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.completed_job_metric_is_admitted,
            &[
                PublicApiArgument::BorrowSliceU8(label),
                PublicApiArgument::BorrowSliceU8(value),
                PublicApiArgument::Bool(carries_secret),
            ],
        )
    }

    /// Evaluate the checked admission policy before one completion event is
    /// handed to the host-created outbound adapter.
    pub fn completed_job_webhook_is_admitted(
        &self,
        signature: &[u8],
        payload_len: u64,
        signed_at: i64,
        now: i64,
        key_exists: bool,
        existing_descriptor: &[u8],
        candidate_descriptor: &[u8],
    ) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            self.identities
                .completed_job_webhook_is_admitted
                .as_deref()
                .ok_or(DecisionRefusal::Unresolved)?,
            &[
                PublicApiArgument::BorrowSliceU8(signature),
                PublicApiArgument::Usize(payload_len),
                PublicApiArgument::I64(signed_at),
                PublicApiArgument::I64(now),
                PublicApiArgument::Bool(key_exists),
                PublicApiArgument::BorrowSliceU8(existing_descriptor),
                PublicApiArgument::BorrowSliceU8(candidate_descriptor),
            ],
        )
    }

    pub fn completed_job_export_is_admitted(
        &self,
        existing_depth: i64,
        batch_count: u64,
        batch_bytes: u64,
        target: &[u8],
    ) -> Result<bool, DecisionRefusal> {
        self.invoke_bool(
            &self.identities.completed_job_export_is_admitted,
            &[
                PublicApiArgument::I64(existing_depth),
                PublicApiArgument::Usize(batch_count),
                PublicApiArgument::Usize(batch_bytes),
                PublicApiArgument::BorrowSliceU8(target),
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
        assert!(engine
            .registration_admitted(b"alice", 0, 64, 19_456, 2, 1)
            .unwrap());
        assert!(!engine
            .registration_admitted(b"alice", 64, 64, 19_456, 2, 1)
            .unwrap());
        assert!(engine.method_is_rejected(b"get").unwrap());
        assert!(!engine.method_is_rejected(b"GET").unwrap());
        assert!(engine.task_owner_authorized(1, 1, true).unwrap());
        assert!(!engine.task_owner_authorized(1, 2, true).unwrap());
        assert!(!engine.task_owner_authorized(1, 1, false).unwrap());
        assert!(engine.session_is_usable(0, 1_000, 1_900, 5_000).unwrap());
        assert!(!engine.session_is_usable(0, 1_900, 1_900, 5_000).unwrap());
        assert!(!engine.session_is_usable(0, 5_000, 6_000, 5_000).unwrap());
        assert_eq!(
            engine
                .session_next_state_on_access(0, 1_000, 1_900, 5_000)
                .unwrap(),
            0
        );
        assert_eq!(
            engine
                .session_next_state_on_access(0, 1_900, 1_900, 5_000)
                .unwrap(),
            3
        );
        assert_eq!(engine.session_next_state_on_logout(0).unwrap(), 5);
        assert_eq!(engine.enqueue_outcome(false, b"", b"job-1").unwrap(), 0);
        assert_eq!(engine.enqueue_outcome(true, b"job-1", b"job-1").unwrap(), 1);
        assert_eq!(engine.enqueue_outcome(true, b"job-1", b"job-2").unwrap(), 2);
        assert!(engine.create_is_committed(0).unwrap());
        assert!(!engine.create_is_committed(1).unwrap());
        assert!(engine.update_is_committed(0).unwrap());
        assert!(!engine.update_is_committed(1).unwrap());
        assert!(engine.delete_is_committed(0).unwrap());
        assert!(!engine.delete_is_committed(1).unwrap());
        assert!(!engine.job_status_is_complete(0).unwrap());
        assert!(engine.job_status_is_complete(4).unwrap());
        assert!(engine
            .completed_job_export_is_admitted(0, 1, 128, b"https://telemetry.example")
            .unwrap());
        assert!(!engine
            .completed_job_export_is_admitted(0, 0, 128, b"https://telemetry.example")
            .unwrap());
        assert_eq!(
            prefix_of("task_service.core.completed_job_webhook_is_admitted"),
            Some("task_service")
        );
        assert!(engine
            .completed_job_webhook_is_admitted(&[b'a'; 64], 128, 1000, 1100, false, b"", b"body")
            .unwrap());
        assert!(!engine
            .completed_job_webhook_is_admitted(&[b'a'; 64], 128, 1000, 1301, false, b"", b"body")
            .unwrap());
        let overlong_target = [b'a'; 129];
        assert!(!engine
            .completed_job_export_is_admitted(0, 1, 128, &overlong_target)
            .unwrap());
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
        // Scalar usize is admitted, but an i64 argument cannot stand in for
        // its declared type, even though the function is linked and checked.
        assert!(revision
            .evaluate_service_decision_v1(
                "task_service.core.job_status_is_complete",
                &[PublicApiArgument::I64(0)],
                DECISION_MAX_STEPS
            )
            .is_err());
        // The contract-bearing standard function remains outside the service
        // vocabulary, while the scaffold wrapper above is admitted.
        let enqueue = revision.evaluate_service_decision_v1(
            "std.jobs.idempotency.enqueue_outcome",
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
