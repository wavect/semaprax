//! Actual continued Prepared selects one Intent; only its durable ACK activates.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedIntentSuccessorV8;
struct IntentAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedIntentSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedContinuedIntentAppendV8<
    'j,
> {
    owner: LivePreparedContinuedEffectV8<'j>,
    selected: EntryV8,
}
struct ContinuedIntentPhaseV8<'j> {
    owner: LivePreparedContinuedEffectV8<'j>,
    ack: IntentAckV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveActivatedContinuedEffectV8<
    'j,
> {
    phase: ContinuedIntentPhaseV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedIntentSelectionFailureV8<
    'j,
> {
    owner: LivePreparedContinuedEffectV8<'j>,
    error: SourceJournalError,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedIntentAcknowledgmentFailureV8<
    'j,
> {
    Before {
        owner: LiveOwnedContinuedIntentAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        error: SourceJournalError,
    },
    Entered {
        phase: ContinuedIntentPhaseV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LivePreparedContinuedEffectV8<'j> {
    fn intent_row(&self) -> Result<EntryV8, SourceJournalError> {
        self.validate_live()?;
        let owner = &self.owner;
        let prior = owner.acks.last().ok_or(SourceJournalError::Order)?;
        let row = owner.authorization.actual()?.owner.continued_intent_row(
            &prior.session,
            &prior.witness,
            owner.authorization.proposal()?,
            &owner.commitments,
            owner.preparation_references()?,
        )?;
        self.validate_live()?;
        Ok(EntryV8::Ordinary(row))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_intent(
        self,
    ) -> Result<LiveOwnedContinuedIntentAppendV8<'j>, LiveContinuedIntentSelectionFailureV8<'j>>
    {
        match self.intent_row() {
            Ok(selected) => Ok(LiveOwnedContinuedIntentAppendV8 {
                owner: self,
                selected,
            }),
            Err(error) => {
                self.owner.journal().quarantine();
                Err(LiveContinuedIntentSelectionFailureV8 { owner: self, error })
            }
        }
    }
}
impl<'j> LiveOwnedContinuedIntentAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected(&self) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.owner.current().sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.owner.current().acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.owner.owner.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        (|| {
            if self.owner.intent_row()? == self.selected {
                Ok(())
            } else {
                Err(SourceJournalError::Binding)
            }
        })()
        .inspect_err(|_| self.owner.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
    ) -> Result<(), SourceJournalError> {
        let owner = &self.owner.owner;
        (|| {
            witness.validate_predecessor(
                owner.journal(),
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_current_session(session)?;
            let prior = owner.acks.last().ok_or(SourceJournalError::Order)?;
            owner
                .authorization
                .actual()?
                .owner
                .validate_continued_intent_successor(
                    &prior.session,
                    &prior.witness,
                    session,
                    witness,
                    owner.authorization.proposal()?,
                    &owner.commitments,
                    owner.preparation_references()?,
                )
        })()
        .inspect_err(|_| owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinuedIntentAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedContinuedIntentAppendPermitV8 { owner: self })
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedContinuedIntentAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveOwnedContinuedIntentAppendV8<'j>,
}
impl<'j> FixedOwnedContinuedIntentAppendPermitV8<'_, 'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_candidate(
        &self,
        row: &EntryV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        if row != self.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        self.validate_selected_prefix(self.owner.owner.owner.journal(), inventory)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.owner.selected
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
        self.owner
            .owner
            .owner
            .authorization
            .actual()?
            .owner
            .validate_continued_intent_append_prefix(journal, inventory, &self.owner.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .owner
            .authorization
            .actual()?
            .owner
            .advance_continued_intent_registry(witness, session)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_continued_intent_v8<
    'j,
>(
    obligation: LiveOwnedContinuedIntentAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedIntentSuccessorV8<'j>,
) -> Result<LiveActivatedContinuedEffectV8<'j>, LiveContinuedIntentAcknowledgmentFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveContinuedIntentAcknowledgmentFailureV8::Before {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    let LiveOwnedContinuedIntentAppendV8 { owner, selected } = obligation;
    let refs = owner
        .owner
        .preparation_references()
        .expect("validated adjacent C ACKs");
    let LivePreparedContinuedEffectV8 { owner } = owner;
    let LiveContinuedEffectV8 {
        authorization,
        commitments,
        staged,
        acks,
    } = owner;
    let LiveContinuedAuthorizationV8 {
        completed,
        state,
        state_digest,
        transfer_digest,
        acks: aacks,
    } = authorization;
    let super::super::super::super::LiveContinuedModelV8 {
        owner,
        request,
        ordinal,
        acks: ma,
        dispatched,
        proposal,
    } = completed;
    let prior = acks.last().expect("actual C ACK");
    let result = owner.activate_continued_intent(
        &prior.session,
        &prior.witness,
        &session,
        &witness,
        proposal.as_ref().expect("actual K"),
        &commitments,
        refs,
    );
    let (owner, admission_error) = match result {
        Ok(owner) => (owner, None),
        Err((owner, error)) => (owner, Some(error)),
    };
    let completed = super::super::super::super::LiveContinuedModelV8 {
        owner,
        request,
        ordinal,
        acks: ma,
        dispatched,
        proposal,
    };
    let authorization = LiveContinuedAuthorizationV8 {
        completed,
        state,
        state_digest,
        transfer_digest,
        acks: aacks,
    };
    let owner = LivePreparedContinuedEffectV8 {
        owner: LiveContinuedEffectV8 {
            authorization,
            commitments,
            staged,
            acks,
        },
    };
    if let Some(error) = admission_error {
        return Err(LiveContinuedIntentAcknowledgmentFailureV8::Before {
            owner: LiveOwnedContinuedIntentAppendV8 { owner, selected },
            session,
            witness,
            error,
        });
    }
    let phase = ContinuedIntentPhaseV8 {
        owner,
        ack: IntentAckV8 { session, witness },
    };
    if let Err(error) = phase.validate_activated() {
        return Err(LiveContinuedIntentAcknowledgmentFailureV8::Entered { phase, error });
    }
    Ok(LiveActivatedContinuedEffectV8 { phase })
}
impl ContinuedIntentPhaseV8<'_> {
    fn validate_activated(&self) -> Result<(), SourceJournalError> {
        let owner = &self.owner.owner;
        let prior = owner.acks.last().ok_or(SourceJournalError::Order)?;
        owner
            .authorization
            .actual()?
            .owner
            .validate_continued_activation(
                &prior.session,
                &prior.witness,
                &self.ack.session,
                &self.ack.witness,
                owner.authorization.proposal()?,
                &owner.commitments,
                owner.preparation_references()?,
            )
    }
}
impl LiveActivatedContinuedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.phase.validate_activated()
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
impl LiveOwnedContinuedIntentAppendV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_cancel_actual(&self) {
        self.owner
            .owner
            .authorization
            .actual()
            .unwrap()
            .owner
            .test_authorize_cancel();
    }
}
