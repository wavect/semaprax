//! Continued Reduce candidate facts and fixed prefix checks.
use super::super::live_upstream::FixedOwnedContinuedReduceReservationAppendPermitV8;
use super::*;

impl InventoryV8<'_> {
    /// Authenticated continued Reduce reservation. This is separate from the
    /// turn-zero proof and accepts only a positive turn after cleanup settlement.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_reduce_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32, &EntryV8), SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        if folded.tail != fold::TailV8::Reduce
            || !folded
                .reduce_fold()
                .is_some_and(|r| r.tail() == reduce_fold::ReduceTailV8::Charged)
        {
            return Err(SourceJournalError::Order);
        }
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let EntryV8::Ordinary(SourceJournalEntry::StageReservation {
            turn,
            attempt: Some(attempt),
            role: crate::live_invocation::source_journal::SourceStageRole::Reduce,
            fuel,
        }) = selected
        else {
            return Err(SourceJournalError::Order);
        };
        if *turn == 0 || Some(*fuel) != context.ordinary.max_steps_per_stage() {
            return Err(SourceJournalError::Binding);
        }
        capacity::outstanding(context, &folded)?.check(self.document.len(), self.entries.len())?;
        Ok((
            folded.reserved_total,
            folded.stages,
            *turn,
            *attempt,
            selected,
        ))
    }
}
impl<'a> InventoryV8<'a> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_fixed_continued_reduce(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedContinuedReduceReservationAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::ContinuedReduce(journal, permit),
                None,
            )
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::ContinuedReduce(journal, permit),
            )
        }
    }
}
impl PendingV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_fixed_continued_reduce_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedContinuedReduceReservationAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
}
