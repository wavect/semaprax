//! Consuming Ready promotion. Matching inert history cannot construct a permit.
use super::super::super::append::owned_effect::VerifiedOwnedAuthorizationConsumedSuccessorV8;
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    promote_live_owned_authorization_v8, LiveReadyAuthorizationV8, LiveReadyPromotionOutcomeV8,
};
use crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8;

/// Constructed only while consuming the fixed adapter's actual Ready envelope.
pub(crate) struct LiveReadyPromotionPermitV8<'p, 'j> {
    journal: &'j SourceOwnedWaitJournalV8,
    held: &'p HeldOwnedWaitStoreV8<'j>,
    witness: &'p VerifiedOwnedEffectReadySuccessorV8<'j>,
    selected: &'p EntryV8,
    predecessor_sequence: usize,
    predecessor_bytes: usize,
    policy: &'j CapabilityPolicy,
    proposal: &'p crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
    clock: &'j dyn crate::live_invocation::SourceInvocationClock,
}
impl LiveReadyPromotionPermitV8<'_, '_> {
    pub(crate) fn binding(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        &self.journal.context().fold().checked_binding
    }
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        self.witness.validate_predecessor(
            self.journal,
            self.predecessor_sequence,
            self.predecessor_bytes,
            self.selected,
        )?;
        let context = self.journal.context();
        check_clock_v8(
            self.held,
            self.witness.sequence(),
            self.witness.acknowledged_bytes(),
            self.cancellation,
            self.clock,
            context.ordinary().clock_domain(),
            context.ordinary().initial_millis(),
            context.ordinary().deadline_millis(),
        )?;
        let (runtime, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &self.held.registration().expected_facts().scope,
            self.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !self.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        self.witness.validate_predecessor(
            self.journal,
            self.predecessor_sequence,
            self.predecessor_bytes,
            self.selected,
        )
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveAuthorizationConsumedAppendV8<
    'j,
> {
    // Real backing disposal precedes the held store/session on abandonment.
    owner: LiveReadyAuthorizationV8,
    session: AppendSessionV8<'j>,
    held: HeldOwnedWaitStoreV8<'j>,
    journal: &'j SourceOwnedWaitJournalV8,
    witness: VerifiedOwnedEffectReadySuccessorV8<'j>,
    ready_row: EntryV8,
    selected: EntryV8,
    predecessor_sequence: usize,
    predecessor_bytes: usize,
    policy: &'j CapabilityPolicy,
    proposal: crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8,
    commitments: CheckedOwnedWaitReadyCommitmentsV8,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
    clock: &'j dyn crate::live_invocation::SourceInvocationClock,
    staged: u32,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveReadyAdvanceFailureV8<'j> {
    Before {
        _obligation: LiveOwnedEffectAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedOwnedEffectReadySuccessorV8<'j>,
        error: SourceJournalError,
    },
    Promotion {
        _owner: LiveReadyPromotionOutcomeV8,
        _session: AppendSessionV8<'j>,
        _held: HeldOwnedWaitStoreV8<'j>,
        _witness: VerifiedOwnedEffectReadySuccessorV8<'j>,
        _proposal: crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8,
        error: SourceJournalError,
    },
    After {
        _obligation: LiveAuthorizationConsumedAppendV8<'j>,
        error: SourceJournalError,
    },
}
impl LiveAuthorizationConsumedAppendV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.journal, journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.session.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.session.acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = self.validate_inner();
        if result.is_err() {
            self.held.quarantine();
        }
        result
    }
    /// The fixed adapter alone supplies this actual post-Consumed witness.
    /// The original Ready session and witness remain unchanged lineage facts.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_consumed_successor(
        &self,
        witness: &VerifiedOwnedAuthorizationConsumedSuccessorV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let authenticate = || {
            witness.validate_predecessor(
                self.journal,
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )
        };
        let result = authenticate().and_then(|_| {
            self.validate_at(witness.sequence(), witness.acknowledged_bytes())?;
            authenticate()
        });
        if result.is_err() {
            self.held.quarantine();
        }
        result
    }
    fn validate_inner(&self) -> Result<(), SourceJournalError> {
        let authenticate = || {
            self.witness.validate_predecessor(
                self.journal,
                self.predecessor_sequence,
                self.predecessor_bytes,
                &self.ready_row,
            )
        };
        authenticate()?;
        self.validate_at(self.sequence(), self.acknowledged_bytes())?;
        authenticate()
    }
    fn validate_at(&self, sequence: usize, bytes: usize) -> Result<(), SourceJournalError> {
        // This comparison is inert lineage, not an old-prefix freshness check.
        if self.sequence() != self.witness.sequence()
            || self.acknowledged_bytes() != self.witness.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        self.validate_guard_at(sequence, bytes)?;
        let (state, decision) = self
            .owner
            .checked_facts(&self.journal.context().fold().checked_binding)
            .ok_or(SourceJournalError::Binding)?;
        let (runtime, execution) = self
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let commitments = checked_owned_wait_ready_commitments_v8(
            runtime,
            execution,
            &self.held.registration().expected_facts().scope,
            0,
            0,
            &state,
            &decision,
            &self.proposal,
        )?;
        if commitments.authorization_binding() != self.commitments.authorization_binding()
            || commitments.grant_digest() != self.commitments.grant_digest()
            || commitments.target_grant_digest() != self.commitments.target_grant_digest()
            || commitments.argument_digest() != self.commitments.argument_digest()
            || commitments.budget() != self.commitments.budget()
            || self.selected
                != EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed {
                    turn: 0,
                    attempt: 0,
                    grant_digest: commitments.grant_digest().into(),
                })
        {
            return Err(SourceJournalError::Binding);
        }
        self.validate_guard_at(sequence, bytes)
    }
    fn validate_guard_at(&self, sequence: usize, bytes: usize) -> Result<(), SourceJournalError> {
        let context = self.journal.context();
        check_clock_v8(
            &self.held,
            sequence,
            bytes,
            self.cancellation,
            self.clock,
            context.ordinary().clock_domain(),
            context.ordinary().initial_millis(),
            context.ordinary().deadline_millis(),
        )?;
        let (runtime, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &self.held.registration().expected_facts().scope,
            &self.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !self.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        self.held.validate_prefix(sequence, bytes)
    }
}

/// Root calls this only by consuming its private actual Ready envelope fields.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_ready_v8<'j>(
    obligation: LiveOwnedEffectAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedEffectReadySuccessorV8<'j>,
) -> Result<LiveAuthorizationConsumedAppendV8<'j>, LiveReadyAdvanceFailureV8<'j>> {
    let before = obligation.validate_ready_successor(&witness).and_then(|_| {
        if session.sequence() != witness.sequence()
            || session.acknowledged_bytes() != witness.acknowledged_bytes()
        {
            Err(SourceJournalError::Binding)
        } else {
            Ok(())
        }
    });
    if let Err(error) = before {
        obligation.owner.held.quarantine();
        return Err(LiveReadyAdvanceFailureV8::Before {
            _obligation: obligation,
            _session: session,
            _witness: witness,
            error,
        });
    }
    let LiveOwnedEffectAppendV8 {
        owner,
        policy,
        selected: ready_row,
        commitments,
    } = obligation;
    let predecessor_sequence = owner.session.sequence();
    let predecessor_bytes = owner.session.acknowledged_bytes();
    let StagedLiveOwnedRunV8 {
        owner,
        held,
        journal,
        proposal,
        staged,
        cancellation,
        clock,
        ..
    } = owner;
    let permit = LiveReadyPromotionPermitV8 {
        journal,
        held: &held,
        witness: &witness,
        selected: &ready_row,
        predecessor_sequence,
        predecessor_bytes,
        policy,
        proposal: &proposal,
        cancellation,
        clock,
    };
    let promoted = promote_live_owned_authorization_v8(permit, owner);
    let owner = match promoted {
        LiveReadyPromotionOutcomeV8::Ready(owner) => owner,
        failed => {
            held.quarantine();
            return Err(LiveReadyAdvanceFailureV8::Promotion {
                _owner: failed,
                _session: session,
                _held: held,
                _witness: witness,
                _proposal: proposal,
                error: SourceJournalError::Binding,
            });
        }
    };
    let selected = EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed {
        turn: 0,
        attempt: 0,
        grant_digest: commitments.grant_digest().into(),
    });
    let next = LiveAuthorizationConsumedAppendV8 {
        owner,
        session,
        held,
        journal,
        witness,
        ready_row,
        selected,
        predecessor_sequence,
        predecessor_bytes,
        policy,
        proposal,
        commitments,
        cancellation,
        clock,
        staged,
    };
    if let Err(error) = next.validate_live() {
        next.held.quarantine();
        return Err(LiveReadyAdvanceFailureV8::After {
            _obligation: next,
            error,
        });
    }
    Ok(next)
}
#[cfg(test)]
mod tests;

mod prepared;
pub(crate) use prepared::LiveEffectAuthorizationPermitV8;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use prepared::{
    advance_verified_authorization_v8, advance_verified_intent_v8,
    FixedOwnedEffectIntentAppendPermitV8, LiveActivatedOwnedEffectV8,
    LiveEffectAuthorizationFailureV8, LiveEffectIntentActivationFailureV8,
    LiveEffectIntentPreparationFailureV8, LiveOwnedEffectIntentAppendV8, LivePreparedOwnedEffectV8,
};

pub(crate) use prepared::LiveEffectIntentPermitV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use prepared::{
    advance_verified_settlement_v8, FixedOwnedEffectSettlementAppendPermitV8,
    LiveEffectSettlementAcknowledgedV8, LiveEffectSettlementFailureV8,
    LiveOwnedEffectSettlementAppendV8, LiveRecordedOwnedEffectV8, LiveSettledOwnedEffectV8,
};

pub(in crate::live_invocation::source_journal::owned_wait_v8) use prepared::cleanup;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use cleanup::reduce::step;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use cleanup::failed_state;
