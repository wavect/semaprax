//! Same-token failed-Observe State cleanup ACKs; credit remains spent.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedFailedObserveStateSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::model;
impl ProspectiveOwnedReduceHoldV8<'_> {
    fn validate_failed_observe_inventory(
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
        let (reserved, stages, turn, selected) =
            inventory.failed_observe_cleanup_current_facts()?;
        let fuel = self.checked_spent_funding(reserved, stages)?;
        let registry = journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        let OwnedReduceHoldPhaseV8::Continuation {
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
            || inventory.sequence() != sequence
            || inventory.acknowledged_bytes() != bytes
            || record.authentication != inventory.authentication_tail()
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_failed_observe_cleanup_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::super::candidate::InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.validate_failed_observe_inventory(
                journal,
                inventory,
                inventory.sequence(),
                inventory.acknowledged_bytes(),
            )?;
            inventory.validate_failed_observe_cleanup_prefix(selected)
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_failed_observe_cleanup_ack(
        &self,
        witness: &VerifiedFailedObserveStateSuccessorV8<'_>,
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
                session.inventory.failed_observe_cleanup_current_facts()?;
            let fuel = self.checked_spent_funding(reserved, stages)?;
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            let OwnedReduceHoldPhaseV8::Continuation {
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
            match (previous, selected) {
                (
                    EntryV8::Owned(model::OwnedBodyV8::OwnedObserveSettled {
                        settlement: model::ObserveSettlementV8::Failed { .. },
                        ..
                    }),
                    EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupStarted {
                        owner: model::OwnerV8::State,
                        attempt: None,
                        wait: None,
                        ..
                    }),
                ) => {}
                (
                    EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupStarted {
                        owner: model::OwnerV8::State,
                        attempt: None,
                        wait: None,
                        ..
                    }),
                    EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupSettled {
                        owner: model::OwnerV8::State,
                        attempt: None,
                        wait: None,
                        receipt,
                        ..
                    }),
                ) if receipt["kind"] == "observed" => {}
                (
                    EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupSettled {
                        owner: model::OwnerV8::State,
                        attempt: None,
                        wait: None,
                        receipt,
                        ..
                    }),
                    EntryV8::Ordinary(SourceJournalEntry::Stop {
                        turn: Some(_),
                        attempt: None,
                        ..
                    }),
                ) if receipt["settlement"] == "completed" => {}
                _ => return Err(SourceJournalError::Binding),
            }
            witness.validate_previous_registry(
                self.journal,
                record.sequence,
                record.bytes,
                &record.authentication,
            )?;
            record.phase = OwnedReduceHoldPhaseV8::Continuation {
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_failed_observe_cleanup_guard(
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
            self.validate_failed_observe_inventory(journal, &current.inventory, sequence, bytes)?;
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
}
