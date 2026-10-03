//! Bounded, authority-neutral planning for a prepared Project interpreter.
//! A plan is a private in-memory value; its JSON is diagnostic evidence only.

use std::sync::Arc;

use sha2::{Digest as _, Sha256};

use super::{
    PreparedProjectExecution, PreparedProjectExecutionOptions, PreparedProjectInterpreter,
    PreparedProjectInterpreterOptions, ProjectExecutionCancellation, ProjectRevision,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedFunction, ResolvedProgram};

pub const HOT_RELOAD_PLAN_SCHEMA: &str = "semaprax.hot-reload-plan.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotReloadReason {
    StaleGeneration,
    StaleCandidate,
    BusyBoundary,
    InvalidCandidate,
    PolicyChanged,
    UnsupportedTarget,
    IncompatibleClosure,
    IdenticalRevision,
    GenerationExhausted,
    TerminalUncertainty,
}

impl HotReloadReason {
    pub const fn name(self) -> &'static str {
        match self {
            Self::StaleGeneration => "stale_generation",
            Self::StaleCandidate => "stale_candidate",
            Self::BusyBoundary => "busy_boundary",
            Self::InvalidCandidate => "invalid_candidate",
            Self::PolicyChanged => "policy_changed",
            Self::UnsupportedTarget => "unsupported_target",
            Self::IncompatibleClosure => "incompatible_closure",
            Self::IdenticalRevision => "identical_revision",
            Self::GenerationExhausted => "generation_exhausted",
            Self::TerminalUncertainty => "terminal_uncertainty",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotReloadDecision {
    EligibleCodeReplacement,
    UnsupportedRestartRequired,
    Rejected,
    Unchanged,
}

impl HotReloadDecision {
    const fn name(self) -> &'static str {
        match self {
            Self::EligibleCodeReplacement => "eligible_code_replacement",
            Self::UnsupportedRestartRequired => "unsupported_restart_required",
            Self::Rejected => "rejected",
            Self::Unchanged => "unchanged",
        }
    }
}

#[derive(Debug)]
pub struct HotReloadFailure {
    pub reason: HotReloadReason,
    pub diagnostics: Vec<Diagnostic>,
}

impl HotReloadFailure {
    fn new(reason: HotReloadReason, message: &'static str) -> Self {
        Self {
            reason,
            diagnostics: vec![Diagnostic::io("SPX-HR400", message)],
        }
    }

    fn candidate(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            reason: HotReloadReason::InvalidCandidate,
            diagnostics,
        }
    }
}

/// A view can be serialized, but it cannot be parsed into an activation plan.
/// All fields are private, including the exact pending-submission identity.
pub struct HotReloadPlan {
    generation: u64,
    submission: u64,
    expected_project_revision: String,
    expected_program_root: String,
    candidate_project_revision: String,
    candidate_program_root: String,
    entry_id: String,
    test_id: String,
    decision: HotReloadDecision,
    reason: Option<HotReloadReason>,
    digest: String,
}

impl HotReloadPlan {
    pub fn decision(&self) -> HotReloadDecision {
        self.decision
    }

    pub fn reason(&self) -> Option<HotReloadReason> {
        self.reason
    }

    pub fn to_json(&self) -> String {
        serde_json::json!({
            "schema": HOT_RELOAD_PLAN_SCHEMA,
            "generation": self.generation,
            "expected_project_revision": self.expected_project_revision,
            "expected_program_root": self.expected_program_root,
            "candidate_project_revision": self.candidate_project_revision,
            "candidate_program_root": self.candidate_program_root,
            "entry_id": self.entry_id,
            "test_id": self.test_id,
            "decision": self.decision.name(),
            "reason": self.reason.map(HotReloadReason::name),
            "digest": self.digest,
            "authority": "none",
        })
        .to_string()
    }
}

/// Holds one active revision and at most one checked pending candidate. The
/// embedded worker is private, so replacement has one session commit path.
pub struct HotReloadSession {
    generation: u64,
    submission: u64,
    active: Arc<ProjectRevision>,
    pending: Option<Arc<ProjectRevision>>,
    worker: PreparedProjectInterpreter,
    terminal: bool,
}

impl HotReloadSession {
    pub fn new(
        active: Arc<ProjectRevision>,
        options: PreparedProjectInterpreterOptions,
    ) -> Result<Self, HotReloadFailure> {
        active.check().map_err(HotReloadFailure::candidate)?;
        let worker = active
            .prepare_interpreter(options)
            .map_err(HotReloadFailure::candidate)?;
        Ok(Self {
            generation: 0,
            submission: 0,
            active,
            pending: None,
            worker,
            terminal: false,
        })
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn active_project_revision(&self) -> &str {
        self.active.project_revision()
    }

    pub fn terminal(&self) -> bool {
        self.terminal
    }

    /// Candidate admission is read-only and does not touch the worker.
    pub fn admit_candidate(
        &mut self,
        candidate: Arc<ProjectRevision>,
    ) -> Result<(), HotReloadFailure> {
        if self.terminal {
            return Err(HotReloadFailure::new(
                HotReloadReason::TerminalUncertainty,
                "hot reload worker is terminal",
            ));
        }
        let next = self.submission.checked_add(1).ok_or_else(|| {
            HotReloadFailure::new(
                HotReloadReason::GenerationExhausted,
                "hot reload submission identity is exhausted",
            )
        })?;
        candidate.check().map_err(HotReloadFailure::candidate)?;
        self.pending = Some(candidate);
        self.submission = next;
        Ok(())
    }

    /// Reconstruct compatibility from retained checked HIR, never a cache
    /// label, caller-provided digest or previously rendered plan JSON.
    pub fn plan(&self) -> Result<HotReloadPlan, HotReloadFailure> {
        if self.terminal {
            return Err(HotReloadFailure::new(
                HotReloadReason::TerminalUncertainty,
                "hot reload worker is terminal",
            ));
        }
        let candidate = self.pending.as_ref().ok_or_else(|| {
            HotReloadFailure::new(
                HotReloadReason::InvalidCandidate,
                "hot reload has no admitted candidate",
            )
        })?;
        let old_root = self
            .active
            .program_root()
            .map_err(HotReloadFailure::candidate)?;
        let new_root = candidate
            .program_root()
            .map_err(HotReloadFailure::candidate)?;
        let entry_id = self.active.entry_program().entrypoint.as_str().to_owned();
        let test_id = self.active.test_program().entrypoint.as_str().to_owned();
        let (decision, reason) = if self.active.project_revision() == candidate.project_revision() {
            (
                HotReloadDecision::Unchanged,
                Some(HotReloadReason::IdenticalRevision),
            )
        } else if !self.active.source_agents().is_empty() || !candidate.source_agents().is_empty() {
            // Durable checkpoint handoff is owned by the source migration
            // protocol, not by the prepared interpreter replacement lane.
            (
                HotReloadDecision::UnsupportedRestartRequired,
                Some(HotReloadReason::UnsupportedTarget),
            )
        } else if self.active.manifest().project_profile() != candidate.manifest().project_profile()
        {
            (
                HotReloadDecision::Rejected,
                Some(HotReloadReason::PolicyChanged),
            )
        } else if compatible_program(self.active.entry_program(), candidate.entry_program())
            && compatible_program(self.active.test_program(), candidate.test_program())
        {
            (HotReloadDecision::EligibleCodeReplacement, None)
        } else {
            (
                HotReloadDecision::UnsupportedRestartRequired,
                Some(HotReloadReason::IncompatibleClosure),
            )
        };
        let expected_project_revision = self.active.project_revision().to_owned();
        let candidate_project_revision = candidate.project_revision().to_owned();
        let expected_program_root = old_root.program_root().to_owned();
        let candidate_program_root = new_root.program_root().to_owned();
        let digest = plan_digest(
            self.generation,
            self.submission,
            &expected_project_revision,
            &expected_program_root,
            &candidate_project_revision,
            &candidate_program_root,
            &entry_id,
            &test_id,
            decision,
            reason,
        );
        Ok(HotReloadPlan {
            generation: self.generation,
            submission: self.submission,
            expected_project_revision,
            expected_program_root,
            candidate_project_revision,
            candidate_program_root,
            entry_id,
            test_id,
            decision,
            reason,
            digest,
        })
    }

    pub fn activate(&mut self, plan: HotReloadPlan) -> Result<(), HotReloadFailure> {
        if self.terminal {
            return Err(HotReloadFailure::new(
                HotReloadReason::TerminalUncertainty,
                "hot reload worker is terminal",
            ));
        }
        if plan.generation != self.generation {
            return Err(HotReloadFailure::new(
                HotReloadReason::StaleGeneration,
                "hot reload plan targets an older generation",
            ));
        }
        let pending = self.pending.as_ref().ok_or_else(|| {
            HotReloadFailure::new(
                HotReloadReason::StaleCandidate,
                "hot reload candidate is no longer pending",
            )
        })?;
        if plan.submission != self.submission
            || plan.expected_project_revision != self.active.project_revision()
            || plan.candidate_project_revision != pending.project_revision()
            || plan.digest
                != plan_digest(
                    plan.generation,
                    plan.submission,
                    &plan.expected_project_revision,
                    &plan.expected_program_root,
                    &plan.candidate_project_revision,
                    &plan.candidate_program_root,
                    &plan.entry_id,
                    &plan.test_id,
                    plan.decision,
                    plan.reason,
                )
        {
            return Err(HotReloadFailure::new(
                HotReloadReason::StaleCandidate,
                "hot reload plan no longer matches admitted candidate",
            ));
        }
        let fresh = self.plan()?;
        if fresh.digest != plan.digest {
            return Err(HotReloadFailure::new(
                HotReloadReason::StaleCandidate,
                "hot reload compatibility facts changed",
            ));
        }
        if plan.decision != HotReloadDecision::EligibleCodeReplacement {
            return Err(HotReloadFailure::new(
                plan.reason.unwrap_or(HotReloadReason::UnsupportedTarget),
                "hot reload candidate requires an explicit restart or migration",
            ));
        }
        let next = self.generation.checked_add(1).ok_or_else(|| {
            HotReloadFailure::new(
                HotReloadReason::GenerationExhausted,
                "hot reload generation is exhausted",
            )
        })?;
        match self
            .worker
            .replace_revision(self.active.project_revision(), Arc::clone(pending))
        {
            Ok(()) => {
                self.active = self.pending.take().expect("validated pending candidate");
                self.generation = next;
                Ok(())
            }
            Err(diagnostics) => {
                if diagnostics.iter().any(|row| {
                    row.code == "SPX-F109"
                        && row.message
                            == "prepared interpreter already has one outstanding execution"
                }) {
                    Err(HotReloadFailure {
                        reason: HotReloadReason::BusyBoundary,
                        diagnostics,
                    })
                } else if diagnostics.iter().any(|row| row.code == "SPX-F109") {
                    self.terminal = true;
                    Err(HotReloadFailure {
                        reason: HotReloadReason::TerminalUncertainty,
                        diagnostics,
                    })
                } else {
                    self.pending = None;
                    Err(HotReloadFailure {
                        reason: HotReloadReason::InvalidCandidate,
                        diagnostics,
                    })
                }
            }
        }
    }

    pub fn execute_entry(
        &self,
        options: &PreparedProjectExecutionOptions,
        cancellation: &ProjectExecutionCancellation,
    ) -> Result<PreparedProjectExecution, Vec<Diagnostic>> {
        if self.terminal {
            return Err(vec![Diagnostic::io(
                "SPX-HR400",
                "hot reload worker has terminal uncertainty",
            )]);
        }
        self.worker.execute_entry(options, cancellation)
    }
}

fn compatible_program(old: &ResolvedProgram, candidate: &ResolvedProgram) -> bool {
    if old.entrypoint != candidate.entrypoint
        || old.permits != candidate.permits
        || old.types != candidate.types
        || old.interfaces != candidate.interfaces
    {
        return false;
    }
    let old_ids = old
        .functions
        .iter()
        .map(|function| function.id.as_str())
        .collect::<Vec<_>>();
    let new_ids = candidate
        .functions
        .iter()
        .map(|function| function.id.as_str())
        .collect::<Vec<_>>();
    if old_ids != new_ids {
        return false;
    }
    old.functions
        .iter()
        .zip(&candidate.functions)
        .all(|(left, right)| compatible_function(left, right))
}

fn compatible_function(left: &ResolvedFunction, right: &ResolvedFunction) -> bool {
    left.id == right.id
        && left.return_type == right.return_type
        && left.effects == right.effects
        && left.yields == right.yields
        && left.requires == right.requires
        && left.ensures == right.ensures
        && left.params.len() == right.params.len()
        && left.params.iter().zip(&right.params).all(|(left, right)| {
            left.id == right.id && left.ty == right.ty && left.ownership == right.ownership
        })
}

#[allow(clippy::too_many_arguments)]
fn plan_digest(
    generation: u64,
    submission: u64,
    expected_project_revision: &str,
    expected_program_root: &str,
    candidate_project_revision: &str,
    candidate_program_root: &str,
    entry_id: &str,
    test_id: &str,
    decision: HotReloadDecision,
    reason: Option<HotReloadReason>,
) -> String {
    let value = serde_json::json!({
        "schema": HOT_RELOAD_PLAN_SCHEMA,
        "generation": generation,
        "submission": submission,
        "expected_project_revision": expected_project_revision,
        "expected_program_root": expected_program_root,
        "candidate_project_revision": candidate_project_revision,
        "candidate_program_root": candidate_program_root,
        "entry_id": entry_id,
        "test_id": test_id,
        "decision": decision.name(),
        "reason": reason.map(HotReloadReason::name),
    });
    let bytes = serde_json::to_vec(&value).expect("fixed plan view serializes");
    let mut digest = Sha256::new();
    digest.update(b"semaprax.hot-reload-plan.v1\0");
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{ProjectPreparedExecutionOutcome, ProjectProfile};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "semaprax-hot-reload-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(root.join("src")).unwrap();
            let original =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
            for relative in [
                "semaprax.toml",
                "src/app.spx",
                "src/core.spx",
                "src/tests.spx",
            ] {
                std::fs::copy(original.join(relative), root.join(relative)).unwrap();
            }
            Self(root)
        }

        fn revision(&self) -> Arc<ProjectRevision> {
            crate::project::load_snapshot(&self.0.join("semaprax.toml"))
                .unwrap()
                .retain_revision()
        }

        fn rewrite(&self, relative: &str, old: &str, new: &str) {
            let path = self.0.join(relative);
            let source = std::fs::read_to_string(&path).unwrap();
            assert!(source.contains(old));
            let changed = source.replacen(old, new, 1);
            let canonical =
                crate::format::canonical(&crate::parse(&changed, Path::new(relative)).unwrap());
            std::fs::write(path, canonical).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn observed(session: &HotReloadSession) -> ProjectPreparedExecutionOutcome {
        session
            .execute_entry(
                &PreparedProjectExecutionOptions::default(),
                &ProjectExecutionCancellation::new(),
            )
            .unwrap()
            .outcome()
            .clone()
    }

    #[test]
    fn checked_plan_is_separate_from_activation_and_two_plans_cannot_both_commit() {
        let fixture = Fixture::new();
        let active = fixture.revision();
        assert_eq!(
            active.manifest().project_profile(),
            ProjectProfile::ScalarV1
        );
        let mut session =
            HotReloadSession::new(active, PreparedProjectInterpreterOptions::default()).unwrap();
        assert_eq!(
            observed(&session),
            ProjectPreparedExecutionOutcome::Returned(42)
        );
        fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
        let candidate = fixture.revision();
        session.admit_candidate(candidate.clone()).unwrap();
        let first = session.plan().unwrap();
        let second = session.plan().unwrap();
        assert_eq!(first.decision(), HotReloadDecision::EligibleCodeReplacement);
        assert_eq!(
            observed(&session),
            ProjectPreparedExecutionOutcome::Returned(42)
        );
        let view: serde_json::Value = serde_json::from_str(&first.to_json()).unwrap();
        assert_eq!(view["authority"], "none");
        assert_eq!(view["generation"], 0);
        session.activate(first).unwrap();
        assert_eq!(session.generation(), 1);
        assert_eq!(
            session.active_project_revision(),
            candidate.project_revision()
        );
        assert_eq!(
            observed(&session),
            ProjectPreparedExecutionOutcome::Returned(48)
        );
        assert_eq!(
            session.activate(second).unwrap_err().reason,
            HotReloadReason::StaleGeneration
        );
        session.admit_candidate(candidate).unwrap();
        let identical = session.plan().unwrap();
        assert_eq!(identical.decision(), HotReloadDecision::Unchanged);
        assert_eq!(identical.reason(), Some(HotReloadReason::IdenticalRevision));
        assert_eq!(
            session.activate(identical).unwrap_err().reason,
            HotReloadReason::IdenticalRevision
        );
        assert_eq!(session.generation(), 1);
    }

    #[test]
    fn changed_contract_and_superseded_pending_plan_leave_active_worker_usable() {
        let fixture = Fixture::new();
        let active = fixture.revision();
        let mut session =
            HotReloadSession::new(active, PreparedProjectInterpreterOptions::default()).unwrap();
        fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
        session.admit_candidate(fixture.revision()).unwrap();
        let stale = session.plan().unwrap();
        fixture.rewrite("src/core.spx", "requires right != 0", "requires right > 0");
        session.admit_candidate(fixture.revision()).unwrap();
        assert_eq!(
            session.activate(stale).unwrap_err().reason,
            HotReloadReason::StaleCandidate
        );
        let incompatible = session.plan().unwrap();
        assert_eq!(
            incompatible.decision(),
            HotReloadDecision::UnsupportedRestartRequired
        );
        assert_eq!(
            incompatible.reason(),
            Some(HotReloadReason::IncompatibleClosure)
        );
        assert_eq!(
            session.activate(incompatible).unwrap_err().reason,
            HotReloadReason::IncompatibleClosure
        );
        assert_eq!(
            observed(&session),
            ProjectPreparedExecutionOutcome::Returned(42)
        );
        session.generation = u64::MAX;
        fixture.rewrite("src/core.spx", "requires right > 0", "requires right != 0");
        session.admit_candidate(fixture.revision()).unwrap();
        let plan = session.plan().unwrap();
        assert_eq!(plan.decision(), HotReloadDecision::EligibleCodeReplacement);
        assert_eq!(
            session.activate(plan).unwrap_err().reason,
            HotReloadReason::GenerationExhausted
        );
        assert_eq!(
            observed(&session),
            ProjectPreparedExecutionOutcome::Returned(42)
        );
    }
}
