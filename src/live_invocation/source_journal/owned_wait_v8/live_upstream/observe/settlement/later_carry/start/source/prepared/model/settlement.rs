//! The physical SDK outcome selects exactly one turn-two settlement row.
use super::*;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedLaterModelSettlementAppendV8<
    'j,
> {
    owner: LiveLaterModelIntentV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterModelSettledV8<'j> {
    owner: LiveLaterModelIntentV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveLaterModelSettlementFailureV8<
    'j,
> {
    Selection {
        owner: LiveLaterModelIntentV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveOwnedLaterModelSettlementAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveLaterModelIntentV8<'j> {
    fn selected_settlement(&self) -> Result<EntryV8, SourceJournalError> {
        self.owner
            .owner
            .owner
            .validate_model_incurred(&self.session, &self.witness)?;
        let turn = self.owner.owner.owner.turn();
        Ok(EntryV8::Ordinary(
            match self.dispatched.as_ref().ok_or(SourceJournalError::Order)? {
                OwnedModelSettlementV8::Settled { response, .. } => {
                    SourceJournalEntry::AttemptSettled {
                        turn,
                        attempt: 0,
                        response_digest:
                            crate::live_invocation::source_journal::source_response_digest(response),
                        response: response.clone(),
                    }
                }
                OwnedModelSettlementV8::Failed {
                    reason,
                    attempted_bytes,
                    ..
                } => SourceJournalEntry::AttemptFailed {
                    turn,
                    attempt: 0,
                    reason: *reason,
                    attempted_bytes: *attempted_bytes,
                },
            },
        ))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_settlement(
        self,
    ) -> Result<LiveOwnedLaterModelSettlementAppendV8<'j>, LiveLaterModelSettlementFailureV8<'j>>
    {
        match self.selected_settlement() {
            Ok(selected) => Ok(LiveOwnedLaterModelSettlementAppendV8 {
                owner: self,
                selected,
            }),
            Err(error) => Err(LiveLaterModelSettlementFailureV8::Selection { owner: self, error }),
        }
    }
}
impl<'j> LiveOwnedLaterModelSettlementAppendV8<'j> {
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
        std::ptr::eq(self.owner.owner.owner.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let journal = self.owner.owner.owner.journal();
        let result = (|| {
            if self.owner.selected_settlement()? != self.selected {
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
        Ok(FixedOwnedContinuedModelAppendPermitV8::later_settlement(
            self,
        ))
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
        self.owner.owner.owner.owner.validate_model_append_prefix(
            journal,
            inventory,
            &self.selected,
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .owner
            .owner
            .advance_model_registry(witness, session)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let journal = self.owner.owner.owner.journal();
        let result = (|| {
            witness.validate_predecessor(
                journal,
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_current_session(session)?;
            self.owner
                .owner
                .owner
                .owner
                .validate_model_incurred(session, witness)?;
            let (_, _, turn, selected) = session.continued_model_facts()?;
            if turn != self.owner.owner.owner.owner.turn()
                || selected != &self.selected
                || session.continued_model_accounting()?
                    != *self.owner.owner.owner.owner.model_accounting()
            {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| journal.quarantine())
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_later_model_settlement_v8<
    'j,
>(
    obligation: LiveOwnedLaterModelSettlementAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
) -> Result<LiveLaterModelSettledV8<'j>, LiveLaterModelSettlementFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveLaterModelSettlementFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    Ok(LiveLaterModelSettledV8 {
        owner: obligation.owner,
        session,
        witness,
    })
}
impl LiveLaterModelSettledV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .owner
            .owner
            .validate_model_incurred(&self.session, &self.witness)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.session.sequence()
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_accounting(
        &self,
    ) -> &crate::agent_lifecycle::authorization::target_protocol::TargetAccounting {
        self.owner.owner.owner.owner.model_accounting()
    }
}
