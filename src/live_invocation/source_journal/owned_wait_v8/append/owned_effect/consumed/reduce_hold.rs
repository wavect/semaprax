//! Exclusive future Reduce funding acquired only from the live Consumed owner.
//! No fuel charge, Intent, host, matching-Reduce ACK debit or recovery producer.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::{
    advance_verified_authorization_v8, LiveEffectAuthorizationFailureV8, LivePreparedOwnedEffectV8,
};

/// Closed metadata phase. Neither variant changes credit or grants a write.
pub(in crate::live_invocation::source_journal::owned_wait_v8::append) enum OwnedReduceHoldPhaseV8 {
    ObserverState {
        selected: EntryV8,
    },
    Consumed,
    TurnStart {
        selected: EntryV8,
        reserved: u64,
        stages: u32,
    },
    Continuation {
        selected: EntryV8,
        reserved: u64,
        stages: u32,
    },
    Intent {
        selected: SourceJournalEntry,
    },
    Settlement {
        selected: EntryV8,
    },
    Recorded {
        selected: EntryV8,
    },
    CleanupStarted {
        selected: EntryV8,
    },
    CleanupSettled {
        selected: EntryV8,
    },
    SpentReduce {
        selected: EntryV8,
        reserved: u64,
        stages: u32,
    },
    FailedState {
        selected: EntryV8,
    },
    Step {
        selected: EntryV8,
        reserved: u64,
        stages: u32,
    },
}

/// The actual owner is retained first; credit never exists as a detached token.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct HeldOwnedAuthorizationConsumedV8<
    'j,
> {
    owner: VerifiedOwnedAuthorizationConsumedV8<'j>,
    hold: ProspectiveOwnedReduceHoldV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ProspectiveOwnedReduceHoldV8<
    'j,
> {
    journal: &'j SourceOwnedWaitJournalV8,
    identity: u64,
}
impl Drop for ProspectiveOwnedReduceHoldV8<'_> {
    fn drop(&mut self) {
        // No refund, replacement or terminal retirement exists in this packet.
        self.journal.quarantine();
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ReduceHoldRejectionV8<'j> {
    _owner: VerifiedOwnedAuthorizationConsumedV8<'j>,
    error: SourceJournalError,
}
impl ProspectiveOwnedReduceHoldV8<'_> {
    /// Pure prefix/credit check for the sealed actual Intent permit. In particular
    /// it cannot recover/read a file, borrow a lease, call policy/clock, or grant
    /// permission from a selected row without this actual retained hold.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_intent_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            if !std::ptr::eq(self.journal, journal)
                || journal.poisoned.get()
                || !inventory.belongs_to_context(&journal.context)
            {
                return Err(SourceJournalError::Binding);
            }
            let EntryV8::Ordinary(SourceJournalEntry::EffectIntent { turn, attempt, .. }) =
                selected
            else {
                return Err(SourceJournalError::Binding);
            };
            let (reserved, stages, actual_turn, actual_attempt) =
                inventory.prospective_reduce_facts()?;
            let ordinary = journal.context.ordinary();
            let (_, execution) = journal
                .context
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let fuel = u64::try_from(execution.evaluation_fuel())
                .map_err(|_| SourceJournalError::Capacity)?;
            if Some(execution.evaluation_fuel()) != ordinary.max_steps_per_stage() {
                return Err(SourceJournalError::Binding);
            }
            funding(
                reserved,
                stages,
                fuel,
                u64::try_from(
                    ordinary
                        .max_total_steps()
                        .ok_or(SourceJournalError::Binding)?,
                )
                .map_err(|_| SourceJournalError::Capacity)?,
                ordinary.max_stages(),
            )?;
            {
                let registry = journal
                    .prospective_reduce
                    .try_borrow()
                    .map_err(|_| SourceJournalError::Order)?;
                let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
                if !matches!(&record.phase, OwnedReduceHoldPhaseV8::Consumed)
                    || record.identity != self.identity
                    || record.fuel != fuel
                    || record.turn != *turn
                    || record.attempt != *attempt
                    || record.turn != actual_turn
                    || record.attempt != actual_attempt
                    || record.sequence != inventory.sequence()
                    || record.bytes != inventory.acknowledged_bytes()
                    || record.authentication != inventory.authentication_tail()
                {
                    return Err(SourceJournalError::Binding);
                }
            }
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }

    /// Separate fresh Intent phase guard. The old Consumed guard stays closed.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_intent_guard(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        acknowledged_bytes: usize,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            if !std::ptr::eq(self.journal, journal) {
                return Err(SourceJournalError::Binding);
            }
            journal.validate_guard()?;
            let current = journal.begin_session()?;
            self.validate_intent_inventory(&current.inventory, sequence, acknowledged_bytes)?;
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    fn validate_intent_inventory(
        &self,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        sequence: usize,
        acknowledged_bytes: usize,
    ) -> Result<(), SourceJournalError> {
        if !inventory.belongs_to_context(&self.journal.context) || self.journal.poisoned.get() {
            return Err(SourceJournalError::Binding);
        }
        let (reserved, stages, turn, attempt, selected) = inventory.effect_intent_reduce_facts()?;
        let fuel = self.checked_funding(reserved, stages)?;
        let registry = self
            .journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        let OwnedReduceHoldPhaseV8::Intent { selected: actual } = &record.phase else {
            return Err(SourceJournalError::Binding);
        };
        if !matches!(selected, EntryV8::Ordinary(expected) if expected == actual)
            || record.identity != self.identity
            || record.fuel != fuel
            || record.turn != turn
            || record.attempt != attempt
            || record.sequence != sequence
            || inventory.sequence() != sequence
            || record.bytes != acknowledged_bytes
            || inventory.acknowledged_bytes() != acknowledged_bytes
            || record.authentication != inventory.authentication_tail()
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    fn checked_funding(&self, reserved: u64, stages: u32) -> Result<u64, SourceJournalError> {
        let ordinary = self.journal.context.ordinary();
        let (_, execution) = self
            .journal
            .context
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let fuel =
            u64::try_from(execution.evaluation_fuel()).map_err(|_| SourceJournalError::Capacity)?;
        if Some(execution.evaluation_fuel()) != ordinary.max_steps_per_stage() {
            return Err(SourceJournalError::Binding);
        }
        funding(
            reserved,
            stages,
            fuel,
            u64::try_from(
                ordinary
                    .max_total_steps()
                    .ok_or(SourceJournalError::Binding)?,
            )
            .map_err(|_| SourceJournalError::Capacity)?,
            ordinary.max_stages(),
        )?;
        Ok(fuel)
    }
    /// Only an actual fixed Intent ACK witness plus its acknowledged session can
    /// advance this retained registry. No callback or file access under marker.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_intent_ack(
        &self,
        witness: &super::super::intent::VerifiedOwnedEffectIntentSuccessorV8<'_>,
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
                session.inventory.effect_intent_reduce_facts()?;
            let fuel = self.checked_funding(reserved, stages)?;
            let EntryV8::Ordinary(selected) = selected else {
                return Err(SourceJournalError::Binding);
            };
            let selected = selected.clone();
            let authentication = session.inventory.authentication_tail().to_owned();
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            if !matches!(&record.phase, OwnedReduceHoldPhaseV8::Consumed)
                || record.identity != self.identity
                || record.fuel != fuel
                || record.turn != turn
                || record.attempt != attempt
            {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_consumed_registry(
                self.journal,
                record.sequence,
                record.bytes,
                &record.authentication,
            )?;
            record.phase = OwnedReduceHoldPhaseV8::Intent { selected };
            record.sequence = session.sequence();
            record.bytes = session.acknowledged_bytes();
            record.authentication = authentication;
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }

    /// Exact existing settlement phases retain the same prospective credit.
    /// This is callback-free; only the closed live obligation chooses a row.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_settlement_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            if !std::ptr::eq(self.journal, journal) {
                return Err(SourceJournalError::Binding);
            }
            match selected {
                EntryV8::Ordinary(SourceJournalEntry::EffectObserved {turn,attempt,..} | SourceJournalEntry::EffectFailed {turn,attempt,..}) => {
                    self.validate_intent_inventory(inventory,inventory.sequence(),inventory.acknowledged_bytes())?;
                    let (_,_,actual_turn,actual_attempt,_)=inventory.effect_intent_reduce_facts()?;
                    if (*turn,*attempt)!=(actual_turn,actual_attempt){return Err(SourceJournalError::Binding);}
                }
                EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectSettlementRecorded {turn,attempt,settlement,..}) => {
                    self.validate_settlement_inventory(inventory,inventory.sequence(),inventory.acknowledged_bytes())?;
                    let (_,_,actual_turn,actual_attempt,previous)=inventory.effect_settlement_reduce_facts()?;
                    if (*turn,*attempt)!=(actual_turn,actual_attempt) || !matches!(previous,EntryV8::Ordinary(SourceJournalEntry::EffectObserved {..}|SourceJournalEntry::EffectFailed {..})) || usize::try_from(*settlement).ok().and_then(|seq|seq.checked_add(1))!=Some(inventory.sequence()) {return Err(SourceJournalError::Binding);}
                }
                _=>return Err(SourceJournalError::Binding),
            }
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_settlement_guard(
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
            self.validate_settlement_inventory(&current.inventory, sequence, bytes)?;
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    fn validate_settlement_inventory(
        &self,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        sequence: usize,
        bytes: usize,
    ) -> Result<(), SourceJournalError> {
        if !inventory.belongs_to_context(&self.journal.context) || self.journal.poisoned.get() {
            return Err(SourceJournalError::Binding);
        }
        let (reserved, stages, turn, attempt, selected) =
            inventory.effect_settlement_reduce_facts()?;
        let fuel = self.checked_funding(reserved, stages)?;
        let registry = self
            .journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        let actual = match &record.phase {
            OwnedReduceHoldPhaseV8::Settlement { selected }
            | OwnedReduceHoldPhaseV8::Recorded { selected } => selected,
            _ => return Err(SourceJournalError::Binding),
        };
        if actual != selected
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
        Ok(())
    }
    /// Only the real fixed append witness and its ACK session advance a phase.
    /// No clock, policy, lease read or normal fresh guard under this marker.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_settlement_ack(
        &self,
        witness: &super::super::settlement::VerifiedOwnedEffectSettlementSuccessorV8<'_>,
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
                session.inventory.effect_settlement_reduce_facts()?;
            let fuel = self.checked_funding(reserved, stages)?;
            let selected = selected.clone();
            let authentication = session.inventory.authentication_tail().to_owned();
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            if record.identity != self.identity
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
            let phase=match (&record.phase,&selected){
                (OwnedReduceHoldPhaseV8::Intent{..},EntryV8::Ordinary(SourceJournalEntry::EffectObserved{..}|SourceJournalEntry::EffectFailed{..}))=>OwnedReduceHoldPhaseV8::Settlement{selected},
                (OwnedReduceHoldPhaseV8::Settlement{..},EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectSettlementRecorded{..}))=>OwnedReduceHoldPhaseV8::Recorded{selected},
                _=>return Err(SourceJournalError::Binding),
            };
            record.phase = phase;
            record.sequence = session.sequence();
            record.bytes = session.acknowledged_bytes();
            record.authentication = authentication;
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }

    /// Callback-free checked phase/funding match. No cancellation or clock call.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_cleanup_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            if !std::ptr::eq(self.journal, journal) {
                return Err(SourceJournalError::Binding);
            }
            use crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8;
            let (turn, attempt, previous) = match selected {
                EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                    turn,
                    attempt,
                    recorded,
                    ..
                }) => {
                    self.validate_settlement_inventory(
                        inventory,
                        inventory.sequence(),
                        inventory.acknowledged_bytes(),
                    )?;
                    let (_, _, t, a, row) = inventory.effect_settlement_reduce_facts()?;
                    if !matches!(
                        row,
                        EntryV8::Owned(OwnedBodyV8::OwnedEffectSettlementRecorded { .. })
                    ) {
                        return Err(SourceJournalError::Binding);
                    }
                    if (*turn, *attempt) != (t, a) {
                        return Err(SourceJournalError::Binding);
                    }
                    (*turn, *attempt, *recorded)
                }
                EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                    turn,
                    attempt,
                    started,
                    ..
                }) => {
                    self.validate_cleanup_inventory(
                        inventory,
                        inventory.sequence(),
                        inventory.acknowledged_bytes(),
                    )?;
                    let (_, _, t, a, row) = inventory.effect_cleanup_reduce_facts()?;
                    if !matches!(
                        row,
                        EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted { .. })
                    ) {
                        return Err(SourceJournalError::Binding);
                    }
                    if (*turn, *attempt) != (t, a) {
                        return Err(SourceJournalError::Binding);
                    }
                    (*turn, *attempt, *started)
                }
                _ => return Err(SourceJournalError::Binding),
            };
            let _ = (turn, attempt);
            if usize::try_from(previous)
                .ok()
                .and_then(|s| s.checked_add(1))
                != Some(inventory.sequence())
            {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_cleanup_guard(
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
            self.validate_cleanup_inventory(&current.inventory, sequence, bytes)?;
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    fn validate_cleanup_inventory(
        &self,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        sequence: usize,
        bytes: usize,
    ) -> Result<(), SourceJournalError> {
        if !inventory.belongs_to_context(&self.journal.context) || self.journal.poisoned.get() {
            return Err(SourceJournalError::Binding);
        }
        let (reserved, stages, turn, attempt, selected) =
            inventory.effect_cleanup_reduce_facts()?;
        let fuel = self.checked_funding(reserved, stages)?;
        let registry = self
            .journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        let actual = match &record.phase {
            OwnedReduceHoldPhaseV8::CleanupStarted { selected }
            | OwnedReduceHoldPhaseV8::CleanupSettled { selected } => selected,
            _ => return Err(SourceJournalError::Binding),
        };
        if actual != selected
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
        Ok(())
    }
    /// Only the real fixed append witness and its ACK session advance a phase.
    /// No clock, policy, lease read or normal fresh guard under this marker.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_cleanup_ack(
        &self,
        witness: &super::super::settlement::cleanup::VerifiedOwnedEffectCleanupSuccessorV8<'_>,
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
                session.inventory.effect_cleanup_reduce_facts()?;
            let fuel = self.checked_funding(reserved, stages)?;
            let selected = selected.clone();
            let authentication = session.inventory.authentication_tail().to_owned();
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            if record.identity != self.identity
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
            let phase=match (&record.phase,&selected){
                (OwnedReduceHoldPhaseV8::Recorded{..},EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectDecisionCleanupStarted{..}))=>OwnedReduceHoldPhaseV8::CleanupStarted{selected},
                (OwnedReduceHoldPhaseV8::CleanupStarted{..},EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled{..}))=>OwnedReduceHoldPhaseV8::CleanupSettled{selected},
                _=>return Err(SourceJournalError::Binding),
            };
            record.phase = phase;
            record.sequence = session.sequence();
            record.bytes = session.acknowledged_bytes();
            record.authentication = authentication;
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }

    /// The actual owner selects one original full-F Reduce reservation. This
    /// pure check neither charges it nor permits a failed effect to reduce.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_reduce_append_prefix(
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
            if (*actual_turn,*actual_attempt)!=(turn,attempt)||u64::try_from(*actual_fuel).ok()!=Some(fuel)||!matches!(previous,EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled{..})){return Err(SourceJournalError::Binding);}
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    /// Under the append marker only the actual fixed ACK and exact session can
    /// consume the one prospective slot. The fold already charged F and a stage.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_reduce_ack(
        &self,
        witness:&super::super::settlement::cleanup::reduce::VerifiedOwnedReduceReservationSuccessorV8<'_>,
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
                session.inventory.original_reduce_facts()?;
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
            // No refund, new identity, reset, second F addition or second debit.
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
    /// Exact current original-Reduce prefix after the single recorded charge.
    /// Future Step rows require their own closed phase, never this old cursor.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_spent_reduce_guard(
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
            if !inventory.belongs_to_context(&journal.context) {
                return Err(SourceJournalError::Binding);
            }
            let (reserved, stages, turn, attempt, selected) = inventory.original_reduce_facts()?;
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
    /// Borrow-only exact spent lineage for the closed Step producer. The fold
    /// has already charged the original F and stage; no future funding is added.
    fn validate_failed_state_inventory(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        sequence: usize,
        bytes: usize,
    ) -> Result<(), SourceJournalError> {
        if !std::ptr::eq(self.journal, journal)
            || journal.poisoned.get()
            || !inventory.belongs_to_context(&journal.context)
        {
            return Err(SourceJournalError::Binding);
        }
        let (reserved, stages, turn, attempt, selected) = inventory.failed_effect_state_facts()?;
        let fuel = self.checked_funding(reserved, stages)?;
        let registry = journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        let actual = match &record.phase {
            OwnedReduceHoldPhaseV8::CleanupSettled { selected }
            | OwnedReduceHoldPhaseV8::FailedState { selected } => selected,
            _ => return Err(SourceJournalError::Binding),
        };
        if actual != selected
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
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_failed_state_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.validate_failed_state_inventory(
                journal,
                inventory,
                inventory.sequence(),
                inventory.acknowledged_bytes(),
            )?;
            if !matches!(selected,EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectFailureStateCleanupStarted{..}|crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectFailureStateCleanupSettled{..})|EntryV8::Ordinary(SourceJournalEntry::Stop{..})){return Err(SourceJournalError::Binding);}
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    /// Callback-free, real ACK-only phase update under the physical marker.
    /// Retain the charged aggregate R/S and the same identity/F forever.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_failed_state_ack(
        &self,
        witness: &super::super::settlement::cleanup::failed_state::VerifiedFailedEffectStateSuccessorV8<'_>,
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
                session.inventory.failed_effect_state_facts()?;
            let fuel = self.checked_funding(reserved, stages)?;
            let selected = selected.clone();
            let authentication = session.inventory.authentication_tail().to_owned();
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            if record.identity != self.identity
                || record.fuel != fuel
                || record.turn != turn
                || record.attempt != attempt
            {
                return Err(SourceJournalError::Binding);
            }
            let legal=match(&record.phase,&selected){
                (OwnedReduceHoldPhaseV8::CleanupSettled{..},EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectFailureStateCleanupStarted{..}))=>true,
                (OwnedReduceHoldPhaseV8::FailedState{selected:EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectFailureStateCleanupStarted{..})},EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectFailureStateCleanupSettled{..}))=>true,
                (OwnedReduceHoldPhaseV8::FailedState{selected:EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectFailureStateCleanupSettled{receipt,..})},EntryV8::Ordinary(SourceJournalEntry::Stop{status:crate::live_invocation::source_journal::SourceStopStatus::EffectFailed,reason:crate::live_invocation::source_journal::SourceStopReason::EffectFailed,..})) if receipt["settlement"]=="completed"=>true,
                _=>false,
            };
            if !legal {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_previous_registry(
                self.journal,
                record.sequence,
                record.bytes,
                &record.authentication,
            )?;
            record.phase = OwnedReduceHoldPhaseV8::FailedState { selected };
            record.sequence = session.sequence();
            record.bytes = session.acknowledged_bytes();
            record.authentication = authentication;
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_failed_state_guard(
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
            self.validate_failed_state_inventory(journal, &current.inventory, sequence, bytes)?;
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    fn validate_step_inventory(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        sequence: usize,
        bytes: usize,
    ) -> Result<(), SourceJournalError> {
        if !std::ptr::eq(self.journal, journal)
            || journal.poisoned.get()
            || !inventory.belongs_to_context(&journal.context)
        {
            return Err(SourceJournalError::Binding);
        }
        let (reserved, stages, turn, attempt, selected) = inventory.step_reduce_facts()?;
        let fuel = self.checked_spent_funding(reserved, stages)?;
        let registry = journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        let (actual, charged, charged_stages) = match &record.phase {
            OwnedReduceHoldPhaseV8::SpentReduce {
                selected,
                reserved,
                stages,
            }
            | OwnedReduceHoldPhaseV8::Step {
                selected,
                reserved,
                stages,
            } => (selected, *reserved, *stages),
            _ => return Err(SourceJournalError::Binding),
        };
        if actual != selected
            || charged != reserved
            || charged_stages != stages
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
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_step_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.validate_step_inventory(
                journal,
                inventory,
                inventory.sequence(),
                inventory.acknowledged_bytes(),
            )?;
            if !matches!(selected,
                EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceStaged{..}
                    |crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceCleanupStarted{..}
                    |crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceCleanupSettled{..}
                    |crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStepTransferReserved{..}
                    |crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStepTransferCompleted{..})
                |EntryV8::Ordinary(SourceJournalEntry::Transition{..}|SourceJournalEntry::Stop{..})) {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    /// Callback-free, real ACK-only phase update under the physical marker.
    /// Retain the charged aggregate R/S and the same identity/F forever.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_step_ack(
        &self,
        witness: &super::super::settlement::cleanup::reduce::step::VerifiedOwnedStepSuccessorV8<'_>,
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
                session.inventory.step_reduce_facts()?;
            let fuel = self.checked_spent_funding(reserved, stages)?;
            let selected = selected.clone();
            let authentication = session.inventory.authentication_tail().to_owned();
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            let (charged, charged_stages) = match &record.phase {
                OwnedReduceHoldPhaseV8::SpentReduce {
                    reserved, stages, ..
                }
                | OwnedReduceHoldPhaseV8::Step {
                    reserved, stages, ..
                } => (*reserved, *stages),
                _ => return Err(SourceJournalError::Binding),
            };
            if charged != reserved
                || charged_stages != stages
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
            record.phase = OwnedReduceHoldPhaseV8::Step {
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_step_guard(
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
            self.validate_step_inventory(journal, &current.inventory, sequence, bytes)?;
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    fn validate_continue_inventory(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        sequence: usize,
        bytes: usize,
    ) -> Result<(), SourceJournalError> {
        if !std::ptr::eq(self.journal, journal)
            || journal.poisoned.get()
            || !inventory.belongs_to_context(&journal.context)
        {
            return Err(SourceJournalError::Binding);
        }
        let (reserved, stages, turn, selected) = inventory.continuation_facts()?;
        let fuel = self.checked_spent_funding(reserved, stages)?;
        let registry = journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        let (actual, r, s) = match &record.phase {
            OwnedReduceHoldPhaseV8::Step {
                selected,
                reserved,
                stages,
            }
            | OwnedReduceHoldPhaseV8::Continuation {
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
    /// No callbacks/lease reads: only the actual retained token and checked prefix.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continue_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &super::super::super::super::candidate::InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.validate_continue_inventory(
                journal,
                inventory,
                inventory.sequence(),
                inventory.acknowledged_bytes(),
            )?;
            let (_, _, turn, last) = inventory.continuation_facts()?;
            match (last,selected){
                (EntryV8::Ordinary(SourceJournalEntry::Transition{case:crate::live_invocation::source_journal::SourceTransitionCase::Continue,..}),
                    EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStateCommitted{turn:next,..}))
                    if turn.checked_add(1)==Some(*next)=>{},
                (EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStateCommitted{..}),
                    EntryV8::Ordinary(SourceJournalEntry::StageReservation{turn:next,attempt:None,role:crate::live_invocation::source_journal::SourceStageRole::Observe,fuel}))
                    if *next==turn&&Some(*fuel)==journal.context.ordinary().max_steps_per_stage()=>{},
                _=>return Err(SourceJournalError::Binding),
            }
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    /// Only a real persisted ACK can advance this same token; no reopening/refund.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continue_ack(
        &self,
        witness: &super::super::settlement::cleanup::reduce::step::VerifiedOwnedContinueSuccessorV8<
            '_,
        >,
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
            let (reserved, stages, turn, selected) = session.inventory.continuation_facts()?;
            let fuel = self.checked_spent_funding(reserved, stages)?;
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            let (previous, r, s) = match &record.phase {
                OwnedReduceHoldPhaseV8::Step {
                    selected,
                    reserved,
                    stages,
                }
                | OwnedReduceHoldPhaseV8::Continuation {
                    selected,
                    reserved,
                    stages,
                } => (selected, *reserved, *stages),
                _ => return Err(SourceJournalError::Binding),
            };
            if record.identity != self.identity || record.fuel != fuel {
                return Err(SourceJournalError::Binding);
            }
            match (previous,selected){
                (EntryV8::Ordinary(SourceJournalEntry::Transition{case:crate::live_invocation::source_journal::SourceTransitionCase::Continue,..}),
                    EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStateCommitted{..}))
                    if record.turn.checked_add(1)==Some(turn)&&reserved==r&&stages==s=>{},
                (EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStateCommitted{..}),
                    EntryV8::Ordinary(SourceJournalEntry::StageReservation{role:crate::live_invocation::source_journal::SourceStageRole::Observe,attempt:None,fuel:f,..}))
                    if record.turn==turn&&Some(*f)==self.journal.context.ordinary().max_steps_per_stage()
                        &&r.checked_add(fuel)==Some(reserved)&&s.checked_add(1)==Some(stages)=>{},
                _=>return Err(SourceJournalError::Binding),
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
            record.turn = turn;
            record.sequence = session.sequence();
            record.bytes = session.acknowledged_bytes();
            record.authentication = session.inventory.authentication_tail().to_owned();
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continue_guard(
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
            self.validate_continue_inventory(journal, &current.inventory, sequence, bytes)?;
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    fn checked_spent_funding(&self, reserved: u64, stages: u32) -> Result<u64, SourceJournalError> {
        let ordinary = self.journal.context.ordinary();
        let (_, execution) = self
            .journal
            .context
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let fuel =
            u64::try_from(execution.evaluation_fuel()).map_err(|_| SourceJournalError::Capacity)?;
        if Some(execution.evaluation_fuel()) != ordinary.max_steps_per_stage() {
            return Err(SourceJournalError::Binding);
        }
        let total = u64::try_from(
            ordinary
                .max_total_steps()
                .ok_or(SourceJournalError::Binding)?,
        )
        .map_err(|_| SourceJournalError::Capacity)?;
        spent_funding(reserved, stages, fuel, total, ordinary.max_stages())?;
        Ok(fuel)
    }
    /// Borrow-only Consumed-phase guard, never an owner/ACK/token producer.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_guard(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        acknowledged_bytes: usize,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            // Refuse before reading a foreign container; retire our own lineage.
            if !std::ptr::eq(self.journal, journal) {
                return Err(SourceJournalError::Binding);
            }
            journal.validate_guard()?;
            let current = journal.begin_session()?;
            let (reserved, stages, turn, attempt) = current.inventory.prospective_reduce_facts()?;
            let ordinary = journal.context.ordinary();
            let (_, execution) = journal
                .context
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let fuel = u64::try_from(execution.evaluation_fuel())
                .map_err(|_| SourceJournalError::Capacity)?;
            if Some(execution.evaluation_fuel()) != ordinary.max_steps_per_stage() {
                return Err(SourceJournalError::Binding);
            }
            funding(
                reserved,
                stages,
                fuel,
                u64::try_from(
                    ordinary
                        .max_total_steps()
                        .ok_or(SourceJournalError::Binding)?,
                )
                .map_err(|_| SourceJournalError::Capacity)?,
                ordinary.max_stages(),
            )?;
            {
                let registry = journal
                    .prospective_reduce
                    .try_borrow()
                    .map_err(|_| SourceJournalError::Order)?;
                let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
                if !matches!(&record.phase, OwnedReduceHoldPhaseV8::Consumed)
                    || record.identity != self.identity
                    || record.turn != turn
                    || record.attempt != attempt
                    || record.fuel != fuel
                    || record.sequence != sequence
                    || record.bytes != acknowledged_bytes
                    || current.sequence() != sequence
                    || current.acknowledged_bytes() != acknowledged_bytes
                    || record.authentication != current.inventory.authentication_tail()
                {
                    return Err(SourceJournalError::Binding);
                }
            }
            // No registry borrow crosses this final physical guard.
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
}
impl<'j> HeldOwnedAuthorizationConsumedV8<'j> {
    /// Move the actual owner and SAME hold once through the closed core helper.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_authorization(
        self,
    ) -> Result<LivePreparedOwnedEffectV8<'j>, LiveEffectAuthorizationFailureV8<'j>> {
        let Self { owner, hold } = self;
        let VerifiedOwnedAuthorizationConsumedV8 {
            obligation,
            session,
            witness,
        } = owner;
        advance_verified_authorization_v8(obligation, session, witness, hold)
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let guard = || {
            self.hold.validate_guard(
                self.owner.session.journal,
                self.owner.session.sequence(),
                self.owner.session.acknowledged_bytes(),
            )
        };
        let result = (|| {
            self.owner.validate_live()?;
            guard()?;
            self.owner.validate_live()?;
            // Policy/clock callbacks have ended; re-read the physical prefix.
            guard()
        })();
        result.inspect_err(|_| self.hold.journal.quarantine())
    }
}

fn funding(
    reserved: u64,
    stages: u32,
    fuel: u64,
    total: u64,
    max_stages: u32,
) -> Result<(), SourceJournalError> {
    if reserved.checked_add(fuel).is_none_or(|n| n > total)
        || stages.checked_add(1).is_none_or(|n| n > max_stages)
    {
        return Err(SourceJournalError::Capacity);
    }
    Ok(())
}
pub(super) fn reserve<'j>(
    owner: VerifiedOwnedAuthorizationConsumedV8<'j>,
) -> Result<HeldOwnedAuthorizationConsumedV8<'j>, ReduceHoldRejectionV8<'j>> {
    let journal = owner.session.journal;
    let prepared = (|| {
        journal.validate_guard()?;
        if journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?
            .is_some()
        {
            return Err(SourceJournalError::Order);
        }
        owner.validate_live()?;
        // Read the actual physical prefix again, not the envelope's old Vec.
        let current = journal.begin_session()?;
        owner.witness.successor.validate_current()?;
        if current.sequence() != owner.session.sequence()
            || current.acknowledged_bytes() != owner.session.acknowledged_bytes()
            || current.inventory.authentication_tail()
                != owner.session.inventory.authentication_tail()
        {
            journal.quarantine();
            return Err(SourceJournalError::Order);
        }
        let (reserved, stages, turn, attempt) = current.inventory.prospective_reduce_facts()?;
        let ordinary = journal.context.ordinary();
        let (_, execution) = journal
            .context
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let fuel =
            u64::try_from(execution.evaluation_fuel()).map_err(|_| SourceJournalError::Capacity)?;
        if Some(execution.evaluation_fuel()) != ordinary.max_steps_per_stage() {
            return Err(SourceJournalError::Binding);
        }
        funding(
            reserved,
            stages,
            fuel,
            u64::try_from(
                ordinary
                    .max_total_steps()
                    .ok_or(SourceJournalError::Binding)?,
            )
            .map_err(|_| SourceJournalError::Capacity)?,
            ordinary.max_stages(),
        )?;
        let authentication = current.inventory.authentication_tail().to_owned();
        // All policy/runtime callbacks finish before the registry's final insert.
        owner.validate_live()?;
        owner.witness.successor.validate_current()?;
        journal.validate_guard()?;
        let mut registry = journal
            .prospective_reduce
            .try_borrow_mut()
            .map_err(|_| SourceJournalError::Order)?;
        if journal.append_active.get() || registry.is_some() {
            return Err(SourceJournalError::Order);
        }
        let identity = journal
            .prospective_reduce_identity
            .get()
            .checked_add(1)
            .ok_or(SourceJournalError::Capacity)?;
        // Callback-free, borrow-held recheck/increment/insert is indivisible.
        journal.prospective_reduce_identity.set(identity);
        *registry = Some(ProspectiveReduceRegistryV8 {
            identity,
            sequence: current.sequence(),
            bytes: current.acknowledged_bytes(),
            authentication,
            turn,
            attempt,
            fuel,
            phase: OwnedReduceHoldPhaseV8::Consumed,
        });
        Ok(identity)
    })();
    match prepared {
        Ok(identity) => Ok(HeldOwnedAuthorizationConsumedV8 {
            owner,
            hold: ProspectiveOwnedReduceHoldV8 { journal, identity },
        }),
        Err(error) => Err(ReduceHoldRejectionV8 {
            _owner: owner,
            error,
        }),
    }
}

#[cfg(test)]
mod tests;

// Pure already-charged arithmetic. Prospective funding() intentionally adds
// F/one slot; this must not, and must reject an impossible uncharged ledger.
fn spent_funding(
    reserved: u64,
    stages: u32,
    fuel: u64,
    total: u64,
    max_stages: u32,
) -> Result<(), SourceJournalError> {
    if reserved < fuel || stages == 0 || reserved > total || stages > max_stages {
        return Err(SourceJournalError::Capacity);
    }
    Ok(())
}
#[cfg(test)]
mod spent_funding_tests {
    use super::*;
    #[test]
    fn owned_reduce_spent_funding_exact_limit_does_not_charge_twice() {
        assert_eq!(funding(5000, 3, 1000, 6000, 4), Ok(()));
        assert_eq!(spent_funding(6000, 4, 1000, 6000, 4), Ok(()));
        assert_eq!(
            spent_funding(6001, 4, 1000, 6000, 4),
            Err(SourceJournalError::Capacity)
        );
        assert_eq!(
            spent_funding(6000, 5, 1000, 6000, 4),
            Err(SourceJournalError::Capacity)
        );
        assert_eq!(
            spent_funding(999, 1, 1000, 6000, 4),
            Err(SourceJournalError::Capacity)
        );
        assert_eq!(
            spent_funding(1000, 0, 1000, 6000, 4),
            Err(SourceJournalError::Capacity)
        );
        assert_eq!(
            spent_funding(u64::MAX, u32::MAX, 1, u64::MAX, u32::MAX),
            Ok(())
        );
    }
}

mod observe_settlement;

mod failed_observe_cleanup;

mod turn_start;

mod turn_prepared;
