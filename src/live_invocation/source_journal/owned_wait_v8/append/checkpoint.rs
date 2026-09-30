//! Same-session sealed parked checkpoint producer, no key or lease extraction.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::LiveParkedStateV8;
use crate::resumable_effects::owned_frame::v2::{
    encode_live_owned_wait_checkpoint_v8, CheckedOwnedWaitObservationV8,
    OwnedWaitCheckpointExpectationV8,
};
impl AppendSessionV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn live_parked_checkpoint(
        &self,
        parked: &LiveParkedStateV8,
        observation: &CheckedOwnedWaitObservationV8,
    ) -> Result<(Vec<u8>, String), SourceJournalError> {
        self.journal.validate_guard()?;
        let (reserved_total, consumed, argument_digest) =
            self.inventory.live_start_checkpoint_basis(observation)?;
        let consumed_total = consumed
            .checked_add(parked.consumed())
            .ok_or(SourceJournalError::Capacity)?;
        let context = self.journal.context();
        let (_, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        let scope = &context.registration().expected_facts().scope;
        let expected = OwnedWaitCheckpointExpectationV8 {
            scope,
            argument_digest: &argument_digest,
            observation,
            sequence: u64::try_from(self.sequence()).map_err(|_| SourceJournalError::Capacity)?,
            reserved_total,
            consumed_total,
        };
        let (bytes, checked) = encode_live_owned_wait_checkpoint_v8(
            execution.wait(),
            &self.journal.key,
            &expected,
            parked,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        self.journal.validate_guard()?;
        Ok((bytes, checked.outer_digest().into()))
    }
}
