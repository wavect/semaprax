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
impl<'j> ContinuedResumedWaitV8<'j> {
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
