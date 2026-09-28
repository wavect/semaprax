//! SAME token renews only through actual continued Ready/Consumed ACKs.
//! Funding is proved before mutation; R/S and all accounting remain unchanged.
use super::*;
pub(in crate::live_invocation::source_journal::owned_wait_v8::append) enum RenewalPhaseV8 {
    Ready,
    Consumed,
    Intent,
}
fn phase_matches(phase: &OwnedReduceHoldPhaseV8) -> bool {
    match phase {OwnedReduceHoldPhaseV8::TurnAuthorize{selected,..}=>matches!(selected,EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedAuthorizationStaged{..})),OwnedReduceHoldPhaseV8::TurnEffect{phase:RenewalPhaseV8::Ready,selected,..}=>matches!(selected,EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedAuthorizationReady{..})),OwnedReduceHoldPhaseV8::TurnEffect{phase:RenewalPhaseV8::Consumed,selected,..}=>matches!(selected,EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed{..})),_=>false}
}
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedEffectSuccessorV8;
impl ProspectiveOwnedReduceHoldV8<'_> {
    fn validate_effect_inventory(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::super::candidate::InventoryV8<'_>,
        sequence: usize,
        bytes: usize,
    ) -> Result<(), SourceJournalError> {
        if !std::ptr::eq(self.journal, journal)
            || journal.poisoned.get()
            || !inventory.belongs_to_context(&journal.context)
        {
            return Err(SourceJournalError::Binding);
        }
        let (reserved, stages, turn, selected) = inventory.continued_effect_facts()?;
        let fuel = self.checked_funding(reserved, stages)?;
        let registry = journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        if !phase_matches(&record.phase) {
            return Err(SourceJournalError::Binding);
        }
        let (actual, r, s) = match &record.phase {
            OwnedReduceHoldPhaseV8::TurnEffect {
                phase: _,
                selected,
                reserved,
                stages,
            }
            | OwnedReduceHoldPhaseV8::TurnAuthorize {
                selected,
                reserved,
                stages,
            } => (selected, *reserved, *stages),
            _ => return Err(SourceJournalError::Binding),
        };
        if actual != selected
            || r != reserved
            || s != stages
            || record.identity != self.identity
            || record.fuel != fuel
            || record.turn != turn
            || record.sequence != sequence
            || record.bytes != bytes
            || inventory.sequence() != sequence
            || inventory.acknowledged_bytes() != bytes
            || record.authentication != inventory.authentication_tail()
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_effect_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::super::candidate::InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.validate_effect_inventory(
                journal,
                inventory,
                inventory.sequence(),
                inventory.acknowledged_bytes(),
            )?;
            inventory.validate_continued_effect_prefix(selected)
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_effect_ack(
        &self,
        witness: &VerifiedOwnedContinuedEffectSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            if !std::ptr::eq(self.journal, session.journal)
                || self.journal.poisoned.get()
                || !self.journal.append_active.get()
            {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_against_acknowledged_session(session)?;
            let (reserved, stages, turn, selected) = session.inventory.continued_effect_facts()?;
            let fuel = self.checked_funding(reserved, stages)?;
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            if !phase_matches(&record.phase) {
                return Err(SourceJournalError::Binding);
            }
            let (previous, r, s) = match &record.phase {
                OwnedReduceHoldPhaseV8::TurnEffect {
                    phase: _,
                    selected,
                    reserved,
                    stages,
                }
                | OwnedReduceHoldPhaseV8::TurnAuthorize {
                    selected,
                    reserved,
                    stages,
                } => (selected, *reserved, *stages),
                _ => return Err(SourceJournalError::Binding),
            };
            if record.identity != self.identity || record.fuel != fuel || record.turn != turn {
                return Err(SourceJournalError::Binding);
            }

            if !matches!((previous,selected),(EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedAuthorizationStaged{..}),EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedAuthorizationReady{..}))|(EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedAuthorizationReady{..}),EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed{..})))||r!=reserved||s!=stages{return Err(SourceJournalError::Binding);}
            witness.validate_previous_registry(
                self.journal,
                record.sequence,
                record.bytes,
                &record.authentication,
            )?;
            record.sequence = session.sequence();
            record.bytes = session.acknowledged_bytes();
            record.authentication = session.inventory.authentication_tail().into();
            record.phase = if matches!(
                selected,
                EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed { .. })
            ) {
                OwnedReduceHoldPhaseV8::TurnEffect {
                    phase: RenewalPhaseV8::Consumed,
                    selected: selected.clone(),
                    reserved,
                    stages,
                }
            } else {
                OwnedReduceHoldPhaseV8::TurnEffect {
                    phase: RenewalPhaseV8::Ready,
                    selected: selected.clone(),
                    reserved,
                    stages,
                }
            };
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_effect_guard(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        bytes: usize,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            if !std::ptr::eq(self.journal, journal) {
                return Err(SourceJournalError::Binding);
            }
            journal.validate_guard()?;
            let current = journal.begin_session()?;
            self.validate_effect_inventory(journal, &current.inventory, sequence, bytes)?;
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
}

mod intent;
