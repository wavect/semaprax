//! A later Resume consumes the same actual park under its exact original ACK.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    resume_live_continued_wait_v8, LiveContinuedWaitResumeOutcomeV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedModelSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedWaitResumePermitV8;

impl LaterContinueLineageV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_resume_guard(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
    ) -> Result<u32, SourceJournalError> {
        let journal = self.journal;
        let result = (|| {
            let physical = || {
                if !session.belongs_to(journal) {
                    return Err(SourceJournalError::Binding);
                }
                witness.validate_current_session(session)?;
                self.hold()?.validate_continued_model_guard(
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
                    &self.proposal,
                )
                .map_err(|_| SourceJournalError::Binding)?;
                if !self.policy.allows(plan.operation().effect_id()) {
                    return Err(SourceJournalError::Binding);
                }
                witness.validate_current_session(session)
            };
            physical()?;
            if self.cancellation.is_cancelled() {
                return Err(SourceJournalError::Binding);
            }
            let clock = self.source.continued_model_origin()?.1;
            let domain =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| clock.clock_domain()))
                    .map_err(|_| SourceJournalError::Poisoned)?;
            physical()?;
            if self.cancellation.is_cancelled()
                || domain != journal.context().ordinary().clock_domain()
            {
                return Err(SourceJournalError::Binding);
            }
            let now = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| clock.now_millis()))
                .map_err(|_| SourceJournalError::Poisoned)?;
            physical()?;
            if self.cancellation.is_cancelled() {
                return Err(SourceJournalError::Binding);
            }
            let ordinary = journal.context().ordinary();
            if now < ordinary.initial_millis() || now >= ordinary.deadline_millis() {
                return Err(SourceJournalError::Time);
            }
            Ok(self.turn)
        })();
        result.inspect_err(|_| journal.quarantine())
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LaterResumedWaitV8<'j> {
    outcome: LaterResumeOutcomeV8<'j>,
    accounting: TargetAccounting,
    lineage: LaterContinueLineageV8<'j>,
}
enum LaterResumeOutcomeV8<'j> {
    Actual(LiveContinuedWaitResumeOutcomeV8<'j>),
    Before(LiveContinuedWaitStartOutcomeV8<'j>, SourceJournalError),
}
impl<'j> LaterStartedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn resume_actual(
        self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> LaterResumedWaitV8<'j> {
        let checked = (|| {
            self.validate_model_live(session, witness)?;
            let journal = self.lineage.journal;
            let execution = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1;
            let scope = &journal.context().registration().expected_facts().scope;
            if !proposal.matches(execution.wait().binding(), &serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()})) { return Err(SourceJournalError::Binding); }
            Ok((execution.evaluation_fuel(), journal.hold()?))
        })();
        let Self {
            outcome,
            accounting,
            lineage,
        } = self;
        let (fuel, held) = match checked {
            Ok(value) => value,
            Err(error) => {
                return LaterResumedWaitV8 {
                    outcome: LaterResumeOutcomeV8::Before(outcome, error),
                    accounting,
                    lineage,
                }
            }
        };
        let LiveContinuedWaitStartOutcomeV8::Parked(parked) = outcome else {
            return LaterResumedWaitV8 {
                outcome: LaterResumeOutcomeV8::Before(outcome, SourceJournalError::Binding),
                accounting,
                lineage,
            };
        };
        let permit = LiveContinuedWaitResumePermitV8::later(held, &lineage, session, witness, fuel);
        let outcome = resume_live_continued_wait_v8(&permit, parked, proposal.carrier().clone());
        LaterResumedWaitV8 {
            outcome: LaterResumeOutcomeV8::Actual(outcome),
            accounting,
            lineage,
        }
    }
}
impl<'j> LaterResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn journal(
        &self,
    ) -> &'j SourceOwnedWaitJournalV8 {
        self.lineage.journal
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn turn(&self) -> u32 {
        self.lineage.turn
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        &self.accounting
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn consumed(
        &self,
    ) -> Option<u64> {
        match &self.outcome {
            LaterResumeOutcomeV8::Actual(
                LiveContinuedWaitResumeOutcomeV8::Resumed(o)
                | LiveContinuedWaitResumeOutcomeV8::Terminal(o)
                | LiveContinuedWaitResumeOutcomeV8::GuardLost { owner: o, .. },
            ) => Some(o.consumed()),
            _ => None,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn checked_facts(
        &self,
        binding: &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
    ) -> Option<serde_json::Value> {
        let LaterResumeOutcomeV8::Actual(LiveContinuedWaitResumeOutcomeV8::Resumed(owner)) =
            &self.outcome
        else {
            return None;
        };
        owner.checked_facts(binding)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .validate_resume_guard(session, witness)
            .map(|_| ())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_append_prefix(
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .hold()?
            .advance_continued_model_ack(witness, session)
    }
}

impl<'j> LaterResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn into_continued(
        self,
    ) -> Result<
        super::super::super::super::settlement::start::model::ContinuedResumedWaitV8<'j>,
        (Self, SourceJournalError),
    > {
        if !matches!(
            &self.outcome,
            LaterResumeOutcomeV8::Actual(LiveContinuedWaitResumeOutcomeV8::Resumed(_))
        ) || self.lineage.source.hold().is_err()
            || self.lineage.source.continued_model_origin().is_err()
        {
            return Err((self, SourceJournalError::Binding));
        }
        let Self {
            outcome,
            accounting,
            lineage,
        } = self;
        let LaterResumeOutcomeV8::Actual(outcome) = outcome else {
            unreachable!("checked actual Resume")
        };
        Ok(super::super::super::super::settlement::start::model::ContinuedResumedWaitV8::from_later_resume(outcome, accounting, lineage.into_joined()))
    }
}
