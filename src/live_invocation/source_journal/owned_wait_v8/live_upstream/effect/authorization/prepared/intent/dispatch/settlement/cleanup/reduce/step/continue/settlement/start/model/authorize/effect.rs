//! Actual continued Ready promotion and same-token Consumed phase guards.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    promote_live_continued_authorization_v8, LiveContinuedReadyPromotionOutcomeV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedEffectSuccessorV8;
fn guard_effect(
    lineage: &ContinueLineageV8<'_>,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedContinuedEffectSuccessorV8<'_>,
    proposal: &CheckedOwnedWaitProposalV8,
    strict: bool,
) -> Result<(), SourceJournalError> {
    let journal = lineage.journal();
    let result = (|| {
        if !session.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_current_session(session)?;
        let origin = lineage.step.origin();
        origin.hold.validate_continued_effect_guard(
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
            proposal,
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
                origin.hold.validate_continued_effect_guard(
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
        origin.hold.validate_continued_effect_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        witness.validate_current_session(session)
    })();
    result.inspect_err(|_| journal.quarantine())
}

/// Actual ACK lineage only; constructors remain in the consuming methods below.

pub(crate) struct LiveContinuedReadyPromotionPermitV8<'p, 'j> {
    held: HeldOwnedWaitStoreV8<'j>,
    lineage: &'p ContinueLineageV8<'j>,
    session: &'p AppendSessionV8<'j>,
    witness: &'p VerifiedOwnedContinuedEffectSuccessorV8<'j>,
    proposal: &'p CheckedOwnedWaitProposalV8,
}
impl LiveContinuedReadyPromotionPermitV8<'_, '_> {
    pub(crate) fn binding(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        self.lineage
            .journal()
            .context()
            .ready_runtime()
            .expect("actual E")
            .1
            .wait()
    }
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        guard_effect(
            self.lineage,
            self.session,
            self.witness,
            self.proposal,
            true,
        )
    }
    pub(crate) fn matches_held_store(&self, held: &HeldOwnedWaitStoreV8<'_>) -> bool {
        self.held.same_container(held)
    }
}
pub(super) enum ContinuedEffectOutcomeV8<'j> {
    Activation(crate::interpreter::resumable::owned_frame::registered_stage::live_run::LiveContinuedEffectActivationV8<'j>),
    Preparation(prepared::LiveContinuedEffectPreparationV8<'j>),
    Promotion(LiveContinuedReadyPromotionOutcomeV8<'j>),
}
impl<'j> ContinuedResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_effect_live(
        &self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Result<(), SourceJournalError> {
        guard_effect(&self.lineage, session, witness, proposal, true)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn promote_effect_actual(
        self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Self {
        let checked = guard_effect(&self.lineage, session, witness, proposal, true);
        let held = self.lineage.journal().hold();
        let (Ok(()), Ok(held)) = (checked, held) else {
            return self;
        };
        let Self {
            outcome,
            accounting,
            lineage,
        } = self;
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Authorize(
            LiveContinuedAuthorizeOutcomeV8::Staged(owner),
        )) = outcome
        else {
            return Self {
                outcome,
                accounting,
                lineage,
            };
        };
        let permit = LiveContinuedReadyPromotionPermitV8 {
            held,
            lineage: &lineage,
            session,
            witness,
            proposal,
        };
        let actual = promote_live_continued_authorization_v8(&permit, owner);
        Self {
            outcome: ContinuedResumeOutcomeV8::Authorization(
                ContinuedAuthorizationOutcomeV8::Effect(ContinuedEffectOutcomeV8::Promotion(
                    actual,
                )),
            ),
            accounting,
            lineage,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn effect_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<(serde_json::Value, serde_json::Value, u64)> {
        match &self.outcome {
            ContinuedResumeOutcomeV8::Authorization(
                ContinuedAuthorizationOutcomeV8::Authorize(
                    LiveContinuedAuthorizeOutcomeV8::Staged(owner),
                ),
            ) => {
                let (s, d) = owner.checked_facts(binding)?;
                Some((s, d, owner.consumed()))
            }
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
                ContinuedEffectOutcomeV8::Promotion(LiveContinuedReadyPromotionOutcomeV8::Ready(
                    owner,
                )),
            )) => {
                let (s, d) = owner.checked_facts(binding)?;
                Some((s, d, owner.consumed()))
            }
            _ => None,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_effect_append_prefix(
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
            .validate_continued_effect_append_prefix(journal, inventory, selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_effect_registry(
        &self,
        witness: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .step
            .origin()
            .hold
            .advance_continued_effect_ack(witness, session)
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_effect_weak(
        &self,
    ) -> Vec<std::sync::Weak<[u8]>> {
        match &self.outcome {
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
                ContinuedEffectOutcomeV8::Promotion(
                    LiveContinuedReadyPromotionOutcomeV8::Ready(owner)
                    | LiveContinuedReadyPromotionOutcomeV8::GuardLost(owner),
                ),
            )) => owner.test_weak(),
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
                ContinuedEffectOutcomeV8::Promotion(LiveContinuedReadyPromotionOutcomeV8::Refused(
                    owner,
                )),
            )) => owner.test_weak(),
            _ => self.test_authorize_weak(),
        }
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod prepared;
