//! A public custody boundary after the first Model permits explicit shutdown.
use super::*;
impl SourceOwnedAgentJournalV1 {
    /// Execute Initialize/Observe/Model and retain the actual completed State.
    /// No Authorize or target dispatch runs until `finish` is explicitly called.
    pub fn prepare_first_model<'j>(
        &'j self,
        cancellation: &'j AgentCancellation,
        clock: &'j dyn SourceInvocationClock,
        adapter: &mut StreamingSourceProposalAdapter<'_>,
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
        runtime
            .session()
            .run_first_turn_model(input, adapter, clock);
        settle_known_failures(&mut runtime, &mut observe);
        Ok(SourceOwnedAgentRunV1 { runtime })
    }
}
impl<'j> SourceOwnedAgentRunV1<'j> {
    /// Continue the same retained ModelCompleted owner at most once. Reopening
    /// a completed or quarantined custody value cannot dispatch or retry it.
    pub fn finish(
        &mut self,
        policy: &'j crate::resumable_effects::CapabilityPolicy,
        adapter: &mut StreamingSourceProposalAdapter<'_>,
        handler: &mut dyn TargetHostHandler,
        mut observe: impl FnMut(&FinalizeAction),
    ) -> OwnedLifecycleStatusV8 {
        self.runtime
            .session()
            .finish_two_turn_run(policy, adapter, handler, &mut observe);
        settle_known_failures(&mut self.runtime, &mut observe);
        self.runtime.status()
    }
    /// Abandon an eligible completed first-model State through acknowledged
    /// cleanup and Cancelled Stop. Other reached statuses are retained intact;
    /// an uncertain phase cannot be repaired or retried through this method.
    pub fn shutdown(&mut self, observe: impl FnMut(&FinalizeAction)) -> OwnedLifecycleStatusV8 {
        self.runtime.shutdown_completed(observe)
    }
}
