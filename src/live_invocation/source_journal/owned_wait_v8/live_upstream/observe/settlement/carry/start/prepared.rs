//! Actual park selects one checkpoint/Prepared row. Older Start ACKs remain
//! causal facts; only the new Prepared successor validates after its ACK.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedPreparedSuccessorV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedContinuedPreparedAppendV8<
    'j,
> {
    owner: LiveContinuedStartedPhaseV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedPreparedPhaseV8<
    'j,
> {
    owner: LiveContinuedStartedPhaseV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedPreparedSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedPreparedFailureV8<
    'j,
> {
    Selection {
        owner: LiveContinuedStartedPhaseV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveOwnedContinuedPreparedAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedPreparedSuccessorV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveContinuedStartedPhaseV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_checkpoint(
        self,
    ) -> Result<LiveOwnedContinuedPreparedAppendV8<'j>, LiveContinuedPreparedFailureV8<'j>> {
        match self.selected_prepared() {
            Ok(selected) => Ok(LiveOwnedContinuedPreparedAppendV8 {
                owner: self,
                selected,
            }),
            Err(error) => Err(LiveContinuedPreparedFailureV8::Selection { owner: self, error }),
        }
    }
    fn selected_prepared(&self) -> Result<EntryV8, SourceJournalError> {
        self.validate_live()?;
        let current = self.acks.last().ok_or(SourceJournalError::Order)?;
        let EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitReserved {
            turn,
            attempt: 0,
            wait,
            phase: journal_model::PhaseV8::Start,
            replay_of: None,
            ..
        }) = current.witness.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        let (checkpoint, checkpoint_digest) = current.session.continued_parked_checkpoint(self)?;
        Ok(EntryV8::Owned(
            journal_model::OwnedBodyV8::OwnedWaitPrepared {
                turn: *turn,
                attempt: 0,
                wait: wait.clone(),
                reservation: u32::try_from(
                    current
                        .session
                        .sequence()
                        .checked_sub(1)
                        .ok_or(SourceJournalError::Order)?,
                )
                .map_err(|_| SourceJournalError::Capacity)?,
                observation_digest: self.observation.request_digest().into(),
                checkpoint_digest,
                checkpoint: crate::live_invocation::identity::hex(&checkpoint),
                consumed: self.owner.consumed().ok_or(SourceJournalError::Binding)?,
            },
        ))
    }
}
impl<'j> LiveOwnedContinuedPreparedAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected(&self) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.acknowledged_bytes()
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
        let result = (|| {
            if self.owner.selected_prepared()? != self.selected {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        result.inspect_err(|_| self.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinuedPreparedAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedContinuedPreparedAppendPermitV8 { owner: self })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinuedPreparedSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            witness.validate_predecessor(
                self.owner.journal(),
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_current_session(session)?;
            self.owner.owner.validate_prepared_live(session, witness)?;
            if !matches!(&self.selected,EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitPrepared{turn,consumed,..})if *turn==self.owner.owner.turn()&&Some(*consumed)==self.owner.owner.consumed())
            {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| self.owner.journal().quarantine())
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedContinuedPreparedAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveOwnedContinuedPreparedAppendV8<'j>,
}
impl FixedOwnedContinuedPreparedAppendPermitV8<'_, '_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        self.owner.selected()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_preflight(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        self.owner.validate_live()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(journal)
            || inventory.sequence() != self.owner.sequence()
            || inventory.acknowledged_bytes() != self.owner.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        self.owner.owner.owner.validate_prepared_append_prefix(
            journal,
            inventory,
            self.owner.selected(),
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedPreparedSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .owner
            .advance_prepared_registry(witness, session)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_continued_prepared_v8<
    'j,
>(
    obligation: LiveOwnedContinuedPreparedAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedPreparedSuccessorV8<'j>,
) -> Result<LiveContinuedPreparedPhaseV8<'j>, LiveContinuedPreparedFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveContinuedPreparedFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    Ok(LiveContinuedPreparedPhaseV8 {
        owner: obligation.owner,
        session,
        witness,
    })
}
impl LiveContinuedPreparedPhaseV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.witness.validate_current_session(&self.session)?;
            self.owner
                .owner
                .validate_prepared_live(&self.session, &self.witness)?;
            self.witness.validate_current_session(&self.session)
        })();
        result.inspect_err(|_| self.owner.journal().quarantine())
    }
}

#[cfg(test)]
impl LiveContinuedPreparedPhaseV8<'_> {
    pub(super) fn test_sequence(&self) -> usize {
        self.session.sequence()
    }
    pub(super) fn test_accounting(
        &self,
    ) -> &crate::agent_lifecycle::authorization::target_protocol::TargetAccounting {
        self.owner.owner.test_accounting()
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod model;
