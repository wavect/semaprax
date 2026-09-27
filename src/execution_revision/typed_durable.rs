//! Store-backed direct Runtime v2 execution and evidence association.
use super::*;
use crate::agent_lifecycle::iterative::effects::{
    DurableTypedFailure, DurableTypedRun, TargetStageBackend, TypedEffectHandler,
};
use crate::agent_lifecycle::iterative::source_live::{
    SourceIoLimits, SourceLiveFailure, SourceLiveMigrationEndpoint, SourceLiveOutcome,
    SourceLivePolicy, SourceLiveRequest,
};
use crate::agent_lifecycle::CheckpointStore;
use crate::agent_runtime_v2::{SourceModelEvidence, SourceModelPolicyBinding};
use crate::diagnostic::Diagnostic;
use crate::live_invocation::source_journal::{
    recover_source_checkpoint, RecoveredSourceCheckpoint, SourceInvocationBinding,
    SourceJournalEntry, SourceJournalError, SourcePolicyBindingV6,
};
use crate::live_invocation::{CumulativeBudgetLedger, SourceInvocationClock};
use crate::provider_adapter_sdk::StreamingSourceProposalAdapter;

#[path = "typed_durable/migration.rs"]
mod migration;
pub use migration::PreparedAgentRuntimeV2SourceMigration;

impl AgentRuntimeV2 {
    /// Derives the exact unpriced durable checkpoint binding before a caller
    /// constructs a provider adapter or grants provider invocation authority.
    /// This is a read-only recovery preflight; it neither opens the retained
    /// journal nor creates a source capability.
    fn source_live_checkpoint_binding(
        &self,
        binding: &crate::agent_runtime_v2::SourceModelBinding,
        mut policy: SourceLivePolicy,
    ) -> std::result::Result<SourceInvocationBinding, SourceJournalError> {
        if !binding.runtime_matches(
            self.deployment.digest(),
            self.instance.digest(),
            self.lifecycle.proposal_schema().source_revision(),
            self.lifecycle.proposal_schema().schema().digest(),
        ) || policy.deployment_binding != binding.digest()
            || policy.response_limit != binding.max_response_bytes()
            || policy.reservation_units <= 0
        {
            return Err(SourceJournalError::Binding);
        }
        policy.program_root = Some(typed_source_program_root(
            &self.program_root,
            self.lifecycle.digest(),
            self.effects,
        ));
        policy.binding(self.lifecycle.source_lifecycle(), &self.task, self.budget)
    }

    /// Authenticates and checks every initial unpriced recovery refusal before
    /// a host needs to construct a provider adapter. A valid nonterminal
    /// checkpoint is returned as read-only state; this method cannot resume,
    /// append, dispatch, or mint provider authority.
    pub fn preflight_source_live_checkpoint(
        &self,
        model_binding: &crate::agent_runtime_v2::SourceModelBinding,
        policy: SourceLivePolicy,
        document: &str,
        clock: &dyn SourceInvocationClock,
    ) -> std::result::Result<RecoveredSourceCheckpoint, SourceJournalError> {
        let binding = self.source_live_checkpoint_binding(model_binding, policy)?;
        let recovered = recover_source_checkpoint(document, &binding)?;
        if recovered.terminal_snapshot().is_some() {
            return Ok(recovered);
        }
        if matches!(
            recovered.entries().last(),
            Some(SourceJournalEntry::Stop { .. })
        ) {
            if clock.clock_domain() != recovered.clock_domain()
                || clock.now_millis() < recovered.last_checked_millis()
            {
                return Err(SourceJournalError::Time);
            }
            return Ok(recovered);
        }
        if recovered.is_uncertain() {
            return Err(SourceJournalError::Uncertain);
        }
        CumulativeBudgetLedger::resume_source_shared(&recovered, clock)
            .map_err(|_| SourceJournalError::Time)?;
        Ok(recovered)
    }

    /// Produces the exact durable source-policy facts for this typed runtime.
    /// The returned value binds the typed effect registry and narrowed effect
    /// budget into the journal root; callers can retain it for a later checked
    /// source migration, but it grants no source or provider operation.
    pub fn source_live_model_policy(
        &self,
        binding: &crate::agent_runtime_v2::SourceModelBinding,
        model_policy: &SourceModelPolicyBinding,
        mut policy: SourceLivePolicy,
    ) -> std::result::Result<SourceLivePolicy, Vec<Diagnostic>> {
        if !binding.runtime_matches(
            self.deployment.digest(),
            self.instance.digest(),
            self.lifecycle.proposal_schema().source_revision(),
            self.lifecycle.proposal_schema().schema().digest(),
        ) || !model_policy.matches(binding)
            || policy.deployment_binding != binding.digest()
            || policy.response_limit != binding.max_response_bytes()
            || policy.reservation_units <= 0
        {
            return Err(vec![Diagnostic::io(
                "source.model_durable_policy",
                "typed durable source policy refused",
            )]);
        }
        policy.deadline_millis = durable_policy_deadline(&policy, model_policy).map_err(|_| {
            vec![Diagnostic::io(
                "source.model_durable_policy",
                "typed durable source policy refused",
            )]
        })?;
        policy.program_root = Some(typed_source_program_root(
            &self.program_root,
            self.lifecycle.digest(),
            self.effects,
        ));
        Ok(policy)
    }

    /// Forms a source-migration endpoint from this retained typed runtime.
    /// Path and agent identity are revalidated by checked migration preparation;
    /// this helper only prevents callers from substituting a different compiled
    /// lifecycle for the runtime that produced the source checkpoint.
    pub fn source_live_migration_endpoint<'a>(
        &'a self,
        source_path: &'a str,
        agent_id: &'a str,
        policy: &'a SourceLivePolicy,
    ) -> SourceLiveMigrationEndpoint<'a> {
        SourceLiveMigrationEndpoint {
            project: self.project.as_ref(),
            source_path,
            agent_id,
            lifecycle: self.lifecycle.source_lifecycle(),
            policy,
            budget: self.budget,
        }
    }

    /// Execute using a caller-owned single-writer checkpoint store.
    ///
    /// `retained_checkpoint` must be an acknowledged snapshot from that trusted
    /// store. Its hashes detect drift; they do not authenticate host observations.
    /// The checked producer replays pure stages under fresh authorizations before
    /// using retained observations. An uncertain effect intent cannot redispatch.
    pub fn run_durable(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
    ) -> std::result::Result<AgentRuntimeV2DurableEvidence, DurableTypedFailure> {
        let run = self.lifecycle.run_durable(
            &self.task,
            &self.proposals,
            handler,
            self.budget,
            self.effects,
            cancellation,
            self.revision.digest(),
            &self.program_root,
            retained_checkpoint,
            store,
            max_reserved_fuel,
        )?;
        let evidence = root(
            "semaprax.evidence-root.v4",
            json!({
                "execution_revision": self.revision.digest(),
                "instance_root": self.instance.digest(),
                "typed_effect_evidence": run.run().evidence_digest(),
                "checkpoint_digest": run.checkpoint_digest(),
                "max_reserved_fuel": max_reserved_fuel,
            }),
        );
        Ok(AgentRuntimeV2DurableEvidence {
            run,
            evidence,
            revision: self.revision,
            migration_handoff: None,
            migrated_checkpoint: None,
        })
    }

    /// Durable execution with a caller-held native or Core Wasm stage host.
    /// Recovery preflight and journal replay are identical to `run_durable`;
    /// selecting a target cannot authorize a new effect or change identity.
    #[allow(clippy::too_many_arguments)]
    pub fn run_durable_with_backend(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
        selected: TargetStageBackend<'_>,
    ) -> std::result::Result<AgentRuntimeV2DurableEvidence, DurableTypedFailure> {
        let run = self.lifecycle.run_durable_with_backend(
            &self.task,
            &self.proposals,
            handler,
            self.budget,
            self.effects,
            cancellation,
            self.revision.digest(),
            &self.program_root,
            retained_checkpoint,
            store,
            max_reserved_fuel,
            selected,
        )?;
        let evidence = root(
            "semaprax.evidence-root.v4",
            json!({
                "execution_revision": self.revision.digest(),
                "instance_root": self.instance.digest(),
                "typed_effect_evidence": run.run().evidence_digest(),
                "checkpoint_digest": run.checkpoint_digest(),
                "max_reserved_fuel": max_reserved_fuel,
            }),
        );
        Ok(AgentRuntimeV2DurableEvidence {
            run,
            evidence,
            revision: self.revision,
            migration_handoff: None,
            migrated_checkpoint: None,
        })
    }

    /// Runs the explicitly checkpoint-capable, bound source adapter through
    /// the Source Live Journal.  A priced in-memory model ledger cannot be
    /// reconstructed from this journal profile, so that opt-in route refuses
    /// here before it can issue a provider call.
    #[allow(clippy::too_many_arguments)]
    pub fn run_live_bound_model_durable(
        self,
        source: &mut StreamingSourceProposalAdapter<'_>,
        handler: &mut dyn TypedEffectHandler,
        mut policy: SourceLivePolicy,
        clock: &dyn SourceInvocationClock,
        cancellation: &AgentCancellation,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
    ) -> std::result::Result<AgentRuntimeV2DurableModelEvidence, AgentRuntimeV2DurableModelFailure>
    {
        let binding = match source.model_binding() {
            Some(binding)
                if binding.runtime_matches(
                    self.deployment.digest(),
                    self.instance.digest(),
                    self.lifecycle.proposal_schema().source_revision(),
                    self.lifecycle.proposal_schema().schema().digest(),
                ) =>
            {
                binding
            }
            _ => {
                return Err(AgentRuntimeV2DurableModelFailure::preflight(
                    &self, source, "binding",
                ))
            }
        };
        if source.model_policy_binding().is_some() {
            return self.run_live_bound_model_durable_policy(
                source,
                handler,
                policy,
                clock,
                cancellation,
                retained_checkpoint,
                store,
                None,
            );
        }
        if !self.proposals.is_empty()
            || !source.model_evidence().attempts().is_empty()
            || policy.deployment_binding != binding.digest()
            || policy.response_limit != binding.max_response_bytes()
            || policy.reservation_units <= 0
        {
            return Err(AgentRuntimeV2DurableModelFailure::preflight(
                &self,
                source,
                "source_preflight",
            ));
        }
        let checkpoint_root =
            typed_source_program_root(&self.program_root, self.lifecycle.digest(), self.effects);
        let binding_digest = binding.digest().to_owned();
        source.configure_durable_boundary(cancellation, policy.deadline_millis);
        policy.program_root = Some(checkpoint_root.clone());
        let request = SourceLiveRequest {
            task: &self.task,
            budget: self.budget,
            policy: &policy,
            clock,
            cancellation,
            checkpoint: retained_checkpoint,
        };
        match self
            .lifecycle
            .run_live_durable_source(request, source, handler, self.effects, store)
        {
            Ok(run) => {
                let model_evidence = source.model_evidence().clone();
                let evidence = durable_model_evidence_root(
                    &self,
                    &run,
                    &model_evidence,
                    &binding_digest,
                    &checkpoint_root,
                    "completed",
                );
                Ok(AgentRuntimeV2DurableModelEvidence {
                    run,
                    model_evidence,
                    evidence,
                    revision: self.revision,
                    source_binding: None,
                    source_policy: None,
                    source_model_binding_digest: None,
                    source_model_policy: None,
                })
            }
            Err(failure) => {
                let model_evidence = source.model_evidence().clone();
                let evidence = durable_model_failure_root(
                    &self,
                    &failure,
                    &model_evidence,
                    &binding_digest,
                    &checkpoint_root,
                    "source_live_failed",
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

    /// Runs the additive V6 model-policy route with the existing V5
    /// cumulative provider-I/O limits bound into its checkpoint identity.
    /// The older entry retains its unpriced, no-I/O byte contract.
    #[allow(clippy::too_many_arguments)]
    pub fn run_live_bound_model_durable_with_io_limits(
        self,
        source: &mut StreamingSourceProposalAdapter<'_>,
        handler: &mut dyn TypedEffectHandler,
        policy: SourceLivePolicy,
        clock: &dyn SourceInvocationClock,
        cancellation: &AgentCancellation,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        limits: &SourceIoLimits,
    ) -> std::result::Result<AgentRuntimeV2DurableModelEvidence, AgentRuntimeV2DurableModelFailure>
    {
        self.run_live_bound_model_durable_policy(
            source,
            handler,
            policy,
            clock,
            cancellation,
            retained_checkpoint,
            store,
            Some(limits),
        )
    }

    /// Runs the V6 durable source route whose acknowledged model intents
    /// retain the exact #179 reservation and restore it before recovery can
    /// continue. The older durable entry remains the unpriced V2 profile.
    #[allow(clippy::too_many_arguments)]
    fn run_live_bound_model_durable_policy(
        self,
        source: &mut StreamingSourceProposalAdapter<'_>,
        handler: &mut dyn TypedEffectHandler,
        mut policy: SourceLivePolicy,
        clock: &dyn SourceInvocationClock,
        cancellation: &AgentCancellation,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        io_limits: Option<&SourceIoLimits>,
    ) -> std::result::Result<AgentRuntimeV2DurableModelEvidence, AgentRuntimeV2DurableModelFailure>
    {
        let binding = match source.model_binding() {
            Some(binding)
                if binding.runtime_matches(
                    self.deployment.digest(),
                    self.instance.digest(),
                    self.lifecycle.proposal_schema().source_revision(),
                    self.lifecycle.proposal_schema().schema().digest(),
                ) =>
            {
                binding
            }
            _ => {
                return Err(AgentRuntimeV2DurableModelFailure::preflight(
                    &self,
                    source,
                    "policy_binding",
                ))
            }
        };
        let policy_binding = match source.model_policy_binding() {
            Some(policy_binding) if policy_binding.matches(binding) => policy_binding,
            _ => {
                return Err(AgentRuntimeV2DurableModelFailure::preflight(
                    &self,
                    source,
                    "source_model_policy",
                ))
            }
        };
        policy = match self.source_live_model_policy(binding, policy_binding, policy) {
            Ok(policy) => policy,
            Err(_) => {
                return Err(AgentRuntimeV2DurableModelFailure::preflight(
                    &self,
                    source,
                    "policy_deadline",
                ))
            }
        };
        let model_deadline = policy.deadline_millis;
        if !source.model_policy_deadline_matches(model_deadline) {
            return Err(AgentRuntimeV2DurableModelFailure::preflight(
                &self,
                source,
                "policy_deadline",
            ));
        }
        if !self.proposals.is_empty()
            || !source.model_evidence().attempts().is_empty()
            || policy.deployment_binding != binding.digest()
            || policy.response_limit != binding.max_response_bytes()
            || policy.reservation_units <= 0
        {
            return Err(AgentRuntimeV2DurableModelFailure::preflight(
                &self,
                source,
                "policy_source_preflight",
            ));
        }
        let journal_policy = source_policy_binding_v6(binding, policy_binding).map_err(|_| {
            AgentRuntimeV2DurableModelFailure::preflight(&self, source, "policy_journal")
        })?;
        let checkpoint_root =
            typed_source_program_root(&self.program_root, self.lifecycle.digest(), self.effects);
        let binding_digest = binding.digest().to_owned();
        let policy_digest = policy_binding.digest().to_owned();
        let retained_model_policy = policy_binding.clone();
        let source_binding = match io_limits {
            Some(limits) => policy.binding_with_model_policy_and_io_limits(
                self.lifecycle.source_lifecycle(),
                &self.task,
                self.budget,
                journal_policy.clone(),
                limits,
            ),
            None => policy.binding_with_model_policy(
                self.lifecycle.source_lifecycle(),
                &self.task,
                self.budget,
                journal_policy.clone(),
            ),
        }
        .map_err(|_| {
            AgentRuntimeV2DurableModelFailure::preflight(&self, source, "policy_journal")
        })?;
        source.configure_durable_boundary(cancellation, policy.deadline_millis);
        let request = SourceLiveRequest {
            task: &self.task,
            budget: self.budget,
            policy: &policy,
            clock,
            cancellation,
            checkpoint: retained_checkpoint,
        };
        let run = match io_limits {
            Some(limits) => self
                .lifecycle
                .run_live_durable_source_with_model_policy_and_io_limits(
                    request,
                    journal_policy,
                    limits,
                    source,
                    handler,
                    self.effects,
                    store,
                ),
            None => self.lifecycle.run_live_durable_source_with_model_policy(
                request,
                journal_policy,
                source,
                handler,
                self.effects,
                store,
            ),
        };
        match run {
            Ok(run) => {
                let model_evidence = source.model_evidence().clone();
                let evidence = durable_model_policy_evidence_root(
                    &self,
                    &run,
                    &model_evidence,
                    &binding_digest,
                    &policy_digest,
                    &checkpoint_root,
                    "completed",
                );
                Ok(AgentRuntimeV2DurableModelEvidence {
                    run,
                    model_evidence,
                    evidence,
                    revision: self.revision,
                    source_binding: Some(source_binding),
                    source_policy: Some(policy),
                    source_model_binding_digest: Some(binding_digest),
                    source_model_policy: Some(retained_model_policy),
                })
            }
            Err(failure) => {
                let model_evidence = source.model_evidence().clone();
                let evidence = durable_model_policy_failure_root(
                    &self,
                    &failure,
                    &model_evidence,
                    &binding_digest,
                    &policy_digest,
                    &checkpoint_root,
                    "source_live_failed",
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

fn source_policy_binding_v6(
    binding: &crate::agent_runtime_v2::SourceModelBinding,
    policy: &SourceModelPolicyBinding,
) -> std::result::Result<SourcePolicyBindingV6, ()> {
    SourcePolicyBindingV6::new(
        binding.digest().to_owned(),
        policy.digest().to_owned(),
        policy.provider_id().to_owned(),
        policy.effective(),
    )
    .map_err(|_| ())
}

fn durable_policy_deadline(
    policy: &SourceLivePolicy,
    model: &SourceModelPolicyBinding,
) -> std::result::Result<i64, ()> {
    let model_deadline = if model.effective().limits().max_latency_millis == i64::MAX {
        policy.deadline_millis
    } else {
        policy
            .initial_millis
            .checked_add(model.effective().limits().max_latency_millis)
            .ok_or(())?
    };
    let deadline = policy.deadline_millis.min(model_deadline);
    if deadline <= policy.initial_millis {
        return Err(());
    }
    Ok(deadline)
}

fn typed_source_program_root(
    program_root: &str,
    registry: &str,
    effects: crate::agent_lifecycle::iterative::effects::EffectBudget,
) -> String {
    crate::live_invocation::identity::digest(
        b"semaprax.source-live.typed-effect-profile.v1\0",
        format!(
            "{}\0{}\0{}\0{}\0{}\0{}",
            program_root,
            registry,
            effects.max_calls,
            effects.max_argument_bytes,
            effects.max_result_bytes,
            effects.max_total_bytes,
        )
        .as_bytes(),
    )
}

/// Durable evidence binds the acknowledged journal product to both the exact
/// source-model selection and the typed registry/effect-cap profile.
pub struct AgentRuntimeV2DurableModelEvidence {
    run: SourceLiveOutcome,
    model_evidence: SourceModelEvidence,
    evidence: ExecutionRoot,
    revision: ExecutionRoot,
    source_binding: Option<SourceInvocationBinding>,
    source_policy: Option<SourceLivePolicy>,
    source_model_binding_digest: Option<String>,
    source_model_policy: Option<SourceModelPolicyBinding>,
}

impl AgentRuntimeV2DurableModelEvidence {
    pub fn run(&self) -> &SourceLiveOutcome {
        &self.run
    }
    pub fn model_evidence(&self) -> &SourceModelEvidence {
        &self.model_evidence
    }
    pub fn evidence_root(&self) -> &ExecutionRoot {
        &self.evidence
    }
    pub fn execution_revision(&self) -> &ExecutionRoot {
        &self.revision
    }
    /// Exact journal binding retained by the V6 source-policy route.
    pub fn source_binding(&self) -> Option<&SourceInvocationBinding> {
        self.source_binding.as_ref()
    }
    /// Effective policy retained by the V6 source-policy route.
    pub fn source_policy(&self) -> Option<&SourceLivePolicy> {
        self.source_policy.as_ref()
    }
    pub(crate) fn source_model_binding_digest(&self) -> Option<&str> {
        self.source_model_binding_digest.as_deref()
    }
    pub(crate) fn source_model_policy(&self) -> Option<&SourceModelPolicyBinding> {
        self.source_model_policy.as_ref()
    }
}

/// A durable source failure retains its last acknowledged journal prefix and
/// redacted model evidence; it exposes neither provider request nor response.
pub struct AgentRuntimeV2DurableModelFailure {
    failure: SourceLiveFailure,
    model_evidence: SourceModelEvidence,
    evidence: ExecutionRoot,
    revision: ExecutionRoot,
}

impl AgentRuntimeV2DurableModelFailure {
    fn preflight(
        runtime: &AgentRuntimeV2,
        source: &StreamingSourceProposalAdapter<'_>,
        status: &str,
    ) -> Self {
        let model_evidence = SourceModelEvidence::default();
        let failure = SourceLiveFailure {
            diagnostics: vec![Diagnostic::io(
                "source.model_durable_preflight",
                "durable bound model source refused",
            )],
            selected: None,
            checked_run: None,
            checkpoint: None,
            stage_rows: Vec::new(),
            model_dispatches: 0,
            effect_dispatches: 0,
            journal_error: None,
        };
        let evidence = durable_model_failure_root(
            runtime,
            &failure,
            &model_evidence,
            source
                .model_binding()
                .map_or("", |binding| binding.digest()),
            "",
            status,
        );
        Self {
            failure,
            model_evidence,
            evidence,
            revision: runtime.revision.clone(),
        }
    }
    pub fn failure(&self) -> &SourceLiveFailure {
        &self.failure
    }
    pub fn model_evidence(&self) -> &SourceModelEvidence {
        &self.model_evidence
    }
    pub fn evidence_root(&self) -> &ExecutionRoot {
        &self.evidence
    }
    pub fn execution_revision(&self) -> &ExecutionRoot {
        &self.revision
    }
}

fn durable_model_evidence_root(
    runtime: &AgentRuntimeV2,
    run: &SourceLiveOutcome,
    model_evidence: &SourceModelEvidence,
    binding: &str,
    typed_profile: &str,
    status: &str,
) -> ExecutionRoot {
    root(
        "semaprax.evidence-root.v4",
        json!({
            "execution_revision": runtime.revision.digest(),
            "instance_root": runtime.instance.digest(),
            "source_model_binding": binding,
            "typed_effect_profile": typed_profile,
            "source_checkpoint": run.checkpoint.chain(),
            "model_evidence": model_evidence.digest(),
            "model_dispatches": run.model_dispatches,
            "effect_dispatches": run.effect_dispatches,
            "status": status,
        }),
    )
}

fn durable_model_failure_root(
    runtime: &AgentRuntimeV2,
    failure: &SourceLiveFailure,
    model_evidence: &SourceModelEvidence,
    binding: &str,
    typed_profile: &str,
    status: &str,
) -> ExecutionRoot {
    root(
        "semaprax.evidence-root.v4",
        json!({
            "execution_revision": runtime.revision.digest(),
            "instance_root": runtime.instance.digest(),
            "source_model_binding": binding,
            "typed_effect_profile": typed_profile,
            "model_evidence": model_evidence.digest(),
            "model_dispatches": failure.model_dispatches,
            "effect_dispatches": failure.effect_dispatches,
            "selected": failure.selected.map(|status| status.as_str()),
            "status": status,
        }),
    )
}

fn durable_model_policy_evidence_root(
    runtime: &AgentRuntimeV2,
    run: &SourceLiveOutcome,
    model_evidence: &SourceModelEvidence,
    binding: &str,
    policy_binding: &str,
    typed_profile: &str,
    status: &str,
) -> ExecutionRoot {
    root(
        "semaprax.evidence-root.v5",
        json!({
            "execution_revision": runtime.revision.digest(),
            "instance_root": runtime.instance.digest(),
            "source_model_binding": binding,
            "source_model_policy": policy_binding,
            "typed_effect_profile": typed_profile,
            "source_checkpoint": run.checkpoint.chain(),
            "model_evidence": model_evidence.digest(),
            "model_dispatches": run.model_dispatches,
            "effect_dispatches": run.effect_dispatches,
            "status": status,
        }),
    )
}

fn durable_model_policy_failure_root(
    runtime: &AgentRuntimeV2,
    failure: &SourceLiveFailure,
    model_evidence: &SourceModelEvidence,
    binding: &str,
    policy_binding: &str,
    typed_profile: &str,
    status: &str,
) -> ExecutionRoot {
    root(
        "semaprax.evidence-root.v5",
        json!({
            "execution_revision": runtime.revision.digest(),
            "instance_root": runtime.instance.digest(),
            "source_model_binding": binding,
            "source_model_policy": policy_binding,
            "typed_effect_profile": typed_profile,
            "model_evidence": model_evidence.digest(),
            "model_dispatches": failure.model_dispatches,
            "effect_dispatches": failure.effect_dispatches,
            "selected": failure.selected.map(|status| status.as_str()),
            "status": status,
        }),
    )
}

/// Evidence from the invocation that performed or replayed the durable run.
pub struct AgentRuntimeV2DurableEvidence {
    run: DurableTypedRun,
    evidence: ExecutionRoot,
    revision: ExecutionRoot,
    migration_handoff: Option<String>,
    migrated_checkpoint: Option<String>,
}
impl AgentRuntimeV2DurableEvidence {
    pub(super) fn from_migration(
        run: DurableTypedRun,
        evidence: ExecutionRoot,
        revision: ExecutionRoot,
        handoff: String,
        checkpoint: String,
    ) -> Self {
        Self {
            run,
            evidence,
            revision,
            migration_handoff: Some(handoff),
            migrated_checkpoint: Some(checkpoint),
        }
    }
    /// Snapshot from the caller-owned store, including any durable migration handoff.
    pub fn checkpoint(&self) -> &str {
        self.migrated_checkpoint
            .as_deref()
            .unwrap_or_else(|| self.run.checkpoint())
    }
    pub fn migration_handoff_digest(&self) -> Option<&str> {
        self.migration_handoff.as_deref()
    }
    pub fn run(&self) -> &DurableTypedRun {
        &self.run
    }
    pub fn evidence_root(&self) -> &ExecutionRoot {
        &self.evidence
    }
    pub fn execution_revision(&self) -> &ExecutionRoot {
        &self.revision
    }
}
