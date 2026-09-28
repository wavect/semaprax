//! Same token advances only from actual original Start into actual Prepared.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedPreparedSuccessorV8;
impl ProspectiveOwnedReduceHoldV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_prepared_append_prefix(
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
            inventory.validate_continued_prepared_prefix(selected)
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_prepared_ack(
        &self,
        witness: &VerifiedOwnedContinuedPreparedSuccessorV8<'_>,
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
            let (reserved, stages, turn, selected) =
                session.inventory.continued_prepared_facts()?;
            let fuel = self.checked_spent_funding(reserved, stages)?;
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            let OwnedReduceHoldPhaseV8::TurnStart {
                selected: previous,
                reserved: r,
                stages: s,
            } = &record.phase
            else {
                return Err(SourceJournalError::Binding);
            };
            if record.identity != self.identity
                || record.fuel != fuel
                || record.turn != turn
                || *r != reserved
                || *s != stages
            {
                return Err(SourceJournalError::Binding);
            }
            if !matches!((previous,selected),
                (EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitReserved{turn:t,attempt:0,wait,phase:crate::live_invocation::source_journal::owned_wait_v8::model::PhaseV8::Start,replay_of:None,..}),
                EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitPrepared{turn:next,attempt:0,wait:next_wait,reservation,consumed,..}))if t==next&&wait==next_wait&&usize::try_from(*reservation).ok()==record.sequence.checked_sub(1)&&*consumed<=fuel)
            {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_previous_registry(
                self.journal,
                record.sequence,
                record.bytes,
                &record.authentication,
            )?;
            record.sequence = session.sequence();
            record.bytes = session.acknowledged_bytes();
            record.authentication = session.inventory.authentication_tail().to_owned();
            record.phase = OwnedReduceHoldPhaseV8::TurnStart {
                selected: selected.clone(),
                reserved,
                stages,
            };
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_prepared_guard(
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
            current.inventory.continued_prepared_facts()?;
            self.validate_start_inventory(journal, &current.inventory, sequence, bytes)?;
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
}
