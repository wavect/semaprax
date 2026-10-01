//! Authenticated settlement facts only; fixed owner permit supplies write authority.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::{
    checked_cumulative_owned_effect_settlement_v8, CheckedOwnedEffectSettlementV8,
    OwnedEffectSettlementInputsV8,
};
impl InventoryV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn check_continued_effect_settlement(
        &self,
        inputs: OwnedEffectSettlementInputsV8<'_>,
        ordinary: &SourceJournalEntry,
        evidence: &[u8],
        result: Option<&[u8]>,
    ) -> Result<CheckedOwnedEffectSettlementV8, SourceJournalError> {
        let context = match self.context {
            ContextV8::Checked(context) => context,
            #[cfg(test)]
            ContextV8::Synthetic(_) => return Err(SourceJournalError::Binding),
        };
        let proof = self
            .accounting
            .as_ref()
            .ok_or(SourceJournalError::Binding)?;
        if inputs.turn == 0
            || !proof.matches(context, self.document.len(), self.entries.len(), &self.mac)
        {
            return Err(SourceJournalError::Binding);
        }
        let prefix = inventory::cumulative::checked_prefix(
            context.fold(),
            &self.entries,
            &inputs,
            proof.previous(),
        )?;
        checked_cumulative_owned_effect_settlement_v8(inputs, &prefix, ordinary, evidence, result)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_settlement_facts(
        &self,
    ) -> Result<(u64, u32, u32, &EntryV8), SourceJournalError> {
        if !self.context.fold().cumulative_initialization {
            return Err(SourceJournalError::Binding);
        }
        let (r, s, t, a, row) = self.effect_settlement_reduce_facts()?;
        if t == 0 || a != 0 {
            return Err(SourceJournalError::Binding);
        }
        self.continued_model_accounting()?;
        Ok((r, s, t, row))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_settlement_prefix(
        &self,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        match selected {
            EntryV8::Ordinary(
                SourceJournalEntry::EffectObserved {
                    turn, attempt: 0, ..
                }
                | SourceJournalEntry::EffectFailed {
                    turn, attempt: 0, ..
                },
            ) => {
                let (_, _, t, _) = self.continued_intent_facts()?;
                if *turn != t {
                    return Err(SourceJournalError::Binding);
                }
            }
            EntryV8::Owned(model::OwnedBodyV8::OwnedEffectSettlementRecorded {
                turn,
                attempt: 0,
                settlement,
                ..
            }) => {
                let (_, _, t, previous) = self.continued_settlement_facts()?;
                if *turn != t
                    || !matches!(
                        previous,
                        EntryV8::Ordinary(
                            SourceJournalEntry::EffectObserved { .. }
                                | SourceJournalEntry::EffectFailed { .. }
                        )
                    )
                    || usize::try_from(*settlement)
                        .ok()
                        .and_then(|n| n.checked_add(1))
                        != Some(self.sequence())
                {
                    return Err(SourceJournalError::Binding);
                }
            }
            _ => return Err(SourceJournalError::Binding),
        };
        Ok(())
    }
}
impl<'a> InventoryV8<'a> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_fixed_continued_settlement(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        _journal: &SourceOwnedWaitJournalV8,
        permit: &super::super::live_upstream::FixedOwnedContinuedSettlementAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::ContinuedSettlement(permit),
                None,
            )
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::ContinuedSettlement(permit))
        }
    }
}
impl PendingV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_fixed_continued_settlement_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &super::super::live_upstream::FixedOwnedContinuedSettlementAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
}
