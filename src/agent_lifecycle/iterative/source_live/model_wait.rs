//! Additive v7 source-session construction and prewrite checkpoint validation.
use super::*;
use crate::agent_lifecycle::iterative::model_wait::{carrier_digest, SourceModelWaitBinding};
use crate::interpreter::resumable::checkpoint;
use crate::live_invocation::source_journal::{
    SourceExecutionEntryV7, SourceModelWaitEntryV7, SourceModelWaitProfileV7,
};
use crate::resumable_effects::source_checkpoint::{
    decode_source_checkpoint_v7, SourceCheckpointKey, SourceCheckpointScope,
    SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V7,
};

impl CompiledIterativeLifecycle {
    pub(crate) fn run_live_durable_with_model_wait(
        &self,
        request: SourceLiveRequest<'_>,
        source: &mut dyn driver::ProposalSource,
        driver: &mut dyn driver::IterativeDriver,
        store: &mut dyn CheckpointStore,
        wait: &SourceModelWaitBinding,
        key: &SourceCheckpointKey,
    ) -> Result<SourceLiveOutcome, SourceLiveFailure> {
        if !wait.matches(self)
            || wait.evaluation_fuel() > request.budget.max_steps_per_stage
            || request.policy.program_root.is_some()
        {
            return Err(SourceLiveFailure::initial(
                SourceJournalError::Binding,
                None,
            ));
        }
        let profile = SourceModelWaitProfileV7::new(
            wait.digest().into(),
            wait.source_revision().into(),
            wait.evaluation_fuel(),
        )
        .map_err(|e| SourceLiveFailure::initial(e, None))?;
        let binding = request
            .policy
            .binding(self, request.task, request.budget)
            .and_then(|b| b.with_model_wait_v7(profile))
            .map_err(|e| SourceLiveFailure::initial(e, None))?;
        self.run_live_durable_bound_inner(
            request,
            source,
            driver,
            store,
            binding,
            Some((wait, key)),
        )
    }
}

pub(super) fn validate_recovered(
    lifecycle: &CompiledIterativeLifecycle,
    wait: &SourceModelWaitBinding,
    key: &SourceCheckpointKey,
    recovered: &RecoveredSourceCheckpoint,
) -> Result<(), SourceJournalError> {
    for (_, entry) in recovered.execution_entries_v7() {
        let SourceExecutionEntryV7::Wait(SourceModelWaitEntryV7::Prepared {
            turn,
            attempt,
            wait: id,
            observation_digest,
            checkpoint: bytes,
            ..
        }) = entry
        else {
            continue;
        };
        if bytes.len() > 32768 {
            return Err(SourceJournalError::Capacity);
        }
        if id != recovered.binding().model_wait_id(turn, attempt)? {
            return Err(SourceJournalError::Binding);
        }
        let envelope: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| SourceJournalError::Malformed)?;
        if envelope["schema"] != SOURCE_RESUMABLE_CHECKPOINT_SCHEMA_V7 {
            return Err(SourceJournalError::Binding);
        }
        let encoded = envelope["arguments"]
            .as_array()
            .filter(|a| a.len() == 1)
            .ok_or(SourceJournalError::Malformed)?;
        let argument = checkpoint::channel_from_json(&encoded[0])
            .map_err(|_| SourceJournalError::Malformed)?;
        if carrier_digest(&id, "observation", &argument).as_deref() != Some(&observation_digest) {
            return Err(SourceJournalError::Binding);
        }
        let scope = SourceCheckpointScope::new(wait.source_revision(), &id, 0)
            .map_err(|_| SourceJournalError::Binding)?;
        decode_source_checkpoint_v7(
            &lifecycle.inner.program,
            key,
            &scope,
            wait.wrapper_id(),
            &[argument],
            &bytes,
        )
        .map_err(|_| SourceJournalError::Binding)?;
    }
    Ok(())
}
