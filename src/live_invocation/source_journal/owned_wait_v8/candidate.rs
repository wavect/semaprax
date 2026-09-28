//! Consuming inert append choreography. No sink, File, runtime owner, or ACK factory.
use super::append::SourceOwnedWaitJournalV8;
use super::live_upstream::effect::authorization::cleanup::reduce::FixedOwnedReduceReservationAppendPermitV8;
use super::live_upstream::effect::authorization::cleanup::FixedOwnedEffectCleanupAppendPermitV8;
use super::live_upstream::effect::authorization::failed_state::FixedFailedEffectStateAppendPermitV8;
use super::live_upstream::effect::authorization::step::r#continue::FixedOwnedContinueAppendPermitV8;
use super::live_upstream::effect::authorization::step::FixedOwnedStepAppendPermitV8;
use super::live_upstream::effect::authorization::FixedOwnedEffectIntentAppendPermitV8;
use super::live_upstream::effect::authorization::FixedOwnedEffectSettlementAppendPermitV8;
use super::*;
use crate::resumable_effects::owned_frame::SourceOwnedWaitLeaseV8;

enum ProducerV8<'p, 'j> {
    Continue(
        &'p SourceOwnedWaitJournalV8,
        &'p FixedOwnedContinueAppendPermitV8<'p, 'j>,
    ),
    FailedState(
        &'p SourceOwnedWaitJournalV8,
        &'p FixedFailedEffectStateAppendPermitV8<'p, 'j>,
    ),
    Step(
        &'p SourceOwnedWaitJournalV8,
        &'p FixedOwnedStepAppendPermitV8<'p, 'j>,
    ),
    Cleanup(
        &'p SourceOwnedWaitJournalV8,
        &'p FixedOwnedEffectCleanupAppendPermitV8<'p, 'j>,
    ),
    OriginalReduce(
        &'p SourceOwnedWaitJournalV8,
        &'p FixedOwnedReduceReservationAppendPermitV8<'p, 'j>,
    ),
    Generic,
    Intent(
        &'p SourceOwnedWaitJournalV8,
        &'p FixedOwnedEffectIntentAppendPermitV8<'p, 'j>,
    ),
    Settlement(
        &'p SourceOwnedWaitJournalV8,
        &'p FixedOwnedEffectSettlementAppendPermitV8<'p, 'j>,
    ),
}
enum ContextV8<'a> {
    Checked(&'a CheckedOwnedWaitJournalContextV8),
    #[cfg(test)]
    Synthetic(&'a FoldContextV8),
}
impl ContextV8<'_> {
    fn fold(&self) -> &FoldContextV8 {
        match self {
            Self::Checked(context) => context.fold(),
            #[cfg(test)]
            Self::Synthetic(context) => context,
        }
    }
}
/// Exact acknowledged inventory; no caller-shaped validated entries are admitted.
pub(super) struct InventoryV8<'a> {
    context: ContextV8<'a>,
    key: &'a SourceCheckpointKey,
    entries: Vec<ValidatedEntryV8>,
    invocation: String,
    generation: String,
    mac: String,
    document: Vec<u8>,
}
pub(super) struct CandidateV8<'a> {
    inventory: InventoryV8<'a>,
    row: ValidatedEntryV8,
    encoded: Vec<u8>,
    successor_mac: String,
}
pub(super) struct PendingV8<'a>(CandidateV8<'a>);
/// Append uncertainty permanently retires the retained data; no retry/extraction.
pub(super) struct PoisonedV8<'a> {
    _pending: PendingV8<'a>,
}
pub(super) struct CandidateRejectionV8<'a> {
    pub inventory: InventoryV8<'a>,
    pub row: EntryV8,
    pub error: SourceJournalError,
    pub physical: bool,
}
pub(super) struct AckRejectionV8<'a> {
    pub error: SourceJournalError,
    _poisoned: PoisonedV8<'a>,
}
/// No production constructor. Only a separately leased trusted physical adapter
/// may mint an ACK after the same-store append/sync acknowledgment.
pub(super) struct TrustedAppendAckV8 {
    invocation: String,
    generation: String,
    predecessor_seq: usize,
    predecessor_mac: String,
    successor_mac: String,
    encoded_bytes: usize,
}
impl<'a> InventoryV8<'a> {
    /// Immutable context membership only; synthetic unit inventories are excluded.
    pub(super) fn belongs_to_context(&self, expected: &CheckedOwnedWaitJournalContextV8) -> bool {
        match self.context {
            ContextV8::Checked(actual) => std::ptr::eq(actual, expected),
            #[cfg(test)]
            ContextV8::Synthetic(_) => false,
        }
    }
    pub(super) fn recover(
        context: &'a CheckedOwnedWaitJournalContextV8,
        lease: &SourceOwnedWaitLeaseV8,
        key: &'a SourceCheckpointKey,
        document: &[u8],
    ) -> Result<Self, SourceJournalError> {
        let checked = inventory::checked_inventory_v8(context, lease, key, document)?;
        let (entries, mac) = checked.into_parts();
        Ok(Self {
            context: ContextV8::Checked(context),
            key,
            entries,
            invocation: context.ordinary().invocation().to_owned(),
            generation: context.generation().to_owned(),
            mac,
            document: document.to_vec(),
        })
    }
    pub(super) fn fresh(
        context: &'a CheckedOwnedWaitJournalContextV8,
        lease: &SourceOwnedWaitLeaseV8,
        key: &'a SourceCheckpointKey,
    ) -> Result<Self, SourceJournalError> {
        Self::recover(context, lease, key, &[])
    }
    #[cfg(test)]
    pub(super) fn fold_for_live_test(&self) -> fold::FoldV8 {
        fold::fold(self.context.fold(), &self.entries).expect("actual ACKed inventory")
    }
    pub(super) fn live_start_checkpoint_basis(
        &self,
        observation: &crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8,
    ) -> Result<(u64, u64, String), SourceJournalError> {
        let folded = fold::fold(self.context.fold(), &self.entries)?;
        if folded.tail != fold::TailV8::StartReserved {
            return Err(SourceJournalError::Order);
        }
        let [.., created, reserved] = self.entries.as_slice() else {
            return Err(SourceJournalError::Order);
        };
        let EntryV8::Owned(model::OwnedBodyV8::OwnedWaitReserved {
            turn: 0,
            attempt: 0,
            wait,
            phase: model::PhaseV8::Start,
            replay_of: None,
            ..
        }) = &reserved.entry
        else {
            return Err(SourceJournalError::Order);
        };
        let EntryV8::Owned(model::OwnedBodyV8::OwnedWaitCreated {
            turn: 0,
            attempt: 0,
            wait: created_wait,
            argument_digest,
            copy_arguments,
            ..
        }) = &created.entry
        else {
            return Err(SourceJournalError::Order);
        };
        if wait != created_wait || copy_arguments != observation.copy_arguments() {
            return Err(SourceJournalError::Binding);
        }
        Ok((
            folded.reserved_total,
            folded.consumed_recorded,
            argument_digest.clone(),
        ))
    }
    // Authenticated current fold only; this returns no live credit/token.
    pub(super) fn prospective_reduce_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32), SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        if folded.tail != fold::TailV8::ReadyPair {
            return Err(SourceJournalError::Order);
        }
        let Some(EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed {
            turn,
            attempt,
            ..
        })) = self.entries.last().map(|e| &e.entry)
        else {
            return Err(SourceJournalError::Order);
        };
        capacity::outstanding(context, &folded)?.check(self.document.len(), self.entries.len())?;
        Ok((folded.reserved_total, folded.stages, *turn, *attempt))
    }
    pub(super) fn effect_intent_reduce_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32, &EntryV8), SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        if folded.tail != fold::TailV8::EffectInDoubt {
            return Err(SourceJournalError::Order);
        }
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let EntryV8::Ordinary(SourceJournalEntry::EffectIntent { turn, attempt, .. }) = selected
        else {
            return Err(SourceJournalError::Order);
        };
        capacity::outstanding(context, &folded)?.check(self.document.len(), self.entries.len())?;
        Ok((
            folded.reserved_total,
            folded.stages,
            *turn,
            *attempt,
            selected,
        ))
    }
    // Only exact authenticated current effect settlement phases, no live ACK.
    pub(super) fn effect_settlement_reduce_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32, &EntryV8), SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let (turn, attempt) = match (folded.tail, selected) {
            (
                fold::TailV8::EffectSettlementUncommitted,
                EntryV8::Ordinary(
                    SourceJournalEntry::EffectObserved { turn, attempt, .. }
                    | SourceJournalEntry::EffectFailed { turn, attempt, .. },
                ),
            ) => (*turn, *attempt),
            (
                fold::TailV8::EffectSettled,
                EntryV8::Owned(model::OwnedBodyV8::OwnedEffectSettlementRecorded {
                    turn,
                    attempt,
                    ..
                }),
            ) => (*turn, *attempt),
            _ => return Err(SourceJournalError::Order),
        };
        capacity::outstanding(context, &folded)?.check(self.document.len(), self.entries.len())?;
        Ok((
            folded.reserved_total,
            folded.stages,
            turn,
            attempt,
            selected,
        ))
    }
    pub(super) fn effect_cleanup_reduce_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32, &EntryV8), SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let (turn, attempt) = match (folded.tail, selected) {
            (
                fold::TailV8::EffectCleanupInDoubt,
                EntryV8::Owned(model::OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                    turn,
                    attempt,
                    ..
                }),
            ) => (*turn, *attempt),
            (
                fold::TailV8::EffectDecisionReleased
                | fold::TailV8::EffectFailedState
                | fold::TailV8::EffectCleanupFailed,
                EntryV8::Owned(model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                    turn,
                    attempt,
                    ..
                }),
            ) => (*turn, *attempt),
            _ => return Err(SourceJournalError::Order),
        };
        capacity::outstanding(context, &folded)?.check(self.document.len(), self.entries.len())?;
        Ok((
            folded.reserved_total,
            folded.stages,
            turn,
            attempt,
            selected,
        ))
    }
    /// Authenticated successful effect cleanup only, never a failed target or
    /// failed observer. This borrows proof data and creates no owner or ACK.
    pub(super) fn released_reduce_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32, &EntryV8), SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        if folded.tail != fold::TailV8::EffectDecisionReleased {
            return Err(SourceJournalError::Order);
        }
        self.effect_cleanup_reduce_facts()
    }
    /// Full authenticated fold at the original Reduce reservation only. This
    /// borrows proof data, never a stage budget, owner or restoration permit.
    pub(super) fn original_reduce_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32, &EntryV8), SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        if folded.tail != fold::TailV8::Reduce
            || !folded
                .reduce_fold()
                .is_some_and(|r| r.tail() == reduce_fold::ReduceTailV8::Charged)
        {
            return Err(SourceJournalError::Order);
        }
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let EntryV8::Ordinary(SourceJournalEntry::StageReservation {
            turn: 0,
            attempt: Some(attempt),
            role: super::super::SourceStageRole::Reduce,
            fuel,
        }) = selected
        else {
            return Err(SourceJournalError::Order);
        };
        if Some(*fuel) != context.ordinary.max_steps_per_stage() {
            return Err(SourceJournalError::Binding);
        }
        capacity::outstanding(context, &folded)?.check(self.document.len(), self.entries.len())?;
        Ok((folded.reserved_total, folded.stages, 0, *attempt, selected))
    }
    /// Authenticated first-turn Reduce lineage and complete closure check.
    /// Includes charged original and all closed Step/Stop phases, not owners.
    /// Authenticated closed continuation prefix, never owner authority.
    pub(super) fn continuation_facts(
        &self,
    ) -> Result<(u64, u32, u32, &EntryV8), SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        if !context.cumulative_initialization
            || !folded.continuation_profile_selected()
            || folded.failure_selected()
        {
            return Err(SourceJournalError::Order);
        }
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        match selected {
            EntryV8::Ordinary(SourceJournalEntry::Transition {
                turn,
                case: super::super::SourceTransitionCase::Continue,
                ..
            }) if *turn == folded.current_turn()
                && folded.tail == fold::TailV8::Reduce
                && folded
                    .reduce_fold()
                    .is_some_and(|r| r.tail() == super::reduce_fold::ReduceTailV8::Continued) => {}
            EntryV8::Owned(model::OwnedBodyV8::OwnedStateCommitted { turn, .. })
                if *turn == folded.current_turn()
                    && *turn > 0
                    && folded.tail == fold::TailV8::CommittedState => {}
            EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn,
                attempt: None,
                role: super::super::SourceStageRole::Observe,
                fuel,
            }) if *turn == folded.current_turn()
                && *turn > 0
                && folded.tail == fold::TailV8::ObserveReserved
                && Some(*fuel) == context.ordinary.max_steps_per_stage() => {}
            _ => return Err(SourceJournalError::Order),
        }
        capacity::outstanding(context, &folded)?.check(self.document.len(), self.entries.len())?;
        Ok((
            folded.reserved_total,
            folded.stages,
            folded.current_turn(),
            selected,
        ))
    }
    pub(super) fn step_reduce_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32, &EntryV8), SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        if folded.tail != fold::TailV8::Reduce || folded.reduce_fold().is_none() {
            return Err(SourceJournalError::Order);
        }
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let (turn, attempt) = match selected {
            EntryV8::Owned(
                model::OwnedBodyV8::OwnedReduceStaged { turn, attempt, .. }
                | model::OwnedBodyV8::OwnedReduceCleanupStarted { turn, attempt, .. }
                | model::OwnedBodyV8::OwnedReduceCleanupSettled { turn, attempt, .. }
                | model::OwnedBodyV8::OwnedStepTransferReserved { turn, attempt, .. }
                | model::OwnedBodyV8::OwnedStepTransferCompleted { turn, attempt, .. },
            ) => (*turn, *attempt),
            EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn,
                attempt: Some(attempt),
                role: super::super::SourceStageRole::Reduce,
                fuel,
            }) if Some(*fuel) == context.ordinary.max_steps_per_stage() => (*turn, *attempt),
            EntryV8::Ordinary(SourceJournalEntry::Transition { turn, attempt, .. }) => {
                (*turn, *attempt)
            }
            EntryV8::Ordinary(SourceJournalEntry::Stop {
                turn: Some(turn),
                attempt: Some(attempt),
                ..
            }) => (*turn, *attempt),
            _ => return Err(SourceJournalError::Order),
        };
        if turn != 0 {
            return Err(SourceJournalError::Binding);
        }
        capacity::outstanding(context, &folded)?.check(self.document.len(), self.entries.len())?;
        Ok((
            folded.reserved_total,
            folded.stages,
            turn,
            attempt,
            selected,
        ))
    }
    /// Exact authenticated target-failure State closure, never failed observer
    /// receipt or successful effect/Reduce. This yields no physical authority.
    pub(super) fn failed_effect_state_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32, &EntryV8), SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let (turn, attempt) = match (folded.tail, selected) {
            (
                fold::TailV8::EffectFailedState,
                EntryV8::Owned(model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                    turn,
                    attempt,
                    receipt,
                    ..
                }),
            ) if receipt["settlement"] == "completed" => (*turn, *attempt),
            (
                fold::TailV8::Reduce,
                EntryV8::Owned(
                    model::OwnedBodyV8::OwnedEffectFailureStateCleanupStarted {
                        turn, attempt, ..
                    }
                    | model::OwnedBodyV8::OwnedEffectFailureStateCleanupSettled {
                        turn, attempt, ..
                    },
                ),
            ) if folded.failed_effect_state_fold().is_some() => (*turn, *attempt),
            (
                fold::TailV8::Reduce,
                EntryV8::Ordinary(SourceJournalEntry::Stop {
                    turn: Some(turn),
                    attempt: Some(attempt),
                    status: super::super::SourceStopStatus::EffectFailed,
                    reason: super::super::SourceStopReason::EffectFailed,
                }),
            ) if folded.failed_effect_state_fold().is_some() => (*turn, *attempt),
            _ => return Err(SourceJournalError::Order),
        };
        if turn != 0 {
            return Err(SourceJournalError::Binding);
        }
        capacity::outstanding(context, &folded)?.check(self.document.len(), self.entries.len())?;
        Ok((
            folded.reserved_total,
            folded.stages,
            turn,
            attempt,
            selected,
        ))
    }
    pub(super) fn sequence(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn acknowledged_bytes(&self) -> usize {
        self.document.len()
    }
    pub(super) fn authentication_tail(&self) -> &str {
        &self.mac
    }
    pub(super) fn prepare(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        row: EntryV8,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        #[cfg(test)]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::Generic, None)
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::Generic)
        }
    }
    pub(super) fn prepare_fixed_intent(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedEffectIntentAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::Intent(journal, permit), None)
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::Intent(journal, permit))
        }
    }
    pub(super) fn prepare_fixed_settlement(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedEffectSettlementAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::Settlement(journal, permit),
                None,
            )
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::Settlement(journal, permit))
        }
    }
    pub(super) fn prepare_fixed_cleanup(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedEffectCleanupAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::Cleanup(journal, permit), None)
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::Cleanup(journal, permit))
        }
    }
    pub(super) fn prepare_fixed_original_reduce(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedReduceReservationAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::OriginalReduce(journal, permit),
                None,
            )
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::OriginalReduce(journal, permit),
            )
        }
    }
    pub(super) fn prepare_fixed_continue(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedContinueAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::Continue(journal, permit),
                None,
            )
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::Continue(journal, permit))
        }
    }
    pub(super) fn prepare_fixed_step(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedStepAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::Step(journal, permit), None)
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::Step(journal, permit))
        }
    }
    pub(super) fn prepare_fixed_failed_state(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedFailedEffectStateAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::FailedState(journal, permit),
                None,
            )
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(row, Some(lease), ProducerV8::FailedState(journal, permit))
        }
    }
    fn prepare_inner(
        mut self,
        row: EntryV8,
        lease: Option<&SourceOwnedWaitLeaseV8>,
        producer: ProducerV8<'_, '_>,
        #[cfg(test)] synthetic: Option<ValidatedEntryV8>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let mut physical = false;
        let result = (|| {
            let context = self.context.fold();
            let expected = ExpectedRowV8 {
                invocation: &self.invocation,
                generation: &self.generation,
                seq: u32::try_from(self.entries.len()).map_err(|_| SourceJournalError::Capacity)?,
                prev_mac: &self.mac,
                ordinary: &context.ordinary,
            };
            let encoded = wire::encode(&row, &expected, self.key)?;
            let (checked, successor_mac) = match &self.context {
                ContextV8::Checked(context) => {
                    let lease = lease.ok_or(SourceJournalError::Binding)?;
                    // No supplied proof pairs: authenticate the original ACK
                    // prefix and derive this row's facts from that same history.
                    let checked = inventory::checked_candidate_inventory_tagged_v8(
                        context,
                        lease,
                        self.key,
                        &self.document,
                        &encoded,
                    )
                    .map_err(|failure| {
                        physical =
                            matches!(&failure, inventory::InventoryValidationErrorV8::Physical(_));
                        failure.error()
                    })?;
                    let (mut entries, mac) = checked.into_parts();
                    if entries.len() != self.entries.len() + 1 {
                        return Err(SourceJournalError::Binding);
                    }
                    (entries.pop().ok_or(SourceJournalError::Binding)?, mac)
                }
                #[cfg(test)]
                ContextV8::Synthetic(_) => {
                    let checked = synthetic.ok_or(SourceJournalError::Binding)?;
                    if checked.entry != row {
                        return Err(SourceJournalError::Binding);
                    }
                    let envelope = wire::parse(&encoded[..encoded.len() - 1])?;
                    (
                        checked,
                        envelope["authentication"]
                            .as_str()
                            .ok_or(SourceJournalError::Malformed)?
                            .to_owned(),
                    )
                }
            };
            let previous = fold::fold(context, &self.entries)?;
            match producer {
                ProducerV8::Generic => fold::validate_producer_transition(&previous, &checked)?,
                ProducerV8::Intent(journal, permit) => {
                    if checked.entry != *permit.selected_row() {
                        return Err(SourceJournalError::Binding);
                    }
                    permit.validate_selected_prefix(journal, &self)?;
                }
                ProducerV8::Settlement(journal, permit) => {
                    if checked.entry != *permit.selected_row() {
                        return Err(SourceJournalError::Binding);
                    }
                    permit.validate_selected_prefix(journal, &self)?;
                }
                ProducerV8::Cleanup(journal, permit) => {
                    if checked.entry != *permit.selected_row() {
                        return Err(SourceJournalError::Binding);
                    }
                    permit.validate_selected_prefix(journal, &self)?;
                }
                ProducerV8::OriginalReduce(journal, permit) => {
                    if checked.entry != *permit.selected_row() {
                        return Err(SourceJournalError::Binding);
                    }
                    permit.validate_selected_prefix(journal, &self)?;
                }
                ProducerV8::Continue(journal, permit) => {
                    if checked.entry != *permit.selected_row() {
                        return Err(SourceJournalError::Binding);
                    }
                    permit.validate_selected_prefix(journal, &self)?;
                }
                ProducerV8::Step(journal, permit) => {
                    if checked.entry != *permit.selected_row() {
                        return Err(SourceJournalError::Binding);
                    }
                    permit.validate_selected_prefix(journal, &self)?;
                }
                ProducerV8::FailedState(journal, permit) => {
                    if checked.entry != *permit.selected_row() {
                        return Err(SourceJournalError::Binding);
                    }
                    permit.validate_selected_prefix(journal, &self)?;
                }
            }
            self.entries.push(checked);
            let next = fold::fold(context, &self.entries);
            let row = self.entries.pop().expect("prospective row retained");
            let next = next?;
            let bytes = self
                .document
                .len()
                .checked_add(encoded.len())
                .ok_or(SourceJournalError::Capacity)?;
            capacity::outstanding(context, &next)?.check(bytes, self.entries.len() + 1)?;
            // Allocate the future ACK backing before Pending can expose bytes.
            self.document
                .try_reserve(encoded.len())
                .map_err(|_| SourceJournalError::Capacity)?;
            Ok((row, encoded, successor_mac))
        })();
        match result {
            Ok((row, encoded, successor_mac)) => Ok(CandidateV8 {
                inventory: self,
                row,
                encoded,
                successor_mac,
            }),
            Err(error) => Err(CandidateRejectionV8 {
                inventory: self,
                row,
                error,
                physical,
            }),
        }
    }
    #[cfg(test)]
    pub(super) fn synthetic_fresh(
        context: &'a FoldContextV8,
        key: &'a SourceCheckpointKey,
    ) -> Result<Self, SourceJournalError> {
        Self::synthetic_recover(context, key, &[], Vec::new())
    }
    #[cfg(test)]
    pub(super) fn synthetic_recover(
        context: &'a FoldContextV8,
        key: &'a SourceCheckpointKey,
        document: &[u8],
        entries: Vec<ValidatedEntryV8>,
    ) -> Result<Self, SourceJournalError> {
        let model::OwnedBodyV8::OwnedRunCreated {
            execution, binding, ..
        } = &context.created
        else {
            return Err(SourceJournalError::Binding);
        };
        let invocation = wire::recipe_digest(
            wire::RecipeV8::Invocation,
            &serde_json::json!({"execution":execution,"owned_wait_binding":binding}),
        )?;
        let generation = wire::generation_digest_from_created(&context.created)?;
        let zero = "0".repeat(64);
        let decoded = wire::decode_inventory(
            document,
            &ExpectedRowV8 {
                invocation: &invocation,
                generation: &generation,
                seq: 0,
                prev_mac: &zero,
                ordinary: &context.ordinary,
            },
            key,
        )?;
        if decoded.len() != entries.len()
            || decoded.iter().zip(&entries).any(|(a, b)| a != &b.entry)
        {
            return Err(SourceJournalError::Binding);
        }
        fold::fold(context, &entries)?;
        let mac = if document.is_empty() {
            zero
        } else {
            let last = document
                .strip_suffix(b"\n")
                .ok_or(SourceJournalError::Malformed)?
                .rsplit(|b| *b == b'\n')
                .next()
                .ok_or(SourceJournalError::Malformed)?;
            wire::parse(last)?["authentication"]
                .as_str()
                .ok_or(SourceJournalError::Malformed)?
                .to_owned()
        };
        Ok(Self {
            context: ContextV8::Synthetic(context),
            key,
            entries,
            invocation,
            generation,
            mac,
            document: document.to_vec(),
        })
    }
    #[cfg(test)]
    pub(super) fn synthetic_prepare(
        self,
        row: ValidatedEntryV8,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        self.prepare_inner(row.entry.clone(), None, ProducerV8::Generic, Some(row))
    }
}
impl<'a> CandidateV8<'a> {
    /// This consuming transition must happen before the first physical call.
    pub(super) fn into_pending(self) -> PendingV8<'a> {
        PendingV8(self)
    }
}
impl<'a> PendingV8<'a> {
    pub(super) fn validate_fixed_intent_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedEffectIntentAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
    pub(super) fn validate_fixed_settlement_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedEffectSettlementAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
    pub(super) fn validate_fixed_cleanup_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedEffectCleanupAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
    pub(super) fn validate_fixed_original_reduce_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedReduceReservationAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
    pub(super) fn validate_fixed_continue_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedContinueAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
    pub(super) fn validate_fixed_step_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedOwnedStepAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
    /// Only Pending exposes bytes to the future adapter; Candidate cannot I/O.
    pub(super) fn bytes(&self) -> &[u8] {
        &self.0.encoded
    }
    pub(super) fn check_prefix(
        &self,
        lease: &SourceOwnedWaitLeaseV8,
        bytes: &[u8],
    ) -> Result<(), SourceJournalError> {
        let inventory = &self.0.inventory;
        if bytes != inventory.document {
            return Err(SourceJournalError::Binding);
        }
        let ContextV8::Checked(context) = &inventory.context else {
            return Err(SourceJournalError::Binding);
        };
        let checked = super::inventory::checked_inventory_v8(context, lease, inventory.key, bytes)?;
        let (entries, mac) = checked.into_parts();
        if entries.len() != inventory.entries.len() || mac != inventory.mac {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(super) fn check_written(
        &self,
        lease: &SourceOwnedWaitLeaseV8,
        bytes: &[u8],
    ) -> Result<(), SourceJournalError> {
        let inventory = &self.0.inventory;
        let total = inventory
            .document
            .len()
            .checked_add(self.0.encoded.len())
            .ok_or(SourceJournalError::Capacity)?;
        if bytes.len() != total
            || !bytes.starts_with(&inventory.document)
            || bytes[inventory.document.len()..] != self.0.encoded
        {
            return Err(SourceJournalError::Binding);
        }
        let ContextV8::Checked(context) = &inventory.context else {
            return Err(SourceJournalError::Binding);
        };
        let checked = super::inventory::checked_inventory_v8(context, lease, inventory.key, bytes)?;
        let (entries, mac) = checked.into_parts();
        if entries.len() != inventory.entries.len() + 1 || mac != self.0.successor_mac {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    /// Only the fixed adapter can construct this sealed postappend witness.
    pub(super) fn validate_fixed_failed_state_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &FixedFailedEffectStateAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
    pub(super) fn acknowledge_verified(
        self,
        _verified: super::append::AppendVerifiedV8,
    ) -> InventoryV8<'a> {
        self.finish_ack()
    }
    pub(super) fn poison(self) -> PoisonedV8<'a> {
        PoisonedV8 { _pending: self }
    }
    pub(super) fn acknowledge(
        self,
        ack: TrustedAppendAckV8,
    ) -> Result<InventoryV8<'a>, AckRejectionV8<'a>> {
        let candidate = &self.0;
        if ack.invocation != candidate.inventory.invocation
            || ack.generation != candidate.inventory.generation
            || ack.predecessor_seq != candidate.inventory.entries.len()
            || ack.predecessor_mac != candidate.inventory.mac
            || ack.successor_mac != candidate.successor_mac
            || ack.encoded_bytes != candidate.encoded.len()
        {
            return Err(AckRejectionV8 {
                error: SourceJournalError::Binding,
                _poisoned: self.poison(),
            });
        }
        Ok(self.finish_ack())
    }
    fn finish_ack(self) -> InventoryV8<'a> {
        let CandidateV8 {
            mut inventory,
            row,
            encoded,
            successor_mac,
        } = self.0;
        inventory.document.extend_from_slice(&encoded); // reserved before Pending
        inventory.mac = successor_mac;
        inventory.entries.push(row); // capacity retained by the preflight push/pop
        inventory
    }
    #[cfg(test)]
    pub(super) fn synthetic_ack_for_inert_test(&self) -> TrustedAppendAckV8 {
        let candidate = &self.0;
        TrustedAppendAckV8 {
            invocation: candidate.inventory.invocation.clone(),
            generation: candidate.inventory.generation.clone(),
            predecessor_seq: candidate.inventory.entries.len(),
            predecessor_mac: candidate.inventory.mac.clone(),
            successor_mac: candidate.successor_mac.clone(),
            encoded_bytes: candidate.encoded.len(),
        }
    }
}

#[cfg(test)]
impl TrustedAppendAckV8 {
    pub(super) fn alter_predecessor_for_inert_test(&mut self) {
        self.predecessor_seq += 1;
    }
}
