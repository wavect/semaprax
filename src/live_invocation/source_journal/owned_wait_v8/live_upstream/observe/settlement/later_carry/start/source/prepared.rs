//! The later physical park selects one original Prepared checkpoint and ACK.
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod model;
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedPreparedSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::FixedOwnedContinuedPreparedAppendPermitV8;
use crate::resumable_effects::owned_frame::v2::OwnedWaitCheckpointExpectationV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedLaterPreparedAppendV8<
    'j,
> {
    owner: LiveLaterStartedPhaseV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterPreparedPhaseV8<'j> {
    owner: LiveLaterStartedPhaseV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedPreparedSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveLaterPreparedFailureV8<'j> {
    Selection {
        owner: LiveLaterStartedPhaseV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveOwnedLaterPreparedAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedPreparedSuccessorV8<'j>,
        error: SourceJournalError,
    },
}

impl<'j> LiveLaterStartedPhaseV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn journal(
        &self,
    ) -> &'j SourceOwnedWaitJournalV8 {
        self.owner.journal()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.acks
            .last()
            .expect("actual Start ACK")
            .session
            .sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.acks
            .last()
            .expect("actual Start ACK")
            .session
            .acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn observation(
        &self,
    ) -> &CheckedOwnedWaitObservationV8 {
        &self.observation
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn encode_current_checkpoint(
        &self,
        session: &AppendSessionV8<'_>,
        key: &crate::resumable_effects::source_checkpoint::SourceCheckpointKey,
        expected: &OwnedWaitCheckpointExpectationV8<'_>,
    ) -> Result<(Vec<u8>, String), SourceJournalError> {
        if !session.belongs_to(self.journal())
            || session.sequence() != self.sequence()
            || session.acknowledged_bytes() != self.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        self.validate_live()?;
        let current = self.acks.last().ok_or(SourceJournalError::Order)?;
        let result = self
            .owner
            .encode_checkpoint(session, &current.witness, key, expected)?;
        self.validate_live()?;
        Ok(result)
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
        let (checkpoint, checkpoint_digest) = current.session.later_parked_checkpoint(self)?;
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_checkpoint(
        self,
    ) -> Result<LiveOwnedLaterPreparedAppendV8<'j>, LiveLaterPreparedFailureV8<'j>> {
        match self.selected_prepared() {
            Ok(selected) => Ok(LiveOwnedLaterPreparedAppendV8 {
                owner: self,
                selected,
            }),
            Err(error) => Err(LiveLaterPreparedFailureV8::Selection { owner: self, error }),
        }
    }
}

impl<'j> LiveOwnedLaterPreparedAppendV8<'j> {
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
        Ok(FixedOwnedContinuedPreparedAppendPermitV8::later(self))
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
            if !matches!(&self.selected, EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitPrepared { turn, consumed, .. }) if *turn == self.owner.owner.turn() && Some(*consumed) == self.owner.owner.consumed())
            {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| self.owner.journal().quarantine())
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
            .owner
            .validate_prepared_append_prefix(journal, inventory, &self.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedPreparedSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner.owner.advance_prepared_registry(witness, session)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_later_prepared_v8<
    'j,
>(
    obligation: LiveOwnedLaterPreparedAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedPreparedSuccessorV8<'j>,
) -> Result<LiveLaterPreparedPhaseV8<'j>, LiveLaterPreparedFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveLaterPreparedFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    Ok(LiveLaterPreparedPhaseV8 {
        owner: obligation.owner,
        session,
        witness,
    })
}
impl LiveLaterPreparedPhaseV8<'_> {
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.session.sequence()
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_accounting(
        &self,
    ) -> &TargetAccounting {
        self.owner.owner.test_accounting()
    }
}
