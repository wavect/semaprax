//! Authenticated retained prefix data only; no owner or target authority.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::{
    checked_cumulative_owned_effect_request_v8, OwnedEffectSettlementInputsV8,
};
impl InventoryV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn check_continued_effect_request(
        &self,
        inputs: &OwnedEffectSettlementInputsV8<'_>,
        request: &str,
        operation: &str,
    ) -> Result<(), SourceJournalError> {
        let context = match self.context {
            ContextV8::Checked(context) => context,
            #[cfg(test)]
            ContextV8::Synthetic(_) => return Err(SourceJournalError::Binding),
        };
        let proof = self
            .accounting
            .as_ref()
            .ok_or(SourceJournalError::Binding)?;
        if !proof.matches(context, self.document.len(), self.entries.len(), &self.mac)
            || inputs.turn == 0
        {
            return Err(SourceJournalError::Binding);
        }
        let prefix = inventory::cumulative::checked_prefix(
            context.fold(),
            &self.entries,
            inputs,
            proof.previous(),
        )?;
        let checked = checked_cumulative_owned_effect_request_v8(inputs, &prefix)?;
        if checked.request_digest() != request || checked.operation().operation_id() != operation {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_intent_facts(
        &self,
    ) -> Result<(u64, u32, u32, &EntryV8), SourceJournalError> {
        if !self.context.fold().cumulative_initialization {
            return Err(SourceJournalError::Binding);
        }
        let (r, s, turn, attempt, row) = self.effect_intent_reduce_facts()?;
        if turn == 0 || attempt != 0 {
            return Err(SourceJournalError::Binding);
        }
        self.continued_model_accounting()?;
        Ok((r, s, turn, row))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_intent_prefix(
        &self,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let (_, _, turn, previous) = self.continued_effect_facts()?;
        if !matches!(
            previous,
            EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed { .. })
        ) || !matches!(selected,EntryV8::Ordinary(SourceJournalEntry::EffectIntent{turn:actual,attempt:0,..}) if *actual==turn)
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}
impl<'a> InventoryV8<'a> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_fixed_continued_intent(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        _journal: &SourceOwnedWaitJournalV8,
        permit: &super::super::live_upstream::FixedOwnedContinuedIntentAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::ContinuedIntent(permit), None)
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::ContinuedIntent(permit))
        }
    }
}
impl PendingV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_fixed_continued_intent_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &super::super::live_upstream::FixedOwnedContinuedIntentAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
}
