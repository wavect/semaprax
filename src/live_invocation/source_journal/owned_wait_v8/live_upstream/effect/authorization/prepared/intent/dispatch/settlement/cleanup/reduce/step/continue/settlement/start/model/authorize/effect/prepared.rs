//! A sealed continued Consumed lineage enters the existing effect preparer once.
use super::*;
use crate::agent_lifecycle::authorization::CheckedOwnedWaitReadyCommitmentsV8;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::OwnedEffectInputsV8;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::prepare_live_continued_effect_v8;
pub(super) use crate::interpreter::resumable::owned_frame::registered_stage::live_run::LiveContinuedEffectPreparationV8;

pub(crate) struct LiveContinuedEffectAuthorizationPermitV8<'p, 'j> {
    held: HeldOwnedWaitStoreV8<'j>,
    lineage: &'p ContinueLineageV8<'j>,
    session: &'p AppendSessionV8<'j>,
    witness: &'p VerifiedOwnedContinuedEffectSuccessorV8<'j>,
    proposal: &'p CheckedOwnedWaitProposalV8,
    commitments: &'p CheckedOwnedWaitReadyCommitmentsV8,
    references: (u32, u32, u32),
    accounting: &'p TargetAccounting,
    turn: u32,
}
impl<'j> LiveContinuedEffectAuthorizationPermitV8<'_, 'j> {
    pub(crate) fn validate_current(&self) -> Result<(), SourceJournalError> {
        (|| {
            if !matches!(self.witness.selected_row(), EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed { turn, attempt:0, grant_digest }) if *turn==self.turn && grant_digest==self.commitments.grant_digest())
                || self.references.0.checked_add(1)!=Some(self.references.1)
                || self.references.1.checked_add(1)!=Some(self.references.2)
                || usize::try_from(self.references.2).ok().and_then(|n|n.checked_add(1))!=Some(self.session.sequence())
                || self.session.continued_effect_accounting()? != *self.accounting {
                return Err(SourceJournalError::Binding);
            }
            guard_effect(self.lineage,self.session,self.witness,self.proposal,true)
        })().inspect_err(|_|self.lineage.journal().quarantine())
    }
    pub(crate) fn inputs(&self) -> Result<OwnedEffectInputsV8<'j>, SourceJournalError> {
        self.validate_current()?;
        let journal = self.lineage.journal();
        let (runtime, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let origin = self.lineage.step.origin();
        Ok(OwnedEffectInputsV8 {
            runtime,
            execution,
            proposal: self.proposal.clone(),
            store: journal.hold()?,
            policy: origin.policy,
            cancellation: origin.cancellation,
            turn: self.turn,
            attempt: 0,
        })
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
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(execution, inputs.execution)
            || !std::ptr::eq(origin.policy, inputs.policy)
            || !std::ptr::eq(origin.cancellation, inputs.cancellation)
            || !self.matches_held_store(&inputs.store)
            || inputs.turn != self.turn
            || inputs.attempt != 0
            || inputs.proposal.carrier() != self.proposal.carrier()
            || inputs.proposal.ordinary_digest() != self.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        inputs
            .store
            .validate_prefix(self.session.sequence(), self.session.acknowledged_bytes())?;
        self.validate_current()
    }
    pub(crate) fn references(&self) -> (u32, u32, u32) {
        self.references
    }
    pub(crate) fn matches_held_store(&self, held: &HeldOwnedWaitStoreV8<'_>) -> bool {
        self.held.same_container(held)
    }
    pub(crate) fn matches_commitments(&self, actual: &CheckedOwnedWaitReadyCommitmentsV8) -> bool {
        actual.authorization_binding() == self.commitments.authorization_binding()
            && actual.grant_digest() == self.commitments.grant_digest()
            && actual.target_grant_digest() == self.commitments.target_grant_digest()
            && actual.argument_digest() == self.commitments.argument_digest()
            && actual.budget() == self.commitments.budget()
    }
}
impl<'j> ContinuedResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_effect_actual(
        self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
    ) -> Result<Self, (Self, SourceJournalError)> {
        if !matches!(
            &self.outcome,
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
                ContinuedEffectOutcomeV8::Promotion(LiveContinuedReadyPromotionOutcomeV8::Ready(_))
            ))
        ) {
            return Err((self, SourceJournalError::Order));
        }
        let held = match self.lineage.journal().hold() {
            Ok(h) => h,
            Err(error) => return Err((self, error)),
        };
        let turn = self.turn();
        let permit = LiveContinuedEffectAuthorizationPermitV8 {
            held,
            lineage: &self.lineage,
            session,
            witness,
            proposal,
            commitments,
            references,
            accounting: &self.accounting,
            turn,
        };
        #[cfg(test)]
        ADMISSIONS.with(|n| n.set(n.get() + 1));
        if let Err(error) = permit.validate_current() {
            drop(permit);
            return Err((self, error));
        }
        // Preserve the actual held handle from admission; no second fallible
        // reacquisition and no callback occurs while rebuilding the permit.
        let LiveContinuedEffectAuthorizationPermitV8 { held, .. } = permit;
        let Self {
            outcome,
            accounting,
            lineage,
        } = self;
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
            ContinuedEffectOutcomeV8::Promotion(LiveContinuedReadyPromotionOutcomeV8::Ready(owner)),
        )) = outcome
        else {
            return Err((
                Self {
                    outcome,
                    accounting,
                    lineage,
                },
                SourceJournalError::Order,
            ));
        };
        let permit = LiveContinuedEffectAuthorizationPermitV8 {
            held,
            lineage: &lineage,
            session,
            witness,
            proposal,
            commitments,
            references,
            accounting: &accounting,
            turn,
        };
        let actual = prepare_live_continued_effect_v8(&permit, owner);
        Ok(Self {
            outcome: ContinuedResumeOutcomeV8::Authorization(
                ContinuedAuthorizationOutcomeV8::Effect(ContinuedEffectOutcomeV8::Preparation(
                    actual,
                )),
            ),
            accounting,
            lineage,
        })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_prepared_effect(
        &self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
    ) -> Result<(), SourceJournalError> {
        (|| {
            if let ContinuedResumeOutcomeV8::Authorization(
                ContinuedAuthorizationOutcomeV8::Effect(ContinuedEffectOutcomeV8::Preparation(
                    owner,
                )),
            ) = &self.outcome
            {
                if let Some(error) = owner.selected_error() {
                    return Err(error);
                }
            }
            let permit = LiveContinuedEffectAuthorizationPermitV8 {
                held: self.lineage.journal().hold()?,
                lineage: &self.lineage,
                session,
                witness,
                proposal,
                commitments,
                references,
                accounting: &self.accounting,
                turn: self.turn(),
            };
            permit.validate_current()?;
            match &self.outcome {
                ContinuedResumeOutcomeV8::Authorization(
                    ContinuedAuthorizationOutcomeV8::Effect(ContinuedEffectOutcomeV8::Preparation(
                        owner,
                    )),
                ) => owner.validate_live(&permit),
                _ => Err(SourceJournalError::Order),
            }
        })()
        .inspect_err(|_| self.lineage.journal().quarantine())
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_prepared_effect_after(
        &self,
    ) -> bool {
        match &self.outcome {
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
                ContinuedEffectOutcomeV8::Preparation(owner),
            )) => owner.test_after(),
            _ => false,
        }
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_prepared_effect_metadata(
        &self,
    ) -> Option<(u32, u32, u32, u64)> {
        match &self.outcome {
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
                ContinuedEffectOutcomeV8::Preparation(owner),
            )) => owner.test_metadata(),
            _ => None,
        }
    }
}

#[cfg(test)]
impl ContinuedResumedWaitV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_replace_preparation_accounting(
        &mut self,
        accounting: TargetAccounting,
    ) {
        self.accounting = accounting;
    }
}

#[cfg(test)]
thread_local! { static ADMISSIONS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)}; }
#[cfg(test)]
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_continued_preparation_admissions(
) -> usize {
    ADMISSIONS.with(std::cell::Cell::get)
}
