//! Public owned-Agent entry. Only the checked terminal projection and
//! opaque runtime status leave this module; the journal and physical owners do not.
use super::super::wait::{
    FirstTurnPreparedContinuationHostGrantV8, FirstTurnPreparedRecoveryHostGrantV8,
};
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::TargetHostHandler;
use crate::agent_lifecycle::iterative::source_live::SourceLivePolicy;
use crate::agent_lifecycle::iterative::source_live::SourceProposalPolicy;
use crate::diagnostic::Diagnostic;
use crate::execution_revision::typed::AgentRuntimeV2;
use crate::interpreter::resumable::owned_frame::{
    OwnedFrameInput, OwnedFrameInputField, OwnedFrameInputValue,
};
use crate::interpreter::ArgumentValue;
use crate::resumable_effects::owned_frame::v2::compile_owned_agent_wait_v8;
use crate::resumable_effects::owned_frame::{
    fresh_source_owned_wait_v8, prepare_fresh_source_owned_wait_v8, recover_source_owned_wait_v8,
    ExplicitStoreRegistrationGrant, FreshSourceOwnedWaitFactsV8, OwnedFrameError,
    SourceOwnedWaitLimitsV8, SourceOwnedWaitStoreRegistrationV8,
};
use crate::resumable_effects::source_checkpoint::{SourceCheckpointKey, SourceCheckpointScope};
use std::fs::File;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;

/// A failed compiler or physical admission never reaches a model or target.
#[derive(Debug)]
pub enum SourceOwnedAgentOpenErrorV1 {
    Compiler(Vec<Diagnostic>),
    Journal(SourceJournalError),
    Store(OwnedFrameError),
}

/// Holds the one fresh, locked v8 journal. This value exposes neither its
/// lease nor its registration, and dropping it cannot restore an owner.
pub struct SourceOwnedAgentJournalV1 {
    journal: SourceOwnedWaitJournalV8,
}

/// One execution's physical owner custody. A failed phase is retained here;
/// dropping this value retires journal authority before the owners are freed.
pub struct SourceOwnedAgentRunV1<'j> {
    runtime: OwnedLifecycleRuntimeV8<'j>,
}

#[cfg(unix)]
fn checked_execution_and_facts(
    runtime: &Arc<AgentRuntimeV2>,
    source_path: &str,
    agent_id: &str,
    step_id: &str,
    adapter: &StreamingSourceProposalAdapter<'_>,
    policy: &SourceLivePolicy,
    cancellation: &AgentCancellation,
    clock: &dyn SourceInvocationClock,
    evaluation_fuel: usize,
    directory: &File,
    policy_epoch: u64,
) -> Result<
    (
        Arc<crate::execution_revision::typed::CheckedTypedOwnedWaitExecutionV8>,
        FreshSourceOwnedWaitFactsV8,
    ),
    SourceOwnedAgentOpenErrorV1,
> {
    if cancellation.is_cancelled()
        || clock.clock_domain() != policy.clock_domain
        || clock.now_millis() < policy.initial_millis
        || clock.now_millis() >= policy.deadline_millis
    {
        return Err(SourceOwnedAgentOpenErrorV1::Journal(
            SourceJournalError::Time,
        ));
    }
    let source = runtime
        .project_revision()
        .sources()
        .iter()
        .find(|source| source.path() == source_path)
        .ok_or_else(|| {
            SourceOwnedAgentOpenErrorV1::Compiler(vec![Diagnostic::io(
                "SPX-G583",
                "source owned Agent is not retained by runtime Project",
            )])
        })?;
    let wait =
        compile_owned_agent_wait_v8(source.source(), Path::new(source_path), agent_id, step_id)
            .map(Arc::new)
            .map_err(SourceOwnedAgentOpenErrorV1::Compiler)?;
    let execution = Arc::new(
        runtime
            .checked_owned_wait_execution_v8(wait, adapter, policy, evaluation_fuel)
            .map_err(SourceOwnedAgentOpenErrorV1::Journal)?,
    );
    let ordinary = execution.ordinary();
    if ordinary.max_iterations() != 2 {
        return Err(SourceOwnedAgentOpenErrorV1::Journal(
            SourceJournalError::Binding,
        ));
    }
    let limits = SourceOwnedWaitLimitsV8 {
        max_steps_per_stage: ordinary.max_steps_per_stage().ok_or(
            SourceOwnedAgentOpenErrorV1::Journal(SourceJournalError::Binding),
        )?,
        max_total_steps: u64::try_from(ordinary.max_total_steps().ok_or(
            SourceOwnedAgentOpenErrorV1::Journal(SourceJournalError::Binding),
        )?)
        .map_err(|_| SourceOwnedAgentOpenErrorV1::Journal(SourceJournalError::Capacity))?,
        max_stages: ordinary.max_stages() as usize,
        max_attempts: ordinary.max_attempts() as usize,
        response_limit: ordinary.response_limit(),
    };
    let invocation = wire::recipe_digest(wire::RecipeV8::Invocation,
        &serde_json::json!({"execution":ordinary.invocation(),"owned_wait_binding":execution.wait().binding()}))
        .map_err(SourceOwnedAgentOpenErrorV1::Journal)?;
    let scope = SourceCheckpointScope::new(
        execution.wait().lifecycle().source_revision(),
        invocation,
        policy_epoch,
    )
    .map_err(|_| SourceOwnedAgentOpenErrorV1::Journal(SourceJournalError::Binding))?;
    let metadata = directory
        .metadata()
        .map_err(|_| SourceOwnedAgentOpenErrorV1::Journal(SourceJournalError::Binding))?;
    let expected = FreshSourceOwnedWaitFactsV8 {
        scope,
        execution: ordinary.invocation().to_owned(),
        binding: execution.wait().binding().to_owned(),
        limits,
        directory_identity: (metadata.dev(), metadata.ino()),
    };
    Ok((execution, expected))
}

impl SourceOwnedAgentJournalV1 {
    #[cfg(test)]
    pub(crate) fn test_journal(&self) -> &SourceOwnedWaitJournalV8 {
        &self.journal
    }

    /// Compile the Agent from the runtime's authenticated Project revision,
    /// check its exact typed model binding, then create and exclusively lock a
    /// fresh v8 journal. The host must durably retain the complete inert
    /// registration facts before returning `true` from `retain_registration`.
    /// A refusal leaves an empty locked file without any executable owner.
    #[cfg(unix)]
    pub fn create_fresh(
        runtime: Arc<AgentRuntimeV2>,
        source_path: &str,
        agent_id: &str,
        step_id: &str,
        adapter: &StreamingSourceProposalAdapter<'_>,
        policy: &SourceLivePolicy,
        cancellation: &AgentCancellation,
        clock: &dyn SourceInvocationClock,
        evaluation_fuel: usize,
        directory: File,
        policy_epoch: u64,
        key: SourceCheckpointKey,
        protected_history_available: bool,
        retain_registration: impl FnOnce(&serde_json::Value) -> bool,
    ) -> Result<Self, SourceOwnedAgentOpenErrorV1> {
        let (execution, expected) = checked_execution_and_facts(
            &runtime,
            source_path,
            agent_id,
            step_id,
            adapter,
            policy,
            cancellation,
            clock,
            evaluation_fuel,
            &directory,
            policy_epoch,
        )?;
        let grant = ExplicitStoreRegistrationGrant::for_trusted_host(protected_history_available)
            .map_err(SourceOwnedAgentOpenErrorV1::Store)?;
        let prepared = prepare_fresh_source_owned_wait_v8(directory, expected, grant)
            .map_err(SourceOwnedAgentOpenErrorV1::Store)?;
        let (registration, mut lease) =
            fresh_source_owned_wait_v8(prepared).map_err(SourceOwnedAgentOpenErrorV1::Store)?;
        let retained = retain_registration(&registration.retained_facts());
        let retained = registration
            .acknowledge_retained_by_trusted_host(retained)
            .map_err(SourceOwnedAgentOpenErrorV1::Store)?;
        lease
            .authorize_fresh_start(retained)
            .map_err(SourceOwnedAgentOpenErrorV1::Store)?;
        let context = checked_owned_wait_journal_context_v8(execution, &lease, &registration)
            .map_err(SourceOwnedAgentOpenErrorV1::Journal)?
            .with_runtime(runtime, &lease)
            .map_err(SourceOwnedAgentOpenErrorV1::Journal)?
            .with_cumulative_initialization(&lease)
            .map_err(SourceOwnedAgentOpenErrorV1::Journal)?;
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease)
            .map_err(SourceOwnedAgentOpenErrorV1::Journal)?;
        Ok(Self { journal })
    }

    /// Reopen only a host-retained, complete registration for this exact
    /// compiled Project, runtime, policy epoch, directory and checkpoint key.
    /// This constructor does not grant fresh execution or append authority;
    /// only `restart_first_prepared` may restore its one admitted owner.
    #[cfg(unix)]
    pub fn recover(
        runtime: Arc<AgentRuntimeV2>,
        source_path: &str,
        agent_id: &str,
        step_id: &str,
        adapter: &StreamingSourceProposalAdapter<'_>,
        policy: &SourceLivePolicy,
        cancellation: &AgentCancellation,
        clock: &dyn SourceInvocationClock,
        evaluation_fuel: usize,
        directory: File,
        policy_epoch: u64,
        key: SourceCheckpointKey,
        protected_history_available: bool,
        retained_registration: &serde_json::Value,
    ) -> Result<Self, SourceOwnedAgentOpenErrorV1> {
        let (execution, expected) = checked_execution_and_facts(
            &runtime,
            source_path,
            agent_id,
            step_id,
            adapter,
            policy,
            cancellation,
            clock,
            evaluation_fuel,
            &directory,
            policy_epoch,
        )?;
        let registration =
            SourceOwnedWaitStoreRegistrationV8::from_retained_facts(retained_registration)
                .map_err(SourceOwnedAgentOpenErrorV1::Store)?;
        let grant = ExplicitStoreRegistrationGrant::for_trusted_host(protected_history_available)
            .map_err(SourceOwnedAgentOpenErrorV1::Store)?;
        let lease = recover_source_owned_wait_v8(directory, &registration, expected, grant)
            .map_err(SourceOwnedAgentOpenErrorV1::Store)?;
        let context = checked_owned_wait_journal_context_v8(execution, &lease, &registration)
            .map_err(SourceOwnedAgentOpenErrorV1::Journal)?
            .with_runtime(runtime, &lease)
            .map_err(SourceOwnedAgentOpenErrorV1::Journal)?
            .with_cumulative_initialization(&lease)
            .map_err(SourceOwnedAgentOpenErrorV1::Journal)?;
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease)
            .map_err(SourceOwnedAgentOpenErrorV1::Journal)?;
        Ok(Self { journal })
    }

    /// Execute the checked two-turn path. Every reached failure remains in the
    /// returned opaque run until its prescribed settlement or host teardown.
    /// This method is one-use because the journal refuses a second fresh start.
    pub fn run<'j>(
        &'j self,
        policy: &'j crate::resumable_effects::CapabilityPolicy,
        cancellation: &'j AgentCancellation,
        clock: &'j dyn SourceInvocationClock,
        adapter: &mut StreamingSourceProposalAdapter<'_>,
        handler: &mut dyn TargetHostHandler,
        mut observe: impl FnMut(&FinalizeAction),
    ) -> Result<SourceOwnedAgentRunV1<'j>, SourceJournalError> {
        self.validate_run_entry(cancellation, clock, adapter)?;
        let mut runtime = OwnedLifecycleRuntimeV8::open(&self.journal, cancellation)?;
        let context = self.journal.context();
        let (bound, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        let task = bound.owned_wait_task_v8(execution)?;
        let metadata = execution.wait().lifecycle().owned_wait_task_v8();
        let input = OwnedFrameInput {
            declaration: metadata.id.clone(),
            fields: metadata
                .fields()
                .map(|(identity, _)| OwnedFrameInputField {
                    identity: identity.clone(),
                    value: if identity == metadata.objective_field {
                        OwnedFrameInputValue::Bytes(task.objective.clone())
                    } else {
                        OwnedFrameInputValue::Scalar(ArgumentValue::Int(task.budget))
                    },
                })
                .collect(),
        };
        if runtime
            .session()
            .run_first_turn_model(input, adapter, clock)
            == OwnedLifecycleStatusV8::ModelCompleted
        {
            runtime
                .session()
                .finish_two_turn_run(policy, adapter, handler, &mut observe);
        }
        settle_known_failures(&mut runtime, &mut observe);
        Ok(SourceOwnedAgentRunV1 { runtime })
    }

    /// Continue the sole authenticated first-turn Prepared owner after a real
    /// process restart. These are two independent trusted-host assertions;
    /// retained rows, registration data and checkpoint bytes confer neither.
    /// Fresh, answered, terminal and hostile histories refuse before Model.
    pub fn restart_first_prepared<'j>(
        &'j self,
        policy: &'j crate::resumable_effects::CapabilityPolicy,
        cancellation: &'j AgentCancellation,
        clock: &'j dyn SourceInvocationClock,
        adapter: &mut StreamingSourceProposalAdapter<'_>,
        handler: &mut dyn TargetHostHandler,
        mut observe: impl FnMut(&FinalizeAction),
        protected_recovery_key_available: bool,
        protected_continuation_key_available: bool,
    ) -> Result<SourceOwnedAgentRunV1<'j>, SourceJournalError> {
        self.validate_run_entry(cancellation, clock, adapter)?;
        let recovery = FirstTurnPreparedRecoveryHostGrantV8::for_trusted_host(
            protected_recovery_key_available,
        )?;
        let continuation = FirstTurnPreparedContinuationHostGrantV8::for_trusted_host(
            protected_continuation_key_available,
        )?;
        let mut runtime = OwnedLifecycleRuntimeV8::restart_first_prepared(
            &self.journal,
            recovery,
            continuation,
            adapter,
            clock,
            cancellation,
        )?;
        if runtime.status() == OwnedLifecycleStatusV8::ModelCompleted {
            runtime
                .session()
                .finish_two_turn_run(policy, adapter, handler, &mut observe);
        }
        settle_known_failures(&mut runtime, &mut observe);
        Ok(SourceOwnedAgentRunV1 { runtime })
    }

    fn validate_run_entry(
        &self,
        cancellation: &AgentCancellation,
        clock: &dyn SourceInvocationClock,
        adapter: &StreamingSourceProposalAdapter<'_>,
    ) -> Result<(), SourceJournalError> {
        if cancellation.is_cancelled() {
            return Err(SourceJournalError::Binding);
        }
        let (_, execution) = self
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let ordinary = execution.ordinary();
        if adapter.model_binding().map(|binding| binding.digest())
            != Some(execution.model().digest())
            || !adapter.model_evidence().attempts().is_empty()
            || !adapter.ordinary_checkpoint_matches(&SourceProposalPolicy {
                deployment_binding: execution.model().digest(),
                response_limit: ordinary.response_limit(),
                reservation_units: ordinary.reservation_units(),
            })
            || clock.clock_domain() != ordinary.clock_domain()
            || clock.now_millis() < ordinary.initial_millis()
            || clock.now_millis() >= ordinary.deadline_millis()
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}

fn settle_known_failures(
    runtime: &mut OwnedLifecycleRuntimeV8<'_>,
    mut observe: &mut impl FnMut(&FinalizeAction),
) {
    match runtime.status() {
        OwnedLifecycleStatusV8::ObserveCleanupPending => {
            runtime.settle_failed_observe(&mut observe);
        }
        OwnedLifecycleStatusV8::FailedEffectCleanupPending => {
            runtime.settle_failed_effect(&mut observe);
        }
        OwnedLifecycleStatusV8::ObserverFailureCleanupPending => {
            runtime.settle_failed_observer(&mut observe);
        }
        _ => {}
    }
}

impl SourceOwnedAgentRunV1<'_> {
    pub fn status(&self) -> OwnedLifecycleStatusV8 {
        self.runtime.status()
    }

    /// Only the terminal ACK exposes the checked canonical Report projection.
    pub fn delivery_projection(&self) -> Option<&serde_json::Value> {
        self.runtime.delivery_projection()
    }

    /// Close a completed or acknowledged Stop. An incomplete phase returns
    /// the same custody value to its caller and can never be restarted.
    pub fn try_close(self) -> Result<Option<serde_json::Value>, Self> {
        let projection = self.runtime.delivery_projection().cloned();
        self.runtime
            .try_close()
            .map(|()| projection)
            .map_err(|runtime| Self { runtime })
    }
}
