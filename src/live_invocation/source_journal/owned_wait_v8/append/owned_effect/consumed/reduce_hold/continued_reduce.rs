//! Continued Reduce hold transition.
use super::*;

impl ProspectiveOwnedReduceHoldV8<'_> {
    /// The actual continued cleanup owner selects one positive-turn Reduce
    /// reservation. It consumes the same hold only after a fixed ACK.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_reduce_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            if !std::ptr::eq(self.journal, journal) {
                return Err(SourceJournalError::Binding);
            }
            self.validate_cleanup_inventory(
                inventory,
                inventory.sequence(),
                inventory.acknowledged_bytes(),
            )?;
            let (reserved, stages, turn, attempt, previous) = inventory.released_reduce_facts()?;
            let fuel = self.checked_funding(reserved, stages)?;
            let EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn: actual_turn,
                attempt: Some(actual_attempt),
                role: crate::live_invocation::source_journal::SourceStageRole::Reduce,
                fuel: actual_fuel,
            }) = selected
            else {
                return Err(SourceJournalError::Binding);
            };
            if *actual_turn == 0 || (*actual_turn, *actual_attempt) != (turn, attempt)
                || u64::try_from(*actual_fuel).ok() != Some(fuel)
                || !matches!(previous, EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled { .. }))
            { return Err(SourceJournalError::Binding); }
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    /// Fixed continued reservation ACK advances CleanupSettled exactly once.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_reduce_ack(
        &self,
        witness: &super::super::VerifiedOwnedContinuedReduceSuccessorV8<'_>,
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
            let (reserved, stages, turn, attempt, selected) =
                session.inventory.continued_reduce_facts()?;
            let fuel = self.checked_spent_funding(reserved, stages)?;
            let EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                fuel: actual_fuel, ..
            }) = selected
            else {
                return Err(SourceJournalError::Binding);
            };
            if u64::try_from(*actual_fuel).ok() != Some(fuel) {
                return Err(SourceJournalError::Binding);
            }
            let selected = selected.clone();
            let authentication = session.inventory.authentication_tail().to_owned();
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            if !matches!(&record.phase, OwnedReduceHoldPhaseV8::CleanupSettled { .. })
                || record.identity != self.identity
                || record.fuel != fuel
                || record.turn != turn
                || record.attempt != attempt
            {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_previous_registry(
                self.journal,
                record.sequence,
                record.bytes,
                &record.authentication,
            )?;
            record.phase = OwnedReduceHoldPhaseV8::SpentReduce {
                selected,
                reserved,
                stages,
            };
            record.sequence = session.sequence();
            record.bytes = session.acknowledged_bytes();
            record.authentication = authentication;
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    /// Exact continued Reduce prefix after its one acknowledged reservation.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_spent_reduce_guard(
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
            let inventory = &current.inventory;
            let (reserved, stages, turn, attempt, selected) = inventory.continued_reduce_facts()?;
            let fuel = self.checked_spent_funding(reserved, stages)?;
            let registry = journal
                .prospective_reduce
                .try_borrow()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
            let OwnedReduceHoldPhaseV8::SpentReduce {
                selected: actual,
                reserved: charged,
                stages: charged_stages,
            } = &record.phase
            else {
                return Err(SourceJournalError::Binding);
            };
            if actual != selected
                || *charged != reserved
                || *charged_stages != stages
                || record.identity != self.identity
                || record.fuel != fuel
                || record.turn != turn
                || record.attempt != attempt
                || record.sequence != sequence
                || inventory.sequence() != sequence
                || record.bytes != bytes
                || inventory.acknowledged_bytes() != bytes
                || record.authentication != inventory.authentication_tail()
            {
                return Err(SourceJournalError::Binding);
            }
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
}
