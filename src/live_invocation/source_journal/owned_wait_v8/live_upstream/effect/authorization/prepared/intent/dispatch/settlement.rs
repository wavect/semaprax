//! Actual dispatched owner to existing settlement/Recorded ACKs. No Decision
//! release, Outcome mint, reducer allowance or recovered-owner producer.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::CheckedLiveOwnedEffectSettlementV8;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedEffectSettlementSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::{
    candidate::InventoryV8, model::OwnedBodyV8,
};

// Original Intent session/permit becomes inert lineage after settlement ACK.
// Current phase carries its own ACK session and checks, never a stale Intent guard.
struct SettlementAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedEffectSettlementSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedEffectSettlementAppendV8<
    'j,
> {
    owner: LiveDispatchedOwnedEffectV8<'j>,
    facts: CheckedLiveOwnedEffectSettlementV8,
    previous: Option<SettlementAckV8<'j>>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveSettledOwnedEffectV8<'j> {
    owner: LiveDispatchedOwnedEffectV8<'j>,
    facts: CheckedLiveOwnedEffectSettlementV8,
    ack: SettlementAckV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveRecordedOwnedEffectV8<'j> {
    owner: LiveDispatchedOwnedEffectV8<'j>,
    facts: CheckedLiveOwnedEffectSettlementV8,
    settlement: SettlementAckV8<'j>,
    recorded: SettlementAckV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveEffectSettlementFailureV8<'j>
{
    Dispatch {
        _owner: LiveDispatchedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    Settled {
        _owner: LiveSettledOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    Before {
        _owner: LiveOwnedEffectSettlementAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedOwnedEffectSettlementSuccessorV8<'j>,
        error: SourceJournalError,
    },
    AfterSettlement {
        _owner: LiveSettledOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    AfterRecorded {
        _owner: LiveRecordedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveDispatchedOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_settlement(
        self,
    ) -> Result<LiveOwnedEffectSettlementAppendV8<'j>, LiveEffectSettlementFailureV8<'j>> {
        let facts = match self
            .validate_live()
            .and_then(|_| self.staged.checked_live_settlement_v8(&self.accounting))
            .and_then(|facts| {
                self.validate_live()?;
                Ok(facts)
            }) {
            Ok(facts) => facts,
            Err(error) => {
                return Err(LiveEffectSettlementFailureV8::Dispatch {
                    _owner: self,
                    error,
                })
            }
        };
        let selected = EntryV8::Ordinary(facts.ordinary().clone());
        Ok(LiveOwnedEffectSettlementAppendV8 {
            owner: self,
            facts,
            previous: None,
            selected,
        })
    }
}
impl<'j> LiveSettledOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        current(&self.owner, &self.facts, &self.ack)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_recorded(
        self,
    ) -> Result<LiveOwnedEffectSettlementAppendV8<'j>, LiveEffectSettlementFailureV8<'j>> {
        if let Err(error) = self.validate_live() {
            return Err(LiveEffectSettlementFailureV8::Settled {
                _owner: self,
                error,
            });
        }
        let selected = match recorded(&self.facts, self.ack.witness.sequence()) {
            Ok(selected) => selected,
            Err(error) => {
                return Err(LiveEffectSettlementFailureV8::Settled {
                    _owner: self,
                    error,
                })
            }
        };
        let Self { owner, facts, ack } = self;
        Ok(LiveOwnedEffectSettlementAppendV8 {
            owner,
            facts,
            previous: Some(ack),
            selected,
        })
    }
}
impl LiveRecordedOwnedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = current(&self.owner, &self.facts, &self.recorded).and_then(|_| {
            self.recorded.witness.validate_predecessor(
                self.owner.lineage.journal,
                self.settlement.session.sequence(),
                self.settlement.session.acknowledged_bytes(),
                &recorded(&self.facts, self.settlement.witness.sequence())?,
            )
        });
        result.inspect_err(|_| self.owner.staged.quarantine_live_dispatch())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        &self.owner.accounting
    }
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
        settlement: u32::try_from(sequence.checked_sub(1).ok_or(SourceJournalError::Order)?)
            .map_err(|_| SourceJournalError::Capacity)?,
        evidence: hex(facts.evidence()),
        evidence_digest: facts.evidence_digest().into(),
        result_wire: facts.result().map(hex),
    }))
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
fn current(
    owner: &LiveDispatchedOwnedEffectV8<'_>,
    facts: &CheckedLiveOwnedEffectSettlementV8,
    ack: &SettlementAckV8<'_>,
) -> Result<(), SourceJournalError> {
    current_session(owner, facts, &ack.session, &ack.witness)
}
fn current_session(
    owner: &LiveDispatchedOwnedEffectV8<'_>,
    facts: &CheckedLiveOwnedEffectSettlementV8,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedEffectSettlementSuccessorV8<'_>,
) -> Result<(), SourceJournalError> {
    let result = (|| {
        let l = &owner.lineage;
        if !session.belongs_to(l.journal) {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_current_session(&session)?;
        l.hold.validate_settlement_guard(
            l.journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        let context = l.journal.context();
        let held = l.journal.hold()?;
        check_clock_v8(
            &held,
            session.sequence(),
            session.acknowledged_bytes(),
            l.cancellation,
            l.clock,
            context.ordinary().clock_domain(),
            context.ordinary().initial_millis(),
            context.ordinary().deadline_millis(),
        )?;
        let (runtime, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &held.registration().expected_facts().scope,
            &l.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !l.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        let actual = owner.staged.checked_live_settlement_v8(&owner.accounting)?;
        if actual.ordinary() != facts.ordinary()
            || actual.evidence() != facts.evidence()
            || actual.result() != facts.result()
            || actual.intent() != facts.intent()
            || actual.evidence_digest() != facts.evidence_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        l.hold.validate_settlement_guard(
            l.journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        witness.validate_current_session(&session)
    })();
    result.inspect_err(|_| owner.staged.quarantine_live_dispatch())
}
impl<'j> LiveOwnedEffectSettlementAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_settlement_successor(
        &self,
        witness: &VerifiedOwnedEffectSettlementSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        witness.validate_predecessor(
            self.owner.lineage.journal,
            self.sequence(),
            self.acknowledged_bytes(),
            &self.selected,
        )?;
        current_session(&self.owner, &self.facts, session, witness)
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.owner.lineage.journal, journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.previous
            .as_ref()
            .map_or(self.owner.lineage.session.sequence(), |ack| {
                ack.session.sequence()
            })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.previous
            .as_ref()
            .map_or(self.owner.lineage.session.acknowledged_bytes(), |ack| {
                ack.session.acknowledged_bytes()
            })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        match &self.previous {
            None => {
                self.owner.validate_live()?;
                if self.selected != EntryV8::Ordinary(self.facts.ordinary().clone()) {
                    return Err(SourceJournalError::Binding);
                }
                self.owner.validate_live()
            }
            Some(ack) => {
                current(&self.owner, &self.facts, ack)?;
                if self.selected != recorded(&self.facts, ack.witness.sequence())? {
                    return Err(SourceJournalError::Binding);
                }
                current(&self.owner, &self.facts, ack)
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedEffectSettlementAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedEffectSettlementAppendPermitV8 { owner: self })
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedEffectSettlementAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveOwnedEffectSettlementAppendV8<'j>,
}
impl FixedOwnedEffectSettlementAppendPermitV8<'_, '_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        self.owner.selected_row()
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &InventoryV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .lineage
            .hold
            .validate_settlement_append_prefix(journal, inventory, &self.owner.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedEffectSettlementSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .lineage
            .hold
            .advance_settlement_ack(witness, session)
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveEffectSettlementAcknowledgedV8<
    'j,
> {
    Settled(LiveSettledOwnedEffectV8<'j>),
    Recorded(LiveRecordedOwnedEffectV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_settlement_v8<
    'j,
>(
    obligation: LiveOwnedEffectSettlementAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedEffectSettlementSuccessorV8<'j>,
) -> Result<LiveEffectSettlementAcknowledgedV8<'j>, LiveEffectSettlementFailureV8<'j>> {
    let valid = witness
        .validate_predecessor(
            obligation.owner.lineage.journal,
            obligation.sequence(),
            obligation.acknowledged_bytes(),
            &obligation.selected,
        )
        .and_then(|_| witness.validate_current_session(&session));
    if let Err(error) = valid {
        obligation.owner.staged.quarantine_live_dispatch();
        return Err(LiveEffectSettlementFailureV8::Before {
            _owner: obligation,
            _session: session,
            _witness: witness,
            error,
        });
    }
    let LiveOwnedEffectSettlementAppendV8 {
        owner,
        facts,
        previous,
        selected: _,
    } = obligation;
    let ack = SettlementAckV8 { session, witness };
    match previous {
        None => {
            let actual = LiveSettledOwnedEffectV8 { owner, facts, ack };
            if let Err(error) = actual.validate_live() {
                return Err(LiveEffectSettlementFailureV8::AfterSettlement {
                    _owner: actual,
                    error,
                });
            }
            Ok(LiveEffectSettlementAcknowledgedV8::Settled(actual))
        }
        Some(settlement) => {
            let actual = LiveRecordedOwnedEffectV8 {
                owner,
                facts,
                settlement,
                recorded: ack,
            };
            if let Err(error) = actual.validate_live() {
                return Err(LiveEffectSettlementFailureV8::AfterRecorded {
                    _owner: actual,
                    error,
                });
            }
            Ok(LiveEffectSettlementAcknowledgedV8::Recorded(actual))
        }
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod cleanup;
