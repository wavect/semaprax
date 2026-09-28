//! Consumes an actual acknowledged Intent without entering a target.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
    activate_live_owned_effect_v8, ActivatedOwnedEffectV8, LiveEffectActivationRejectionV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedEffectIntentSuccessorV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct IntentLineageV8<'j> {
    pub(super) session: AppendSessionV8<'j>,
    pub(super) witness: VerifiedOwnedEffectIntentSuccessorV8<'j>,
    pub(super) hold: ProspectiveOwnedReduceHoldV8<'j>,
    pub(super) journal: &'j SourceOwnedWaitJournalV8,
    selected: EntryV8,
    predecessor_sequence: usize,
    predecessor_bytes: usize,
    staged: u32,
    ready: u32,
    consumed: u32,
    commitments: CheckedOwnedWaitReadyCommitmentsV8,
    pub(super) proposal: crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8,
    pub(super) policy: &'j CapabilityPolicy,
    pub(super) cancellation: &'j crate::agent_runtime::AgentCancellation,
    pub(super) clock: &'j dyn crate::live_invocation::SourceInvocationClock,
}
/// Private fields; only the consuming actual Intent handoff creates this permit.
pub(crate) struct LiveEffectIntentPermitV8<'p, 'j> {
    journal: &'j SourceOwnedWaitJournalV8,
    witness: &'p VerifiedOwnedEffectIntentSuccessorV8<'j>,
    hold: &'p ProspectiveOwnedReduceHoldV8<'j>,
    selected: &'p EntryV8,
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
impl LiveEffectIntentPermitV8<'_, '_> {
    pub(crate) fn validate_current(&self) -> Result<(), SourceJournalError> {
        let l = self;
        l.witness.validate_predecessor(
            l.journal,
            l.predecessor_sequence,
            l.predecessor_bytes,
            l.selected,
        )?;
        l.hold.validate_intent_guard(
            l.journal,
            l.witness.sequence(),
            l.witness.acknowledged_bytes(),
        )?;
        let context = l.journal.context();
        let (runtime, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        let held = l.journal.hold()?;
        check_clock_v8(
            &held,
            l.witness.sequence(),
            l.witness.acknowledged_bytes(),
            l.cancellation,
            l.clock,
            context.ordinary().clock_domain(),
            context.ordinary().initial_millis(),
            context.ordinary().deadline_millis(),
        )?;
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &held.registration().expected_facts().scope,
            l.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !l.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        l.hold.validate_intent_guard(
            l.journal,
            l.witness.sequence(),
            l.witness.acknowledged_bytes(),
        )?;
        l.witness.validate_predecessor(
            l.journal,
            l.predecessor_sequence,
            l.predecessor_bytes,
            l.selected,
        )
    }
    pub(crate) fn validate_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_current()?;
        let l = self;
        let (runtime, execution) = l
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(execution, inputs.execution)
            || !std::ptr::eq(l.policy, inputs.policy)
            || !std::ptr::eq(l.cancellation, inputs.cancellation)
            || inputs.turn != 0
            || inputs.attempt != 0
            || inputs.proposal.carrier() != l.proposal.carrier()
            || inputs.proposal.ordinary_digest() != l.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        inputs
            .store
            .validate_prefix(l.witness.sequence(), l.witness.acknowledged_bytes())?;
        self.validate_current()
    }
    pub(crate) fn matches_commitments(&self, actual: &CheckedOwnedWaitReadyCommitmentsV8) -> bool {
        let c = self.commitments;
        actual.authorization_binding() == c.authorization_binding()
            && actual.grant_digest() == c.grant_digest()
            && actual.target_grant_digest() == c.target_grant_digest()
            && actual.argument_digest() == c.argument_digest()
            && actual.budget() == c.budget()
    }
    pub(crate) fn references(&self) -> Result<(u32, u32, u32, u32), SourceJournalError> {
        Ok((
            self.staged,
            self.ready,
            self.consumed,
            u32::try_from(self.predecessor_sequence).map_err(|_| SourceJournalError::Capacity)?,
        ))
    }
    pub(crate) fn matches_intent_row(
        &self,
        row: &crate::live_invocation::source_journal::SourceJournalEntry,
    ) -> bool {
        *self.selected == EntryV8::Ordinary(row.clone())
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveActivatedOwnedEffectV8<'j>
{
    pub(super) activated: ActivatedOwnedEffectV8<'j>,
    pub(super) accounting: crate::agent_lifecycle::authorization::target_protocol::TargetAccounting,
    pub(super) lineage: IntentLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveEffectIntentActivationFailureV8<
    'j,
> {
    Before {
        _owner: LiveOwnedEffectIntentAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedOwnedEffectIntentSuccessorV8<'j>,
        error: SourceJournalError,
    },
    Preparation {
        _owner: LiveEffectActivationRejectionV8<'j>,
        _lineage: IntentLineageV8<'j>,
        error: SourceJournalError,
    },
    After {
        _owner: LiveActivatedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
}
impl LiveActivatedOwnedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let permit = LiveEffectIntentPermitV8 {
            journal: self.lineage.journal,
            witness: &self.lineage.witness,
            hold: &self.lineage.hold,
            selected: &self.lineage.selected,
            predecessor_sequence: self.lineage.predecessor_sequence,
            predecessor_bytes: self.lineage.predecessor_bytes,
            staged: self.lineage.staged,
            ready: self.lineage.ready,
            consumed: self.lineage.consumed,
            commitments: &self.lineage.commitments,
            proposal: &self.lineage.proposal,
            policy: self.lineage.policy,
            cancellation: self.lineage.cancellation,
            clock: self.lineage.clock,
        };
        let result = if !self.lineage.session.belongs_to(self.lineage.journal)
            || self.lineage.session.sequence() != self.lineage.witness.sequence()
            || self.lineage.session.acknowledged_bytes()
                != self.lineage.witness.acknowledged_bytes()
        {
            Err(SourceJournalError::Binding)
        } else {
            self.activated.validate_live_intent(&permit)
        };
        if result.is_err() {
            self.activated.quarantine_live_intent();
        }
        result
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_intent_v8<'j>(
    obligation: LiveOwnedEffectIntentAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedEffectIntentSuccessorV8<'j>,
) -> Result<LiveActivatedOwnedEffectV8<'j>, LiveEffectIntentActivationFailureV8<'j>> {
    // The original Consumed session is inert lineage after this acknowledged row.
    let valid = (|| {
        if !session.belongs_to(obligation.owner.journal)
            || !obligation
                .owner
                .session
                .belongs_to(obligation.owner.journal)
            || obligation.owner.session.sequence() != obligation.owner.witness.sequence()
            || obligation.owner.session.acknowledged_bytes()
                != obligation.owner.witness.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_against_acknowledged_session(&session)?;
        witness.validate_predecessor(
            obligation.owner.journal,
            obligation.sequence(),
            obligation.acknowledged_bytes(),
            &obligation.selected,
        )
    })();
    if let Err(error) = valid {
        obligation.owner.prepared.quarantine_live_authorization();
        return Err(LiveEffectIntentActivationFailureV8::Before {
            _owner: obligation,
            _session: session,
            _witness: witness,
            error,
        });
    }
    let predecessor_sequence = obligation.sequence();
    let predecessor_bytes = obligation.acknowledged_bytes();
    let LiveOwnedEffectIntentAppendV8 { owner, selected } = obligation;
    let LivePreparedOwnedEffectV8 {
        prepared,
        hold,
        journal,
        staged,
        ready,
        consumed,
        commitments,
        proposal,
        policy,
        cancellation,
        clock,
        ..
    } = owner;
    let lineage = IntentLineageV8 {
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
        proposal,
        policy,
        cancellation,
        clock,
    };
    let permit = LiveEffectIntentPermitV8 {
        journal: lineage.journal,
        witness: &lineage.witness,
        hold: &lineage.hold,
        selected: &lineage.selected,
        predecessor_sequence: lineage.predecessor_sequence,
        predecessor_bytes: lineage.predecessor_bytes,
        staged: lineage.staged,
        ready: lineage.ready,
        consumed: lineage.consumed,
        commitments: &lineage.commitments,
        proposal: &lineage.proposal,
        policy: lineage.policy,
        cancellation: lineage.cancellation,
        clock: lineage.clock,
    };
    let activated = match activate_live_owned_effect_v8(prepared, &permit) {
        Ok(owner) => owner,
        Err(owner) => {
            let error = owner.error();
            return Err(LiveEffectIntentActivationFailureV8::Preparation {
                _owner: owner,
                _lineage: lineage,
                error,
            });
        }
    };
    // Only this successful actual first-Intent handoff creates the invocation
    // ledger. No recovered prefix or caller-provided ledger enters this route.
    let actual = LiveActivatedOwnedEffectV8 {
        activated,
        accounting:
            crate::agent_lifecycle::authorization::target_protocol::TargetAccounting::default(),
        lineage,
    };
    if let Err(error) = actual.validate_live() {
        return Err(LiveEffectIntentActivationFailureV8::After {
            _owner: actual,
            error,
        });
    }
    Ok(actual)
}

pub(super) fn validate_obligation_successor(
    obligation: &LiveOwnedEffectIntentAppendV8<'_>,
    witness: &VerifiedOwnedEffectIntentSuccessorV8<'_>,
) -> Result<(), SourceJournalError> {
    let o = &obligation.owner;
    let result = (|| {
        if !o.session.belongs_to(o.journal)
            || o.session.sequence() != o.witness.sequence()
            || o.session.acknowledged_bytes() != o.witness.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        let permit = LiveEffectIntentPermitV8 {
            journal: o.journal,
            witness,
            hold: &o.hold,
            selected: &obligation.selected,
            predecessor_sequence: obligation.sequence(),
            predecessor_bytes: obligation.acknowledged_bytes(),
            staged: o.staged,
            ready: o.ready,
            consumed: o.consumed,
            commitments: &o.commitments,
            proposal: &o.proposal,
            policy: o.policy,
            cancellation: o.cancellation,
            clock: o.clock,
        };
        o.prepared.validate_live_intent(&permit)
    })();
    if result.is_err() {
        o.prepared.quarantine_live_authorization();
    }
    result
}

impl IntentLineageV8<'_> {
    pub(super) fn permit(&self) -> LiveEffectIntentPermitV8<'_, '_> {
        LiveEffectIntentPermitV8 {
            journal: self.journal,
            witness: &self.witness,
            hold: &self.hold,
            selected: &self.selected,
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
        }
    }
    pub(super) fn validate_live(&self) -> Result<(), SourceJournalError> {
        if !self.session.belongs_to(self.journal)
            || self.session.sequence() != self.witness.sequence()
            || self.session.acknowledged_bytes() != self.witness.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        self.permit().validate_current()
    }
}
