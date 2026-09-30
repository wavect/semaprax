//! Fixed Continue ACK lineage. The witness is never an owner or dispatch grant.
use super::*;

/// Only the fixed physical adapter constructs this after persisted/reread ACK.
/// No Clone, public constructor or independently detachable cursor exists.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedContinueSuccessorV8<
    'j,
> {
    predecessor: OwnedEffectAppendCursorV8<'j>,
    successor: OwnedEffectAppendCursorV8<'j>,
    selected: EntryV8,
}
impl VerifiedOwnedContinueSuccessorV8<'_> {
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
                EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStateCommitted{..})
                |EntryV8::Ordinary(SourceJournalEntry::StageReservation{role:crate::live_invocation::source_journal::SourceStageRole::Observe,attempt:None,..}))
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
        let (_, _, _, selected) = session.inventory.continuation_facts()?;
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

use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveMovedStepV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::{
    advance_verified_continue_v8, LiveContinueAcknowledgedV8, LiveContinueFailureV8,
    LiveObservedContinueV8, LiveOwnedContinueAppendV8,
};

/// The actual unchanged obligation is first. Session/witness never stand alone.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedContinueAppendV8<
    'j,
> {
    obligation: LiveOwnedContinueAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinueSuccessorV8<'j>,
}
impl<'j> VerifiedOwnedContinueAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.obligation
            .validate_continue_successor(&self.witness, &self.session)
            .inspect_err(|_| self.session.journal.quarantine())
    }
    /// Closed move into the actual engine ACK consumer; no host or parts API.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continue(
        self,
    ) -> Result<LiveContinueAcknowledgedV8<'j>, LiveContinueFailureV8<'j>> {
        let Self {
            obligation,
            session,
            witness,
        } = self;
        advance_verified_continue_v8(obligation, session, witness)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveOwnedContinueAppendFailureV8<
    'j,
> {
    Before {
        _obligation: LiveOwnedContinueAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        _obligation: LiveOwnedContinueAppendV8<'j>,
        _failure: AppendFailureV8<'j>,
    },
    Acknowledged {
        _obligation: LiveOwnedContinueAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedOwnedContinueSuccessorV8<'j>,
        error: SourceJournalError,
    },
    After {
        _verified: VerifiedOwnedContinueAppendV8<'j>,
        error: SourceJournalError,
    },
}

/// Owns the unique live holder at every failed continuation boundary. The
/// authenticated rows remain inside these variants: receipt bytes cannot
/// reconstruct State or re-enter Observe.
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinueDriverFailureV8<
    'j,
> {
    Prepare(LiveContinueFailureV8<'j>),
    StateSession {
        owner: LiveOwnedContinueAppendV8<'j>,
        error: SourceJournalError,
    },
    StateAppend(LiveOwnedContinueAppendFailureV8<'j>),
    StateAdvance(LiveContinueFailureV8<'j>),
    StateAcknowledged(LiveContinueAcknowledgedV8<'j>),
    ObservePrepare(LiveContinueFailureV8<'j>),
    ObserveSession {
        owner: LiveOwnedContinueAppendV8<'j>,
        error: SourceJournalError,
    },
    ObserveAppend(LiveOwnedContinueAppendFailureV8<'j>),
    ObserveAdvance(LiveContinueFailureV8<'j>),
    ObserveAcknowledged(LiveContinueAcknowledgedV8<'j>),
}

/// Advances an actual Step::Continue owner through its two fixed durable
/// acknowledgements. A future public driver may call this narrow boundary;
/// it accepts no snapshot and exposes no append receipt as source authority.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continue_v8<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    moved: LiveMovedStepV8<'j>,
) -> Result<LiveObservedContinueV8<'j>, LiveContinueDriverFailureV8<'j>> {
    let state_append = moved
        .prepare_continue()
        .map_err(LiveContinueDriverFailureV8::Prepare)?;
    let state_session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinueDriverFailureV8::StateSession {
                owner: state_append,
                error,
            });
        }
    };
    let state_ack = state_session
        .append_owned_continue(state_append)
        .map_err(LiveContinueDriverFailureV8::StateAppend)?;
    let state = match state_ack
        .advance_continue()
        .map_err(LiveContinueDriverFailureV8::StateAdvance)?
    {
        LiveContinueAcknowledgedV8::State(state) => state,
        acknowledged => return Err(LiveContinueDriverFailureV8::StateAcknowledged(acknowledged)),
    };
    let observe_append = state
        .prepare_observe()
        .map_err(LiveContinueDriverFailureV8::ObservePrepare)?;
    let observe_session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinueDriverFailureV8::ObserveSession {
                owner: observe_append,
                error,
            });
        }
    };
    let observe_ack = observe_session
        .append_owned_continue(observe_append)
        .map_err(LiveContinueDriverFailureV8::ObserveAppend)?;
    match observe_ack
        .advance_continue()
        .map_err(LiveContinueDriverFailureV8::ObserveAdvance)?
    {
        LiveContinueAcknowledgedV8::Observed(observed) => Ok(observed),
        acknowledged => Err(LiveContinueDriverFailureV8::ObserveAcknowledged(acknowledged)),
    }
}
impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_owned_continue(
        self,
        obligation: LiveOwnedContinueAppendV8<'j>,
    ) -> Result<VerifiedOwnedContinueAppendV8<'j>, LiveOwnedContinueAppendFailureV8<'j>> {
        let same_journal = obligation.belongs_to(self.journal);
        let predecessor = match (|| {
            if !same_journal
                || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(obligation.selected_row(),
                    EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStateCommitted{..})
                    |EntryV8::Ordinary(SourceJournalEntry::StageReservation{role:crate::live_invocation::source_journal::SourceStageRole::Observe,attempt:None,..}))
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
                return Err(LiveOwnedContinueAppendFailureV8::Before {
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
                return Err(LiveOwnedContinueAppendFailureV8::Before {
                    _obligation: obligation,
                    _session: self,
                    error,
                });
            }
        };
        let selected = permit.selected_row().clone();
        let (pending, verified, attempting) = match self.begin_fixed_continue_append(&permit) {
            Ok(completion) => completion,
            Err(failure) => {
                return Err(LiveOwnedContinueAppendFailureV8::Append {
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
        let witness = VerifiedOwnedContinueSuccessorV8 {
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
            return Err(LiveOwnedContinueAppendFailureV8::Acknowledged {
                _obligation: obligation,
                _session: session,
                _witness: witness,
                error,
            });
        }
        attempting.complete.set(true);
        drop(attempting); // continuation postguard requires current cancellation/clock
        let envelope = VerifiedOwnedContinueAppendV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = envelope.validate_live() {
            return Err(LiveOwnedContinueAppendFailureV8::After {
                _verified: envelope,
                error,
            });
        }
        Ok(envelope)
    }
}

#[cfg(test)]
mod tests;
