//! Current authenticated A-prefix facts are descriptive, never an owner factory.
use super::*;
impl InventoryV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_effect_facts(
        &self,
    ) -> Result<(u64, u32, u32, &EntryV8), SourceJournalError> {
        let folded = fold::fold(self.context.fold(), &self.entries)?;
        let turn = folded
            .continued_effect_turn()
            .ok_or(SourceJournalError::Order)?;
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let actual = match selected {
            EntryV8::Owned(
                model::OwnedBodyV8::OwnedAuthorizationStaged {
                    turn, attempt: 0, ..
                }
                | model::OwnedBodyV8::OwnedAuthorizationReady {
                    turn, attempt: 0, ..
                },
            )
            | EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed {
                turn,
                attempt: 0,
                ..
            }) => *turn,
            _ => return Err(SourceJournalError::Order),
        };
        if actual != turn {
            return Err(SourceJournalError::Binding);
        }
        capacity::outstanding(self.context.fold(), &folded)?
            .check(self.document.len(), self.entries.len())?;
        self.continued_model_accounting()?;
        Ok((folded.reserved_total, folded.stages, turn, selected))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_effect_prefix(
        &self,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let (_, _, turn, previous) = self.continued_effect_facts()?;
        let actual = match selected {
            EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationReady {
                turn,
                attempt: 0,
                ..
            })
            | EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed {
                turn,
                attempt: 0,
                ..
            }) => *turn,
            _ => return Err(SourceJournalError::Binding),
        };
        let allowed = matches!(
            (previous, selected),
            (
                EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationStaged { .. }),
                EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationReady { .. })
            ) | (
                EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationReady { .. }),
                EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed { .. })
            )
        );
        if !allowed || actual != turn {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}
impl<'a> InventoryV8<'a> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_fixed_continued_effect(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &super::super::live_upstream::FixedOwnedContinuedEffectAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::ContinuedEffect(journal, permit),
                None,
            )
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::ContinuedEffect(journal, permit),
            )
        }
    }
}
impl PendingV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_fixed_continued_effect_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &super::super::live_upstream::FixedOwnedContinuedEffectAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
}
