//! Fixed observer-failed State ACK lineage. Fresh physical checks are performed
//! through the actual owner seal, never the normal poisoned cursor guard. The witness is never an owner or dispatch grant.
use super::*;

/// Only the fixed physical adapter constructs this after persisted/reread ACK.
/// No Clone, public constructor or independently detachable cursor exists.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedObserverFailedStateSuccessorV8<
    'j,
> {
    predecessor: OwnedEffectAppendCursorV8<'j>,
    successor: OwnedEffectAppendCursorV8<'j>,
    selected: EntryV8,
}
impl VerifiedObserverFailedStateSuccessorV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_predecessor(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        bytes: usize,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        self.validate_predecessor_data(journal, sequence, bytes, selected)?;
        Ok(())
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
                EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectObserverFailureStateCleanupStarted{..}
                    |crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectObserverFailureStateCleanupSettled{..})
                |EntryV8::Ordinary(SourceJournalEntry::Stop{..}))
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
    /// Callback-free comparison to the actual acknowledged session. This
    /// comparison never touches a file or reenters the append marker.
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
        let (_, _, _, _, selected) = session.inventory.observer_state_facts()?;
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
}

use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::observer_failed_state::state::{
    advance_verified_observer_state_v8, LiveObserverStateAcknowledgedV8,
    LiveObserverStateFailureV8, LiveObserverStateAppendV8,
};

/// The actual unchanged obligation is first. Session/witness never stand alone.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedObserverStateAppendV8<
    'j,
> {
    obligation: LiveObserverStateAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedObserverFailedStateSuccessorV8<'j>,
}
impl<'j> VerifiedObserverStateAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.obligation
            .validate_successor(&self.witness, &self.session)
            .inspect_err(|_| self.session.journal.quarantine())
    }
    /// Closed move into the actual engine ACK consumer; no host or parts API.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_observer_state(
        self,
    ) -> Result<LiveObserverStateAcknowledgedV8<'j>, LiveObserverStateFailureV8<'j>> {
        let Self {
            obligation,
            session,
            witness,
        } = self;
        advance_verified_observer_state_v8(obligation, session, witness)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveObserverStateAppendFailureV8<
    'j,
> {
    Before {
        _obligation: LiveObserverStateAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        _obligation: LiveObserverStateAppendV8<'j>,
        _failure: AppendFailureV8<'j>,
    },
    Acknowledged {
        _obligation: LiveObserverStateAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedObserverFailedStateSuccessorV8<'j>,
        error: SourceJournalError,
    },
    After {
        _verified: VerifiedObserverStateAppendV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_observer_state(
        self,
        obligation: LiveObserverStateAppendV8<'j>,
    ) -> Result<VerifiedObserverStateAppendV8<'j>, LiveObserverStateAppendFailureV8<'j>> {
        let same_journal = obligation.belongs_to(self.journal);
        let predecessor = match (|| {
            if !same_journal
                || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(obligation.selected_row(),
                    EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectObserverFailureStateCleanupStarted{..}
                    |crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectObserverFailureStateCleanupSettled{..})
                    |EntryV8::Ordinary(SourceJournalEntry::Stop{..}))
            {
                return Err(SourceJournalError::Binding);
            }
            obligation.validate_live()?;
            Ok(OwnedEffectAppendCursorV8::capture(&self))
        })() {
            Ok(cursor) => cursor,
            Err(error) => {
                // Harmless wrong-container preflight does not poison that other
                // container. Stale actual same-container ownership retires it.
                if same_journal {
                    self.journal.quarantine();
                }
                return Err(LiveObserverStateAppendFailureV8::Before {
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
                return Err(LiveObserverStateAppendFailureV8::Before {
                    _obligation: obligation,
                    _session: self,
                    error,
                });
            }
        };
        let selected = permit.selected_row().clone();
        let (pending, verified, attempting) = match self.begin_fixed_observer_state_append(&permit)
        {
            Ok(completion) => completion,
            Err(failure) => {
                return Err(LiveObserverStateAppendFailureV8::Append {
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
        let witness = VerifiedObserverFailedStateSuccessorV8 {
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
            return Err(LiveObserverStateAppendFailureV8::Acknowledged {
                _obligation: obligation,
                _session: session,
                _witness: witness,
                error,
            });
        }
        attempting.complete.set(true);
        drop(attempting); // incurred cleanup guard deliberately excludes cancel/clock
        let envelope = VerifiedObserverStateAppendV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = envelope.validate_live() {
            return Err(LiveObserverStateAppendFailureV8::After {
                _verified: envelope,
                error,
            });
        }
        Ok(envelope)
    }
}
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests;
