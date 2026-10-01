//! A checked decoded proposal permits one full-fuel Resume reservation.
use super::*;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedLaterModelResumeAppendV8<
    'j,
> {
    owner: LiveLaterModelUsageV8<'j>,
    proposal: CheckedOwnedWaitProposalV8,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterModelResumeReservedV8<
    'j,
> {
    owner: LiveLaterModelUsageV8<'j>,
    proposal: CheckedOwnedWaitProposalV8,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveLaterModelResumeFailureV8<'j>
{
    Selection {
        owner: LiveLaterModelUsageV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveOwnedLaterModelResumeAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveLaterModelUsageV8<'j> {
    fn selected_resume(&self) -> Result<(CheckedOwnedWaitProposalV8, EntryV8), SourceJournalError> {
        self.validate_model_live()?;
        let journal = self.journal();
        let execution = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?
            .1;
        let Some(OwnedModelSettlementV8::Settled { decoded, .. }) = &self.owner.owner.dispatched
        else {
            return Err(SourceJournalError::Binding);
        };
        let proposal = crate::resumable_effects::owned_frame::v2::bind_owned_wait_proposal_v8(
            execution.wait(),
            &journal.context().registration().expected_facts().scope,
            decoded,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        let fuel = execution.evaluation_fuel();
        if journal.context().fold().ordinary.max_steps_per_stage() != Some(fuel) {
            return Err(SourceJournalError::Binding);
        }
        let row = EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitReserved {
            turn: self.turn(),
            attempt: 0,
            wait: self.owner.wait()?.into(),
            phase: crate::live_invocation::source_journal::owned_wait_v8::model::PhaseV8::Resume,
            replay_of: None,
            fuel: u64::try_from(fuel).map_err(|_| SourceJournalError::Capacity)?,
        });
        self.validate_model_live()?;
        Ok((proposal, row))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_resume_reservation(
        self,
    ) -> Result<LiveOwnedLaterModelResumeAppendV8<'j>, LiveLaterModelResumeFailureV8<'j>> {
        match self.selected_resume() {
            Ok((proposal, selected)) => Ok(LiveOwnedLaterModelResumeAppendV8 {
                owner: self,
                proposal,
                selected,
            }),
            Err(error) => Err(LiveLaterModelResumeFailureV8::Selection { owner: self, error }),
        }
    }
}
impl<'j> LiveOwnedLaterModelResumeAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected(&self) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.session.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.session.acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.owner.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let journal = self.owner.journal();
        let result = (|| {
            let (expected, selected) = self.owner.selected_resume()?;
            if expected.ordinary_digest() != self.proposal.ordinary_digest()
                || expected.answer_digest() != self.proposal.answer_digest()
                || expected.canonical_proposal() != self.proposal.canonical_proposal()
                || selected != self.selected
            {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        result.inspect_err(|_| journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinuedModelAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedContinuedModelAppendPermitV8::later_resume(self))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        if !self.belongs_to(journal)
            || inventory.sequence() != self.sequence()
            || inventory.acknowledged_bytes() != self.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        self.owner
            .validate_append_prefix(journal, inventory, &self.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner.advance_registry(witness, session)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let journal = self.owner.journal();
        let result = (|| {
            witness.validate_predecessor(
                journal,
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_current_session(session)?;
            self.owner.validate_current(session, witness)?;
            let (_, _, turn, selected) = session.continued_model_facts()?;
            if turn != self.owner.turn()
                || selected != &self.selected
                || session.continued_model_accounting()? != *self.owner.accounting()
            {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| journal.quarantine())
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_later_model_resume_v8<
    'j,
>(
    obligation: LiveOwnedLaterModelResumeAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
) -> Result<LiveLaterModelResumeReservedV8<'j>, LiveLaterModelResumeFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveLaterModelResumeFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    Ok(LiveLaterModelResumeReservedV8 {
        owner: obligation.owner,
        proposal: obligation.proposal,
        session,
        witness,
    })
}
impl LiveLaterModelResumeReservedV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.owner.validate_current(&self.session, &self.witness)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.session.sequence()
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_accounting(
        &self,
    ) -> &crate::agent_lifecycle::authorization::target_protocol::TargetAccounting {
        self.owner.accounting()
    }
}
