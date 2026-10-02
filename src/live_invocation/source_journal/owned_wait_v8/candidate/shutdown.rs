//! Exact current Completed facts only; this data cannot restore a State owner.
use super::*;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8;
impl InventoryV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn completed_shutdown_basis(
        &self,
        completed: u32,
        wait: &str,
        state: &Value,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Result<(), SourceJournalError> {
        let folded = fold::fold(self.context.fold(), &self.entries)?;
        if folded.tail != fold::TailV8::Completed || completed as usize + 1 != self.sequence() {
            return Err(SourceJournalError::Order);
        }
        let Some(ValidatedEntryV8 {
            entry:
                EntryV8::Owned(model::OwnedBodyV8::OwnedWaitCompleted {
                    turn: 0,
                    attempt: 0,
                    wait: actual_wait,
                    proposal: actual_proposal,
                    proposal_digest,
                    result_digest,
                    ..
                }),
            ..
        }) = self.entries.last()
        else {
            return Err(SourceJournalError::Order);
        };
        let argument = wire::record_argument_digest(state);
        let original = self
            .entries
            .iter()
            .rev()
            .find_map(|row| match &row.entry {
                EntryV8::Owned(model::OwnedBodyV8::OwnedWaitCreated {
                    turn: 0,
                    attempt: 0,
                    wait: w,
                    argument_digest,
                    ..
                }) if w == wait => Some(argument_digest),
                _ => None,
            })
            .ok_or(SourceJournalError::Binding)?;
        if actual_wait != wait
            || original != &argument
            || proposal.value() != actual_proposal
            || proposal.ordinary_digest() != proposal_digest
            || proposal
                .result_digest(&argument)
                .map_err(|_| SourceJournalError::Binding)?
                != *result_digest
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}
