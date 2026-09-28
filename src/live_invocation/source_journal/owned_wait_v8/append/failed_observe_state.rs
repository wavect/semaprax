//! Fixed failed-Observe State ACKs. No owner reconstruction or result delivery.
use super::super::live_upstream::{
    advance_verified_failed_observe_state_v8, LiveFailedObserveStateAcknowledgedV8,
    LiveFailedObserveStateAppendV8, LiveFailedObserveStateFailureV8,
};
use super::owned_effect::OwnedEffectAppendCursorV8;
use super::*;

/// Only the fixed physical adapter constructs this after persisted/reread ACK.
/// No Clone, public constructor or independently detachable cursor exists.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedFailedObserveStateSuccessorV8<
    'j,
> {
    predecessor: OwnedEffectAppendCursorV8<'j>,
    successor: OwnedEffectAppendCursorV8<'j>,
    selected: EntryV8,
}
impl VerifiedFailedObserveStateSuccessorV8<'_> {
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
                EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedCleanupStarted{owner:crate::live_invocation::source_journal::owned_wait_v8::model::OwnerV8::State,attempt:None,wait:None,..}|crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedCleanupSettled{owner:crate::live_invocation::source_journal::owned_wait_v8::model::OwnerV8::State,attempt:None,wait:None,..})
                |EntryV8::Ordinary(SourceJournalEntry::Stop{turn:Some(_),attempt:None,..}))
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
        let (_, _, _, selected) = session.inventory.failed_observe_cleanup_current_facts()?;
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
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedFailedObserveStateAppendV8<
    'j,
> {
    obligation: LiveFailedObserveStateAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedFailedObserveStateSuccessorV8<'j>,
}
impl<'j> VerifiedFailedObserveStateAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.obligation
            .validate_successor(&self.witness, &self.session)
            .inspect_err(|_| self.session.journal.quarantine())
    }
    /// Closed move into the actual engine ACK consumer; no host or parts API.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_failed_observe_state(
        self,
    ) -> Result<LiveFailedObserveStateAcknowledgedV8<'j>, LiveFailedObserveStateFailureV8<'j>> {
        let Self {
            obligation,
            session,
            witness,
        } = self;
        advance_verified_failed_observe_state_v8(obligation, session, witness)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveFailedObserveStateAppendFailureV8<
    'j,
> {
    Before {
        _obligation: LiveFailedObserveStateAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        _obligation: LiveFailedObserveStateAppendV8<'j>,
        _failure: AppendFailureV8<'j>,
    },
    Acknowledged {
        _obligation: LiveFailedObserveStateAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedFailedObserveStateSuccessorV8<'j>,
        error: SourceJournalError,
    },
    After {
        _verified: VerifiedFailedObserveStateAppendV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_failed_observe_state(
        self,
        obligation: LiveFailedObserveStateAppendV8<'j>,
    ) -> Result<VerifiedFailedObserveStateAppendV8<'j>, LiveFailedObserveStateAppendFailureV8<'j>>
    {
        let same_journal = obligation.belongs_to(self.journal);
        let predecessor = match (|| {
            if !same_journal
                || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(obligation.selected_row(),
                    EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedCleanupStarted{owner:crate::live_invocation::source_journal::owned_wait_v8::model::OwnerV8::State,attempt:None,wait:None,..}|crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedCleanupSettled{owner:crate::live_invocation::source_journal::owned_wait_v8::model::OwnerV8::State,attempt:None,wait:None,..})
                    |EntryV8::Ordinary(SourceJournalEntry::Stop{turn:Some(_),attempt:None,..}))
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
                return Err(LiveFailedObserveStateAppendFailureV8::Before {
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
                return Err(LiveFailedObserveStateAppendFailureV8::Before {
                    _obligation: obligation,
                    _session: self,
                    error,
                });
            }
        };
        let selected = permit.selected_row().clone();
        let (pending, verified, attempting) =
            match self.begin_fixed_failed_observe_state_append(&permit) {
                Ok(completion) => completion,
                Err(failure) => {
                    return Err(LiveFailedObserveStateAppendFailureV8::Append {
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
        let witness = VerifiedFailedObserveStateSuccessorV8 {
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
            return Err(LiveFailedObserveStateAppendFailureV8::Acknowledged {
                _obligation: obligation,
                _session: session,
                _witness: witness,
                error,
            });
        }
        attempting.complete.set(true);
        drop(attempting); // Started/receipt postguard is incurred; Stop is full.
        let envelope = VerifiedFailedObserveStateAppendV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = envelope.validate_live() {
            return Err(LiveFailedObserveStateAppendFailureV8::After {
                _verified: envelope,
                error,
            });
        }
        Ok(envelope)
    }
}

impl AppendSessionV8<'_> {
    /// Descriptive authenticated failed-Observe basis; never an owner or ACK.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failed_observe_cleanup_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32, &str, &serde_json::Value, &EntryV8), SourceJournalError> {
        self.inventory.failed_observe_cleanup_facts()
    }
}
