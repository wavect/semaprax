//! Two real ACKs retain the actual Staged/ledger/token; no cleanup authority.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::CheckedLiveOwnedEffectSettlementV8;
use crate::live_invocation::source_journal::owned_wait_v8::{
    append::VerifiedOwnedContinuedSettlementSuccessorV8, candidate::InventoryV8, model::OwnedBodyV8,
};
struct Ack<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct Phase<'j> {
    owner: LiveDispatchedContinuedEffectV8<'j>,
    facts: CheckedLiveOwnedEffectSettlementV8,
    acks: Vec<Ack<'j>>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedContinuedSettlementAppendV8<
    'j,
> {
    phase: Phase<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveSettledContinuedEffectV8<
    'j,
> {
    phase: Phase<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveRecordedContinuedEffectV8<
    'j,
> {
    phase: Phase<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedSettlementAcknowledgedV8<
    'j,
> {
    Settled(LiveSettledContinuedEffectV8<'j>),
    Recorded(LiveRecordedContinuedEffectV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedSettlementAcknowledgmentFailureV8<
    'j,
> {
    Dispatch {
        owner: LiveDispatchedContinuedEffectV8<'j>,
        error: SourceJournalError,
    },
    Selected {
        phase: Phase<'j>,
        error: SourceJournalError,
    },
    Before {
        owner: LiveOwnedContinuedSettlementAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        error: SourceJournalError,
    },
    After {
        phase: Phase<'j>,
        error: SourceJournalError,
    },
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(DIGITS[(b >> 4) as usize] as char);
        s.push(DIGITS[(b & 15) as usize] as char);
    }
    s
}
fn recorded(
    facts: &CheckedLiveOwnedEffectSettlementV8,
    sequence: usize,
) -> Result<EntryV8, SourceJournalError> {
    let (turn, attempt) = match facts.ordinary() {
        SourceJournalEntry::EffectObserved { turn, attempt, .. }
        | SourceJournalEntry::EffectFailed { turn, attempt, .. } => (*turn, *attempt),
        _ => return Err(SourceJournalError::Binding),
    };
    Ok(EntryV8::Owned(OwnedBodyV8::OwnedEffectSettlementRecorded {
        turn,
        attempt,
        intent: facts.intent(),
        settlement: sequence
            .checked_sub(1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(SourceJournalError::Capacity)?,
        evidence: hex(facts.evidence()),
        evidence_digest: facts.evidence_digest().into(),
        result_wire: facts.result().map(hex),
    }))
}
fn equal(
    actual: &CheckedLiveOwnedEffectSettlementV8,
    expected: &CheckedLiveOwnedEffectSettlementV8,
) -> bool {
    actual.ordinary() == expected.ordinary()
        && actual.evidence() == expected.evidence()
        && actual.evidence_digest() == expected.evidence_digest()
        && actual.result() == expected.result()
        && actual.intent() == expected.intent()
}
impl<'j> LiveDispatchedContinuedEffectV8<'j> {
    fn facts_at(
        &self,
        previous: Option<(
            &AppendSessionV8<'j>,
            &VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        )>,
        current: Option<(
            &AppendSessionV8<'j>,
            &VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        )>,
    ) -> Result<CheckedLiveOwnedEffectSettlementV8, SourceJournalError> {
        let owner = &self.phase.owner.owner;
        let prior = owner.acks.last().ok_or(SourceJournalError::Order)?;
        owner
            .authorization
            .actual()?
            .owner
            .continued_settlement_facts(
                &prior.session,
                &prior.witness,
                &self.phase.ack.session,
                &self.phase.ack.witness,
                owner.authorization.proposal()?,
                &owner.commitments,
                owner.preparation_references()?,
                previous,
                current,
            )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_settlement(
        self,
    ) -> Result<
        LiveOwnedContinuedSettlementAppendV8<'j>,
        LiveContinuedSettlementAcknowledgmentFailureV8<'j>,
    > {
        let facts = match self.validate_live().and_then(|_| self.facts_at(None, None)) {
            Ok(f) => f,
            Err(error) => {
                return Err(LiveContinuedSettlementAcknowledgmentFailureV8::Dispatch {
                    owner: self,
                    error,
                })
            }
        };
        let selected = EntryV8::Ordinary(facts.ordinary().clone());
        Ok(LiveOwnedContinuedSettlementAppendV8 {
            phase: Phase {
                owner: self,
                facts,
                acks: Vec::with_capacity(2),
            },
            selected,
        })
    }
}
impl<'j> Phase<'j> {
    fn current(&self) -> &AppendSessionV8<'j> {
        self.acks
            .last()
            .map_or(&self.owner.phase.ack.session, |ack| &ack.session)
    }
    fn validate_at(
        &self,
        previous: Option<(
            &AppendSessionV8<'j>,
            &VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        )>,
        current: Option<(
            &AppendSessionV8<'j>,
            &VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        )>,
    ) -> Result<(), SourceJournalError> {
        (|| {
            let facts = self.owner.facts_at(previous, current)?;
            if !equal(&facts, &self.facts) {
                return Err(SourceJournalError::Binding);
            }
            if let Some((_, witness)) = current {
                let expected = if let Some((previous, _)) = previous {
                    recorded(&facts, previous.sequence())?
                } else {
                    EntryV8::Ordinary(facts.ordinary().clone())
                };
                if witness.selected_row() != &expected {
                    return Err(SourceJournalError::Binding);
                }
            }
            Ok(())
        })()
        .inspect_err(|_| self.owner.journal().quarantine())
    }
    fn validate_live(&self) -> Result<(), SourceJournalError> {
        match self.acks.as_slice() {
            [] => self.validate_at(None, None),
            [current] => self.validate_at(None, Some((&current.session, &current.witness))),
            [previous, current] => self.validate_at(
                Some((&previous.session, &previous.witness)),
                Some((&current.session, &current.witness)),
            ),
            _ => Err(SourceJournalError::Order),
        }
    }
}
impl<'j> LiveSettledContinuedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_recorded(
        self,
    ) -> Result<
        LiveOwnedContinuedSettlementAppendV8<'j>,
        LiveContinuedSettlementAcknowledgmentFailureV8<'j>,
    > {
        let result = self.phase.validate_live().and_then(|_| {
            if self.phase.acks.len() == 1 {
                recorded(&self.phase.facts, self.phase.current().sequence())
            } else {
                Err(SourceJournalError::Order)
            }
        });
        match result {
            Ok(selected) => Ok(LiveOwnedContinuedSettlementAppendV8 {
                phase: self.phase,
                selected,
            }),
            Err(error) => Err(LiveContinuedSettlementAcknowledgmentFailureV8::Selected {
                phase: self.phase,
                error,
            }),
        }
    }
}
impl LiveRecordedContinuedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        if self.phase.acks.len() != 2 {
            return Err(SourceJournalError::Order);
        }
        self.phase.validate_live()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        self.phase.owner.accounting()
    }
}
impl<'j> LiveOwnedContinuedSettlementAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected(&self) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.phase.current().sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.phase.current().acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.phase.owner.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        (|| {
            self.phase.validate_live()?;
            let expected = match self.phase.acks.len() {
                0 => EntryV8::Ordinary(self.phase.facts.ordinary().clone()),
                1 => recorded(&self.phase.facts, self.sequence())?,
                _ => return Err(SourceJournalError::Order),
            };
            if expected != self.selected {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })()
        .inspect_err(|_| self.phase.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
    ) -> Result<(), SourceJournalError> {
        (|| {
            witness.validate_predecessor(
                self.phase.owner.journal(),
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            let previous = match self.phase.acks.as_slice() {
                [] => None,
                [previous] => Some((&previous.session, &previous.witness)),
                _ => return Err(SourceJournalError::Order),
            };
            self.phase.validate_at(previous, Some((session, witness)))
        })()
        .inspect_err(|_| self.phase.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinuedSettlementAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedContinuedSettlementAppendPermitV8 { owner: self })
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedContinuedSettlementAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveOwnedContinuedSettlementAppendV8<'j>,
}
impl<'j> FixedOwnedContinuedSettlementAppendPermitV8<'_, 'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.owner.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_preflight(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        self.owner.validate_live()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_candidate(
        &self,
        row: &EntryV8,
        inventory: &InventoryV8<'_>,
    ) -> Result<(), SourceJournalError> {
        if row != self.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        self.validate_selected_prefix(self.owner.phase.owner.journal(), inventory)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &InventoryV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .phase
            .owner
            .phase
            .owner
            .owner
            .authorization
            .actual()?
            .owner
            .validate_continued_settlement_append_prefix(journal, inventory, &self.owner.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .phase
            .owner
            .phase
            .owner
            .owner
            .authorization
            .actual()?
            .owner
            .advance_continued_settlement_registry(witness, session)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_continued_settlement_v8<
    'j,
>(
    owner: LiveOwnedContinuedSettlementAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
) -> Result<
    LiveContinuedSettlementAcknowledgedV8<'j>,
    LiveContinuedSettlementAcknowledgmentFailureV8<'j>,
> {
    if let Err(error) = owner.validate_successor(&witness, &session) {
        return Err(LiveContinuedSettlementAcknowledgmentFailureV8::Before {
            owner,
            session,
            witness,
            error,
        });
    }
    let LiveOwnedContinuedSettlementAppendV8 { mut phase, .. } = owner;
    phase.acks.push(Ack { session, witness });
    if let Err(error) = phase.validate_live() {
        return Err(LiveContinuedSettlementAcknowledgmentFailureV8::After { phase, error });
    }
    match phase.acks.len() {
        1 => Ok(LiveContinuedSettlementAcknowledgedV8::Settled(
            LiveSettledContinuedEffectV8 { phase },
        )),
        2 => Ok(LiveContinuedSettlementAcknowledgedV8::Recorded(
            LiveRecordedContinuedEffectV8 { phase },
        )),
        _ => Err(LiveContinuedSettlementAcknowledgmentFailureV8::After {
            phase,
            error: SourceJournalError::Order,
        }),
    }
}
#[cfg(all(test, unix))]
mod tests;

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod cleanup;
