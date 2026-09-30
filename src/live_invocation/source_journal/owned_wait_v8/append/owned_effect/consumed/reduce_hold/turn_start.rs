//! SAME spent token follows only owner-bound continued Created/Start ACKs.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedStartSuccessorV8;
impl ProspectiveOwnedReduceHoldV8<'_> {
    pub(super) fn validate_start_inventory(
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
        let (reserved, stages, turn, selected) = inventory.continued_start_facts()?;
        let fuel = self.checked_spent_funding(reserved, stages)?;
        let registry = journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        let (actual, r, s) = match &record.phase {
            OwnedReduceHoldPhaseV8::Continuation {
                selected,
                reserved,
                stages,
            }
            | OwnedReduceHoldPhaseV8::TurnStart {
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_start_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::super::candidate::InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.validate_start_inventory(
                journal,
                inventory,
                inventory.sequence(),
                inventory.acknowledged_bytes(),
            )?;
            inventory.validate_continued_start_prefix(selected)
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_start_ack(
        &self,
        witness: &VerifiedOwnedContinuedStartSuccessorV8<'_>,
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
            let (reserved, stages, turn, selected) = session.inventory.continued_start_facts()?;
            let fuel = self.checked_spent_funding(reserved, stages)?;
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            let (previous, r, s) = match &record.phase {
                OwnedReduceHoldPhaseV8::Continuation {
                    selected,
                    reserved,
                    stages,
                }
                | OwnedReduceHoldPhaseV8::TurnStart {
                    selected,
                    reserved,
                    stages,
                } => (selected, *reserved, *stages),
                _ => return Err(SourceJournalError::Binding),
            };
            if record.identity != self.identity
                || record.fuel != fuel
                || record.turn != turn
                || s != stages
            {
                return Err(SourceJournalError::Binding);
            }
            let addition=match (previous,selected){
                (EntryV8::Ordinary(SourceJournalEntry::TurnObserved{..}),EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitCreated{..}))=>0,
                (EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitCreated{wait,..}),EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitReserved{wait:next,phase:crate::live_invocation::source_journal::owned_wait_v8::model::PhaseV8::Start,replay_of:None,fuel:actual,..}))if wait==next&&*actual==fuel=>fuel,
                _=>return Err(SourceJournalError::Binding),
            };
            if r.checked_add(addition) != Some(reserved) {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_previous_registry(
                self.journal,
                record.sequence,
                record.bytes,
                &record.authentication,
            )?;
            record.phase = OwnedReduceHoldPhaseV8::TurnStart {
                selected: selected.clone(),
                reserved,
                stages,
            };
            record.sequence = session.sequence();
            record.bytes = session.acknowledged_bytes();
            record.authentication = session.inventory.authentication_tail().into();
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_start_guard(
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
            self.validate_start_inventory(journal, &current.inventory, sequence, bytes)?;
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
}
