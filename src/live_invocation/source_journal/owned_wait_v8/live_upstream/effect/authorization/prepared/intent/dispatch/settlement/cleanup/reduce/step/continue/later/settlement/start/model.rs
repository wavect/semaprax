//! Borrowed turn-two Model facts and guard from the same physical Parked State.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedModelSuccessorV8;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LaterModelAdmissionV8 {
    Current,
    Cancelled,
    Deadline,
}
impl LaterModelAdmissionV8 {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failure(
        &self,
    ) -> Option<crate::live_invocation::source_journal::SourceAttemptFailure> {
        use crate::live_invocation::source_journal::SourceAttemptFailure;
        match self {
            Self::Current => None,
            Self::Cancelled => Some(SourceAttemptFailure::Cancelled),
            Self::Deadline => Some(SourceAttemptFailure::Timeout),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn error(
        &self,
    ) -> Option<SourceJournalError> {
        match self {
            Self::Current => None,
            Self::Cancelled => Some(SourceJournalError::Binding),
            Self::Deadline => Some(SourceJournalError::Time),
        }
    }
}

impl<'j> LaterStartedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn model_clock(
        &self,
    ) -> Result<&dyn crate::live_invocation::SourceInvocationClock, SourceJournalError> {
        Ok(self.lineage.source.continued_model_origin()?.1)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn model_cancelled(
        &self,
    ) -> bool {
        self.lineage.cancellation.is_cancelled()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn configure_model_adapter(
        &self,
        adapter: &mut crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
    ) {
        adapter.configure_durable_boundary(
            self.lineage.cancellation,
            self.lineage.journal.context().ordinary().deadline_millis(),
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_model_admission(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
    ) -> Result<LaterModelAdmissionV8, SourceJournalError> {
        let journal = self.lineage.journal;
        let physical = || {
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
            held.validate_prefix(session.sequence(), session.acknowledged_bytes())?;
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
            let LiveContinuedWaitStartOutcomeV8::Parked(owner) = &self.outcome else {
                return Err(SourceJournalError::Binding);
            };
            owner
                .checked_incurred_facts(execution.wait())
                .ok_or(SourceJournalError::Binding)?;
            witness.validate_current_session(session)
        };
        let checked = (|| {
            physical()?;
            if self.model_cancelled() {
                return Ok(LaterModelAdmissionV8::Cancelled);
            }
            let ordinary = journal.context().ordinary();
            let clock = self.model_clock()?;
            let domain =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| clock.clock_domain()))
                    .map_err(|_| SourceJournalError::Poisoned)?;
            physical()?;
            if self.model_cancelled() {
                return Ok(LaterModelAdmissionV8::Cancelled);
            }
            if domain != ordinary.clock_domain() {
                return Err(SourceJournalError::Binding);
            }
            let now = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| clock.now_millis()))
                .map_err(|_| SourceJournalError::Poisoned)?;
            physical()?;
            if self.model_cancelled() {
                return Ok(LaterModelAdmissionV8::Cancelled);
            }
            if now < ordinary.initial_millis() {
                return Err(SourceJournalError::Time);
            }
            if now >= ordinary.deadline_millis() {
                return Ok(LaterModelAdmissionV8::Deadline);
            }
            Ok(LaterModelAdmissionV8::Current)
        })();
        checked.inspect_err(|_| journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_model_incurred(
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
            let execution = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1;
            let LiveContinuedWaitStartOutcomeV8::Parked(owner) = &self.outcome else {
                return Err(SourceJournalError::Binding);
            };
            owner
                .checked_incurred_facts(execution.wait())
                .ok_or(SourceJournalError::Binding)?;
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| journal.quarantine())
    }
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
