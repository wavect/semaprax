//! The same authenticated session supplies checkpoint accounting and the key;
//! the actual parked owner stays borrowed behind its closed source wrapper.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedStartedPhaseV8;
use crate::resumable_effects::owned_frame::v2::OwnedWaitCheckpointExpectationV8;
impl AppendSessionV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_parked_checkpoint(
        &self,
        owner: &LiveContinuedStartedPhaseV8<'_>,
    ) -> Result<(Vec<u8>, String), SourceJournalError> {
        if !self.belongs_to(owner.journal())
            || self.sequence() != owner.sequence()
            || self.acknowledged_bytes() != owner.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        owner.validate_live()?;
        let (reserved_total, consumed, argument_digest) = self
            .inventory
            .continued_start_checkpoint_basis(owner.observation())?;
        let consumed_total = consumed
            .checked_add(owner.consumed().ok_or(SourceJournalError::Binding)?)
            .ok_or(SourceJournalError::Capacity)?;
        let expected = OwnedWaitCheckpointExpectationV8 {
            scope: &self.journal.context().registration().expected_facts().scope,
            argument_digest: &argument_digest,
            observation: owner.observation(),
            sequence: u64::try_from(self.sequence()).map_err(|_| SourceJournalError::Capacity)?,
            reserved_total,
            consumed_total,
        };
        let result = owner.encode_current_checkpoint(self, &self.journal.key, &expected)?;
        owner.validate_live()?;
        Ok(result)
    }
}
