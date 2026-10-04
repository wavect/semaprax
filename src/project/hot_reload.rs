//! Bounded, authority-neutral planning for a prepared Project interpreter.
//! A plan is a private in-memory value; its JSON is diagnostic evidence only.

use std::collections::BTreeMap;
use std::sync::Arc;

use sha2::{Digest as _, Sha256};

use super::{
    PreparedProjectExecution, PreparedProjectExecutionOptions, PreparedProjectInterpreter,
    PreparedProjectInterpreterOptions, ProjectExecutionCancellation, ProjectRevision,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedFunction, ResolvedProgram};

pub const HOT_RELOAD_PLAN_SCHEMA: &str = "semaprax.hot-reload-plan.v1";
pub const HOT_RELOAD_SOURCE_AGENT_HANDOFF_SCHEMA: &str =
    "semaprax.hot-reload-source-agent-handoff.v2";

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
pub enum HotReloadSourceAgentHandoffStatus {
    Ready,
    WaitingForCheckpoint,
    MigrationRequired,
    Activated,
    TerminalUncertainty,
}

impl HotReloadSourceAgentHandoffStatus {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::WaitingForCheckpoint => "waiting_for_checkpoint",
            Self::MigrationRequired => "migration_required",
            Self::Activated => "activated",
            Self::TerminalUncertainty => "terminal_uncertainty",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotReloadDecision {
    EligibleCodeReplacement,
    EligibleSourceAgentCheckpointHandoff,
    UnsupportedRestartRequired,
    Rejected,
    Unchanged,
}

impl HotReloadDecision {
    const fn name(self) -> &'static str {
        match self {
            Self::EligibleCodeReplacement => "eligible_code_replacement",
            Self::EligibleSourceAgentCheckpointHandoff => {
                "eligible_source_agent_checkpoint_handoff"
            }
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
#[derive(Clone)]
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
    source_agent_handoffs: Vec<HotReloadSourceAgentHandoff>,
    source_agent_handoff_digest: String,
    digest: String,
}

/// Compiler-derived compatibility facts for one source Agent checkpoint handoff.
///
/// This is an opaque, authority-free plan. It deliberately carries no checkpoint
/// bytes, lifecycle binding, store, host capability, or migration function. The
/// source-live migration owner must independently bind and replay all of those
/// before it can run a destination.
#[derive(Clone)]
pub struct HotReloadSourceAgentHandoff {
    agent_id: String,
    previous: SourceAgentEndpointFacts,
    destination: SourceAgentEndpointFacts,
    digest: String,
}

#[derive(Clone)]
struct SourceAgentEndpointFacts {
    definition_digest: String,
    graph_digest: String,
    runtime_profile_digest: String,
    proposal_type_id: String,
    proposal_type_revision: String,
    observation_type_id: String,
    observation_type_revision: String,
    proposal_schema_digest: String,
    observation_schema_digest: String,
}

impl HotReloadSourceAgentHandoff {
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Recomputes this row against the two retained Projects. This is only a
    /// restrictive selection check; it supplies no checkpoint, store, or
    /// runtime authority to the caller.
    pub fn matches_endpoints(
        &self,
        previous: &ProjectRevision,
        destination: &ProjectRevision,
        previous_agent_id: &str,
        destination_agent_id: &str,
    ) -> bool {
        self.agent_id == previous_agent_id
            && self.agent_id == destination_agent_id
            && source_agent_endpoint_facts(previous, previous_agent_id)
                .as_ref()
                .is_some_and(|facts| endpoint_facts_equal(facts, &self.previous))
            && source_agent_endpoint_facts(destination, destination_agent_id)
                .as_ref()
                .is_some_and(|facts| endpoint_facts_equal(facts, &self.destination))
            && self.digest
                == source_agent_handoff_row_digest(
                    &self.agent_id,
                    &self.previous,
                    &self.destination,
                )
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": HOT_RELOAD_SOURCE_AGENT_HANDOFF_SCHEMA,
            "agent_id": self.agent_id,
            "previous": endpoint_facts_json(&self.previous),
            "destination": endpoint_facts_json(&self.destination),
            "digest": self.digest,
            "authority": "none",
        })
    }
}

impl HotReloadPlan {
    pub fn decision(&self) -> HotReloadDecision {
        self.decision
    }

    pub fn reason(&self) -> Option<HotReloadReason> {
        self.reason
    }

    /// Exact compiler-derived source Agent facts, stable-ID ordered.
    pub fn source_agent_handoffs(&self) -> &[HotReloadSourceAgentHandoff] {
        &self.source_agent_handoffs
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
            "source_agent_handoffs": self.source_agent_handoffs.iter().map(HotReloadSourceAgentHandoff::json).collect::<Vec<_>>(),
            "source_agent_handoff_digest": self.source_agent_handoff_digest,
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
    source_agent_handoff_status: HotReloadSourceAgentHandoffStatus,
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
            source_agent_handoff_status: HotReloadSourceAgentHandoffStatus::Ready,
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

    /// The source-Agent handoff lifecycle is observational coordination only.
    /// Migration and checkpoint authority remain with the source-live owner.
    pub fn source_agent_handoff_status(&self) -> HotReloadSourceAgentHandoffStatus {
        self.source_agent_handoff_status
    }

    /// Opaque identity of the prepared worker retained for this explicit
    /// development session. It is an in-process continuity observation, not
    /// a transport value or activation receipt.
    pub fn worker_id(&self) -> std::thread::ThreadId {
        self.worker.worker_id()
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
        let (decision, reason, source_agent_handoffs) = if self.active.project_revision()
            == candidate.project_revision()
        {
            (
                HotReloadDecision::Unchanged,
                Some(HotReloadReason::IdenticalRevision),
                Vec::new(),
            )
        } else if self.active.manifest().project_profile() != candidate.manifest().project_profile()
        {
            (
                HotReloadDecision::Rejected,
                Some(HotReloadReason::PolicyChanged),
                Vec::new(),
            )
        } else if let Some(handoffs) = source_agent_handoffs(&self.active, candidate) {
            if handoffs.is_empty()
                && compatible_program(self.active.entry_program(), candidate.entry_program())
                && compatible_program(self.active.test_program(), candidate.test_program())
            {
                (HotReloadDecision::EligibleCodeReplacement, None, handoffs)
            } else if handoffs.is_empty() {
                (
                    HotReloadDecision::UnsupportedRestartRequired,
                    Some(HotReloadReason::IncompatibleClosure),
                    handoffs,
                )
            } else {
                (
                    HotReloadDecision::EligibleSourceAgentCheckpointHandoff,
                    None,
                    handoffs,
                )
            }
        } else {
            (
                HotReloadDecision::UnsupportedRestartRequired,
                Some(HotReloadReason::IncompatibleClosure),
                Vec::new(),
            )
        };
        let expected_project_revision = self.active.project_revision().to_owned();
        let candidate_project_revision = candidate.project_revision().to_owned();
        let expected_program_root = old_root.program_root().to_owned();
        let candidate_program_root = new_root.program_root().to_owned();
        let source_agent_handoff_digest = source_agent_handoff_digest(&source_agent_handoffs);
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
            &source_agent_handoff_digest,
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
            source_agent_handoffs,
            source_agent_handoff_digest,
            digest,
        })
    }

    /// Enters the source-Agent checkpoint handoff lifecycle after replaying the
    /// retained plan. This does not expose a checkpoint or start a destination.
    pub fn wait_for_source_agent_handoff(
        &mut self,
        plan: &HotReloadPlan,
        agent_id: &str,
    ) -> Result<HotReloadSourceAgentHandoff, HotReloadFailure> {
        self.validate_source_agent_handoff_plan(plan)?;
        let handoff = plan
            .source_agent_handoffs
            .iter()
            .find(|handoff| handoff.agent_id == agent_id)
            .cloned()
            .ok_or_else(|| {
                HotReloadFailure::new(
                    HotReloadReason::UnsupportedTarget,
                    "hot reload plan does not select the requested source Agent",
                )
            })?;
        self.source_agent_handoff_status = HotReloadSourceAgentHandoffStatus::WaitingForCheckpoint;
        Ok(handoff)
    }

    /// Records that the existing source-live owner has authenticated a
    /// checkpoint and needs its explicit checked State migration.
    pub(crate) fn require_source_agent_migration(
        &mut self,
        plan: &HotReloadPlan,
        handoff: &HotReloadSourceAgentHandoff,
    ) -> Result<(), HotReloadFailure> {
        self.validate_source_agent_handoff_plan(plan)?;
        if !plan
            .source_agent_handoffs
            .iter()
            .any(|row| row.agent_id == handoff.agent_id && row.digest == handoff.digest)
        {
            return Err(HotReloadFailure::new(
                HotReloadReason::StaleCandidate,
                "source Agent handoff no longer matches the retained plan",
            ));
        }
        self.source_agent_handoff_status = HotReloadSourceAgentHandoffStatus::MigrationRequired;
        Ok(())
    }

    /// Commits the supervisor projection only after the authenticated
    /// source-live owner has completed its one destination traversal. It never
    /// pivots the prepared interpreter or creates a second dispatch path.
    pub(crate) fn activate_source_agent_handoff(
        &mut self,
        plan: HotReloadPlan,
        handoff: &HotReloadSourceAgentHandoff,
    ) -> Result<(), HotReloadFailure> {
        self.require_source_agent_migration(&plan, handoff)?;
        let next = self.generation.checked_add(1).ok_or_else(|| {
            HotReloadFailure::new(
                HotReloadReason::GenerationExhausted,
                "hot reload generation is exhausted",
            )
        })?;
        self.active = self.pending.take().expect("validated pending candidate");
        self.generation = next;
        self.source_agent_handoff_status = HotReloadSourceAgentHandoffStatus::Activated;
        Ok(())
    }

    /// A refusal keeps the pending candidate for a later explicitly prepared
    /// attempt. Uncertain journal state is terminal and never supports an
    /// in-memory rollback or retry.
    pub(crate) fn refuse_source_agent_handoff(&mut self, uncertainty: bool) {
        if uncertainty {
            self.terminal = true;
            self.source_agent_handoff_status =
                HotReloadSourceAgentHandoffStatus::TerminalUncertainty;
        } else {
            self.source_agent_handoff_status = HotReloadSourceAgentHandoffStatus::MigrationRequired;
        }
    }

    fn validate_source_agent_handoff_plan(
        &self,
        plan: &HotReloadPlan,
    ) -> Result<(), HotReloadFailure> {
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
            || plan.decision != HotReloadDecision::EligibleSourceAgentCheckpointHandoff
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
                    &plan.source_agent_handoff_digest,
                )
        {
            return Err(HotReloadFailure::new(
                HotReloadReason::StaleCandidate,
                "hot reload plan no longer matches admitted source Agent candidate",
            ));
        }
        let fresh = self.plan()?;
        if fresh.digest != plan.digest {
            return Err(HotReloadFailure::new(
                HotReloadReason::StaleCandidate,
                "hot reload compatibility facts changed",
            ));
        }
        Ok(())
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
                    &plan.source_agent_handoff_digest,
                )
        {
            return Err(HotReloadFailure::new(
                HotReloadReason::StaleCandidate,
                "hot reload plan no longer matches admitted candidate",
            ));
        }
        if plan.decision == HotReloadDecision::EligibleSourceAgentCheckpointHandoff {
            return Err(HotReloadFailure::new(
                HotReloadReason::UnsupportedTarget,
                "source Agent checkpoint handoff requires authenticated source migration",
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
        if self.source_agent_handoff_status == HotReloadSourceAgentHandoffStatus::Activated {
            return Err(vec![Diagnostic::io(
                "SPX-HR400",
                "source Agent handoff execution remains owned by the source journal",
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
    let old_functions = old
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let new_functions = candidate
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    if old_functions.len() != old.functions.len()
        || new_functions.len() != candidate.functions.len()
        || old_functions.len() != new_functions.len()
    {
        return false;
    }
    old_functions.iter().all(|(id, left)| {
        new_functions
            .get(id)
            .is_some_and(|right| compatible_function(left, right))
    })
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

fn source_agent_handoffs(
    active: &ProjectRevision,
    candidate: &ProjectRevision,
) -> Option<Vec<HotReloadSourceAgentHandoff>> {
    let active_definitions = active.agent_definitions();
    let candidate_definitions = candidate.agent_definitions();
    if active_definitions.len() != candidate_definitions.len()
        || active.source_agents().len() != active_definitions.len()
        || candidate.source_agents().len() != candidate_definitions.len()
    {
        return None;
    }
    let (Some(active_facts), Some(candidate_facts)) = (
        active.agent_interaction_contract_facts(),
        candidate.agent_interaction_contract_facts(),
    ) else {
        return active_definitions.is_empty().then(Vec::new);
    };
    if active_facts.facts().len() != active_definitions.len()
        || candidate_facts.facts().len() != candidate_definitions.len()
    {
        return None;
    }
    let mut handoffs = Vec::with_capacity(active_definitions.len());
    for (left, right) in active_definitions.iter().zip(candidate_definitions) {
        let agent_id = left.definition().agent_id();
        if agent_id != right.definition().agent_id() {
            return None;
        }
        let Some(previous) =
            source_agent_endpoint_facts_with_contracts(left, active_facts.fact(agent_id)?)
        else {
            return None;
        };
        let Some(destination) =
            source_agent_endpoint_facts_with_contracts(right, candidate_facts.fact(agent_id)?)
        else {
            return None;
        };
        let digest = source_agent_handoff_row_digest(agent_id, &previous, &destination);
        handoffs.push(HotReloadSourceAgentHandoff {
            agent_id: agent_id.to_owned(),
            previous,
            destination,
            digest,
        });
    }
    Some(handoffs)
}

fn source_agent_endpoint_facts(
    project: &ProjectRevision,
    agent_id: &str,
) -> Option<SourceAgentEndpointFacts> {
    if project.source_agents().len() != project.agent_definitions().len()
        || project
            .agent_definitions()
            .iter()
            .filter(|definition| definition.definition().agent_id() == agent_id)
            .count()
            != 1
    {
        return None;
    }
    let definition = project
        .agent_definitions()
        .iter()
        .find(|definition| definition.definition().agent_id() == agent_id)?;
    let contracts = project.agent_interaction_contract_facts()?;
    if contracts.facts().len() != project.agent_definitions().len() {
        return None;
    }
    source_agent_endpoint_facts_with_contracts(definition, contracts.fact(agent_id)?)
}

fn source_agent_endpoint_facts_with_contracts(
    definition: &crate::agent_definition::CompiledAgentDefinition,
    contract: &super::AgentInteractionContractFact,
) -> Option<SourceAgentEndpointFacts> {
    if definition.definition().agent_id() != contract.agent_id()
        || contract.proposal_type_id().is_empty()
        || contract.proposal_type_revision().is_empty()
        || contract.observation_type_id().is_empty()
        || contract.observation_type_revision().is_empty()
    {
        return None;
    }
    Some(SourceAgentEndpointFacts {
        definition_digest: definition.definition().digest().to_owned(),
        graph_digest: definition.graph().digest().to_owned(),
        runtime_profile_digest: digest_bytes(
            b"semaprax.hot-reload-source-agent-runtime.v1\0",
            definition.runtime_v1_profile().as_bytes(),
        ),
        proposal_type_id: contract.proposal_type_id().to_owned(),
        proposal_type_revision: contract.proposal_type_revision().to_owned(),
        observation_type_id: contract.observation_type_id().to_owned(),
        observation_type_revision: contract.observation_type_revision().to_owned(),
        proposal_schema_digest: contract.proposal_schema_digest().to_owned(),
        observation_schema_digest: contract.observation_schema_digest().to_owned(),
    })
}

fn endpoint_facts_equal(left: &SourceAgentEndpointFacts, right: &SourceAgentEndpointFacts) -> bool {
    left.definition_digest == right.definition_digest
        && left.graph_digest == right.graph_digest
        && left.runtime_profile_digest == right.runtime_profile_digest
        && left.proposal_type_id == right.proposal_type_id
        && left.proposal_type_revision == right.proposal_type_revision
        && left.observation_type_id == right.observation_type_id
        && left.observation_type_revision == right.observation_type_revision
        && left.proposal_schema_digest == right.proposal_schema_digest
        && left.observation_schema_digest == right.observation_schema_digest
}

fn endpoint_facts_json(facts: &SourceAgentEndpointFacts) -> serde_json::Value {
    serde_json::json!({
        "definition_digest": facts.definition_digest,
        "graph_digest": facts.graph_digest,
        "runtime_profile_digest": facts.runtime_profile_digest,
        "proposal_type_id": facts.proposal_type_id,
        "proposal_type_revision": facts.proposal_type_revision,
        "observation_type_id": facts.observation_type_id,
        "observation_type_revision": facts.observation_type_revision,
        "proposal_schema_digest": facts.proposal_schema_digest,
        "observation_schema_digest": facts.observation_schema_digest,
    })
}

fn source_agent_handoff_row_digest(
    agent_id: &str,
    previous: &SourceAgentEndpointFacts,
    destination: &SourceAgentEndpointFacts,
) -> String {
    digest_bytes(
        b"semaprax.hot-reload-source-agent-handoff.v1\0",
        serde_json::to_string(&serde_json::json!({
            "agent_id": agent_id,
            "previous": endpoint_facts_json(previous),
            "destination": endpoint_facts_json(destination),
        }))
        .expect("fixed source Agent handoff row serializes")
        .as_bytes(),
    )
}

fn source_agent_handoff_digest(handoffs: &[HotReloadSourceAgentHandoff]) -> String {
    digest_bytes(
        b"semaprax.hot-reload-source-agent-handoffs.v1\0",
        serde_json::to_string(
            &handoffs
                .iter()
                .map(HotReloadSourceAgentHandoff::digest)
                .collect::<Vec<_>>(),
        )
        .expect("fixed source Agent handoff digest serializes")
        .as_bytes(),
    )
}

fn digest_bytes(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
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
    source_agent_handoff_digest: &str,
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
        "source_agent_handoff_digest": source_agent_handoff_digest,
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
    use crate::project::{
        verify_project_source_trace_against_revision, ProjectPreparedExecutionOutcome,
        ProjectProfile,
    };
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

    #[test]
    fn checked_identity_cases_and_first_over_bound_submission_preserve_active_code() {
        for (name, file, old, new, expected) in [
            (
                "display-rename",
                "src/core.spx",
                "fn multiply(",
                "fn productX(",
                Some(HotReloadDecision::EligibleCodeReplacement),
            ),
            (
                "entry-identity",
                "src/app.spx",
                "@id(\"calculator.app.main\")",
                "@id(\"calculator.app.other\")",
                Some(HotReloadDecision::UnsupportedRestartRequired),
            ),
            (
                "missing-import-identity",
                "src/core.spx",
                "@id(\"calculator.multiply\")",
                "@id(\"calculator.productX\")",
                None,
            ),
        ] {
            let fixture = Fixture::new();
            let mut session = HotReloadSession::new(
                fixture.revision(),
                PreparedProjectInterpreterOptions::default(),
            )
            .unwrap();
            fixture.rewrite(file, old, new);
            match expected {
                Some(decision) => {
                    session.admit_candidate(fixture.revision()).unwrap();
                    assert_eq!(session.plan().unwrap().decision(), decision, "{name}");
                }
                None => {
                    let diagnostics =
                        crate::project::load_snapshot(&fixture.0.join("semaprax.toml"))
                            .err()
                            .expect("missing stable import must refuse Project admission");
                    assert_eq!(diagnostics[0].code, "SPX-G172", "{name}");
                }
            }
            assert_eq!(
                observed(&session),
                ProjectPreparedExecutionOutcome::Returned(42)
            );
        }

        let fixture = Fixture::new();
        let mut session = HotReloadSession::new(
            fixture.revision(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
        session.submission = u64::MAX;
        assert_eq!(
            session
                .admit_candidate(fixture.revision())
                .unwrap_err()
                .reason,
            HotReloadReason::GenerationExhausted
        );
        assert!(session.pending.is_none());
        assert_eq!(
            observed(&session),
            ProjectPreparedExecutionOutcome::Returned(42)
        );
    }

    #[test]
    fn source_agent_checkpoint_selection_cannot_replace_the_prepared_worker() {
        let fixture = Fixture::new();
        let mut session = HotReloadSession::new(
            fixture.revision(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
        session.admit_candidate(fixture.revision()).unwrap();
        let mut selection = session.plan().unwrap();
        selection.decision = HotReloadDecision::EligibleSourceAgentCheckpointHandoff;
        selection.digest = plan_digest(
            selection.generation,
            selection.submission,
            &selection.expected_project_revision,
            &selection.expected_program_root,
            &selection.candidate_project_revision,
            &selection.candidate_program_root,
            &selection.entry_id,
            &selection.test_id,
            selection.decision,
            selection.reason,
            &selection.source_agent_handoff_digest,
        );
        assert_eq!(
            session.activate(selection).unwrap_err().reason,
            HotReloadReason::UnsupportedTarget
        );
        assert_eq!(session.generation(), 0);
        assert_eq!(
            observed(&session),
            ProjectPreparedExecutionOutcome::Returned(42)
        );
    }

    #[test]
    fn real_a_to_b_to_c_keeps_one_worker_and_binds_each_trace_to_its_revision() {
        let fixture = Fixture::new();
        let active = fixture.revision();
        let mut session = HotReloadSession::new(
            Arc::clone(&active),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let worker = session.worker_id();
        let a = session
            .execute_entry(
                &PreparedProjectExecutionOptions::default(),
                &ProjectExecutionCancellation::new(),
            )
            .unwrap();
        assert_eq!(a.outcome(), &ProjectPreparedExecutionOutcome::Returned(42));
        verify_project_source_trace_against_revision(&active, a.trace().envelope()).unwrap();

        fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
        let b = fixture.revision();
        session.admit_candidate(Arc::clone(&b)).unwrap();
        session.activate(session.plan().unwrap()).unwrap();
        assert_eq!(session.worker_id(), worker);
        let b_run = session
            .execute_entry(
                &PreparedProjectExecutionOptions::default(),
                &ProjectExecutionCancellation::new(),
            )
            .unwrap();
        assert_eq!(
            b_run.outcome(),
            &ProjectPreparedExecutionOutcome::Returned(48)
        );
        verify_project_source_trace_against_revision(&b, b_run.trace().envelope()).unwrap();
        assert!(verify_project_source_trace_against_revision(&b, a.trace().envelope()).is_err());

        fixture.rewrite("src/app.spx", "multiply(6, 8)", "multiply(6, 9)");
        let c = fixture.revision();
        session.admit_candidate(Arc::clone(&c)).unwrap();
        session.activate(session.plan().unwrap()).unwrap();
        assert_eq!(session.worker_id(), worker);
        assert_eq!(
            observed(&session),
            ProjectPreparedExecutionOutcome::Returned(54)
        );
    }
}
