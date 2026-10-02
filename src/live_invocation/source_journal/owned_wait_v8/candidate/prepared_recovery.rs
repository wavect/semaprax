//! The one reviewed restart classification: an exact first-turn Prepared tail.
//! It returns checked checkpoint facts only; its caller still needs a sealed
//! host grant to materialize fresh process-local backing.
use super::*;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FirstTurnPreparedRecoveryFactsV8
{
    pub(in crate::live_invocation::source_journal::owned_wait_v8) checkpoint:
        crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitCheckpointV8,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) observation:
        crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) wait: String,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) reservation: u32,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) prepared: u32,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) consumed: u64,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) sequence: usize,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) bytes: usize,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) authentication: String,
}

impl InventoryV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn first_turn_prepared_recovery(
        &self,
    ) -> Result<FirstTurnPreparedRecoveryFactsV8, SourceJournalError> {
        let prepared = self.entries.last().ok_or(SourceJournalError::Order)?;
        let EntryV8::Owned(model::OwnedBodyV8::OwnedWaitPrepared {
            turn,
            attempt,
            wait,
            reservation,
            observation_digest,
            checkpoint_digest,
            checkpoint,
            consumed,
        }) = &prepared.entry
        else {
            return Err(SourceJournalError::Order);
        };
        if *turn != 0 || *attempt != 0 || self.entries.len() < 2 {
            return Err(SourceJournalError::Order);
        }
        let previous = &self.entries[self.entries.len() - 2].entry;
        if !matches!(previous,
            EntryV8::Owned(model::OwnedBodyV8::OwnedWaitReserved {
                turn: 0, attempt: 0, wait: prior, phase: model::PhaseV8::Start,
                replay_of: None, ..
            }) if prior == wait
        ) {
            return Err(SourceJournalError::Order);
        }
        let Some(observation) = prepared.observation.clone() else {
            return Err(SourceJournalError::Binding);
        };
        if observation.ordinary_digest() != observation_digest {
            return Err(SourceJournalError::Binding);
        }
        let created = self
            .entries
            .iter()
            .find_map(|entry| match &entry.entry {
                EntryV8::Owned(model::OwnedBodyV8::OwnedWaitCreated {
                    turn: 0,
                    attempt: 0,
                    wait: created_wait,
                    argument_digest,
                    ..
                }) if created_wait == wait => Some(argument_digest),
                _ => None,
            })
            .ok_or(SourceJournalError::Binding)?;
        let prior = fold::fold(self.context.fold(), &self.entries[..self.entries.len() - 1])?;
        let context = match self.context {
            ContextV8::Checked(context) => context,
            #[cfg(test)]
            ContextV8::Synthetic(_) => return Err(SourceJournalError::Binding),
        };
        let scope = &context.registration().expected_facts().scope;
        let bytes = crate::live_invocation::identity::unhex(checkpoint)
            .ok_or(SourceJournalError::Binding)?;
        let expected =
            crate::resumable_effects::owned_frame::v2::OwnedWaitCheckpointExpectationV8 {
                scope,
                argument_digest: created,
                observation: &observation,
                sequence: u64::try_from(self.entries.len() - 1)
                    .map_err(|_| SourceJournalError::Capacity)?,
                reserved_total: prior.reserved_total,
                consumed_total: prior
                    .consumed_recorded
                    .checked_add(*consumed)
                    .ok_or(SourceJournalError::Capacity)?,
            };
        let binding = context
            .ready_runtime()
            .map(|(_, execution)| execution.wait())
            .ok_or(SourceJournalError::Binding)?;
        let checked = crate::resumable_effects::owned_frame::v2::validate_owned_wait_checkpoint_v8(
            binding, self.key, &expected, &bytes,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if checked.outer_digest() != checkpoint_digest
            || usize::try_from(*reservation).ok() != Some(self.entries.len() - 2)
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(FirstTurnPreparedRecoveryFactsV8 {
            checkpoint: checked,
            observation,
            wait: wait.clone(),
            reservation: *reservation,
            prepared: u32::try_from(self.entries.len() - 1)
                .map_err(|_| SourceJournalError::Capacity)?,
            consumed: *consumed,
            sequence: self.sequence(),
            bytes: self.acknowledged_bytes(),
            authentication: self.authentication_tail().to_owned(),
        })
    }
}
