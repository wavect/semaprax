//! Fixed ContinuedSettlement ACK lineage. The witness is never an owner or dispatch grant.
use super::super::live_upstream::{
    advance_verified_continued_settlement_v8, LiveContinuedSettlementAcknowledgedV8,
    LiveContinuedSettlementAcknowledgmentFailureV8, LiveOwnedContinuedSettlementAppendV8,
};
use super::owned_effect::OwnedEffectAppendCursorV8;
use super::*;

/// Only the fixed physical adapter constructs this after persisted/reread ACK.
/// No Clone, public constructor or independently detachable cursor exists.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedContinuedSettlementSuccessorV8<
    'j,
> {
    predecessor: OwnedEffectAppendCursorV8<'j>,
    successor: OwnedEffectAppendCursorV8<'j>,
    selected: EntryV8,
}
impl VerifiedOwnedContinuedSettlementSuccessorV8<'_> {
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
            || !matches!(
                selected,
                EntryV8::Ordinary(
                    SourceJournalEntry::EffectObserved { .. }
                        | SourceJournalEntry::EffectFailed { .. }
                ) | EntryV8::Owned(
                    super::super::model::OwnedBodyV8::OwnedEffectSettlementRecorded { .. }
                )
            )
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
        let (_, _, _, selected) = session.inventory.continued_settlement_facts()?;
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
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedContinuedSettlementAppendV8<
    'j,
> {
    obligation: LiveOwnedContinuedSettlementAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
}
impl<'j> VerifiedOwnedContinuedSettlementAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.obligation
            .validate_successor(&self.witness, &self.session)
            .inspect_err(|_| self.session.journal.quarantine())
    }
    /// Closed move into the actual engine ACK consumer; no host or parts API.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_settlement(
        self,
    ) -> Result<
        LiveContinuedSettlementAcknowledgedV8<'j>,
        LiveContinuedSettlementAcknowledgmentFailureV8<'j>,
    > {
        let Self {
            obligation,
            session,
            witness,
        } = self;
        advance_verified_continued_settlement_v8(obligation, session, witness)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveOwnedContinuedSettlementAppendFailureV8<
    'j,
> {
    Before {
        _obligation: LiveOwnedContinuedSettlementAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        _obligation: LiveOwnedContinuedSettlementAppendV8<'j>,
        _failure: AppendFailureV8<'j>,
    },
    Acknowledged {
        _obligation: LiveOwnedContinuedSettlementAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        error: SourceJournalError,
    },
    After {
        _verified: VerifiedOwnedContinuedSettlementAppendV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_owned_continued_settlement(
        self,
        obligation: LiveOwnedContinuedSettlementAppendV8<'j>,
    ) -> Result<
        VerifiedOwnedContinuedSettlementAppendV8<'j>,
        LiveOwnedContinuedSettlementAppendFailureV8<'j>,
    > {
        let same_journal = obligation.belongs_to(self.journal);
        let predecessor = match (|| {
            if !same_journal
                || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(
                    obligation.selected(),
                    EntryV8::Ordinary(
                        SourceJournalEntry::EffectObserved { .. }
                            | SourceJournalEntry::EffectFailed { .. }
                    ) | EntryV8::Owned(
                        super::super::model::OwnedBodyV8::OwnedEffectSettlementRecorded { .. }
                    )
                )
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
                return Err(LiveOwnedContinuedSettlementAppendFailureV8::Before {
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
                return Err(LiveOwnedContinuedSettlementAppendFailureV8::Before {
                    _obligation: obligation,
                    _session: self,
                    error,
                });
            }
        };
        let selected = permit.selected_row().clone();
        let (pending, verified, attempting) =
            match self.begin_fixed_continued_settlement_append(&permit) {
                Ok(completion) => completion,
                Err(failure) => {
                    return Err(LiveOwnedContinuedSettlementAppendFailureV8::Append {
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
        let witness = VerifiedOwnedContinuedSettlementSuccessorV8 {
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
            return Err(LiveOwnedContinuedSettlementAppendFailureV8::Acknowledged {
                _obligation: obligation,
                _session: session,
                _witness: witness,
                error,
            });
        }
        attempting.complete.set(true);
        drop(attempting); // continuation postguard requires current cancellation/clock
        let envelope = VerifiedOwnedContinuedSettlementAppendV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = envelope.validate_live() {
            return Err(LiveOwnedContinuedSettlementAppendFailureV8::After {
                _verified: envelope,
                error,
            });
        }
        Ok(envelope)
    }
}

mod funnel;

#[cfg(test)]
impl LiveOwnedContinuedSettlementAppendFailureV8<'_> {
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

impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_settlement_facts(
        &self,
    ) -> Result<(u64, u32, u32, &EntryV8), SourceJournalError> {
        self.inventory.continued_settlement_facts()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_settlement_accounting(
        &self,
    ) -> Result<
        crate::agent_lifecycle::authorization::target_protocol::TargetAccounting,
        SourceJournalError,
    > {
        self.inventory.continued_model_accounting()
    }
}

impl VerifiedOwnedContinuedSettlementSuccessorV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_actual_predecessor(
        &self,
        prior: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        if !prior.inventory.belongs_to_context(&prior.journal.context) {
            return Err(SourceJournalError::Binding);
        }
        self.validate_previous_registry(
            prior.journal,
            prior.sequence(),
            prior.acknowledged_bytes(),
            prior.inventory.authentication_tail(),
        )
    }
}
impl AppendSessionV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn check_continued_effect_settlement(&self,inputs:crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::OwnedEffectSettlementInputsV8<'_>,ordinary:&SourceJournalEntry,evidence:&[u8],result:Option<&[u8]>)->Result<crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::CheckedOwnedEffectSettlementV8,SourceJournalError>{
        self.inventory
            .check_continued_effect_settlement(inputs, ordinary, evidence, result)
    }
}

#[cfg(test)]
impl SourceOwnedWaitJournalV8 {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_continued_settlement_encoded(
        &self,
        owner: &LiveOwnedContinuedSettlementAppendV8<'_>,
        prefix: &[u8],
    ) -> Vec<u8> {
        let line = prefix.split_inclusive(|b| *b == b'\n').last().unwrap();
        let mac = wire::parse(&line[..line.len() - 1]).unwrap()["authentication"]
            .as_str()
            .unwrap()
            .to_owned();
        wire::encode(
            owner.selected(),
            &ExpectedRowV8 {
                invocation: self.context().ordinary().invocation(),
                generation: self.context().generation(),
                seq: u32::try_from(owner.sequence()).unwrap(),
                prev_mac: &mac,
                ordinary: self.context().ordinary(),
            },
            &self.key,
        )
        .unwrap()
    }
}
