//! The original continued owner supplies the fixed model guard and Resume
//! permit. Neither bytes nor a history witness can construct a physical owner.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    resume_live_continued_wait_v8, LiveContinuedWaitResumeOutcomeV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedModelSuccessorV8;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8;

/// Private typed admission disposition. Authority loss is always Err; neither
/// cancellation nor a future deadline can reclassify a failed physical check.
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum SdkModelAdmissionV8 {
    Current,
    Cancelled,
    Deadline,
}
impl SdkModelAdmissionV8 {
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
fn guard_model(
    lineage: &ContinueLineageV8<'_>,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
    strict: bool,
) -> Result<(), SourceJournalError> {
    let result = guard_model_checked(lineage, session, witness, strict, None)
        .and_then(|admission| admission.error().map_or(Ok(()), Err));
    result.inspect_err(|_| lineage.journal().quarantine())
}
fn guard_model_checked(
    lineage: &ContinueLineageV8<'_>,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
    strict: bool,
    actual_park: Option<&crate::interpreter::resumable::owned_frame::registered_stage::live_run::LiveContinuedParkedStateV8<'_>>,
) -> Result<SdkModelAdmissionV8, SourceJournalError> {
    let journal = lineage.journal();
    (|| {
        if !session.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_current_session(session)?;
        let origin = lineage.step.origin();
        origin.hold.validate_continued_model_guard(
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
            &origin.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !origin.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        let check_root = || {
            if let Some(owner) = actual_park {
                owner
                    .checked_incurred_facts(execution.wait())
                    .ok_or(SourceJournalError::Binding)?;
            }
            Ok(())
        };
        check_root()?;
        if strict {
            let ordinary = journal.context().ordinary();
            // Every clock callback is external. Its postguard checks BOTH the
            // true physical Intent/current prefix and the same registry phase
            // before the next clock or any SDK/source work can occur.
            let clock_guard = || {
                held.validate_prefix(session.sequence(), session.acknowledged_bytes())?;
                origin.hold.validate_continued_model_guard(
                    journal,
                    session.sequence(),
                    session.acknowledged_bytes(),
                )?;
                witness.validate_current_session(session)?;
                check_root()?;
                if origin.cancellation.is_cancelled() {
                    return Ok(SdkModelAdmissionV8::Cancelled);
                }
                Ok(SdkModelAdmissionV8::Current)
            };
            match clock_guard()? {
                SdkModelAdmissionV8::Current => (),
                refusal => return Ok(refusal),
            }
            let domain = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                origin.clock.clock_domain()
            }));
            let domain = match domain {
                Ok(v) => v,
                Err(_) => {
                    journal.quarantine();
                    return Err(SourceJournalError::Poisoned);
                }
            };
            let admission = clock_guard()?;
            if domain != ordinary.clock_domain() {
                return Err(SourceJournalError::Binding);
            }
            match admission {
                SdkModelAdmissionV8::Current => (),
                refusal => return Ok(refusal),
            }
            let now = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                origin.clock.now_millis()
            }));
            let now = match now {
                Ok(v) => v,
                Err(_) => {
                    journal.quarantine();
                    return Err(SourceJournalError::Poisoned);
                }
            };
            let admission = clock_guard()?;
            if now < ordinary.initial_millis() {
                return Err(SourceJournalError::Time);
            }
            match admission {
                SdkModelAdmissionV8::Current => (),
                refusal => return Ok(refusal),
            }
            if now >= ordinary.deadline_millis() {
                return Ok(SdkModelAdmissionV8::Deadline);
            }
        }
        origin.hold.validate_continued_model_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        witness.validate_current_session(session)?;
        check_root()?;
        Ok(SdkModelAdmissionV8::Current)
    })()
}

/// Private construction occurs only below, from the actual whole park and true
/// original Resume ACK. Its borrowed lineage cannot replace that owner.
pub(crate) struct LiveContinuedWaitResumePermitV8<'p, 'j> {
    held: HeldOwnedWaitStoreV8<'j>,
    lineage: &'p ContinueLineageV8<'j>,
    session: &'p AppendSessionV8<'j>,
    witness: &'p VerifiedOwnedContinuedModelSuccessorV8<'j>,
    fuel: usize,
}
impl LiveContinuedWaitResumePermitV8<'_, '_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        guard_model(self.lineage, self.session, self.witness, true)?;
        if !matches!(self.witness.selected_row(),EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitReserved{turn,attempt:0,phase:crate::live_invocation::source_journal::owned_wait_v8::model::PhaseV8::Resume,replay_of:None,fuel,..})if *turn==self.lineage.turn&&*fuel==self.fuel as u64)
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(crate) fn matches_held_store(&self, held: &HeldOwnedWaitStoreV8<'_>) -> bool {
        self.held.same_container(held)
    }
    pub(crate) fn fuel(&self) -> usize {
        self.fuel
    }
}
/// All failures retain the actual park or actual terminal before ledger/hold.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ContinuedResumedWaitV8<'j> {
    outcome: ContinuedResumeOutcomeV8<'j>,
    accounting: TargetAccounting,
    lineage: ContinueLineageV8<'j>,
}
enum ContinuedResumeOutcomeV8<'j> {
    Actual(LiveContinuedWaitResumeOutcomeV8<'j>),
    Before(LiveContinuedWaitStartOutcomeV8<'j>, SourceJournalError),
}
impl<'j> ContinuedStartedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn model_journal(
        &self,
    ) -> &'j SourceOwnedWaitJournalV8 {
        self.lineage.journal()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn checked_model_facts(
        &self,
        binding: &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn model_clock(
        &self,
    ) -> &dyn crate::live_invocation::SourceInvocationClock {
        self.lineage.step.origin().clock
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn model_cancelled(
        &self,
    ) -> bool {
        self.lineage.step.origin().cancellation.is_cancelled()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn configure_model_adapter(
        &self,
        adapter: &mut crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
    ) {
        adapter.configure_durable_boundary(
            self.lineage.step.origin().cancellation,
            self.lineage
                .journal()
                .context()
                .ordinary()
                .deadline_millis(),
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_model_live(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        strict: bool,
    ) -> Result<(), SourceJournalError> {
        guard_model(&self.lineage, session, witness, strict)?;
        let binding = self
            .model_journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?
            .1
            .wait();
        let LiveContinuedWaitStartOutcomeV8::Parked(owner) = &self.outcome else {
            return Err(SourceJournalError::Binding);
        };
        let facts = if strict {
            owner.checked_facts(binding)
        } else {
            owner.checked_incurred_facts(binding)
        };
        facts.ok_or(SourceJournalError::Binding)?;
        Ok(())
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
            .step
            .origin()
            .hold
            .validate_continued_model_append_prefix(journal, inventory, selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_model_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .step
            .origin()
            .hold
            .advance_continued_model_ack(witness, session)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn resume_actual(
        self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> ContinuedResumedWaitV8<'j> {
        let checked = (|| {
            self.validate_model_live(session, witness, true)?;
            let execution = self
                .model_journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1;
            let scope = &self
                .model_journal()
                .context()
                .registration()
                .expected_facts()
                .scope;
            if !proposal.matches(execution.wait().binding(),&serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()})){return Err(SourceJournalError::Binding)}
            Ok(execution.evaluation_fuel())
        })();
        let Self {
            outcome,
            accounting,
            lineage,
        } = self;
        let fuel = match checked {
            Ok(v) => v,
            Err(error) => {
                return ContinuedResumedWaitV8 {
                    outcome: ContinuedResumeOutcomeV8::Before(outcome, error),
                    accounting,
                    lineage,
                }
            }
        };
        let LiveContinuedWaitStartOutcomeV8::Parked(parked) = outcome else {
            return ContinuedResumedWaitV8 {
                outcome: ContinuedResumeOutcomeV8::Before(outcome, SourceJournalError::Binding),
                accounting,
                lineage,
            };
        };
        let held = match lineage.journal().hold() {
            Ok(v) => v,
            Err(error) => {
                return ContinuedResumedWaitV8 {
                    outcome: ContinuedResumeOutcomeV8::Before(
                        LiveContinuedWaitStartOutcomeV8::Parked(parked),
                        error,
                    ),
                    accounting,
                    lineage,
                }
            }
        };
        let permit = LiveContinuedWaitResumePermitV8 {
            held,
            lineage: &lineage,
            session,
            witness,
            fuel,
        };
        let outcome = resume_live_continued_wait_v8(&permit, parked, proposal.carrier().clone());
        ContinuedResumedWaitV8 {
            outcome: ContinuedResumeOutcomeV8::Actual(outcome),
            accounting,
            lineage,
        }
    }
}
impl<'j> ContinuedResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn model_journal(
        &self,
    ) -> &'j SourceOwnedWaitJournalV8 {
        self.lineage.journal()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn turn(&self) -> u32 {
        self.lineage.turn
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn model_accounting(
        &self,
    ) -> &TargetAccounting {
        &self.accounting
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn model_clock(
        &self,
    ) -> &dyn crate::live_invocation::SourceInvocationClock {
        self.lineage.step.origin().clock
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn model_cancelled(
        &self,
    ) -> bool {
        self.lineage.step.origin().cancellation.is_cancelled()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn configure_model_adapter(
        &self,
        adapter: &mut crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
    ) {
        adapter.configure_durable_boundary(
            self.lineage.step.origin().cancellation,
            self.model_journal().context().ordinary().deadline_millis(),
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn consumed(
        &self,
    ) -> Option<u64> {
        match &self.outcome {
            ContinuedResumeOutcomeV8::Actual(
                LiveContinuedWaitResumeOutcomeV8::Resumed(o)
                | LiveContinuedWaitResumeOutcomeV8::Terminal(o)
                | LiveContinuedWaitResumeOutcomeV8::GuardLost { owner: o, .. },
            ) => Some(o.consumed()),
            _ => None,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn checked_model_facts(
        &self,
        binding: &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
    ) -> Option<serde_json::Value> {
        let ContinuedResumeOutcomeV8::Actual(LiveContinuedWaitResumeOutcomeV8::Resumed(o)) =
            &self.outcome
        else {
            return None;
        };
        o.checked_facts(binding)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_model_live(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        strict: bool,
    ) -> Result<(), SourceJournalError> {
        guard_model(&self.lineage, session, witness, strict)
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
            .step
            .origin()
            .hold
            .validate_continued_model_append_prefix(journal, inventory, selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_model_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .step
            .origin()
            .hold
            .advance_continued_model_ack(witness, session)
    }
}

impl ContinuedStartedWaitV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_sdk_live(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
    ) -> Result<SdkModelAdmissionV8, SourceJournalError> {
        let checked = (|| {
            let LiveContinuedWaitStartOutcomeV8::Parked(owner) = &self.outcome else {
                return Err(SourceJournalError::Binding);
            };
            let admission =
                guard_model_checked(&self.lineage, session, witness, true, Some(owner))?;
            // Even an admission refusal is followed by fresh physical/root guards.
            guard_model_checked(&self.lineage, session, witness, false, Some(owner))?;
            Ok(admission)
        })();
        checked.inspect_err(|_| self.model_journal().quarantine())
    }
}

#[cfg(test)]
impl<'j> ContinuedStartedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_model_policy(
        &mut self,
        policy: &'j crate::resumable_effects::capability::CapabilityPolicy,
    ) {
        self.lineage.step.reduce.cleanup.recorded.intent.policy = policy;
    }
}
