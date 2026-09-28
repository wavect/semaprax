//! Authenticated failed-Observe append facts, limited to its original State.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::FixedFailedObserveStateAppendPermitV8;
impl<'a> InventoryV8<'a> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failed_observe_cleanup_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32, &str, &serde_json::Value, &EntryV8), SourceJournalError> {
        let c = self.context.fold();
        let f = fold::fold(c, &self.entries)?;
        if !c.cumulative_initialization {
            return Err(SourceJournalError::Order);
        }
        let turn = f.failed_observe_cleanup_turn()?;
        let (basis, state, terminal) = f.failed_observe_cleanup_original()?;
        let last = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let EntryV8::Owned(model::OwnedBodyV8::OwnedObserveSettled {
            turn: t,
            state_digest,
            settlement: model::ObserveSettlementV8::Failed { status },
            ..
        }) = last
        else {
            return Err(SourceJournalError::Order);
        };
        if *t != turn || state_digest != state || status != terminal {
            return Err(SourceJournalError::Binding);
        }
        capacity::outstanding(c, &f)?.check(self.document.len(), self.entries.len())?;
        Ok((
            f.reserved_total,
            f.stages,
            turn,
            basis,
            state_digest,
            status,
            last,
        ))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failed_observe_cleanup_current_facts(
        &self,
    ) -> Result<(u64, u32, u32, &EntryV8), SourceJournalError> {
        let c = self.context.fold();
        let f = fold::fold(c, &self.entries)?;
        if !c.cumulative_initialization {
            return Err(SourceJournalError::Order);
        }
        let turn = f.failed_observe_cleanup_turn()?;
        let last = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        match last {
            EntryV8::Owned(model::OwnedBodyV8::OwnedObserveSettled {
                turn: t,
                settlement: model::ObserveSettlementV8::Failed { .. },
                ..
            }) if *t == turn && f.tail == fold::TailV8::FailedState => {}
            EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupStarted {
                turn: t,
                attempt: None,
                wait: None,
                owner: model::OwnerV8::State,
                ..
            }) if *t == turn && f.tail == fold::TailV8::CleanupInDoubt => {}
            EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupSettled {
                turn: t,
                attempt: None,
                wait: None,
                owner: model::OwnerV8::State,
                receipt,
                ..
            }) if *t == turn
                && f.tail == fold::TailV8::MetadataOnly
                && receipt["kind"] == "observed" => {}
            EntryV8::Ordinary(SourceJournalEntry::Stop {
                turn: Some(t),
                attempt: None,
                ..
            }) if *t == turn && f.tail == fold::TailV8::Stopped => {}
            _ => return Err(SourceJournalError::Order),
        }
        capacity::outstanding(c, &f)?.check(self.document.len(), self.entries.len())?;
        Ok((f.reserved_total, f.stages, turn, last))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_failed_observe_cleanup_prefix(
        &self,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let (_, _, turn, last) = self.failed_observe_cleanup_current_facts()?;
        match (last, selected) {
            (
                EntryV8::Owned(model::OwnedBodyV8::OwnedObserveSettled {
                    settlement: model::ObserveSettlementV8::Failed { status },
                    ..
                }),
                EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupStarted {
                    turn: t,
                    attempt: None,
                    wait: None,
                    owner: model::OwnerV8::State,
                    basis,
                    terminal,
                    ..
                }),
            ) if *t == turn && terminal == status => {
                let (_, _, _, b, _, _, _) = self.failed_observe_cleanup_facts()?;
                if *basis != b {
                    return Err(SourceJournalError::Binding);
                }
                Ok(())
            }
            (
                EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupStarted {
                    owner: model::OwnerV8::State,
                    ..
                }),
                EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupSettled {
                    turn: t,
                    attempt: None,
                    wait: None,
                    owner: model::OwnerV8::State,
                    started,
                    receipt,
                    ..
                }),
            ) if *t == turn
                && Some(*started)
                    == self
                        .sequence()
                        .checked_sub(1)
                        .and_then(|n| u32::try_from(n).ok())
                && receipt["kind"] == "observed" =>
            {
                Ok(())
            }
            (
                EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupSettled {
                    owner: model::OwnerV8::State,
                    receipt,
                    ..
                }),
                EntryV8::Ordinary(SourceJournalEntry::Stop {
                    turn: Some(t),
                    attempt: None,
                    ..
                }),
            ) if *t == turn && receipt["settlement"] == "completed" => Ok(()),
            _ => Err(SourceJournalError::Order),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_fixed_failed_observe_state(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedFailedObserveStateAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::FailedObserveState(journal, permit),
                None,
            )
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::FailedObserveState(journal, permit),
            )
        }
    }
}
impl PendingV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_fixed_failed_observe_state_prefix(
        &self,
        j: &SourceOwnedWaitJournalV8,
        p: &FixedFailedObserveStateAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *p.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        p.validate_selected_prefix(j, &self.0.inventory)
    }
}
