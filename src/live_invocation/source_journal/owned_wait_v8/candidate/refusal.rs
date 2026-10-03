//! Exact descriptive first-turn staged Refused coordinates; no physical owner.
use super::*;
impl InventoryV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn refused_wait(
        &self,
        transfer: u32,
        staged: u32,
    ) -> Result<String, SourceJournalError> {
        let folded = fold::fold(self.context.fold(), &self.entries)?;
        if folded.tail != fold::TailV8::PendingRefusal || staged as usize + 1 != self.sequence() {
            return Err(SourceJournalError::Order);
        }
        let Some(ValidatedEntryV8 {
            entry:
                EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationStaged {
                    turn: 0,
                    attempt: 0,
                    transfer: actual,
                    ..
                }),
            ..
        }) = self.entries.get(staged as usize)
        else {
            return Err(SourceJournalError::Binding);
        };
        if *actual != transfer {
            return Err(SourceJournalError::Binding);
        }
        let Some(ValidatedEntryV8 {
            entry:
                EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferCompleted {
                    turn: 0,
                    attempt: 0,
                    wait,
                    ..
                }),
            ..
        }) = self.entries.get(transfer as usize)
        else {
            return Err(SourceJournalError::Binding);
        };
        Ok(wait.clone())
    }
}
