//! Checked source execution over the source checkpoint journal.
//! The ordinary live API remains separate; unsupported sources fail closed.
use super::*;
mod session;
#[cfg(test)]
mod tests;
use crate::live_invocation::{
    source_journal::{
        RecoveredSourceCheckpoint, SourceCheckpointSink, SourceInvocationBinding,
        SourceInvocationSeed, SourceJournalError, SourceTerminalStatus,
    },
    CumulativeBudgetLedger, SourceInvocationClock,
};
pub(crate) use session::SourceExecutionSession;

/// Host-selected policy. Source, lifecycle, task and stage identities are
/// derived by the checked driver, never accepted from proposal text.
#[derive(Clone, Debug)]
pub struct SourceLivePolicy {
    pub deployment_binding: String,
    pub response_limit: usize,
    pub ceiling: i64,
    pub reservation_units: i64,
    pub unit: String,
    pub clock_domain: String,
    pub initial_millis: i64,
    pub deadline_millis: i64,
    pub max_total_steps: usize,
    pub program_root: Option<String>,
}

/// The policy an explicit checkpoint-capable source is prepared to execute.
#[derive(Clone, Copy, Debug)]
pub struct SourceProposalPolicy<'a> {
    pub deployment_binding: &'a str,
    pub response_limit: usize,
    pub reservation_units: i64,
}

/// Canonical host request identity, prepared without charging or dispatch.
/// Replay compares all fields before exposing stored response bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAttemptIdentity {
    pub request_digest: String,
    pub prompt_digest: String,
    pub request_bytes: usize,
}

/// One host adapter attempt. Dispatches count entries into its model handler,
/// not remote billing; an uncertain timeout never proves zero provider work.
pub struct SourceProposalOutcome {
    pub terminal_failure: Option<SourceTerminalStatus>,
    pub result: Result<String, Vec<Diagnostic>>,
    pub model_dispatches: u32,
}

impl SourceLivePolicy {
    pub fn binding(
        &self,
        compiled: &CompiledIterativeLifecycle,
        task: &LifecycleTask,
        budget: IterativeBudget,
    ) -> Result<SourceInvocationBinding, SourceJournalError> {
        SourceInvocationBinding::bind_execution(
            SourceInvocationSeed {
                lifecycle_digest: compiled.digest().to_owned(),
                source_revision: compiled.source_revision().to_owned(),
                deployment_binding: self.deployment_binding.clone(),
                task: task.objective.clone(),
                task_budget: task.budget,
                proposal_schema_digest: compiled.proposal_schema().schema().digest().to_owned(),
                response_limit: self.response_limit,
                max_iterations: u32::try_from(budget.max_iterations)
                    .map_err(|_| SourceJournalError::Binding)?,
                max_stages: u32::try_from(budget.max_stages)
                    .map_err(|_| SourceJournalError::Binding)?,
                max_attempts: driver::MAX_PROPOSAL_ATTEMPTS as u32,
                max_steps_per_stage: budget.max_steps_per_stage,
                max_total_steps: self.max_total_steps,
                ceiling: self.ceiling,
                reservation_units: self.reservation_units,
                unit: self.unit.clone(),
                clock_domain: self.clock_domain.clone(),
                initial_millis: self.initial_millis,
                deadline_millis: self.deadline_millis,
                program_root: self.program_root.clone(),
            },
            &digest(
                b"semaprax.source-stage-evaluator.v2\0",
                b"checked-retained-interpreter;left-to-right;full-stage-reservation;profile-1",
            ),
        )
    }
}

/// Invocation inputs are host-selected and bound before any stage executes.
/// Recovery bytes must be the latest generation from an exclusively held,
/// trusted store; integrity hashes do not authenticate storage or freshness.
pub struct SourceLiveRequest<'a> {
    pub task: &'a LifecycleTask,
    pub budget: IterativeBudget,
    pub policy: &'a SourceLivePolicy,
    pub clock: &'a dyn SourceInvocationClock,
    pub cancellation: &'a AgentCancellation,
    pub checkpoint: Option<&'a str>,
}

/// Fresh runs retain their checked carrier; recovered terminals expose only
/// their opaque receipt. Neither variant authorizes an effect or migration.
pub struct SourceLiveOutcome {
    pub checked_run: Option<IterativeRun>,
    pub checkpoint: RecoveredSourceCheckpoint,
    pub model_dispatches: u32,
    pub effect_dispatches: u32,
}

/// Failure retains the last acknowledged prefix and this traversal's partial
/// evidence. A poisoned store may contain a newer generation than this prefix.
pub struct SourceLiveFailure {
    pub diagnostics: Vec<Diagnostic>,
    pub selected: Option<SourceTerminalStatus>,
    pub checked_run: Option<IterativeRun>,
    pub checkpoint: Option<RecoveredSourceCheckpoint>,
    pub stage_rows: Vec<crate::live_invocation::source_journal::SourceStageSummary>,
    pub model_dispatches: u32,
    pub effect_dispatches: u32,
    pub journal_error: Option<SourceJournalError>,
}
impl std::fmt::Debug for SourceLiveFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceLiveFailure")
            .field("diagnostics", &self.diagnostics)
            .field("selected", &self.selected)
            .field("journal_error", &self.journal_error)
            .field("model_dispatches", &self.model_dispatches)
            .field("effect_dispatches", &self.effect_dispatches)
            .finish()
    }
}
impl SourceLiveFailure {
    fn initial(error: SourceJournalError, checkpoint: Option<RecoveredSourceCheckpoint>) -> Self {
        Self {
            diagnostics: vec![bad("source.binding_or_recovery")],
            selected: None,
            checked_run: None,
            checkpoint,
            stage_rows: Vec::new(),
            model_dispatches: 0,
            effect_dispatches: 0,
            journal_error: Some(error),
        }
    }
}
impl CompiledIterativeLifecycle {
    /// Executes the ordinary checked source loop with acknowledged stage,
    /// proposal and effect boundaries. Unresolved intents never redispatch.
    pub fn run_live_durable(
        &self,
        request: SourceLiveRequest<'_>,
        source: &mut dyn driver::ProposalSource,
        read: &mut dyn AgentReadOperation,
        store: &mut dyn CheckpointStore,
    ) -> Result<SourceLiveOutcome, SourceLiveFailure> {
        use crate::live_invocation::source_journal::{
            recover_source_checkpoint, SourceJournalEntry,
        };
        let binding = request
            .policy
            .binding(self, request.task, request.budget)
            .map_err(|error| SourceLiveFailure::initial(error, None))?;
        let recovered = request
            .checkpoint
            .map(|document| recover_source_checkpoint(document, &binding))
            .transpose()
            .map_err(|error| SourceLiveFailure::initial(error, None))?;
        if let Some(checkpoint) = recovered.as_ref() {
            if checkpoint.terminal_snapshot().is_some() {
                return Ok(SourceLiveOutcome {
                    checked_run: None,
                    checkpoint: checkpoint.clone(),
                    model_dispatches: 0,
                    effect_dispatches: 0,
                });
            }
            if let Some(SourceJournalEntry::Stop { turn, status, .. }) = checkpoint.entries().last()
            {
                let turn = *turn;
                let status = (*status).into();
                if request.clock.clock_domain() != checkpoint.clock_domain()
                    || request.clock.now_millis() < checkpoint.last_checked_millis()
                {
                    return Err(SourceLiveFailure::initial(
                        SourceJournalError::Time,
                        recovered,
                    ));
                }
                let mut sink = SourceCheckpointSink::resume(store, checkpoint.clone())
                    .map_err(|error| SourceLiveFailure::initial(error, recovered.clone()))?;
                let entry = sink
                    .terminal_snapshot_entry(
                        turn,
                        status,
                        None,
                        crate::live_invocation::source_journal::SourceTerminalEvidenceInput {
                            completed_stages: 0,
                            omitted_stage_rows: 0,
                            stage_rows: Vec::new(),
                            checked_run_evidence: None,
                        },
                    )
                    .map_err(|error| SourceLiveFailure::initial(error, recovered.clone()))?;
                sink.append_at(entry, request.clock.now_millis())
                    .map_err(|error| SourceLiveFailure::initial(error, recovered.clone()))?;
                return Ok(SourceLiveOutcome {
                    checked_run: None,
                    checkpoint: sink
                        .checkpoint()
                        .map_err(|error| SourceLiveFailure::initial(error, recovered.clone()))?,
                    model_dispatches: 0,
                    effect_dispatches: 0,
                });
            }
            if checkpoint.is_uncertain() {
                return Err(SourceLiveFailure::initial(
                    SourceJournalError::Uncertain,
                    recovered,
                ));
            }
        }
        if !source.checkpoint_policy().is_some_and(|policy| {
            policy.deployment_binding == request.policy.deployment_binding
                && policy.response_limit == request.policy.response_limit
                && policy.reservation_units == request.policy.reservation_units
        }) {
            return Err(SourceLiveFailure::initial(
                SourceJournalError::Binding,
                recovered,
            ));
        }
        let ledger = match recovered.as_ref() {
            Some(checkpoint) => {
                CumulativeBudgetLedger::resume_source_shared(checkpoint, request.clock)
            }
            None => CumulativeBudgetLedger::start_source(&binding, request.clock),
        }
        .map_err(|_| SourceLiveFailure::initial(SourceJournalError::Time, recovered.clone()))?;
        let sink = match recovered {
            Some(checkpoint) => SourceCheckpointSink::resume(store, checkpoint)
                .map_err(|error| SourceLiveFailure::initial(error, None))?,
            None => SourceCheckpointSink::new(store, binding),
        };
        let mut session =
            SourceExecutionSession::new(sink, ledger, request.clock, request.cancellation);
        if let Some(SourceJournalEntry::Stop { status, .. }) =
            session.sink.journal().entries().last()
        {
            let selected = (*status).into();
            return session.complete(None, Some(selected), Vec::new());
        }
        if let Err(errors) = session.opened() {
            return Err(session.failure(None, errors));
        }
        let mut driver = driver::ReadDriver { read };
        match self.run_with_driver_live_session(
            request.task,
            source,
            &mut driver,
            request.budget,
            request.cancellation,
            Some(&mut session),
        ) {
            Ok(run) => session.complete(Some(run), None, Vec::new()),
            Err(driver::DriverFailure::Diagnostics(errors)) => session.complete(None, None, errors),
            Err(driver::DriverFailure::Persistence {
                terminal,
                diagnostics,
            }) => session.complete(Some(*terminal), None, diagnostics),
        }
    }
}
