//! SAME renewed token advances only after the actual continued Intent ACK.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::{
    append::VerifiedOwnedContinuedIntentSuccessorV8, candidate::InventoryV8,
};
impl ProspectiveOwnedReduceHoldV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_intent_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        (|| {
            self.validate_effect_inventory(
                journal,
                inventory,
                inventory.sequence(),
                inventory.acknowledged_bytes(),
            )?;
            let registry = journal
                .prospective_reduce
                .try_borrow()
                .map_err(|_| SourceJournalError::Order)?;
            if !matches!(
                registry.as_ref().map(|r| &r.phase),
                Some(OwnedReduceHoldPhaseV8::TurnEffect {
                    phase: RenewalPhaseV8::Consumed,
                    ..
                })
            ) {
                return Err(SourceJournalError::Binding);
            }
            inventory.validate_continued_intent_prefix(selected)
        })()
        .inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_intent_ack(
        &self,
        witness: &VerifiedOwnedContinuedIntentSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        (|| {
            if !std::ptr::eq(self.journal,session.journal) || self.journal.poisoned.get() || !self.journal.append_active.get() {return Err(SourceJournalError::Binding);}
            witness.validate_against_acknowledged_session(session)?;
            let (reserved,stages,turn,selected)=session.inventory.continued_intent_facts()?;
            let fuel=self.checked_funding(reserved,stages)?;
            let mut registry=self.journal.prospective_reduce.try_borrow_mut().map_err(|_|SourceJournalError::Order)?;
            let record=registry.as_mut().ok_or(SourceJournalError::Binding)?;
            let OwnedReduceHoldPhaseV8::TurnEffect{phase:RenewalPhaseV8::Consumed,selected:previous,reserved:r,stages:s}=&record.phase else {return Err(SourceJournalError::Binding);};
            if !matches!(previous,EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed{turn:t,attempt:0,..}) if *t==turn)
                || *r!=reserved || *s!=stages || record.turn!=turn || record.fuel!=fuel || record.identity!=self.identity {return Err(SourceJournalError::Binding);}
            witness.validate_previous_registry(self.journal,record.sequence,record.bytes,&record.authentication)?;
            record.sequence=session.sequence();record.bytes=session.acknowledged_bytes();record.authentication=session.inventory.authentication_tail().into();
            record.phase=OwnedReduceHoldPhaseV8::TurnEffect{phase:RenewalPhaseV8::Intent,selected:selected.clone(),reserved,stages};
            Ok(())
        })().inspect_err(|_|self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_intent_guard(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        bytes: usize,
    ) -> Result<(), SourceJournalError> {
        (|| {
            if !std::ptr::eq(self.journal, journal) {
                return Err(SourceJournalError::Binding);
            }
            journal.validate_guard()?;
            let current = journal.begin_session()?;
            if !current.inventory.belongs_to_context(&journal.context) {
                return Err(SourceJournalError::Binding);
            }
            let (reserved, stages, turn, selected) = current.inventory.continued_intent_facts()?;
            let fuel = self.checked_funding(reserved, stages)?;
            let registry = journal
                .prospective_reduce
                .try_borrow()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
            let OwnedReduceHoldPhaseV8::TurnEffect {
                phase: RenewalPhaseV8::Intent,
                selected: actual,
                reserved: r,
                stages: s,
            } = &record.phase
            else {
                return Err(SourceJournalError::Binding);
            };
            if actual != selected
                || *r != reserved
                || *s != stages
                || record.identity != self.identity
                || record.fuel != fuel
                || record.turn != turn
                || record.sequence != sequence
                || record.bytes != bytes
                || current.sequence() != sequence
                || current.acknowledged_bytes() != bytes
                || record.authentication != current.inventory.authentication_tail()
            {
                return Err(SourceJournalError::Binding);
            }
            journal.validate_guard()
        })()
        .inspect_err(|_| self.journal.quarantine())
    }
}
