//! The later usage ACK uses the fixed model writer and retains its Parked owner.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::{
    advance_verified_later_model_usage_v8, LiveLaterModelUsageFailureV8, LiveLaterModelUsageV8,
    LiveOwnedLaterModelUsageAppendV8,
};

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedLaterModelUsageAppendV8<
    'j,
> {
    obligation: LiveOwnedLaterModelUsageAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
}
impl<'j> VerifiedOwnedLaterModelUsageAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_model(
        self,
    ) -> Result<LiveLaterModelUsageV8<'j>, LiveLaterModelUsageFailureV8<'j>> {
        advance_verified_later_model_usage_v8(self.obligation, self.session, self.witness)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveOwnedLaterModelUsageAppendFailureV8<
    'j,
> {
    Before {
        owner: LiveOwnedLaterModelUsageAppendV8<'j>,
        session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        owner: LiveOwnedLaterModelUsageAppendV8<'j>,
        failure: AppendFailureV8<'j>,
    },
    Acknowledged {
        owner: LiveOwnedLaterModelUsageAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
        error: SourceJournalError,
    },
    After {
        verified: VerifiedOwnedLaterModelUsageAppendV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_owned_later_model_usage(
        self,
        obligation: LiveOwnedLaterModelUsageAppendV8<'j>,
    ) -> Result<VerifiedOwnedLaterModelUsageAppendV8<'j>, LiveOwnedLaterModelUsageAppendFailureV8<'j>>
    {
        let same_journal = obligation.belongs_to(self.journal);
        let predecessor = match (|| {
            if !same_journal
                || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(
                    obligation.selected(),
                    EntryV8::Ordinary(SourceJournalEntry::AttemptUsage { .. })
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
                return Err(LiveOwnedLaterModelUsageAppendFailureV8::Before {
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
                return Err(LiveOwnedLaterModelUsageAppendFailureV8::Before {
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
                return Err(LiveOwnedLaterModelUsageAppendFailureV8::Append {
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
            return Err(LiveOwnedLaterModelUsageAppendFailureV8::Acknowledged {
                owner: obligation,
                session,
                witness,
                error,
            });
        }
        attempting.complete.set(true);
        drop(attempting);
        let envelope = VerifiedOwnedLaterModelUsageAppendV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = envelope
            .obligation
            .validate_successor(&envelope.witness, &envelope.session)
        {
            return Err(LiveOwnedLaterModelUsageAppendFailureV8::After {
                verified: envelope,
                error,
            });
        }
        Ok(envelope)
    }
}
