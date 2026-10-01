//! Borrowed turn-two Model facts and guard from the same physical Parked State.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedModelSuccessorV8;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8;

impl<'j> LaterStartedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn checked_model_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<serde_json::Value> {
        let LiveContinuedWaitStartOutcomeV8::Parked(owner) = &self.outcome else {
            return None;
        };
        owner.checked_facts(binding)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn model_request(
        &self,
    ) -> Option<&crate::interpreter::resumable::ResumableChannelValue> {
        let LiveContinuedWaitStartOutcomeV8::Parked(owner) = &self.outcome else {
            return None;
        };
        Some(owner.request())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn model_accounting(
        &self,
    ) -> &TargetAccounting {
        &self.accounting
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_model_live(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let journal = self.lineage.journal;
        let result = (|| {
            if !session.belongs_to(journal) {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_current_session(session)?;
            self.lineage.hold()?.validate_continued_model_guard(
                journal,
                session.sequence(),
                session.acknowledged_bytes(),
            )?;
            let held = journal.hold()?;
            let (runtime, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            runtime
                .owned_wait_effects_v8(execution)
                .map_err(|_| SourceJournalError::Binding)?;
            let plan = plan_owned_effect_v8(
                runtime,
                execution,
                &held.registration().expected_facts().scope,
                &self.lineage.proposal,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            if !self.lineage.policy.allows(plan.operation().effect_id()) {
                return Err(SourceJournalError::Binding);
            }
            let ordinary = journal.context().ordinary();
            check_clock_v8(
                &held,
                session.sequence(),
                session.acknowledged_bytes(),
                self.lineage.cancellation,
                self.lineage.source.continued_model_origin()?.1,
                ordinary.clock_domain(),
                ordinary.initial_millis(),
                ordinary.deadline_millis(),
            )?;
            self.checked_model_facts(execution.wait())
                .ok_or(SourceJournalError::Binding)?;
            self.lineage.hold()?.validate_continued_model_guard(
                journal,
                session.sequence(),
                session.acknowledged_bytes(),
            )?;
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_model_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .hold()?
            .validate_continued_model_append_prefix(journal, inventory, selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_model_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .hold()?
            .advance_continued_model_ack(witness, session)
    }
}
