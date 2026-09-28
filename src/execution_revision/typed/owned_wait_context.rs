//! Sealed typed execution facts for the private ordinary owned-wait profile.
//! Construction is read-only: no file, evaluator, adapter factory or authority.
use super::*;
use crate::agent_lifecycle::iterative::source_live::{SourceLivePolicy, SourceProposalPolicy};
use crate::live_invocation::source_journal::{SourceInvocationBinding, SourceJournalError};
use crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8;

pub(crate) struct CheckedTypedOwnedWaitExecutionV8 {
    project: Arc<ProjectRevision>,
    revision: ExecutionRoot,
    model: SourceModelBinding,
    wait: Arc<CheckedOwnedAgentWaitBindingV8>,
    ordinary: SourceInvocationBinding,
    evaluation_fuel: usize,
}
impl CheckedTypedOwnedWaitExecutionV8 {
    pub(crate) fn wait_arc(&self) -> Arc<CheckedOwnedAgentWaitBindingV8> {
        Arc::clone(&self.wait)
    }

    pub(crate) fn ordinary(&self) -> &SourceInvocationBinding {
        &self.ordinary
    }
    pub(crate) fn wait(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        &self.wait
    }
    pub(crate) fn evaluation_fuel(&self) -> usize {
        self.evaluation_fuel
    }
    pub(crate) fn revision(&self) -> &ExecutionRoot {
        &self.revision
    }
    pub(crate) fn model(&self) -> &SourceModelBinding {
        &self.model
    }
    pub(crate) fn project(&self) -> &ProjectRevision {
        &self.project
    }
}
impl AgentRuntimeV2 {
    /// Borrow the actual immutable registry only for this retained typed run.
    /// This check and borrower grant no effect/ACK/owner authority.
    pub(crate) fn owned_wait_effects_v8<'a>(
        &'a self,
        context: &CheckedTypedOwnedWaitExecutionV8,
    ) -> std::result::Result<&'a CompiledTypedEffects, SourceJournalError> {
        let schema = self.lifecycle.proposal_schema();
        let plain: serde_json::Value =
            serde_json::from_str(context.wait.lifecycle().canonical_json())
                .map_err(|_| SourceJournalError::Binding)?;
        let typed: serde_json::Value = serde_json::from_str(self.lifecycle.canonical_json())
            .map_err(|_| SourceJournalError::Binding)?;
        if !Arc::ptr_eq(&self.project, &context.project)
            || self.revision.digest() != context.revision.digest()
            || !context.model.runtime_matches(
                self.deployment.digest(),
                self.instance.digest(),
                schema.source_revision(),
                schema.schema().digest(),
            )
            || context.wait.agent().as_str() != schema.schema().agent_id()
            || context.wait.lifecycle().source_revision() != schema.source_revision()
            || context.wait.lifecycle().proposal_schema().schema().digest()
                != schema.schema().digest()
            || typed.get("lifecycle") != Some(&plain)
            || context.evaluation_fuel == 0
            || context.evaluation_fuel > self.budget.max_steps_per_stage
            || !self.proposals.is_empty()
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(&self.lifecycle)
    }
    pub(crate) fn owned_wait_effect_limits_v8(
        &self,
        context: &CheckedTypedOwnedWaitExecutionV8,
    ) -> std::result::Result<EffectBudget, SourceJournalError> {
        self.owned_wait_effects_v8(context)?;
        Ok(self.effects)
    }

    /// Derive E only from this runtime's retained source, task, registry, effect
    /// ceilings and genuine model-bound ordinary checkpoint adapter.
    pub(crate) fn checked_owned_wait_execution_v8(
        &self,
        wait: Arc<CheckedOwnedAgentWaitBindingV8>,
        source: &StreamingSourceProposalAdapter<'_>,
        policy: &SourceLivePolicy,
        evaluation_fuel: usize,
    ) -> std::result::Result<CheckedTypedOwnedWaitExecutionV8, SourceJournalError> {
        let plain: serde_json::Value = serde_json::from_str(wait.lifecycle().canonical_json())
            .map_err(|_| SourceJournalError::Binding)?;
        let typed: serde_json::Value = serde_json::from_str(self.lifecycle.canonical_json())
            .map_err(|_| SourceJournalError::Binding)?;
        let schema = self.lifecycle.proposal_schema();
        let model = source
            .model_binding()
            .filter(|m| {
                m.runtime_matches(
                    self.deployment.digest(),
                    self.instance.digest(),
                    schema.source_revision(),
                    schema.schema().digest(),
                )
            })
            .ok_or(SourceJournalError::Binding)?;
        if evaluation_fuel == 0
            || evaluation_fuel > self.budget.max_steps_per_stage
            || wait.agent().as_str() != schema.schema().agent_id()
            || wait.lifecycle().source_revision() != schema.source_revision()
            || typed.get("lifecycle") != Some(&plain)
            || wait.lifecycle().proposal_schema().schema().digest() != schema.schema().digest()
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
            return Err(SourceJournalError::Binding);
        }
        // No ProgramRoot augmentation: v8 deliberately uses the checked ordinary
        // execution identity. Model's digest already binds typed deployment and
        // instance roots; those own the registry/effect/task/budget commitments.
        let ordinary =
            policy.binding(self.lifecycle.source_lifecycle(), &self.task, self.budget)?;
        Ok(CheckedTypedOwnedWaitExecutionV8 {
            project: Arc::clone(&self.project),
            revision: self.revision.clone(),
            model: model.clone(),
            wait,
            ordinary,
            evaluation_fuel,
        })
    }
}

#[cfg(test)]
#[path = "owned_wait_context/tests.rs"]
mod tests;
