//! The later Prepared ACK moves the same physical park through the fixed writer.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::{
    advance_verified_later_prepared_v8, LiveLaterPreparedFailureV8, LiveLaterPreparedPhaseV8,
    LiveOwnedLaterPreparedAppendV8,
};

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedLaterPreparedAppendV8<
    'j,
> {
    obligation: LiveOwnedLaterPreparedAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedPreparedSuccessorV8<'j>,
}
impl<'j> VerifiedOwnedLaterPreparedAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_prepared(
        self,
    ) -> Result<LiveLaterPreparedPhaseV8<'j>, LiveLaterPreparedFailureV8<'j>> {
        advance_verified_later_prepared_v8(self.obligation, self.session, self.witness)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveOwnedLaterPreparedAppendFailureV8<
    'j,
> {
    Before {
        owner: LiveOwnedLaterPreparedAppendV8<'j>,
        session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        owner: LiveOwnedLaterPreparedAppendV8<'j>,
        failure: AppendFailureV8<'j>,
    },
    Acknowledged {
        owner: LiveOwnedLaterPreparedAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedPreparedSuccessorV8<'j>,
        error: SourceJournalError,
    },
    After {
        verified: VerifiedOwnedLaterPreparedAppendV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_owned_later_prepared(
        self,
        obligation: LiveOwnedLaterPreparedAppendV8<'j>,
    ) -> Result<VerifiedOwnedLaterPreparedAppendV8<'j>, LiveOwnedLaterPreparedAppendFailureV8<'j>>
    {
        let same_journal = obligation.belongs_to(self.journal);
        let predecessor = match (|| {
            if !same_journal
                || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(obligation.selected(), EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitPrepared { .. }))
            { return Err(SourceJournalError::Binding); }
            obligation.validate_live()?;
            self.effect_cursor()
        })() {
            Ok(cursor) => cursor,
            Err(error) => {
                if same_journal {
                    self.journal.quarantine();
                }
                return Err(LiveOwnedLaterPreparedAppendFailureV8::Before {
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
                return Err(LiveOwnedLaterPreparedAppendFailureV8::Before {
                    owner: obligation,
                    session: self,
                    error,
                });
            }
        };
        let selected = permit.selected_row().clone();
        let (pending, verified, attempting) =
            match self.begin_fixed_continued_prepared_append(&permit) {
                Ok(completion) => completion,
                Err(failure) => {
                    return Err(LiveOwnedLaterPreparedAppendFailureV8::Append {
                        owner: obligation,
                        failure,
                    })
                }
            };
        let session = AppendSessionV8 {
            journal: attempting.journal,
            inventory: pending.acknowledge_verified(verified),
        };
        let witness = VerifiedOwnedContinuedPreparedSuccessorV8 {
            predecessor,
            successor: OwnedEffectAppendCursorV8::capture(&session),
            selected,
        };
        let advanced = witness
            .validate_against_acknowledged_session(&session)
            .and_then(|_| permit.advance_registry(&witness, &session));
        if let Err(error) = advanced {
            drop(attempting);
            return Err(LiveOwnedLaterPreparedAppendFailureV8::Acknowledged {
                owner: obligation,
                session,
                witness,
                error,
            });
        }
        attempting.complete.set(true);
        drop(attempting);
        let envelope = VerifiedOwnedLaterPreparedAppendV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = envelope
            .obligation
            .validate_successor(&envelope.witness, &envelope.session)
        {
            return Err(LiveOwnedLaterPreparedAppendFailureV8::After {
                verified: envelope,
                error,
            });
        }
        Ok(envelope)
    }
}
