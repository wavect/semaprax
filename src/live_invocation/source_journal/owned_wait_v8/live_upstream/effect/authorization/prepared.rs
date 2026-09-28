//! Actual Consumed lineage plus its exclusive future Reduce hold. No target entry.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
    OwnedEffectInputsV8, PreparedOwnedEffectV8,
};
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::LiveReadyEffectPreparationRejectionV8;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::ProspectiveOwnedReduceHoldV8;

/// Borrowed only during the actual consuming envelope's interpreter handoff.
/// Neither a raw row nor an unheld Consumed envelope constructs this permit.
pub(crate) struct LiveEffectAuthorizationPermitV8<'p, 'j> {
    journal: &'j SourceOwnedWaitJournalV8,
    witness: &'p VerifiedOwnedAuthorizationConsumedSuccessorV8<'j>,
    selected: &'p EntryV8,
    hold: &'p ProspectiveOwnedReduceHoldV8<'j>,
    predecessor_sequence: usize,
    predecessor_bytes: usize,
    staged: u32,
    ready: u32,
    consumed: u32,
    commitments: &'p CheckedOwnedWaitReadyCommitmentsV8,
    proposal: &'p crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8,
    policy: &'j CapabilityPolicy,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
    clock: &'j dyn crate::live_invocation::SourceInvocationClock,
}
impl LiveEffectAuthorizationPermitV8<'_, '_> {
    pub(crate) fn validate_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_current()?;
        let (runtime, execution) = self
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(execution, inputs.execution)
            || !std::ptr::eq(self.policy, inputs.policy)
            || !std::ptr::eq(self.cancellation, inputs.cancellation)
            || inputs.turn != 0
            || inputs.attempt != 0
            || inputs.proposal.carrier() != self.proposal.carrier()
            || inputs.proposal.ordinary_digest() != self.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        inputs
            .store
            .validate_prefix(self.witness.sequence(), self.witness.acknowledged_bytes())?;
        self.validate_current()
    }
    pub(crate) fn validate_current(&self) -> Result<(), SourceJournalError> {
        self.witness.validate_predecessor(
            self.journal,
            self.predecessor_sequence,
            self.predecessor_bytes,
            self.selected,
        )?;
        self.hold.validate_guard(
            self.journal,
            self.witness.sequence(),
            self.witness.acknowledged_bytes(),
        )?;
        let context = self.journal.context();
        let (runtime, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        let held = self.journal.hold()?;
        check_clock_v8(
            &held,
            self.witness.sequence(),
            self.witness.acknowledged_bytes(),
            self.cancellation,
            self.clock,
            context.ordinary().clock_domain(),
            context.ordinary().initial_millis(),
            context.ordinary().deadline_millis(),
        )?;
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &held.registration().expected_facts().scope,
            self.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !self.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        self.hold.validate_guard(
            self.journal,
            self.witness.sequence(),
            self.witness.acknowledged_bytes(),
        )?;
        self.witness.validate_predecessor(
            self.journal,
            self.predecessor_sequence,
            self.predecessor_bytes,
            self.selected,
        )
    }
    pub(crate) fn references(&self) -> (u32, u32, u32) {
        (self.staged, self.ready, self.consumed)
    }
    pub(crate) fn matches_commitments(&self, actual: &CheckedOwnedWaitReadyCommitmentsV8) -> bool {
        actual.authorization_binding() == self.commitments.authorization_binding()
            && actual.grant_digest() == self.commitments.grant_digest()
            && actual.target_grant_digest() == self.commitments.target_grant_digest()
            && actual.argument_digest() == self.commitments.argument_digest()
            && actual.budget() == self.commitments.budget()
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LivePreparedOwnedEffectV8<'j> {
    // Actual State/Decision and Held store backing precede the remaining lineage.
    prepared: PreparedOwnedEffectV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedAuthorizationConsumedSuccessorV8<'j>,
    hold: ProspectiveOwnedReduceHoldV8<'j>,
    journal: &'j SourceOwnedWaitJournalV8,
    selected: EntryV8,
    predecessor_sequence: usize,
    predecessor_bytes: usize,
    staged: u32,
    ready: u32,
    consumed: u32,
    commitments: CheckedOwnedWaitReadyCommitmentsV8,
    proposal: crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8,
    policy: &'j CapabilityPolicy,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
    clock: &'j dyn crate::live_invocation::SourceInvocationClock,
}
impl LivePreparedOwnedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let permit = LiveEffectAuthorizationPermitV8 {
            journal: self.journal,
            witness: &self.witness,
            selected: &self.selected,
            hold: &self.hold,
            predecessor_sequence: self.predecessor_sequence,
            predecessor_bytes: self.predecessor_bytes,
            staged: self.staged,
            ready: self.ready,
            consumed: self.consumed,
            commitments: &self.commitments,
            proposal: &self.proposal,
            policy: self.policy,
            cancellation: self.cancellation,
            clock: self.clock,
        };
        let result = if !self.session.belongs_to(self.journal)
            || self.session.sequence() != self.witness.sequence()
            || self.session.acknowledged_bytes() != self.witness.acknowledged_bytes()
        {
            Err(SourceJournalError::Binding)
        } else {
            self.prepared.validate_live_authorization(&permit)
        };
        if result.is_err() {
            self.prepared.quarantine_live_authorization();
        }
        result
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveEffectAuthorizationFailureV8<
    'j,
> {
    Before {
        _obligation: LiveAuthorizationConsumedAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedOwnedAuthorizationConsumedSuccessorV8<'j>,
        _hold: ProspectiveOwnedReduceHoldV8<'j>,
        error: SourceJournalError,
    },
    Preparation {
        _owner: LiveReadyEffectPreparationRejectionV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedOwnedAuthorizationConsumedSuccessorV8<'j>,
        _hold: ProspectiveOwnedReduceHoldV8<'j>,
        error: SourceJournalError,
    },
    After {
        _owner: LivePreparedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
}
/// Called only by the actual held Consumed bundle's private consuming delegate.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_authorization_v8<
    'j,
>(
    obligation: LiveAuthorizationConsumedAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedAuthorizationConsumedSuccessorV8<'j>,
    hold: ProspectiveOwnedReduceHoldV8<'j>,
) -> Result<LivePreparedOwnedEffectV8<'j>, LiveEffectAuthorizationFailureV8<'j>> {
    let before = obligation
        .validate_consumed_successor(&witness)
        .and_then(|_| {
            if !session.belongs_to(obligation.journal)
                || session.sequence() != witness.sequence()
                || session.acknowledged_bytes() != witness.acknowledged_bytes()
            {
                return Err(SourceJournalError::Binding);
            }
            hold.validate_guard(
                obligation.journal,
                witness.sequence(),
                witness.acknowledged_bytes(),
            )
        });
    if let Err(error) = before {
        obligation.held.quarantine();
        return Err(LiveEffectAuthorizationFailureV8::Before {
            _obligation: obligation,
            _session: session,
            _witness: witness,
            _hold: hold,
            error,
        });
    }
    let predecessor_sequence = obligation.sequence();
    let predecessor_bytes = obligation.acknowledged_bytes();
    let ready = obligation
        .witness
        .sequence()
        .checked_sub(1)
        .and_then(|n| u32::try_from(n).ok());
    let consumed = witness
        .sequence()
        .checked_sub(1)
        .and_then(|n| u32::try_from(n).ok());
    let (Some(ready), Some(consumed)) = (ready, consumed) else {
        obligation.held.quarantine();
        return Err(LiveEffectAuthorizationFailureV8::Before {
            _obligation: obligation,
            _session: session,
            _witness: witness,
            _hold: hold,
            error: SourceJournalError::Capacity,
        });
    };
    let LiveAuthorizationConsumedAppendV8 {
        owner,
        held,
        journal,
        selected,
        policy,
        proposal,
        commitments,
        cancellation,
        clock,
        staged,
        ..
    } = obligation;
    let retained_proposal = proposal.clone();
    let permit = LiveEffectAuthorizationPermitV8 {
        journal,
        witness: &witness,
        selected: &selected,
        hold: &hold,
        predecessor_sequence,
        predecessor_bytes,
        staged,
        ready,
        consumed,
        commitments: &commitments,
        proposal: &retained_proposal,
        policy,
        cancellation,
        clock,
    };
    // Checked by the original obligation before it was consumed; the retained
    // context owns both borrowers for the entire same-journal holder lifetime.
    let (runtime, execution) = journal
        .context()
        .ready_runtime()
        .expect("validated retained runtime");
    let inputs = OwnedEffectInputsV8 {
        runtime,
        execution,
        proposal,
        store: held,
        policy,
        cancellation,
        turn: 0,
        attempt: 0,
    };
    let prepared = match owner.prepare_live_effect_v8(inputs, &permit) {
        Ok(prepared) => prepared,
        Err(rejected) => {
            rejected.quarantine();
            let error = rejected.error();
            return Err(LiveEffectAuthorizationFailureV8::Preparation {
                _owner: rejected,
                _session: session,
                _witness: witness,
                _hold: hold,
                error,
            });
        }
    };
    let actual = LivePreparedOwnedEffectV8 {
        prepared,
        session,
        witness,
        hold,
        journal,
        selected,
        predecessor_sequence,
        predecessor_bytes,
        staged,
        ready,
        consumed,
        commitments,
        proposal: retained_proposal,
        policy,
        cancellation,
        clock,
    };
    if let Err(error) = actual.validate_live() {
        return Err(LiveEffectAuthorizationFailureV8::After {
            _owner: actual,
            error,
        });
    }
    Ok(actual)
}

#[cfg(test)]
mod tests;

mod intent;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use intent::{
    advance_verified_intent_v8, FixedOwnedEffectIntentAppendPermitV8, LiveActivatedOwnedEffectV8,
    LiveEffectIntentActivationFailureV8, LiveEffectIntentPreparationFailureV8,
    LiveOwnedEffectIntentAppendV8,
};

pub(crate) use intent::LiveEffectIntentPermitV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use intent::dispatch::settlement::{
    advance_verified_settlement_v8,
    FixedOwnedEffectSettlementAppendPermitV8,
    LiveOwnedEffectSettlementAppendV8,
    LiveEffectSettlementAcknowledgedV8,
    LiveEffectSettlementFailureV8,
    LiveSettledOwnedEffectV8,
    LiveRecordedOwnedEffectV8,
};
