//! Fixed ContinuedModel ACK lineage. The witness is never an owner or dispatch grant.
use super::super::live_upstream::{
    advance_verified_continued_model_v8, LiveContinuedModelAcknowledgmentFailureV8,
    LiveContinuedModelV8, LiveOwnedContinuedModelAppendV8,
};
use super::owned_effect::OwnedEffectAppendCursorV8;
use super::*;

/// Only the fixed physical adapter constructs this after persisted/reread ACK.
/// No Clone, public constructor or independently detachable cursor exists.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedContinuedModelSuccessorV8<
    'j,
> {
    predecessor: OwnedEffectAppendCursorV8<'j>,
    successor: OwnedEffectAppendCursorV8<'j>,
    selected: EntryV8,
}
impl VerifiedOwnedContinuedModelSuccessorV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_predecessor(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        bytes: usize,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        self.validate_predecessor_data(journal, sequence, bytes, selected)?;
        self.successor.validate_current()
    }
    fn validate_predecessor_data(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        bytes: usize,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        if !std::ptr::eq(self.predecessor.journal, journal)
            || !std::ptr::eq(self.successor.journal, journal)
            || self.predecessor.sequence != sequence
            || self.predecessor.bytes != bytes
            || &self.selected != selected
            || !matches!(selected,
                EntryV8::Ordinary(SourceJournalEntry::AttemptIntent{..}|SourceJournalEntry::PricedAttemptIntent(..)|SourceJournalEntry::AttemptSettled{..}|SourceJournalEntry::AttemptFailed{..}|SourceJournalEntry::AttemptUsage{..}) | EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitReserved{phase:crate::live_invocation::source_journal::owned_wait_v8::model::PhaseV8::Resume,..}|crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitCompleted{..}))
            || self.successor.sequence
                != sequence
                    .checked_add(1)
                    .ok_or(SourceJournalError::Capacity)?
            || self.successor.bytes <= bytes
            || self.predecessor.authentication == self.successor.authentication
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    /// Callback-free comparison to the actual acknowledged session. Unlike the
    /// fresh validator this never touches a file or reenters the append marker.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_against_acknowledged_session(
        &self,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_predecessor_data(
            session.journal,
            self.predecessor.sequence,
            self.predecessor.bytes,
            &self.selected,
        )?;
        let (_, _, _, selected) = session.inventory.continued_model_facts()?;
        if !session
            .inventory
            .belongs_to_context(&session.journal.context)
            || session.sequence() != self.successor.sequence
            || session.acknowledged_bytes() != self.successor.bytes
            || session.inventory.authentication_tail() != self.successor.authentication
            || selected != &self.selected
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_current_session(
        &self,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_against_acknowledged_session(session)?;
        self.successor.validate_current()
    }
    /// Borrow-only old registry lineage comparison; never a cursor setter.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_previous_registry(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        bytes: usize,
        authentication: &str,
    ) -> Result<(), SourceJournalError> {
        self.validate_predecessor_data(journal, sequence, bytes, &self.selected)?;
        if self.predecessor.authentication != authentication {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.successor.sequence
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.successor.bytes
    }
}

/// The actual unchanged obligation is first. Session/witness never stand alone.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedContinuedModelAppendV8<
    'j,
> {
    obligation: LiveOwnedContinuedModelAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
}
impl<'j> VerifiedOwnedContinuedModelAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.obligation
            .validate_successor(&self.witness, &self.session)
            .inspect_err(|_| self.session.journal.quarantine())
    }
    /// Closed move into the actual engine ACK consumer; no host or parts API.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_model(
        self,
    ) -> Result<LiveContinuedModelV8<'j>, LiveContinuedModelAcknowledgmentFailureV8<'j>> {
        let Self {
            obligation,
            session,
            witness,
        } = self;
        advance_verified_continued_model_v8(obligation, session, witness)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveOwnedContinuedModelAppendFailureV8<
    'j,
> {
    Before {
        _obligation: LiveOwnedContinuedModelAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        _obligation: LiveOwnedContinuedModelAppendV8<'j>,
        _failure: AppendFailureV8<'j>,
    },
    Acknowledged {
        _obligation: LiveOwnedContinuedModelAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
        error: SourceJournalError,
    },
    After {
        _verified: VerifiedOwnedContinuedModelAppendV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_owned_continued_model(
        self,
        obligation: LiveOwnedContinuedModelAppendV8<'j>,
    ) -> Result<VerifiedOwnedContinuedModelAppendV8<'j>, LiveOwnedContinuedModelAppendFailureV8<'j>>
    {
        let same_journal = obligation.belongs_to(self.journal);
        let predecessor = match (|| {
            if !same_journal
                || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(obligation.selected(),
                    EntryV8::Ordinary(SourceJournalEntry::AttemptIntent{..}|SourceJournalEntry::PricedAttemptIntent(..)|SourceJournalEntry::AttemptSettled{..}|SourceJournalEntry::AttemptFailed{..}|SourceJournalEntry::AttemptUsage{..}) | EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitReserved{phase:crate::live_invocation::source_journal::owned_wait_v8::model::PhaseV8::Resume,..}|crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitCompleted{..}))
            {
                return Err(SourceJournalError::Binding);
            }
            obligation.validate_live()?;
            self.effect_cursor()
        })() {
            Ok(cursor) => cursor,
            Err(error) => {
                // Harmless wrong-container preflight does not poison that other
                // container. Stale actual same-container ownership retires it.
                if same_journal {
                    self.journal.quarantine();
                }
                return Err(LiveOwnedContinuedModelAppendFailureV8::Before {
                    _obligation: obligation,
                    _session: self,
                    error,
                });
            }
        };
        // This permit is borrowed only from this actual owner-containing object.
        let permit = match obligation.fixed_append_permit() {
            Ok(permit) => permit,
            Err(error) => {
                self.journal.quarantine();
                return Err(LiveOwnedContinuedModelAppendFailureV8::Before {
                    _obligation: obligation,
                    _session: self,
                    error,
                });
            }
        };
        let selected = permit.selected_row().clone();
        let (pending, verified, attempting) = match self.begin_fixed_continued_model_append(&permit)
        {
            Ok(completion) => completion,
            Err(failure) => {
                return Err(LiveOwnedContinuedModelAppendFailureV8::Append {
                    _obligation: obligation,
                    _failure: failure,
                })
            }
        };
        let session = AppendSessionV8 {
            journal: attempting.journal,
            inventory: pending.acknowledge_verified(verified),
        };
        // Sole literal constructor: actual same-FD write/sync/reread has ACKed
        // this Pending. Recovered inventory never reaches this construction.
        let witness = VerifiedOwnedContinuedModelSuccessorV8 {
            predecessor,
            successor: OwnedEffectAppendCursorV8::capture(&session),
            selected,
        };
        let advanced = witness
            .validate_against_acknowledged_session(&session)
            .and_then(|_| permit.advance_registry(&witness, &session));
        if let Err(error) = advanced {
            // Attempting remains incomplete and poisons before any return.
            drop(attempting);
            return Err(LiveOwnedContinuedModelAppendFailureV8::Acknowledged {
                _obligation: obligation,
                _session: session,
                _witness: witness,
                error,
            });
        }
        attempting.complete.set(true);
        drop(attempting); // continuation postguard requires current cancellation/clock
        let envelope = VerifiedOwnedContinuedModelAppendV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = envelope.validate_live() {
            return Err(LiveOwnedContinuedModelAppendFailureV8::After {
                _verified: envelope,
                error,
            });
        }
        Ok(envelope)
    }
}

mod funnel;
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod later;
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod later_settlement;

impl AppendSessionV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_model_facts(
        &self,
    ) -> Result<(u64, u32, u32, &EntryV8), SourceJournalError> {
        self.inventory.continued_model_facts()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_model_accounting(
        &self,
    ) -> Result<
        crate::agent_lifecycle::authorization::target_protocol::TargetAccounting,
        SourceJournalError,
    > {
        self.inventory.continued_model_accounting()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_model_request_basis(
        &self,
    ) -> Result<
        (
            u32,
            Option<Vec<u8>>,
            crate::agent_lifecycle::authorization::target_protocol::TargetAccounting,
        ),
        SourceJournalError,
    > {
        self.inventory.continued_model_request_basis()
    }
}

#[cfg(test)]
impl LiveOwnedContinuedModelAppendFailureV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_is_in_doubt(
        &self,
    ) -> bool {
        matches!(
            self,
            Self::Append {
                _failure: AppendFailureV8::InDoubt { .. },
                ..
            }
        )
    }
}
