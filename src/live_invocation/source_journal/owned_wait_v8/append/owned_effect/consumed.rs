//! Fixed AuthorizationConsumed persistence, retaining the actual Ready owner.
//! No Prepared/Intent/host, owner restoration, detachable ACK or witness factory.
use super::*;
mod reduce_hold;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::LiveAuthorizationConsumedAppendV8;
pub(in crate::live_invocation::source_journal::owned_wait_v8::append) use reduce_hold::OwnedReduceHoldPhaseV8;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use reduce_hold::{
    HeldOwnedAuthorizationConsumedV8, ProspectiveOwnedReduceHoldV8, ReduceHoldRejectionV8,
};

/// Constructed only after the selected row's fixed physical append succeeds.
/// Neither this witness nor its envelope is Clone or independently detachable.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedAuthorizationConsumedSuccessorV8<
    'j,
> {
    predecessor: OwnedEffectAppendCursorV8<'j>,
    successor: OwnedEffectAppendCursorV8<'j>,
    selected: EntryV8,
}
impl VerifiedOwnedAuthorizationConsumedSuccessorV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_predecessor(
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
            || self.successor.sequence
                != sequence
                    .checked_add(1)
                    .ok_or(SourceJournalError::Capacity)?
            || self.successor.bytes <= bytes
            || self.predecessor.authentication == self.successor.authentication
        {
            return Err(SourceJournalError::Binding);
        }
        self.successor.validate_current()
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

/// Owns the unchanged real obligation, new physical session and its exact witness.
/// No owner, witness, grant, ACK or successor cursor can be extracted.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedAuthorizationConsumedV8<
    'j,
> {
    obligation: LiveAuthorizationConsumedAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedAuthorizationConsumedSuccessorV8<'j>,
}
impl<'j> VerifiedOwnedAuthorizationConsumedV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn reserve_owned_reduce(
        self,
    ) -> Result<HeldOwnedAuthorizationConsumedV8<'j>, ReduceHoldRejectionV8<'j>> {
        reduce_hold::reserve(self)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.obligation
            .validate_consumed_successor(&self.witness)
            .inspect_err(|_| {
                self.session.journal.quarantine();
            })
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveOwnedAuthorizationConsumedAppendFailureV8<
    'j,
> {
    Before {
        _obligation: LiveAuthorizationConsumedAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        _obligation: LiveAuthorizationConsumedAppendV8<'j>,
        _failure: AppendFailureV8<'j>,
    },
    After {
        _verified: VerifiedOwnedAuthorizationConsumedV8<'j>,
        error: SourceJournalError,
    },
}

impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_owned_authorization_consumed(
        self,
        obligation: LiveAuthorizationConsumedAppendV8<'j>,
    ) -> Result<
        VerifiedOwnedAuthorizationConsumedV8<'j>,
        LiveOwnedAuthorizationConsumedAppendFailureV8<'j>,
    > {
        let same_journal = obligation.belongs_to(self.journal);
        let prepared = (|| {
            if !same_journal
                || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(
                    obligation.selected_row(),
                    EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed { .. })
                )
            {
                return Err(SourceJournalError::Binding);
            }
            obligation.validate_live()?;
            self.effect_cursor()
        })();
        let predecessor = match prepared {
            Ok(cursor) => cursor,
            Err(error) => {
                // The closed obligation cannot select a caller-supplied row. A
                // stale same-container owner lineage is permanently retired;
                // wrong-container preflight neither writes nor poisons it.
                if same_journal {
                    self.journal.quarantine();
                }
                return Err(LiveOwnedAuthorizationConsumedAppendFailureV8::Before {
                    _obligation: obligation,
                    _session: self,
                    error,
                });
            }
        };
        // Clone only inert canonical metadata; the actual obligation moves once.
        let selected = obligation.selected_row().clone();
        let session = match self.append(selected.clone()) {
            Ok(session) => session,
            Err(failure) => {
                return Err(LiveOwnedAuthorizationConsumedAppendFailureV8::Append {
                    _obligation: obligation,
                    _failure: failure,
                })
            }
        };
        // This construction is reachable only through actual persisted/reread ACK.
        let witness = VerifiedOwnedAuthorizationConsumedSuccessorV8 {
            predecessor,
            successor: OwnedEffectAppendCursorV8::capture(&session),
            selected,
        };
        let verified = VerifiedOwnedAuthorizationConsumedV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = verified.validate_live() {
            verified.session.journal.quarantine();
            return Err(LiveOwnedAuthorizationConsumedAppendFailureV8::After {
                _verified: verified,
                error,
            });
        }
        Ok(verified)
    }
}

#[cfg(test)]
mod tests;
