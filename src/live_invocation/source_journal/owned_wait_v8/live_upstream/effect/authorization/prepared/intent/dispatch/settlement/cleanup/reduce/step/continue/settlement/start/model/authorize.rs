//! Actual continued terminal-to-authorization consuming delegate. Its original
//! context/ledger/token remains outside each borrowed stage guard.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    authorize_live_continued_state_v8, transfer_live_continued_state_v8,
    LiveContinuedAuthorizeOutcomeV8, LiveContinuedTransferOutcomeV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedAuthorizeSuccessorV8;
use crate::resumable_effects::owned_frame::v2::{
    CheckedOwnedAgentWaitBindingV8, CheckedOwnedWaitProposalV8,
};
fn guard_authorize(
    lineage: &ContinueLineageV8<'_>,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedContinuedAuthorizeSuccessorV8<'_>,
    strict: bool,
) -> Result<(), SourceJournalError> {
    let journal = lineage.journal();
    let result = (|| {
        if !session.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_current_session(session)?;
        let origin = lineage.step.origin();
        origin.hold.validate_continued_authorize_guard(
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
        if strict {
            let ordinary = journal.context().ordinary();
            // Every clock callback is external. Its postguard checks BOTH the
            // true physical Intent/current prefix and the same registry phase
            // before the next clock or any SDK/source work can occur.
            let clock_guard = || {
                held.validate_prefix(session.sequence(), session.acknowledged_bytes())?;
                origin.hold.validate_continued_authorize_guard(
                    journal,
                    session.sequence(),
                    session.acknowledged_bytes(),
                )?;
                witness.validate_current_session(session)?;
                if origin.cancellation.is_cancelled() {
                    return Err(SourceJournalError::Binding);
                }
                Ok(())
            };
            clock_guard()?;
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
            clock_guard()?;
            if domain != ordinary.clock_domain() {
                return Err(SourceJournalError::Binding);
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
            clock_guard()?;
            if now < ordinary.initial_millis() || now >= ordinary.deadline_millis() {
                return Err(SourceJournalError::Time);
            }
        }
        origin.hold.validate_continued_authorize_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        witness.validate_current_session(session)
    })();
    result.inspect_err(|_| journal.quarantine())
}

/// Actual ACK lineage only; constructors remain in the consuming methods below.
pub(crate) struct LiveContinuedStateTransferPermitV8<'p, 'j> {
    held: HeldOwnedWaitStoreV8<'j>,
    lineage: &'p ContinueLineageV8<'j>,
    session: &'p AppendSessionV8<'j>,
    witness: &'p VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
}
impl LiveContinuedStateTransferPermitV8<'_, '_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        guard_authorize(self.lineage, self.session, self.witness, true)?;
        if !matches!(self.witness.selected_row(), EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStateTransferReserved {turn,attempt:0,..}) if *turn==self.lineage.turn)
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(crate) fn binding(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        self.lineage
            .journal()
            .context()
            .ready_runtime()
            .expect("validated real E/B")
            .1
            .wait()
    }
    pub(crate) fn matches_held_store(&self, held: &HeldOwnedWaitStoreV8<'_>) -> bool {
        self.held.same_container(held)
    }
}
pub(crate) struct LiveContinuedAuthorizePermitV8<'p, 'j> {
    held: HeldOwnedWaitStoreV8<'j>,
    lineage: &'p ContinueLineageV8<'j>,
    session: &'p AppendSessionV8<'j>,
    witness: &'p VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
    fuel: usize,
}
impl LiveContinuedAuthorizePermitV8<'_, '_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        guard_authorize(self.lineage, self.session, self.witness, true)?;
        let journal = self.lineage.journal();
        if self.fuel
            != journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1
                .evaluation_fuel()
            || Some(self.fuel as u64)
                != journal
                    .context()
                    .fold()
                    .ordinary
                    .max_steps_per_stage()
                    .map(|x| x as u64)
            || !matches!(self.witness.selected_row(),EntryV8::Ordinary(SourceJournalEntry::StageReservation {turn,attempt:Some(0),role:crate::live_invocation::source_journal::SourceStageRole::Authorize,fuel}) if *turn==self.lineage.turn&&*fuel==self.fuel)
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(crate) fn binding(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        self.lineage
            .journal()
            .context()
            .ready_runtime()
            .expect("validated real E/B")
            .1
            .wait()
    }
    pub(crate) fn matches_held_store(&self, held: &HeldOwnedWaitStoreV8<'_>) -> bool {
        self.held.same_container(held)
    }
    pub(crate) fn fuel(&self) -> usize {
        self.fuel
    }
}
pub(super) enum ContinuedAuthorizationOutcomeV8<'j> {
    Effect(effect::ContinuedEffectOutcomeV8<'j>),
    Transfer(LiveContinuedTransferOutcomeV8<'j>),
    Authorize(LiveContinuedAuthorizeOutcomeV8<'j>),
}
impl<'j> ContinuedResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn transfer_ready(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> bool {
        matches!(&self.outcome, ContinuedResumeOutcomeV8::Actual(LiveContinuedWaitResumeOutcomeV8::Resumed(o)) if o.transfer_ready(binding,proposal))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_authorize_live(
        &self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
    ) -> Result<(), SourceJournalError> {
        guard_authorize(&self.lineage, session, witness, true)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn transfer_authorize_actual(
        self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Self {
        let checked = guard_authorize(&self.lineage, session, witness, true);
        let held = self.lineage.journal().hold();
        let (Ok(()), Ok(held)) = (checked, held) else {
            return self;
        };
        let Self {
            outcome,
            accounting,
            lineage,
        } = self;
        let ContinuedResumeOutcomeV8::Actual(LiveContinuedWaitResumeOutcomeV8::Resumed(owner)) =
            outcome
        else {
            return Self {
                outcome,
                accounting,
                lineage,
            };
        };
        let permit = LiveContinuedStateTransferPermitV8 {
            held,
            lineage: &lineage,
            session,
            witness,
        };
        let actual = transfer_live_continued_state_v8(&permit, owner, proposal);
        Self {
            outcome: ContinuedResumeOutcomeV8::Authorization(
                ContinuedAuthorizationOutcomeV8::Transfer(actual),
            ),
            accounting,
            lineage,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn evaluate_authorize_actual(
        self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
    ) -> Self {
        let checked = guard_authorize(&self.lineage, session, witness, true);
        let held = self.lineage.journal().hold();
        let (Ok(()), Ok(held)) = (checked, held) else {
            return self;
        };
        let fuel = self
            .lineage
            .journal()
            .context()
            .ready_runtime()
            .expect("real E")
            .1
            .evaluation_fuel();
        let Self {
            outcome,
            accounting,
            lineage,
        } = self;
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Transfer(
            LiveContinuedTransferOutcomeV8::Moved(owner),
        )) = outcome
        else {
            return Self {
                outcome,
                accounting,
                lineage,
            };
        };
        let permit = LiveContinuedAuthorizePermitV8 {
            held,
            lineage: &lineage,
            session,
            witness,
            fuel,
        };
        let actual = authorize_live_continued_state_v8(&permit, owner);
        Self {
            outcome: ContinuedResumeOutcomeV8::Authorization(
                ContinuedAuthorizationOutcomeV8::Authorize(actual),
            ),
            accounting,
            lineage,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn transferred_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Option<serde_json::Value> {
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Transfer(
            LiveContinuedTransferOutcomeV8::Moved(o),
        )) = &self.outcome
        else {
            return None;
        };
        o.checked_facts(binding, proposal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn staged_authorize_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<(serde_json::Value, serde_json::Value, u64)> {
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Authorize(
            LiveContinuedAuthorizeOutcomeV8::Staged(o),
        )) = &self.outcome
        else {
            return None;
        };
        let (state, decision) = o.checked_facts(binding)?;
        Some((state, decision, o.consumed()))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn authorize_failure(
        &self,
    ) -> Option<(
        crate::interpreter::resumable::owned_frame::OwnedFrameFailure,
        u64,
    )> {
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Authorize(
            LiveContinuedAuthorizeOutcomeV8::Failed(owner)
            | LiveContinuedAuthorizeOutcomeV8::GuardLost(owner),
        )) = &self.outcome
        else {
            return None;
        };
        Some((owner.failure()?.clone(), owner.consumed()))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_authorize_append_prefix(
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
            .validate_continued_authorize_append_prefix(journal, inventory, selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_authorize_registry(
        &self,
        witness: &VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .step
            .origin()
            .hold
            .advance_continued_authorize_ack(witness, session)
    }
}

#[cfg(test)]
impl ContinuedResumedWaitV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_authorize_weak(
        &self,
    ) -> Vec<std::sync::Weak<[u8]>> {
        match &self.outcome {
            ContinuedResumeOutcomeV8::Actual(
                LiveContinuedWaitResumeOutcomeV8::Resumed(x)
                | LiveContinuedWaitResumeOutcomeV8::Terminal(x)
                | LiveContinuedWaitResumeOutcomeV8::GuardLost { owner: x, .. },
            ) => x.test_weak(),
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Transfer(
                LiveContinuedTransferOutcomeV8::Moved(x)
                | LiveContinuedTransferOutcomeV8::GuardLost(x),
            )) => x.test_weak(),
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Transfer(
                LiveContinuedTransferOutcomeV8::Refused(x),
            )) => x.test_weak(),
            ContinuedResumeOutcomeV8::Authorization(
                ContinuedAuthorizationOutcomeV8::Authorize(
                    LiveContinuedAuthorizeOutcomeV8::Staged(x)
                    | LiveContinuedAuthorizeOutcomeV8::Failed(x)
                    | LiveContinuedAuthorizeOutcomeV8::GuardLost(x),
                ),
            ) => x.test_weak(),
            ContinuedResumeOutcomeV8::Authorization(
                ContinuedAuthorizationOutcomeV8::Authorize(
                    LiveContinuedAuthorizeOutcomeV8::Refused(x),
                ),
            ) => x.test_weak(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
impl<'j> ContinuedResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_authorize_clock(
        &mut self,
        clock: &'j dyn crate::live_invocation::SourceInvocationClock,
    ) {
        self.lineage
            .step
            .first_mut()
            .reduce
            .cleanup
            .recorded
            .intent
            .clock = clock;
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_authorize_cancel(&self) {
        self.lineage.step.origin().cancellation.cancel();
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod effect;
