//! The later Completed ACK uses the fixed model writer and retains its resumed State.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::{
    advance_verified_later_model_completed_v8, LiveLaterModelCompletedFailureV8,
    LiveLaterModelCompletedV8, LiveOwnedLaterModelCompletedAppendV8,
};

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedLaterModelCompletedAppendV8<
    'j,
> {
    obligation: LiveOwnedLaterModelCompletedAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
}
impl<'j> VerifiedOwnedLaterModelCompletedAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_model(
        self,
    ) -> Result<LiveLaterModelCompletedV8<'j>, LiveLaterModelCompletedFailureV8<'j>> {
        advance_verified_later_model_completed_v8(self.obligation, self.session, self.witness)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveOwnedLaterModelCompletedAppendFailureV8<
    'j,
> {
    Before {
        owner: LiveOwnedLaterModelCompletedAppendV8<'j>,
        session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        owner: LiveOwnedLaterModelCompletedAppendV8<'j>,
        failure: AppendFailureV8<'j>,
    },
    Acknowledged {
        owner: LiveOwnedLaterModelCompletedAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
        error: SourceJournalError,
    },
    After {
        verified: VerifiedOwnedLaterModelCompletedAppendV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_owned_later_model_completed(
        self,
        obligation: LiveOwnedLaterModelCompletedAppendV8<'j>,
    ) -> Result<
        VerifiedOwnedLaterModelCompletedAppendV8<'j>,
        LiveOwnedLaterModelCompletedAppendFailureV8<'j>,
    > {
        let same_journal = obligation.belongs_to(self.journal);
        let predecessor = match (|| {
            if !same_journal
                || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(
                    obligation.selected(),
                    EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitCompleted { .. })
                )
            {
                return Err(SourceJournalError::Binding);
            }
            obligation.validate_live()?;
            self.effect_cursor()
        })() {
            Ok(cursor) => cursor,
            Err(error) => {
                if same_journal {
                    self.journal.quarantine();
                }
                return Err(LiveOwnedLaterModelCompletedAppendFailureV8::Before {
                    owner: obligation,
                    session: self,
                    error,
                });
            }
        };
        let permit = match obligation.fixed_append_permit() {
            Ok(permit) => permit,
            Err(error) => {
                self.journal.quarantine();
                return Err(LiveOwnedLaterModelCompletedAppendFailureV8::Before {
                    owner: obligation,
                    session: self,
                    error,
                });
            }
        };
        let selected = permit.selected_row().clone();
        let (pending, verified, attempting) = match self.begin_fixed_continued_model_append(&permit)
        {
            Ok(completion) => completion,
            Err(failure) => {
                return Err(LiveOwnedLaterModelCompletedAppendFailureV8::Append {
                    owner: obligation,
                    failure,
                });
            }
        };
        let session = AppendSessionV8 {
            journal: attempting.journal,
            inventory: pending.acknowledge_verified(verified),
        };
        let witness = VerifiedOwnedContinuedModelSuccessorV8 {
            predecessor,
            successor: OwnedEffectAppendCursorV8::capture(&session),
            selected,
        };
        let advanced = witness
            .validate_against_acknowledged_session(&session)
            .and_then(|_| permit.advance_registry(&witness, &session));
        if let Err(error) = advanced {
            drop(attempting);
            return Err(LiveOwnedLaterModelCompletedAppendFailureV8::Acknowledged {
                owner: obligation,
                session,
                witness,
                error,
            });
        }
        attempting.complete.set(true);
        drop(attempting);
        let envelope = VerifiedOwnedLaterModelCompletedAppendV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = envelope
            .obligation
            .validate_successor(&envelope.witness, &envelope.session)
        {
            return Err(LiveOwnedLaterModelCompletedAppendFailureV8::After {
                verified: envelope,
                error,
            });
        }
        Ok(envelope)
    }
}
