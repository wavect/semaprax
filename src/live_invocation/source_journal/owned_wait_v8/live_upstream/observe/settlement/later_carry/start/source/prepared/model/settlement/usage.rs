//! The actual SDK settlement supplies one reported Usage row.
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod resume;
use super::*;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedLaterModelUsageAppendV8<
    'j,
> {
    owner: LiveLaterModelSettledV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterModelUsageV8<'j> {
    owner: LiveLaterModelSettledV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveLaterModelUsageFailureV8<'j>
{
    Selection {
        owner: LiveLaterModelSettledV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveOwnedLaterModelUsageAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveLaterModelSettledV8<'j> {
    fn selected_usage(&self) -> Result<EntryV8, SourceJournalError> {
        self.validate_live()?;
        let usage = match self
            .owner
            .dispatched
            .as_ref()
            .ok_or(SourceJournalError::Order)?
        {
            OwnedModelSettlementV8::Settled { usage, .. }
            | OwnedModelSettlementV8::Failed { usage, .. } => *usage,
        };
        let reported = usage.map(|(input, output, _)| {
            crate::live_invocation::source_journal::SourceReportedUsage {
                total: input.checked_add(output),
                input: Some(input),
                output: Some(output),
                reasoning: None,
                cache_read: None,
                cache_write: None,
            }
        });
        Ok(EntryV8::Ordinary(SourceJournalEntry::AttemptUsage {
            turn: self.turn(),
            attempt: 0,
            reported,
        }))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_usage(
        self,
    ) -> Result<LiveOwnedLaterModelUsageAppendV8<'j>, LiveLaterModelUsageFailureV8<'j>> {
        match self.selected_usage() {
            Ok(selected) => Ok(LiveOwnedLaterModelUsageAppendV8 {
                owner: self,
                selected,
            }),
            Err(error) => Err(LiveLaterModelUsageFailureV8::Selection { owner: self, error }),
        }
    }
}
impl<'j> LiveOwnedLaterModelUsageAppendV8<'j> {
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
            if self.owner.selected_usage()? != self.selected {
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
        Ok(FixedOwnedContinuedModelAppendPermitV8::later_usage(self))
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
            self.owner
                .owner
                .owner
                .owner
                .owner
                .validate_model_incurred(session, witness)?;
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
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_later_model_usage_v8<
    'j,
>(
    obligation: LiveOwnedLaterModelUsageAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
) -> Result<LiveLaterModelUsageV8<'j>, LiveLaterModelUsageFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveLaterModelUsageFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    Ok(LiveLaterModelUsageV8 {
        owner: obligation.owner,
        session,
        witness,
    })
}
impl LiveLaterModelUsageV8<'_> {
    fn journal(&self) -> &SourceOwnedWaitJournalV8 {
        self.owner.journal()
    }
    fn turn(&self) -> u32 {
        self.owner.turn()
    }
    fn accounting(
        &self,
    ) -> &crate::agent_lifecycle::authorization::target_protocol::TargetAccounting {
        self.owner.accounting()
    }
    fn validate_current(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .owner
            .owner
            .owner
            .validate_model_incurred(session, witness)
    }
    fn validate_model_live(&self) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .owner
            .owner
            .owner
            .validate_model_live(&self.session, &self.witness)
    }
    fn validate_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .validate_append_prefix(journal, inventory, selected)
    }
    fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner.advance_registry(witness, session)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.validate_current(&self.session, &self.witness)
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
