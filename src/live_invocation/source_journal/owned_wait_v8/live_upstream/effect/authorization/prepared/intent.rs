//! Consuming actual Prepared lineage. Inert Intent history is not authority.
use super::*;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedEffectIntentAppendV8<
    'j,
> {
    owner: LivePreparedOwnedEffectV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveEffectIntentPreparationFailureV8<
    'j,
> {
    _owner: LivePreparedOwnedEffectV8<'j>,
    error: SourceJournalError,
}
/// A fixed adapter borrows only this actual owner-containing obligation.
/// Matching inert history cannot mint it; no token or owner is extracted.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedEffectIntentAppendPermitV8<
    'p,
    'j,
> {
    obligation: &'p LiveOwnedEffectIntentAppendV8<'j>,
}
impl FixedOwnedEffectIntentAppendPermitV8<'_, '_> {
    /// All clock/current-policy work runs before the fixed append marker/borrows.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_preflight(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> Result<(), SourceJournalError> {
        if !self.obligation.belongs_to(journal) {
            self.obligation
                .owner
                .prepared
                .quarantine_live_authorization();
            return Err(SourceJournalError::Binding);
        }
        self.obligation.validate_live()
    }
    /// Fixed append checks already-authenticated proof data while its marker and
    /// lease borrow are held. This path performs no callback or physical read.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        self.obligation.owner.hold.validate_intent_append_prefix(
            journal,
            inventory,
            &self.obligation.selected,
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        self.obligation.selected_row()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.obligation.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.obligation.acknowledged_bytes()
    }
}
impl<'j> LiveOwnedEffectIntentAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedEffectIntentAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedEffectIntentAppendPermitV8 { obligation: self })
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.owner.journal, journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.session.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.session.acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = self.owner.validate_live().and_then(|_| {
            if self.selected != EntryV8::Ordinary(self.owner.prepared.live_intent_row()?) {
                return Err(SourceJournalError::Binding);
            }
            self.owner.validate_live()
        });
        if result.is_err() {
            self.owner.prepared.quarantine_live_authorization();
        }
        result
    }
}
impl<'j> LivePreparedOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_intent(
        self,
    ) -> Result<LiveOwnedEffectIntentAppendV8<'j>, LiveEffectIntentPreparationFailureV8<'j>> {
        if let Err(error) = self.validate_live() {
            return Err(LiveEffectIntentPreparationFailureV8 {
                _owner: self,
                error,
            });
        }
        let selected = match self.prepared.live_intent_row() {
            Ok(row) => EntryV8::Ordinary(row),
            Err(error) => {
                self.prepared.quarantine_live_authorization();
                return Err(LiveEffectIntentPreparationFailureV8 {
                    _owner: self,
                    error,
                });
            }
        };
        let obligation = LiveOwnedEffectIntentAppendV8 {
            owner: self,
            selected,
        };
        if let Err(error) = obligation.validate_live() {
            return Err(LiveEffectIntentPreparationFailureV8 {
                _owner: obligation.owner,
                error,
            });
        }
        Ok(obligation)
    }
}

#[cfg(test)]
mod tests;
