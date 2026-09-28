//! Actual continued Observe owner, ledger and token remain inseparable.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::checked_continued_observe_facts_v8;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedObserveSettlementSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::observe::settlement::{
    select_observe_settlement_v8, LiveObserveSettlementFailureV8, LiveObserveSettlementOwnerV8,
    LiveOwnedObserveSettlementAppendV8, ObserveDataV8,
};
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ContinuedObserveSettlementV8<
    'j,
> {
    owner: LiveObservedContinueV8<'j>,
}
impl<'j> LiveObservedContinueV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_observe_settlement(
        self,
    ) -> Result<LiveOwnedObserveSettlementAppendV8<'j>, LiveObserveSettlementFailureV8<'j>> {
        select_observe_settlement_v8(LiveObserveSettlementOwnerV8::Continued(
            ContinuedObserveSettlementV8 { owner: self },
        ))
    }
}
impl<'j> ContinuedObserveSettlementV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn journal(
        &self,
    ) -> &'j SourceOwnedWaitJournalV8 {
        self.owner.lineage.journal()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner
            .lineage
            .current()
            .expect("actual Observe ACK")
            .sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn bytes(&self) -> usize {
        self.owner
            .lineage
            .current()
            .expect("actual Observe ACK")
            .acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn turn(&self) -> u32 {
        self.owner.turn()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn reservation(&self) -> u32 {
        u32::try_from(self.sequence() - 1).expect("bounded journal")
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn data(
        &self,
    ) -> Result<ObserveDataV8, SourceJournalError> {
        let facts = checked_continued_observe_facts_v8(&self.owner.outcome)?;
        if facts.consumed != self.owner.consumed {
            return Err(SourceJournalError::Binding);
        }
        Ok(ObserveDataV8 {
            state: facts.state,
            observation: facts.observation,
            failure: facts.failure,
            consumed: facts.consumed as u64,
        })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn guard_at(
        &self,
        seq: usize,
        bytes: usize,
    ) -> Result<(), SourceJournalError> {
        let journal = self.journal();
        let origin = self.owner.lineage.step.origin();
        origin
            .hold
            .validate_observe_settlement_guard(journal, seq, bytes)?;
        let held = journal.hold()?;
        let (runtime, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &held.registration().expected_facts().scope,
            &origin.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !origin.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        let ordinary = journal.context().ordinary();
        check_clock_v8(
            &held,
            seq,
            bytes,
            origin.cancellation,
            origin.clock,
            ordinary.clock_domain(),
            ordinary.initial_millis(),
            ordinary.deadline_millis(),
        )?;
        self.data()?;
        held.validate_prefix(seq, bytes)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .lineage
            .step
            .origin()
            .hold
            .validate_observe_settlement_append_prefix(journal, inventory, selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedObserveSettlementSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .lineage
            .step
            .origin()
            .hold
            .advance_observe_settlement_ack(witness, session)
    }
}

#[cfg(test)]
impl ContinuedObserveSettlementV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_accounting(
        &self,
    ) -> &TargetAccounting {
        self.owner.accounting()
    }
}
