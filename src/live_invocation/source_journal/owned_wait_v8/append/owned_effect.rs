//! Actual Ready-only append. A persisted row never supplies an owner or host grant.
use super::*;

mod consumed;
mod intent;
mod settlement;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::LiveOwnedEffectAppendV8;
pub(super) use consumed::OwnedReduceHoldPhaseV8;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use consumed::{
    HeldOwnedAuthorizationConsumedV8, LiveOwnedAuthorizationConsumedAppendFailureV8,
    ProspectiveOwnedReduceHoldV8, ReduceHoldRejectionV8,
    VerifiedOwnedAuthorizationConsumedSuccessorV8, VerifiedOwnedAuthorizationConsumedV8,
};
pub(in crate::live_invocation::source_journal::owned_wait_v8) use intent::{
    LiveOwnedEffectIntentAppendFailureV8, VerifiedOwnedEffectIntentAppendV8,
    VerifiedOwnedEffectIntentSuccessorV8,
};
pub(in crate::live_invocation::source_journal::owned_wait_v8) use settlement::{
    LiveOwnedEffectSettlementAppendFailureV8, VerifiedOwnedEffectSettlementAppendV8,
    VerifiedOwnedEffectSettlementSuccessorV8,
};

/// Move-only exact prefix, bound to the immutable actual E/B/store context.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct OwnedEffectAppendCursorV8<'j> {
    journal: &'j SourceOwnedWaitJournalV8,
    sequence: usize,
    bytes: usize,
    authentication: String,
}
impl<'j> OwnedEffectAppendCursorV8<'j> {
    fn capture(session: &AppendSessionV8<'j>) -> Self {
        Self {
            journal: session.journal,
            sequence: session.sequence(),
            bytes: session.acknowledged_bytes(),
            authentication: session.inventory.authentication_tail().into(),
        }
    }
    fn validate_current(&self) -> Result<(), SourceJournalError> {
        self.journal.validate_guard()?;
        let current = self.journal.begin_session()?;
        if current.sequence() != self.sequence
            || current.acknowledged_bytes() != self.bytes
            || current.inventory.authentication_tail() != self.authentication
        {
            return Err(SourceJournalError::Order);
        }
        self.journal.validate_guard()
    }
}

/// Constructed only after the selected row's fixed physical append succeeds.
/// Neither this witness nor its envelope is Clone or independently detachable.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedEffectReadySuccessorV8<
    'j,
> {
    predecessor: OwnedEffectAppendCursorV8<'j>,
    successor: OwnedEffectAppendCursorV8<'j>,
    selected: EntryV8,
}
impl VerifiedOwnedEffectReadySuccessorV8<'_> {
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
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedEffectAppendV8<'j>
{
    obligation: LiveOwnedEffectAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedEffectReadySuccessorV8<'j>,
}
impl<'j> VerifiedOwnedEffectAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.obligation
            .validate_ready_successor(&self.witness)
            .inspect_err(|_| {
                self.session.journal.quarantine();
            })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_ready(
        self,
    ) -> Result<
        super::super::live_upstream::effect::authorization::LiveAuthorizationConsumedAppendV8<'j>,
        super::super::live_upstream::effect::authorization::LiveReadyAdvanceFailureV8<'j>,
    > {
        let Self {
            obligation,
            session,
            witness,
        } = self;
        super::super::live_upstream::effect::authorization::advance_verified_ready_v8(
            obligation, session, witness,
        )
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveOwnedEffectAppendFailureV8<
    'j,
> {
    Before {
        _obligation: LiveOwnedEffectAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        _obligation: LiveOwnedEffectAppendV8<'j>,
        _failure: AppendFailureV8<'j>,
    },
    After {
        _verified: VerifiedOwnedEffectAppendV8<'j>,
        error: SourceJournalError,
    },
}

impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn effect_cursor(
        &self,
    ) -> Result<OwnedEffectAppendCursorV8<'j>, SourceJournalError> {
        let cursor = OwnedEffectAppendCursorV8::capture(self);
        cursor.validate_current()?;
        Ok(cursor)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_owned_effect(
        self,
        obligation: LiveOwnedEffectAppendV8<'j>,
    ) -> Result<VerifiedOwnedEffectAppendV8<'j>, LiveOwnedEffectAppendFailureV8<'j>> {
        let prepared = (|| {
            if !obligation.belongs_to(self.journal)
                || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(
                    obligation.selected_row(),
                    EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationReady { .. })
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
                return Err(LiveOwnedEffectAppendFailureV8::Before {
                    _obligation: obligation,
                    _session: self,
                    error,
                })
            }
        };
        // Clone only inert canonical metadata; the actual obligation moves once.
        let selected = obligation.selected_row().clone();
        let session = match self.append(selected.clone()) {
            Ok(session) => session,
            Err(failure) => {
                return Err(LiveOwnedEffectAppendFailureV8::Append {
                    _obligation: obligation,
                    _failure: failure,
                })
            }
        };
        // This construction is reachable only through actual persisted/reread ACK.
        let witness = VerifiedOwnedEffectReadySuccessorV8 {
            predecessor,
            successor: OwnedEffectAppendCursorV8::capture(&session),
            selected,
        };
        let verified = VerifiedOwnedEffectAppendV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = verified.validate_live() {
            verified.session.journal.quarantine();
            return Err(LiveOwnedEffectAppendFailureV8::After {
                _verified: verified,
                error,
            });
        }
        Ok(verified)
    }
}

#[cfg(test)]
mod tests;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use settlement::cleanup::{
    LiveOwnedEffectCleanupAppendFailureV8, VerifiedOwnedEffectCleanupAppendV8,
    VerifiedOwnedEffectCleanupSuccessorV8,
};

#[cfg(test)]
pub(in crate::live_invocation::source_journal::owned_wait_v8) use settlement::cleanup::reduce::test_evaluated;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use settlement::cleanup::reduce::VerifiedOwnedReduceReservationSuccessorV8;
#[cfg(test)]
pub(in crate::live_invocation::source_journal::owned_wait_v8) use settlement::cleanup::tests::test_executed;

#[cfg(test)]
pub(in crate::live_invocation::source_journal::owned_wait_v8) use settlement::cleanup::reduce::test_evaluated_failed;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use settlement::cleanup::reduce::step::VerifiedOwnedStepSuccessorV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use settlement::cleanup::failed_state::VerifiedFailedEffectStateSuccessorV8;
#[cfg(test)]
pub(in crate::live_invocation::source_journal::owned_wait_v8) use settlement::cleanup::{test_failed_target,TestFailedTargetV8};

pub(in crate::live_invocation::source_journal::owned_wait_v8) use settlement::cleanup::reduce::step::VerifiedOwnedContinueSuccessorV8;

#[cfg(test)]
pub(in crate::live_invocation::source_journal::owned_wait_v8) use settlement::cleanup::tests::test_observer_failed_receipt;
