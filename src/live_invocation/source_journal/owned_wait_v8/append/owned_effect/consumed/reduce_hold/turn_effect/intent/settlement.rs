//! The SAME token retains funding through real ordinary/Recorded ACKs.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedSettlementSuccessorV8;
fn phase_matches(phase: &RenewalPhaseV8, row: &EntryV8) -> bool {
    matches!((phase,row),
 (RenewalPhaseV8::Intent,EntryV8::Ordinary(SourceJournalEntry::EffectIntent{..}))|
 (RenewalPhaseV8::Settlement,EntryV8::Ordinary(SourceJournalEntry::EffectObserved{..}|SourceJournalEntry::EffectFailed{..}))|
 (RenewalPhaseV8::Recorded,EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectSettlementRecorded{..})))
}
impl ProspectiveOwnedReduceHoldV8<'_> {
    fn validate_continued_settlement_inventory(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &InventoryV8<'_>,
        sequence: usize,
        bytes: usize,
    ) -> Result<(), SourceJournalError> {
        if !std::ptr::eq(self.journal, journal) || !inventory.belongs_to_context(&journal.context) {
            return Err(SourceJournalError::Binding);
        }
        let (r, s, t, selected) = match inventory.continued_intent_facts() {
            Ok(facts) => facts,
            Err(SourceJournalError::Order) => inventory.continued_settlement_facts()?,
            Err(error) => return Err(error),
        };
        let fuel = self.checked_funding(r, s)?;
        let registry = journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        let OwnedReduceHoldPhaseV8::TurnEffect {
            phase,
            selected: actual,
            reserved,
            stages,
        } = &record.phase
        else {
            return Err(SourceJournalError::Binding);
        };
        if !phase_matches(phase, selected)
            || actual != selected
            || *reserved != r
            || *stages != s
            || record.identity != self.identity
            || record.fuel != fuel
            || record.turn != t
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_settlement_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        (|| {
            self.validate_continued_settlement_inventory(
                journal,
                inventory,
                inventory.sequence(),
                inventory.acknowledged_bytes(),
            )?;
            inventory.validate_continued_settlement_prefix(selected)
        })()
        .inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_settlement_ack(
        &self,
        witness: &VerifiedOwnedContinuedSettlementSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        (||{
      if !std::ptr::eq(self.journal,session.journal)||self.journal.poisoned.get()||!self.journal.append_active.get(){return Err(SourceJournalError::Binding);}
      witness.validate_against_acknowledged_session(session)?;
      let (r,s,t,selected)=session.inventory.continued_settlement_facts()?;let fuel=self.checked_funding(r,s)?;
      let mut registry=self.journal.prospective_reduce.try_borrow_mut().map_err(|_|SourceJournalError::Order)?;let record=registry.as_mut().ok_or(SourceJournalError::Binding)?;
      let OwnedReduceHoldPhaseV8::TurnEffect{phase,selected:previous,reserved,stages}=&record.phase else{return Err(SourceJournalError::Binding);};
      if !phase_matches(phase,previous)||record.identity!=self.identity||record.fuel!=fuel||record.turn!=t||*reserved!=r||*stages!=s{return Err(SourceJournalError::Binding);}
      let phase=match (phase,selected){
        (RenewalPhaseV8::Intent,EntryV8::Ordinary(SourceJournalEntry::EffectObserved{..}|SourceJournalEntry::EffectFailed{..}))=>RenewalPhaseV8::Settlement,
        (RenewalPhaseV8::Settlement,EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectSettlementRecorded{settlement,..})) if usize::try_from(*settlement).ok().and_then(|n|n.checked_add(1))==Some(record.sequence)=>RenewalPhaseV8::Recorded,
        _=>return Err(SourceJournalError::Binding),
      };
      witness.validate_previous_registry(self.journal,record.sequence,record.bytes,&record.authentication)?;
      record.sequence=session.sequence();record.bytes=session.acknowledged_bytes();record.authentication=session.inventory.authentication_tail().into();
      record.phase=OwnedReduceHoldPhaseV8::TurnEffect{phase,selected:selected.clone(),reserved:r,stages:s};Ok(())
    })().inspect_err(|_|self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_settlement_guard(
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
            current.inventory.continued_settlement_facts()?;
            self.validate_continued_settlement_inventory(
                journal,
                &current.inventory,
                sequence,
                bytes,
            )?;
            journal.validate_guard()
        })()
        .inspect_err(|_| self.journal.quarantine())
    }
}
