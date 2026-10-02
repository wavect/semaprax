//! Actual cumulative Decision release under the shared fixed Started ACK.
use super::*;
use crate::cleanup_plan::FinalizeAction;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedEffectCleanupSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8;
use crate::live_invocation::source_journal::SourceEffectFailure;
use serde_json::Value;
pub(crate) struct LiveContinuedDecisionCleanupPermitV8<'p, 'j> {
    lineage: &'p ContinueLineageV8<'j>,
    session: &'p AppendSessionV8<'j>,
    witness: &'p VerifiedOwnedEffectCleanupSuccessorV8<'j>,
    proposal: &'p CheckedOwnedWaitProposalV8,
    facts: &'p CheckedLiveOwnedEffectSettlementV8,
}
impl LiveContinuedDecisionCleanupPermitV8<'_, '_> {
    pub(crate) fn validate_cleanup_current(&self) -> Result<(), SourceJournalError> {
        let journal = self.lineage.journal();
        (|| {
            if !self.session.belongs_to(journal) {
                return Err(SourceJournalError::Binding);
            }
            self.witness.validate_current_session(self.session)?;
            let origin = self.lineage.step.origin();
            origin.hold.validate_cleanup_guard(
                journal,
                self.session.sequence(),
                self.session.acknowledged_bytes(),
            )?;
            let held = journal.hold()?;
            let (runtime, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = plan_owned_effect_v8(
                runtime,
                execution,
                &held.registration().expected_facts().scope,
                self.proposal,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            if !origin.policy.allows(plan.operation().effect_id()) {
                return Err(SourceJournalError::Binding);
            }
            match self.witness.selected_row() {
                EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                    turn,
                    attempt: 0,
                    ..
                }) if *turn == self.lineage.turn => (),
                _ => return Err(SourceJournalError::Binding),
            }
            held.validate_prefix(self.session.sequence(), self.session.acknowledged_bytes())?;
            origin.hold.validate_cleanup_guard(
                journal,
                self.session.sequence(),
                self.session.acknowledged_bytes(),
            )?;
            self.witness.validate_current_session(self.session)
        })()
        .inspect_err(|_| journal.quarantine())
    }
    pub(crate) fn validate_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_cleanup_current()?;
        let journal = self.lineage.journal();
        let (runtime, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let origin = self.lineage.step.origin();
        let held = journal.hold()?;
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(execution, inputs.execution)
            || !std::ptr::eq(origin.policy, inputs.policy)
            || !std::ptr::eq(origin.cancellation, inputs.cancellation)
            || !held.same_container(&inputs.store)
            || inputs.turn != self.lineage.turn
            || inputs.attempt != 0
            || inputs.proposal.carrier() != self.proposal.carrier()
            || inputs.proposal.ordinary_digest() != self.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        self.validate_cleanup_current()
    }
    pub(crate) fn references(
        &self,
    ) -> Result<(u32, u32, u32, u32, u32, u32, u32), SourceJournalError> {
        let EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
            staged,
            ready,
            consumed,
            intent,
            settlement,
            recorded,
            ..
        }) = self.witness.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        let started = self
            .session
            .sequence()
            .checked_sub(1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(SourceJournalError::Capacity)?;
        Ok((
            *staged,
            *ready,
            *consumed,
            *intent,
            *settlement,
            *recorded,
            started,
        ))
    }
    pub(crate) fn operations(&self) -> Result<&Value, SourceJournalError> {
        match self.witness.selected_row() {
            EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                operations, ..
            }) => Ok(operations),
            _ => Err(SourceJournalError::Binding),
        }
    }
    pub(crate) fn matches_settlement(
        &self,
        intent: u32,
        evidence: &str,
        observation: Option<&[u8]>,
        reason: Option<SourceEffectFailure>,
    ) -> bool {
        if intent != self.facts.intent() || evidence != self.facts.evidence_digest() {
            return false;
        }
        match self.facts.ordinary() {
            SourceJournalEntry::EffectObserved {
                observation: actual,
                ..
            } => observation == Some(actual.as_slice()) && reason.is_none(),
            SourceJournalEntry::EffectFailed { reason: actual, .. } => {
                observation.is_none() && reason == Some(*actual)
            }
            _ => false,
        }
    }
}
/// Only the actual Settled ACK and the same live continuation can authorize
/// the one-use physical State/Outcome handoff.
pub(crate) struct LiveContinuedOutcomePermitV8<'p, 'j> {
    lineage: &'p ContinueLineageV8<'j>,
    session: &'p AppendSessionV8<'j>,
    witness: &'p VerifiedOwnedEffectCleanupSuccessorV8<'j>,
    proposal: &'p CheckedOwnedWaitProposalV8,
}
impl LiveContinuedOutcomePermitV8<'_, '_> {
    pub(crate) fn validate_current(&self) -> Result<(), SourceJournalError> {
        let journal = self.lineage.journal();
        let result = (|| {
            if !self.session.belongs_to(journal) {
                return Err(SourceJournalError::Binding);
            }
            self.witness.validate_current_session(self.session)?;
            let EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                turn,
                attempt: 0,
                ..
            }) = self.witness.selected_row()
            else {
                return Err(SourceJournalError::Binding);
            };
            if *turn != self.lineage.turn {
                return Err(SourceJournalError::Binding);
            }
            let origin = self.lineage.step.origin();
            origin.hold.validate_cleanup_guard(
                journal,
                self.session.sequence(),
                self.session.acknowledged_bytes(),
            )?;
            let held = journal.hold()?;
            let (runtime, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = plan_owned_effect_v8(
                runtime,
                execution,
                &held.registration().expected_facts().scope,
                self.proposal,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            if !origin.policy.allows(plan.operation().effect_id()) {
                return Err(SourceJournalError::Binding);
            }
            let ordinary = journal.context().ordinary();
            check_clock_v8(
                &held,
                self.session.sequence(),
                self.session.acknowledged_bytes(),
                origin.cancellation,
                origin.clock,
                ordinary.clock_domain(),
                ordinary.initial_millis(),
                ordinary.deadline_millis(),
            )?;
            origin.hold.validate_cleanup_guard(
                journal,
                self.session.sequence(),
                self.session.acknowledged_bytes(),
            )?;
            self.witness.validate_current_session(self.session)
        })();
        result.inspect_err(|_| journal.quarantine())
    }
    pub(crate) fn validate_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_current()?;
        let journal = self.lineage.journal();
        let (runtime, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let origin = self.lineage.step.origin();
        let held = journal.hold()?;
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(execution, inputs.execution)
            || !std::ptr::eq(origin.policy, inputs.policy)
            || !std::ptr::eq(origin.cancellation, inputs.cancellation)
            || !held.same_container(&inputs.store)
            || inputs.turn != self.lineage.turn
            || inputs.attempt != 0
            || inputs.proposal.carrier() != self.proposal.carrier()
            || inputs.proposal.ordinary_digest() != self.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        self.validate_current()
    }
    pub(crate) fn settled_receipt(&self) -> Result<(u32, u32, &Value), SourceJournalError> {
        let EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
            started, receipt, ..
        }) = self.witness.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        let settled = self
            .session
            .sequence()
            .checked_sub(1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(SourceJournalError::Capacity)?;
        Ok((*started, settled, receipt))
    }
}
impl<'j> ContinuedResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn mint_continued_outcome(
        &mut self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedEffectCleanupSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Result<(), SourceJournalError> {
        let permit = LiveContinuedOutcomePermitV8 {
            lineage: &self.lineage,
            session,
            witness,
            proposal,
        };
        permit.validate_current()?;
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
            ContinuedEffectOutcomeV8::Dispatch(owner, _),
        )) = &mut self.outcome
        else {
            return Err(SourceJournalError::Order);
        };
        owner
            .mint_outcome(&permit)
            .inspect_err(|_| self.lineage.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_outcome_minted(
        &self,
    ) -> bool {
        matches!(&self.outcome,
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
                ContinuedEffectOutcomeV8::Dispatch(owner, _)
            )) if owner.outcome_minted())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn take_continued_reduce_outcome(
        &mut self,
    ) -> Result<crate::interpreter::resumable::owned_frame::registered_stage::effect::ExecutedOwnedAgentTurnV2<'j>, SourceJournalError>{
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
            ContinuedEffectOutcomeV8::Dispatch(owner, _),
        )) = &mut self.outcome
        else {
            return Err(SourceJournalError::Order);
        };
        owner
            .take_reduce_outcome()
            .inspect_err(|_| self.lineage.journal().quarantine())
    }
    /// Remains valid after the physical Outcome moves into Reduce. No old
    /// cleanup cursor or in-place Outcome field is used as authority.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_spent_reduce_context(
        &self,
        sequence: usize,
        bytes: usize,
        proposal: &CheckedOwnedWaitProposalV8,
        step: bool,
        incurred: bool,
    ) -> Result<(), SourceJournalError> {
        let journal = self.lineage.journal();
        let result = (|| {
            let origin = self.lineage.step.origin();
            if step {
                origin.hold.validate_step_guard(journal, sequence, bytes)?;
            } else {
                origin
                    .hold
                    .validate_continued_spent_reduce_guard(journal, sequence, bytes)?;
            }
            let held = journal.hold()?;
            let (runtime, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = plan_owned_effect_v8(
                runtime,
                execution,
                &held.registration().expected_facts().scope,
                proposal,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            if !origin.policy.allows(plan.operation().effect_id()) {
                return Err(SourceJournalError::Binding);
            }
            let ordinary = journal.context().ordinary();
            if !incurred {
                check_clock_v8(
                    &held,
                    sequence,
                    bytes,
                    origin.cancellation,
                    origin.clock,
                    ordinary.clock_domain(),
                    ordinary.initial_millis(),
                    ordinary.deadline_millis(),
                )?;
            }
            if step {
                origin.hold.validate_step_guard(journal, sequence, bytes)
            } else {
                origin
                    .hold
                    .validate_continued_spent_reduce_guard(journal, sequence, bytes)
            }
        })();
        result.inspect_err(|_| journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn release_continued_decision(
        &mut self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedEffectCleanupSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        facts: &CheckedLiveOwnedEffectSettlementV8,
        observe: impl FnMut(&FinalizeAction),
    ) -> Result<(), SourceJournalError> {
        let permit = LiveContinuedDecisionCleanupPermitV8 {
            lineage: &self.lineage,
            session,
            witness,
            proposal,
            facts,
        };
        permit.validate_cleanup_current()?;
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
            ContinuedEffectOutcomeV8::Dispatch(owner, _),
        )) = &mut self.outcome
        else {
            return Err(SourceJournalError::Order);
        };
        owner
            .release_decision(&permit, observe)
            .inspect_err(|_| self.lineage.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_decision_receipt(
        &self,
    ) -> Result<&Value, SourceJournalError> {
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
            ContinuedEffectOutcomeV8::Dispatch(owner, _),
        )) = &self.outcome
        else {
            return Err(SourceJournalError::Order);
        };
        owner.decision_receipt()
    }
}

impl<'j> ContinuedResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failed_state_origin<'p>(
        &'p self, proposal: &'p CheckedOwnedWaitProposalV8,
    ) -> crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::failed_state::continued::OriginV8<'p, 'j>{
        let origin = self.lineage.step.origin();
        crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::failed_state::continued::OriginV8 {
            journal: self.lineage.journal(), hold: origin.hold, policy: origin.policy,
            cancellation: origin.cancellation, clock: origin.clock, proposal, turn: self.lineage.turn,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_failed_state_facts(
        &self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedEffectCleanupSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Result<(SourceEffectFailure, Value), SourceJournalError> {
        let permit = LiveContinuedOutcomePermitV8 {
            lineage: &self.lineage,
            session,
            witness,
            proposal,
        };
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
            ContinuedEffectOutcomeV8::Dispatch(owner, _),
        )) = &self.outcome
        else {
            return Err(SourceJournalError::Order);
        };
        owner.failed_state_facts(&permit)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn take_continued_failed_state(
        &mut self, session: &AppendSessionV8<'j>, witness: &VerifiedOwnedEffectCleanupSuccessorV8<'j>, proposal: &CheckedOwnedWaitProposalV8,
    ) -> Result<crate::interpreter::resumable::owned_frame::registered_stage::effect::PendingOwnedEffectReceiptV8<'j>, SourceJournalError>{
        let permit = LiveContinuedOutcomePermitV8 {
            lineage: &self.lineage,
            session,
            witness,
            proposal,
        };
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
            ContinuedEffectOutcomeV8::Dispatch(owner, _),
        )) = &mut self.outcome
        else {
            return Err(SourceJournalError::Order);
        };
        owner.take_failed_state(&permit)
    }
}
