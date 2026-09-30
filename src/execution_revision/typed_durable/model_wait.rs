//! Opt-in real model dispatch through the one authoritative source wait journal.
use super::*;
use crate::agent_lifecycle::iterative::model_wait::SourceModelWaitBinding;
use crate::agent_lifecycle::iterative::source_live::SourceProposalPolicy;
use crate::resumable_effects::source_checkpoint::SourceCheckpointKey;

pub struct AgentRuntimeV2DurableModelWaitEvidence {
    model: AgentRuntimeV2DurableModelEvidence,
    wait_evidence: Vec<u8>,
    evidence: ExecutionRoot,
}
impl AgentRuntimeV2DurableModelWaitEvidence {
    pub fn model(&self) -> &AgentRuntimeV2DurableModelEvidence {
        &self.model
    }
    pub fn wait_evidence(&self) -> &[u8] {
        &self.wait_evidence
    }
    pub fn evidence_root(&self) -> &ExecutionRoot {
        &self.evidence
    }
}

impl AgentRuntimeV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn run_live_bound_model_durable_with_wait(
        self,
        wait: &SourceModelWaitBinding,
        key: &SourceCheckpointKey,
        source: &mut StreamingSourceProposalAdapter<'_>,
        handler: &mut dyn TypedEffectHandler,
        policy: SourceLivePolicy,
        clock: &dyn SourceInvocationClock,
        cancellation: &AgentCancellation,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
    ) -> std::result::Result<
        AgentRuntimeV2DurableModelWaitEvidence,
        AgentRuntimeV2DurableModelFailure,
    > {
        let model = source
            .model_binding()
            .filter(|binding| {
                binding.runtime_matches(
                    self.deployment.digest(),
                    self.instance.digest(),
                    self.lifecycle.proposal_schema().source_revision(),
                    self.lifecycle.proposal_schema().schema().digest(),
                )
            })
            .ok_or_else(|| {
                AgentRuntimeV2DurableModelFailure::preflight(&self, source, "wait.model_binding")
            })?;
        if !wait.matches(self.lifecycle.source_lifecycle())
            || wait.evaluation_fuel() > self.budget.max_steps_per_stage
            || !self.proposals.is_empty()
            || !source.model_evidence().attempts().is_empty()
            || policy.program_root.is_some()
            || policy.deployment_binding != model.digest()
            || policy.response_limit != model.max_response_bytes()
            || policy.reservation_units <= 0
            || !source.ordinary_checkpoint_matches(&SourceProposalPolicy {
                deployment_binding: &policy.deployment_binding,
                response_limit: policy.response_limit,
                reservation_units: policy.reservation_units,
            })
        {
            return Err(AgentRuntimeV2DurableModelFailure::preflight(
                &self,
                source,
                "wait.profile",
            ));
        }
        let model_digest = model.digest().to_owned();
        let typed_profile =
            typed_source_program_root(&self.program_root, self.lifecycle.digest(), self.effects);
        source.configure_durable_boundary(cancellation, policy.deadline_millis);
        let request = SourceLiveRequest {
            task: &self.task,
            budget: self.budget,
            policy: &policy,
            clock,
            cancellation,
            checkpoint: retained_checkpoint,
        };
        match self.lifecycle.run_live_durable_source_with_wait(
            request,
            source,
            handler,
            self.effects,
            store,
            wait,
            key,
        ) {
            Ok(run) => {
                let model_evidence = source.model_evidence().clone();
                let terminal_digest = match run.checkpoint.entries().last() {
                    Some(SourceJournalEntry::TerminalSnapshot {
                        evidence_digest, ..
                    }) => evidence_digest,
                    _ => {
                        return Err(postflight_failure(
                            &self,
                            source,
                            &run,
                            &model_digest,
                            &typed_profile,
                            "wait.terminal",
                        ))
                    }
                };
                let ordinary_evidence = durable_model_evidence_root(
                    &self,
                    &run,
                    &model_evidence,
                    &model_digest,
                    &typed_profile,
                    "completed",
                );
                let wait_evidence = run
                    .checkpoint
                    .wait_evidence(terminal_digest, ordinary_evidence.digest())
                    .map_err(|_| {
                        postflight_failure(
                            &self,
                            source,
                            &run,
                            &model_digest,
                            &typed_profile,
                            "wait.evidence",
                        )
                    })?;
                let evidence = ExecutionRoot {
                    digest: run
                        .checkpoint
                        .wait_evidence_digest(terminal_digest, ordinary_evidence.digest())
                        .map_err(|_| {
                            postflight_failure(
                                &self,
                                source,
                                &run,
                                &model_digest,
                                &typed_profile,
                                "wait.evidence",
                            )
                        })?,
                    json: String::from_utf8(wait_evidence.clone())
                        .expect("canonical wait evidence UTF-8"),
                };
                let source_binding = Some(run.checkpoint.binding().clone());
                Ok(AgentRuntimeV2DurableModelWaitEvidence {
                    model: AgentRuntimeV2DurableModelEvidence {
                        run,
                        model_evidence,
                        evidence: ordinary_evidence,
                        revision: self.revision,
                        source_binding,
                        source_policy: Some(policy),
                        source_model_binding_digest: Some(model_digest),
                        source_model_policy: None,
                    },
                    wait_evidence,
                    evidence,
                })
            }
            Err(failure) => {
                let model_evidence = source.model_evidence().clone();
                let evidence = durable_model_failure_root(
                    &self,
                    &failure,
                    &model_evidence,
                    &model_digest,
                    &typed_profile,
                    "source_wait_failed",
                );
                Err(AgentRuntimeV2DurableModelFailure {
                    failure,
                    model_evidence,
                    evidence,
                    revision: self.revision,
                })
            }
        }
    }
}

fn postflight_failure(
    runtime: &AgentRuntimeV2,
    source: &StreamingSourceProposalAdapter<'_>,
    run: &SourceLiveOutcome,
    binding: &str,
    typed_profile: &str,
    field: &str,
) -> AgentRuntimeV2DurableModelFailure {
    let failure = SourceLiveFailure {
        diagnostics: vec![Diagnostic::io("source.model_wait_evidence", field)],
        selected: None,
        checked_run: None,
        checkpoint: Some(run.checkpoint.clone()),
        stage_rows: vec![],
        model_dispatches: run.model_dispatches,
        effect_dispatches: run.effect_dispatches,
        journal_error: Some(SourceJournalError::Binding),
    };
    let model_evidence = source.model_evidence().clone();
    let evidence = durable_model_failure_root(
        runtime,
        &failure,
        &model_evidence,
        binding,
        typed_profile,
        field,
    );
    AgentRuntimeV2DurableModelFailure {
        failure,
        model_evidence,
        evidence,
        revision: runtime.revision.clone(),
    }
}
