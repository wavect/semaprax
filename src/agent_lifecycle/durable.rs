//! Agent Checkpoint v1: a revision-bound, opaque checkpoint and a resume that
//! never repeats an uncertain external operation.
//!
//! One durable run is the Lifecycle v1 pass split at its single external
//! boundary. The deterministic prefix — `initialize`, `observe`, the scripted
//! offline proposal, `authorize` — is re-executable by construction: the
//! lifecycle compiler already rejects a declared effect on any of those
//! stages. The one registered external read is not re-executable, so the run
//! commits an **intent** before crossing the boundary and a **settled
//! observation** after it. A crash between those two commits leaves the
//! operation's delivery *uncertain*, and an uncertain operation is never
//! retried automatically: it needs an explicit host reconciliation or it ends
//! in a terminal unknown state.
//!
//! # A resumed run cannot forge an authorization
//!
//! Nothing in a checkpoint is an input to minting one. A checkpoint carries no
//! [`Authorized`], no grant seal, and no state carrier — only digests of them.
//! There is no decoder from checkpoint bytes to a `RetainedValue`, so a
//! resumed run cannot even reconstruct the state a grant was made against; it
//! recomputes that state by re-running `initialize` on a caller-supplied task
//! and re-running the validated authorizing transition through
//! [`super::authorization::run_authorize_stage`], the crate's only mint site.
//! The checkpoint's recorded operation identity is then compared against the
//! identity the *live* grant derives. A forged, truncated or reordered journal
//! therefore has exactly two possible effects: the resume refuses, or the
//! resume declines to perform an effect it would otherwise have performed.
//! Neither direction produces authority.
//!
//! What a checkpoint does not defend against is a party who can rewrite the
//! caller's storage: the journal chain carries no key material, so such a
//! party can recompute it. That is a declared nonclaim, not a gap the runtime
//! papers over — checkpoint integrity is the caller's storage contract.
//!
//! # Authority separation
//!
//! Persistence is the caller's: [`CheckpointStore`] is a trait this module
//! implements nowhere, and this module opens no file, spawns no process and
//! contacts no network. Live effect authority is the caller's too: the single
//! [`AgentReadOperation`] is injected per invocation and is never named,
//! captured or described by checkpoint bytes.
//!
//! # Cancellation
//!
//! Cancellation is observed at every deterministic stage boundary and once
//! more immediately before the intent is committed. It is deliberately not
//! observed after that: abandoning a run between its intent and its settlement
//! would manufacture the very uncertainty the intent exists to bound, so once
//! the boundary is committed to, the run finishes recording what happened.

use std::path::Path;

use crate::agent_deployment::BoundAgentDeployment;
use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::interpreter::retained_call::{RetainedCallOutcome, RetainedValue};

use super::authorization::{self, Authorized};
use super::{
    compile_agent_lifecycle, encode_value, invariant, payload, terminal, AgentReadOperation,
    CompiledAgentLifecycle, LifecycleTask, StageRecord,
};

mod checkpoint;
mod journal;

#[cfg(test)]
mod tests;

use checkpoint::{
    digest, CheckpointBudgets, OBSERVATION_DOMAIN, PROPOSAL_DOMAIN, RESULT_DOMAIN, SOURCE_DOMAIN,
    STATE_DOMAIN, TASK_DOMAIN,
};
use journal::JournalEntry;

pub use checkpoint::{
    AgentCheckpoint, CheckpointBinding, ProgramCounter, Retention, CHECKPOINT_SCHEMA,
};

/// Schema identity of one durable invocation's evidence document.
pub const DURABLE_EVIDENCE_SCHEMA: &str = "semaprax.agent-checkpoint-evidence.v1";

const DURABLE_EVIDENCE_DOMAIN: &[u8] = b"semaprax.agent-checkpoint-evidence.digest.v1\0";

/// The default fuel one whole durable run may spend across every stage,
/// including the stages a resume re-executes.
pub const DEFAULT_TOTAL_STEPS: usize = 1_000_000;

/// A caller-owned durable store failure. It carries no detail: the runtime
/// treats every storage failure alike and fails closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointStoreError;

/// Caller-owned checkpoint persistence.
///
/// # Declared storage contract
///
/// A `commit` either replaces the whole stored generation or leaves the
/// previous one intact. Nothing in between may become visible to a later
/// recovery. A filesystem store satisfies this by writing a temporary file in
/// the same directory and renaming it over the target.
///
/// Recovery does not take that contract on trust. Checkpoint bytes are
/// self-verifying, so [`AgentCheckpoint::decode`] rejects a partially written
/// generation rather than adopting it; a store that violates its contract
/// loses the newer generation, it does not smuggle a half one through.
pub trait CheckpointStore {
    /// Atomically replaces the stored generation with `document`.
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError>;
}

/// The budget ledgers one durable run starts with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableBudget {
    /// Interpreter fuel for the whole run, including re-executed stages.
    pub total_steps: usize,
    /// Fuel any single stage may spend.
    pub max_steps_per_stage: usize,
    /// How many times the run may cross the external boundary. Consumed at an
    /// intent and never refunded.
    pub effect_grants: usize,
}

impl Default for DurableBudget {
    fn default() -> Self {
        Self {
            total_steps: DEFAULT_TOTAL_STEPS,
            max_steps_per_stage: super::DEFAULT_STAGE_STEPS,
            effect_grants: 1,
        }
    }
}

/// What a host independently determined about an uncertain or redacted
/// operation.
///
/// A reconciliation is a *finding*, not a permission. It can supply an
/// observation the runtime may no longer obtain, or close the operation out.
/// It never causes the external boundary to be crossed again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reconciliation<'a> {
    /// The host made no determination. An uncertain operation stays uncertain.
    None,
    /// The host determined the operation settled with exactly these bytes.
    Settled(&'a [u8]),
    /// The host determined the operation must not be completed.
    Abandoned,
}

/// Deterministic crash injection for this module's own gate.
///
/// A crash abandons the invocation at a named boundary exactly as a process
/// death would: the caller receives no result, and a later recovery sees only
/// the generations committed strictly before that boundary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CrashPoint {
    #[default]
    Never,
    /// After the deterministic prefix is durable, before the intent is.
    BeforeIntent,
    /// After the intent is durable, before the boundary is crossed.
    AfterIntent,
    /// After the boundary is crossed, before the settlement is durable.
    AfterEffect,
    /// After the settlement is durable, before the reduction runs.
    AfterSettlement,
    /// After the reduction is durable, before the result reaches the caller.
    BeforeDelivery,
}

/// The terminal condition of one durable invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableStatus {
    /// The reduction published a result and the delivery is durable.
    Completed,
    /// A deterministic stage refused or did not decide. No effect.
    Rejected,
    /// The scripted model produced no proposal this grammar admits.
    ModelFailed,
    /// The registered read reported failure. A reported failure is not
    /// evidence of non-occurrence, so the operation stays uncertain.
    EffectFailed,
    /// Cancellation was observed at a stage boundary.
    Cancelled,
    /// A fuel, depth or effect-grant ledger was exhausted.
    BudgetExhausted,
    /// The operation's delivery is uncertain and no host reconciliation was
    /// supplied. Terminal for this invocation; the checkpoint stays
    /// reconcilable.
    Unknown,
    /// The host reconciled the operation to abandonment.
    Abandoned,
    /// The checkpoint is bound to a different revision or a revoked epoch.
    Stale,
    /// The checkpoint already records delivery. Nothing was performed.
    AlreadyDelivered,
    /// A durable commit failed, so the run stopped before its next step.
    StoreFailed,
    /// Injected crash. The caller receives no result.
    Crashed,
}

impl DurableStatus {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Rejected => "rejected",
            Self::ModelFailed => "model_failed",
            Self::EffectFailed => "effect_failed",
            Self::Cancelled => "cancelled",
            Self::BudgetExhausted => "budget_exhausted",
            Self::Unknown => "unknown",
            Self::Abandoned => "abandoned",
            Self::Stale => "stale",
            Self::AlreadyDelivered => "already_delivered",
            Self::StoreFailed => "store_failed",
            Self::Crashed => "crashed",
        }
    }
}

/// The complete record of one durable invocation.
pub struct DurableRun {
    status: DurableStatus,
    reason: &'static str,
    stages: Vec<StageRecord>,
    checkpoint: Option<AgentCheckpoint>,
    result: Option<RetainedValue>,
    result_digest: Option<String>,
    boundary_crossings: usize,
    evidence: String,
    evidence_digest: String,
}

impl DurableRun {
    #[must_use]
    pub const fn status(&self) -> DurableStatus {
        self.status
    }

    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }

    #[must_use]
    pub fn stages(&self) -> &[StageRecord] {
        &self.stages
    }

    /// The last generation this invocation durably committed, if any.
    #[must_use]
    pub const fn checkpoint(&self) -> Option<&AgentCheckpoint> {
        self.checkpoint.as_ref()
    }

    #[must_use]
    pub const fn result(&self) -> Option<&RetainedValue> {
        self.result.as_ref()
    }

    /// The digest of the published Result carrier.
    #[must_use]
    pub fn result_digest(&self) -> Option<&str> {
        self.result_digest.as_deref()
    }

    /// How many times *this invocation* crossed the external boundary.
    #[must_use]
    pub const fn boundary_crossings(&self) -> usize {
        self.boundary_crossings
    }

    /// The canonical evidence document, including its terminal LF. Identities,
    /// digests and counts only; never a stage payload and never an
    /// observation.
    #[must_use]
    pub fn evidence(&self) -> &str {
        &self.evidence
    }

    #[must_use]
    pub fn evidence_digest(&self) -> &str {
        &self.evidence_digest
    }
}

/// One compiled lifecycle bound to one immutable deployment and one policy
/// epoch: the revision a checkpoint is anchored to.
pub struct DurableAgent {
    lifecycle: CompiledAgentLifecycle,
    binding: CheckpointBinding,
}

/// Binds one checked module to one bound deployment for durable execution.
///
/// Compilation stays pure: the bound deployment authenticates both revisions
/// without contacting a provider, and this function adds only digests of what
/// the caller already supplied.
pub fn bind_durable_agent(
    module_source: &str,
    module_path: impl AsRef<Path>,
    bound: &BoundAgentDeployment,
    policy_epoch: u64,
) -> Result<DurableAgent, Vec<Diagnostic>> {
    let lifecycle =
        compile_agent_lifecycle(module_source, module_path, bound.runtime_v1_definition())?;
    if lifecycle.agent_id() != bound.semantic_definition().agent_id() {
        return Err(vec![invariant("durable.agent_id")]);
    }
    let binding = CheckpointBinding {
        bound_digest: bound.digest().to_owned(),
        definition_digest: bound.semantic_definition().digest().to_owned(),
        deployment_digest: bound.deployment().digest().to_owned(),
        source_digest: digest(SOURCE_DOMAIN, module_source.as_bytes()),
        lifecycle_digest: lifecycle.digest().to_owned(),
        proposal_schema_digest: lifecycle.proposal_schema().schema().digest().to_owned(),
        state_schema: lifecycle.binding.type_id("state").as_str().to_owned(),
        policy_epoch,
    };
    Ok(DurableAgent { lifecycle, binding })
}

impl DurableAgent {
    /// The compiled lifecycle this durable agent executes.
    #[must_use]
    pub const fn lifecycle(&self) -> &CompiledAgentLifecycle {
        &self.lifecycle
    }

    /// The revision every checkpoint of this agent is bound to.
    #[must_use]
    pub const fn binding(&self) -> &CheckpointBinding {
        &self.binding
    }

    /// The digest one task is bound into a checkpoint by. The task payload
    /// itself is never retained.
    fn task_digest(task: &LifecycleTask) -> String {
        let mut bytes = task.objective.clone();
        bytes.push(0);
        bytes.extend_from_slice(task.budget.to_string().as_bytes());
        digest(TASK_DOMAIN, &bytes)
    }

    /// Starts one durable run from generation one.
    ///
    /// `Err` stays reserved for a fail-closed compiler or seam fault. Every
    /// terminal condition of the run itself is an `Ok` carrying its status,
    /// its last durable generation, and its evidence.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &self,
        task: &LifecycleTask,
        proposal_source: &str,
        read: &mut dyn AgentReadOperation,
        budget: DurableBudget,
        retention: Retention,
        cancellation: &AgentCancellation,
        store: &mut dyn CheckpointStore,
        crash: CrashPoint,
    ) -> Result<DurableRun, Vec<Diagnostic>> {
        let mut machine = Machine {
            agent: self,
            retention,
            task_digest: Self::task_digest(task),
            generation: 0,
            budgets: CheckpointBudgets {
                total_steps_remaining: budget.total_steps,
                max_steps_per_stage: budget.max_steps_per_stage,
                effect_grants_remaining: budget.effect_grants,
            },
            entries: Vec::new(),
            records: Vec::new(),
            crossings: 0,
            checkpoint: None,
            crash,
        };
        let prefix = match machine.prefix(task, proposal_source, cancellation)? {
            PrefixOutcome::Ready(prefix) => prefix,
            PrefixOutcome::Terminal(status, reason) => return Ok(machine.finish(status, reason)),
        };
        machine.entries.push(JournalEntry::Prefix {
            state_digest: prefix.state_digest.clone(),
            proposal_digest: prefix.proposal_digest.clone(),
        });
        if !machine.commit(store)? {
            drop(prefix.authorized);
            return Ok(machine.finish(DurableStatus::StoreFailed, "prefix_not_durable"));
        }
        machine.tail(
            *prefix,
            ProgramCounter::Prefix,
            read,
            Reconciliation::None,
            cancellation,
            store,
        )
    }

    /// Resumes one durable run from a stored checkpoint.
    ///
    /// The task and the proposal are supplied again by the caller, never by
    /// the checkpoint: the checkpoint holds only their digests, so a resume
    /// with different inputs is refused rather than silently accepted.
    #[allow(clippy::too_many_arguments)]
    pub fn resume(
        &self,
        stored: &AgentCheckpoint,
        task: &LifecycleTask,
        proposal_source: &str,
        read: &mut dyn AgentReadOperation,
        reconciliation: Reconciliation<'_>,
        cancellation: &AgentCancellation,
        store: &mut dyn CheckpointStore,
    ) -> Result<DurableRun, Vec<Diagnostic>> {
        let mut machine = Machine {
            agent: self,
            retention: stored.retention(),
            task_digest: stored.task_digest().to_owned(),
            generation: stored.generation(),
            budgets: stored.budgets(),
            entries: stored.journal().to_vec(),
            records: Vec::new(),
            crossings: 0,
            checkpoint: None,
            crash: CrashPoint::Never,
        };
        if stored.agent_id() != self.lifecycle.agent_id() {
            return Ok(machine.finish(DurableStatus::Stale, "agent_identity_drift"));
        }
        if let Some(reason) = stored.binding().drift(&self.binding) {
            return Ok(machine.finish(DurableStatus::Stale, reason));
        }
        if stored.task_digest() != Self::task_digest(task) {
            return Ok(machine.finish(DurableStatus::Stale, "task_drift"));
        }
        let counter = stored.program_counter();
        // Terminal counters perform nothing and re-run no stage at all.
        match counter {
            ProgramCounter::Delivered => {
                return Ok(
                    machine.finish(DurableStatus::AlreadyDelivered, "result_already_delivered")
                )
            }
            ProgramCounter::Abandoned => {
                return Ok(machine.finish(DurableStatus::Abandoned, "operation_already_abandoned"))
            }
            _ => {}
        }

        // Re-derive, never deserialize: the state is recomputed from the
        // caller's task and the authorization is minted afresh by the
        // validated authorizing transition.
        let prefix = match machine.prefix(task, proposal_source, cancellation)? {
            PrefixOutcome::Ready(prefix) => prefix,
            PrefixOutcome::Terminal(status, reason) => return Ok(machine.finish(status, reason)),
        };
        let Some(JournalEntry::Prefix {
            state_digest,
            proposal_digest,
        }) = stored.journal().first()
        else {
            return Err(vec![invariant("checkpoint.journal.prefix")]);
        };
        if *state_digest != prefix.state_digest {
            drop(prefix.authorized);
            return Ok(machine.finish(DurableStatus::Rejected, "state_digest_mismatch"));
        }
        if *proposal_digest != prefix.proposal_digest {
            drop(prefix.authorized);
            return Ok(machine.finish(DurableStatus::Rejected, "proposal_digest_mismatch"));
        }
        if let Some(recorded) = stored.operation_identity() {
            if recorded != prefix.operation {
                drop(prefix.authorized);
                return Ok(machine.finish(DurableStatus::Rejected, "operation_identity_mismatch"));
            }
        }
        machine.tail(*prefix, counter, read, reconciliation, cancellation, store)
    }
}

/// The deterministic prefix's harvest, including one freshly minted grant.
struct Prefix {
    state: RetainedValue,
    state_digest: String,
    proposal_canonical: String,
    proposal_digest: String,
    projected: Vec<RetainedValue>,
    authorized: Authorized,
    operation: String,
    granted_budget: i64,
}

enum PrefixOutcome {
    Ready(Box<Prefix>),
    Terminal(DurableStatus, &'static str),
}

struct Machine<'a> {
    agent: &'a DurableAgent,
    retention: Retention,
    task_digest: String,
    generation: u64,
    budgets: CheckpointBudgets,
    entries: Vec<JournalEntry>,
    records: Vec<StageRecord>,
    crossings: usize,
    checkpoint: Option<AgentCheckpoint>,
    crash: CrashPoint,
}

impl Machine<'_> {
    /// The fuel one stage may spend: never more than the per-stage ceiling and
    /// never more than the whole run has left.
    const fn allot(&self) -> usize {
        let per_stage = self.budgets.max_steps_per_stage;
        let remaining = self.budgets.total_steps_remaining;
        if per_stage < remaining {
            per_stage
        } else {
            remaining
        }
    }

    fn spend(&mut self, used: usize) {
        self.budgets.total_steps_remaining =
            self.budgets.total_steps_remaining.saturating_sub(used);
    }

    /// Seals and commits the next generation. `Ok(false)` is a store failure.
    fn commit(&mut self, store: &mut dyn CheckpointStore) -> Result<bool, Vec<Diagnostic>> {
        self.generation += 1;
        let sealed = AgentCheckpoint::seal(
            self.generation,
            self.agent.lifecycle.agent_id(),
            self.agent.binding.clone(),
            self.budgets,
            self.retention,
            &self.task_digest,
            self.entries.clone(),
        )?;
        if store.commit(self.generation, sealed.document()).is_err() {
            return Ok(false);
        }
        self.checkpoint = Some(sealed);
        Ok(true)
    }

    fn finish(&mut self, status: DurableStatus, reason: &'static str) -> DurableRun {
        self.publish(status, reason, None, None)
    }

    fn publish(
        &mut self,
        status: DurableStatus,
        reason: &'static str,
        result: Option<RetainedValue>,
        result_digest: Option<String>,
    ) -> DurableRun {
        let evidence = render_evidence(
            self.agent,
            status,
            reason,
            &self.records,
            self.checkpoint.as_ref(),
            self.budgets,
            self.crossings,
            result_digest.as_deref(),
        );
        DurableRun {
            status,
            reason,
            stages: std::mem::take(&mut self.records),
            checkpoint: self.checkpoint.take(),
            result,
            result_digest,
            boundary_crossings: self.crossings,
            evidence_digest: digest(DURABLE_EVIDENCE_DOMAIN, evidence.as_bytes()),
            evidence,
        }
    }

    /// Runs the effect-free deterministic prefix and mints one authorization.
    ///
    /// Every stage here is re-executable: the lifecycle compiler rejected a
    /// declared effect on all four deterministic roles before this module
    /// could reach them.
    fn prefix(
        &mut self,
        task: &LifecycleTask,
        proposal_source: &str,
        cancellation: &AgentCancellation,
    ) -> Result<PrefixOutcome, Vec<Diagnostic>> {
        let lifecycle = &self.agent.lifecycle;
        if cancellation.is_cancelled() {
            return Ok(PrefixOutcome::Terminal(
                DurableStatus::Cancelled,
                "cancelled_before_initialize",
            ));
        }
        if self.allot() == 0 {
            return Ok(PrefixOutcome::Terminal(
                DurableStatus::BudgetExhausted,
                "no_fuel_for_initialize",
            ));
        }
        let evaluation = lifecycle.evaluate(
            &lifecycle.binding.initialize,
            &[payload(
                &lifecycle.binding.task,
                task.objective.clone(),
                task.budget,
            )],
            self.allot(),
        )?;
        self.spend(evaluation.steps_used);
        self.records
            .push(StageRecord::of(&lifecycle.binding.initialize, &evaluation));
        let RetainedCallOutcome::Returned(state) = evaluation.outcome else {
            return Ok(PrefixOutcome::Terminal(
                durable_terminal(&evaluation.outcome),
                "initialize_did_not_return",
            ));
        };
        if !lifecycle.carries(&state, "state") {
            return Err(vec![invariant("initialize.result.identity")]);
        }
        let state_digest = digest(STATE_DOMAIN, encode_value(&state).as_bytes());

        if cancellation.is_cancelled() {
            return Ok(PrefixOutcome::Terminal(
                DurableStatus::Cancelled,
                "cancelled_before_observe",
            ));
        }
        if self.allot() == 0 {
            return Ok(PrefixOutcome::Terminal(
                DurableStatus::BudgetExhausted,
                "no_fuel_for_observe",
            ));
        }
        let evaluation = lifecycle.evaluate(
            &lifecycle.binding.observe,
            std::slice::from_ref(&state),
            self.allot(),
        )?;
        self.spend(evaluation.steps_used);
        self.records
            .push(StageRecord::of(&lifecycle.binding.observe, &evaluation));
        let RetainedCallOutcome::Returned(observation) = evaluation.outcome else {
            return Ok(PrefixOutcome::Terminal(
                durable_terminal(&evaluation.outcome),
                "observe_did_not_return",
            ));
        };
        if !lifecycle.carries(&observation, "observation") {
            return Err(vec![invariant("observe.result.identity")]);
        }

        let Ok(decoded) = lifecycle.proposal.decode(proposal_source) else {
            return Ok(PrefixOutcome::Terminal(
                DurableStatus::ModelFailed,
                "proposal_rejected_by_its_grammar",
            ));
        };
        let Some(projected) = lifecycle.project(&decoded) else {
            return Ok(PrefixOutcome::Terminal(
                DurableStatus::ModelFailed,
                "proposal_outside_the_stage_projection",
            ));
        };
        let proposal_canonical = decoded.canonical_json().to_owned();
        let proposal_digest = digest(PROPOSAL_DOMAIN, proposal_canonical.as_bytes());

        if cancellation.is_cancelled() {
            return Ok(PrefixOutcome::Terminal(
                DurableStatus::Cancelled,
                "cancelled_before_authorize",
            ));
        }
        if self.allot() == 0 {
            return Ok(PrefixOutcome::Terminal(
                DurableStatus::BudgetExhausted,
                "no_fuel_for_authorize",
            ));
        }
        let mut arguments = Vec::with_capacity(1 + projected.len());
        arguments.push(state.clone());
        arguments.extend(projected.iter().cloned());
        let (decision, record) = authorization::run_authorize_stage(
            &lifecycle.program,
            &lifecycle.binding.authorize,
            &arguments,
            self.allot(),
            &lifecycle.digest,
            &state,
            &proposal_canonical,
        )?;
        self.spend(record.steps_used);
        self.records.push(record);
        let authorized = match decision {
            authorization::AuthorizationOutcome::Granted(authorized) => authorized,
            authorization::AuthorizationOutcome::Refused(_) => {
                return Ok(PrefixOutcome::Terminal(
                    DurableStatus::Rejected,
                    "authorize_refused",
                ))
            }
            authorization::AuthorizationOutcome::Undecided(reason) => {
                let status = if reason == "fuel" || reason == "depth" {
                    DurableStatus::BudgetExhausted
                } else {
                    DurableStatus::Rejected
                };
                return Ok(PrefixOutcome::Terminal(status, "authorize_did_not_decide"));
            }
        };
        Ok(PrefixOutcome::Ready(Box::new(Prefix {
            operation: authorized.binding().to_owned(),
            granted_budget: authorized.granted_budget(),
            state,
            state_digest,
            proposal_canonical,
            proposal_digest,
            projected,
            authorized,
        })))
    }

    /// Executes the boundary and everything after it, from one program
    /// counter. The authorization is spent in exactly one branch.
    fn tail(
        &mut self,
        prefix: Prefix,
        counter: ProgramCounter,
        read: &mut dyn AgentReadOperation,
        reconciliation: Reconciliation<'_>,
        cancellation: &AgentCancellation,
        store: &mut dyn CheckpointStore,
    ) -> Result<DurableRun, Vec<Diagnostic>> {
        // The grant is destructured out of the prefix here, so exactly one
        // branch below can reach it and every other branch drops it unspent.
        let Prefix {
            state,
            state_digest: _,
            proposal_canonical,
            proposal_digest: _,
            projected,
            authorized,
            operation,
            granted_budget,
        } = prefix;
        let observation = match counter {
            ProgramCounter::Prefix => {
                if self.crash == CrashPoint::BeforeIntent {
                    drop(authorized);
                    return Ok(self.finish(DurableStatus::Crashed, "crashed_before_intent"));
                }
                if self.budgets.effect_grants_remaining == 0 {
                    drop(authorized);
                    return Ok(
                        self.finish(DurableStatus::BudgetExhausted, "effect_grant_exhausted")
                    );
                }
                if cancellation.is_cancelled() {
                    drop(authorized);
                    return Ok(self.finish(DurableStatus::Cancelled, "cancelled_before_execute"));
                }
                // The intent is durable strictly before the boundary, and the
                // grant is consumed with it whatever the operation's fate.
                self.budgets.effect_grants_remaining -= 1;
                self.entries.push(JournalEntry::Intent {
                    operation: operation.clone(),
                    granted_budget,
                });
                if !self.commit(store)? {
                    drop(authorized);
                    return Ok(self.finish(DurableStatus::StoreFailed, "intent_not_durable"));
                }
                if self.crash == CrashPoint::AfterIntent {
                    drop(authorized);
                    return Ok(self.finish(DurableStatus::Crashed, "crashed_after_intent"));
                }
                self.crossings += 1;
                let spent =
                    self.agent
                        .lifecycle
                        .spend(authorized, &state, &proposal_canonical, read);
                let Ok(value) = spent else {
                    // A reported failure is not evidence of non-occurrence:
                    // the journal stays at its intent and stays reconcilable.
                    return Ok(self.finish(
                        DurableStatus::EffectFailed,
                        "effect_reported_failure_delivery_uncertain",
                    ));
                };
                if self.crash == CrashPoint::AfterEffect {
                    return Ok(self.finish(DurableStatus::Crashed, "crashed_after_effect"));
                }
                match self.settle(&operation, value, store)? {
                    Ok(observation) => observation,
                    Err((status, reason)) => return Ok(self.finish(status, reason)),
                }
            }
            ProgramCounter::Intent => {
                // Uncertain. The freshly minted grant is dropped unspent, so
                // this branch cannot reach the read operation at all.
                drop(authorized);
                match reconciliation {
                    Reconciliation::None => {
                        return Ok(self.finish(
                            DurableStatus::Unknown,
                            "uncertain_delivery_requires_reconciliation",
                        ))
                    }
                    Reconciliation::Abandoned => {
                        self.entries.push(JournalEntry::Abandoned { operation });
                        if !self.commit(store)? {
                            return Ok(
                                self.finish(DurableStatus::StoreFailed, "abandonment_not_durable")
                            );
                        }
                        return Ok(
                            self.finish(DurableStatus::Abandoned, "host_reconciled_to_abandonment")
                        );
                    }
                    Reconciliation::Settled(bytes) => {
                        match self.settle(&operation, bytes.to_vec(), store)? {
                            Ok(observation) => observation,
                            Err((status, reason)) => return Ok(self.finish(status, reason)),
                        }
                    }
                }
            }
            ProgramCounter::Settled | ProgramCounter::Reduced => {
                drop(authorized);
                match self.recorded_observation(reconciliation) {
                    Ok(observation) => observation,
                    Err((status, reason)) => return Ok(self.finish(status, reason)),
                }
            }
            ProgramCounter::Abandoned | ProgramCounter::Delivered => {
                drop(authorized);
                return Err(vec![invariant("checkpoint.terminal_counter")]);
            }
        };
        self.reduce(
            state,
            projected,
            observation,
            counter == ProgramCounter::Reduced,
            store,
        )
    }

    /// Commits the settled observation under the declared retention mode.
    #[allow(clippy::type_complexity)]
    fn settle(
        &mut self,
        operation: &str,
        value: Vec<u8>,
        store: &mut dyn CheckpointStore,
    ) -> Result<Result<Vec<u8>, (DurableStatus, &'static str)>, Vec<Diagnostic>> {
        let observation_digest = digest(OBSERVATION_DOMAIN, &value);
        self.entries.push(JournalEntry::Settled {
            operation: operation.to_owned(),
            observation_digest,
            observation: match self.retention {
                Retention::ObservationBytes => Some(value.clone()),
                Retention::ObservationDigestOnly => None,
            },
        });
        if !self.commit(store)? {
            // The operation happened but its settlement is not durable, so the
            // stored generation still says "intent" and stays reconcilable.
            return Ok(Err((
                DurableStatus::Unknown,
                "settlement_not_durable_delivery_uncertain",
            )));
        }
        if self.crash == CrashPoint::AfterSettlement {
            return Ok(Err((DurableStatus::Crashed, "crashed_after_settlement")));
        }
        Ok(Ok(value))
    }

    /// Recovers the settled observation of an already-settled journal.
    fn recorded_observation(
        &self,
        reconciliation: Reconciliation<'_>,
    ) -> Result<Vec<u8>, (DurableStatus, &'static str)> {
        let Some(JournalEntry::Settled {
            observation_digest,
            observation,
            ..
        }) = self.entries.iter().find(|entry| entry.kind() == "settled")
        else {
            return Err((DurableStatus::Rejected, "settlement_missing_from_journal"));
        };
        if let Some(bytes) = observation {
            return Ok(bytes.clone());
        }
        match reconciliation {
            Reconciliation::Settled(bytes)
                if digest(OBSERVATION_DOMAIN, bytes) == *observation_digest =>
            {
                Ok(bytes.to_vec())
            }
            Reconciliation::Settled(_) => Err((
                DurableStatus::Rejected,
                "reconciled_observation_digest_mismatch",
            )),
            _ => Err((
                DurableStatus::Unknown,
                "redacted_observation_requires_reconciliation",
            )),
        }
    }

    /// Runs the deterministic reduction and delivers its result.
    fn reduce(
        &mut self,
        state: RetainedValue,
        projected: Vec<RetainedValue>,
        observation: Vec<u8>,
        already_reduced: bool,
        store: &mut dyn CheckpointStore,
    ) -> Result<DurableRun, Vec<Diagnostic>> {
        let lifecycle = &self.agent.lifecycle;
        if self.allot() == 0 {
            return Ok(self.finish(DurableStatus::BudgetExhausted, "no_fuel_for_reduce"));
        }
        let mut arguments = Vec::with_capacity(2 + projected.len());
        arguments.push(state);
        arguments.extend(projected);
        arguments.push(payload(&lifecycle.binding.outcome, observation, 0));
        let evaluation = lifecycle.evaluate(&lifecycle.binding.reduce, &arguments, self.allot())?;
        self.spend(evaluation.steps_used);
        self.records
            .push(StageRecord::of(&lifecycle.binding.reduce, &evaluation));
        let RetainedCallOutcome::Returned(result) = evaluation.outcome else {
            return Ok(self.finish(
                durable_terminal(&evaluation.outcome),
                "reduce_did_not_return",
            ));
        };
        if !lifecycle.carries(&result, "result") {
            return Err(vec![invariant("reduce.result.identity")]);
        }
        let result_digest = digest(RESULT_DOMAIN, encode_value(&result).as_bytes());

        if already_reduced {
            let Some(JournalEntry::Reduced {
                result_digest: recorded,
            }) = self.entries.iter().find(|entry| entry.kind() == "reduced")
            else {
                return Err(vec![invariant("checkpoint.journal.reduced")]);
            };
            if *recorded != result_digest {
                return Ok(self.finish(DurableStatus::Rejected, "result_digest_mismatch"));
            }
        } else {
            self.entries.push(JournalEntry::Reduced {
                result_digest: result_digest.clone(),
            });
            if !self.commit(store)? {
                return Ok(self.finish(DurableStatus::StoreFailed, "reduction_not_durable"));
            }
            if self.crash == CrashPoint::BeforeDelivery {
                return Ok(self.finish(DurableStatus::Crashed, "crashed_before_delivery"));
            }
        }
        self.entries.push(JournalEntry::Delivered {
            result_digest: result_digest.clone(),
        });
        if !self.commit(store)? {
            return Ok(self.finish(DurableStatus::StoreFailed, "delivery_not_durable"));
        }
        Ok(self.publish(
            DurableStatus::Completed,
            "reduce_published_a_result",
            Some(result),
            Some(result_digest),
        ))
    }
}

/// The terminal condition of a stage that did not return.
fn durable_terminal(outcome: &RetainedCallOutcome) -> DurableStatus {
    match terminal(outcome) {
        super::LifecycleStatus::BudgetExhausted => DurableStatus::BudgetExhausted,
        _ => DurableStatus::Rejected,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_evidence(
    agent: &DurableAgent,
    status: DurableStatus,
    reason: &str,
    stages: &[StageRecord],
    checkpoint: Option<&AgentCheckpoint>,
    budgets: CheckpointBudgets,
    crossings: usize,
    result_digest: Option<&str>,
) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},\"lifecycle_digest\":{},\"bound_digest\":{},\"policy_epoch\":{},\"status\":{},\"reason\":{},\"program_counter\":{},\"generation\":{},\"boundary_crossings\":{},\"effect_grants_remaining\":{},\"total_steps_remaining\":{},\"result_digest\":{},\"checkpoint_digest\":{},\"stages\":[",
        quote_json(DURABLE_EVIDENCE_SCHEMA),
        quote_json(agent.lifecycle.agent_id()),
        quote_json(&agent.binding.lifecycle_digest),
        quote_json(&agent.binding.bound_digest),
        quote_json(&agent.binding.policy_epoch.to_string()),
        quote_json(status.name()),
        quote_json(reason),
        checkpoint.map_or_else(
            || "null".to_owned(),
            |point| quote_json(point.program_counter().name())
        ),
        checkpoint.map_or_else(
            || "null".to_owned(),
            |point| quote_json(&point.generation().to_string())
        ),
        quote_json(&crossings.to_string()),
        quote_json(&budgets.effect_grants_remaining.to_string()),
        quote_json(&budgets.total_steps_remaining.to_string()),
        result_digest.map_or_else(|| "null".to_owned(), quote_json),
        checkpoint.map_or_else(|| "null".to_owned(), |point| quote_json(point.digest()))
    );
    for (index, stage) in stages.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"function_id\":{},\"outcome\":{}}}",
            quote_json(stage.role()),
            quote_json(stage.function_id()),
            quote_json(stage.outcome())
        ));
    }
    output.push_str("],\"nonclaims\":[");
    for (index, nonclaim) in checkpoint::NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(nonclaim));
    }
    output.push_str("]}\n");
    output
}
