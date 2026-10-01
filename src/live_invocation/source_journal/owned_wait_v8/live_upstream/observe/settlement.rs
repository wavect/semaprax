//! The live Observe owner, never journal bytes, selects consumption evidence.
use super::*;
use crate::interpreter::resumable::owned_frame::OwnedFrameFailure;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedObserveSettlementSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::settlement::ContinuedObserveSettlementV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::later::settlement::LaterObserveSettlementV8;
use crate::interpreter::resumable::ResumableChannelValue;
use crate::live_invocation::source_journal::owned_wait_v8::model::ObserveSettlementV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveObserveSettlementOwnerV8<'j>
{
    Initial(InitialObserveSettlementV8<'j>),
    Continued(ContinuedObserveSettlementV8<'j>),
    Later(LaterObserveSettlementV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct InitialObserveSettlementV8<'j>
{
    owner: LiveObserveOutcomeV8,
    held: HeldOwnedWaitStoreV8<'j>,
    journal: &'j SourceOwnedWaitJournalV8,
    session: AppendSessionV8<'j>,
    reservation: u32,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedObserveSettlementAppendV8<
    'j,
> {
    owner: LiveObserveSettlementOwnerV8<'j>,
    selected: EntryV8,
    acks: Vec<ObserveSettlementAckV8<'j>>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveSettledObserveV8<'j> {
    owner: LiveObserveSettlementOwnerV8<'j>,
    acks: Vec<ObserveSettlementAckV8<'j>>,
}
struct ObserveSettlementAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedObserveSettlementSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveObserveSettlementFailureV8<
    'j,
> {
    Selection {
        owner: LiveObserveSettlementOwnerV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveOwnedObserveSettlementAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedObserveSettlementSuccessorV8<'j>,
        error: SourceJournalError,
    },
    Settled {
        owner: LiveSettledObserveV8<'j>,
        error: SourceJournalError,
    },
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ObserveDataV8 {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) state: serde_json::Value,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) observation:
        Option<ResumableChannelValue>,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) failure:
        Option<OwnedFrameFailure>,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) consumed: u64,
}
impl InitialObserveSettlementV8<'_> {
    fn data(&self) -> Result<ObserveDataV8, SourceJournalError> {
        match &self.owner {
            LiveObserveOutcomeV8::Observed(o) => Ok(ObserveDataV8 {
                state: o
                    .live_state_facts_v8()
                    .map_err(|_| SourceJournalError::Binding)?,
                observation: Some(o.observation().clone()),
                failure: None,
                consumed: o.consumed(),
            }),
            LiveObserveOutcomeV8::Failed(f) => Ok(ObserveDataV8 {
                state: f
                    .live_state_facts_v8()
                    .map_err(|_| SourceJournalError::Binding)?,
                observation: None,
                failure: Some(f.failure().clone()),
                consumed: f.consumed(),
            }),
            _ => Err(SourceJournalError::Binding),
        }
    }
    fn guard_at(&self, seq: usize, bytes: usize) -> Result<(), SourceJournalError> {
        self.held.validate_prefix(seq, bytes)?;
        if self.cancellation.is_cancelled() {
            return Err(SourceJournalError::Binding);
        }
        let (runtime, execution) = self
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        runtime
            .owned_wait_effects_v8(execution)
            .map_err(|_| SourceJournalError::Binding)?;
        self.data()?;
        self.held.validate_prefix(seq, bytes)
    }
}
impl<'j> LiveObserveSettlementOwnerV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        match self {
            Self::Initial(i) => i.journal,
            Self::Continued(c) => c.journal(),
            Self::Later(c) => c.journal(),
        }
    }
    fn sequence(&self) -> usize {
        match self {
            Self::Initial(i) => i.session.sequence(),
            Self::Continued(c) => c.sequence(),
            Self::Later(c) => c.sequence(),
        }
    }
    fn bytes(&self) -> usize {
        match self {
            Self::Initial(i) => i.session.acknowledged_bytes(),
            Self::Continued(c) => c.bytes(),
            Self::Later(c) => c.bytes(),
        }
    }
    fn turn(&self) -> u32 {
        match self {
            Self::Initial(_) => 0,
            Self::Continued(c) => c.turn(),
            Self::Later(c) => c.turn(),
        }
    }
    fn reservation(&self) -> u32 {
        match self {
            Self::Initial(i) => i.reservation,
            Self::Continued(c) => c.reservation(),
            Self::Later(c) => c.reservation(),
        }
    }
    fn data(&self) -> Result<ObserveDataV8, SourceJournalError> {
        match self {
            Self::Initial(i) => i.data(),
            Self::Continued(c) => c.data(),
            Self::Later(c) => c.data(),
        }
    }
    fn guard_at(&self, seq: usize, bytes: usize) -> Result<(), SourceJournalError> {
        match self {
            Self::Initial(i) => i.guard_at(seq, bytes),
            Self::Continued(c) => c.guard_at(seq, bytes),
            Self::Later(c) => c.guard_at(seq, bytes),
        }
    }
    fn selected_turn_observed(&self) -> Result<EntryV8, SourceJournalError> {
        let data = self.data()?;
        let observation = data.observation.ok_or(SourceJournalError::Order)?;
        if data.failure.is_some() {
            return Err(SourceJournalError::Order);
        }
        let held = self.journal().hold()?;
        let binding = self
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?
            .1
            .wait();
        let checked = bind_owned_wait_observation_v8(
            binding,
            &held.registration().expected_facts().scope,
            &observation,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        let state = owned_wait_ordinary_state_digest_v8(binding, &data.state)
            .map_err(|_| SourceJournalError::Binding)?;
        Ok(EntryV8::Ordinary(SourceJournalEntry::TurnObserved {
            turn: self.turn(),
            state,
            observation: checked.ordinary_digest().into(),
            feedback: crate::live_invocation::identity::digest(
                b"semaprax.source-feedback.v2\0",
                b"none",
            ),
        }))
    }
    fn selected_settlement(&self) -> Result<EntryV8, SourceJournalError> {
        let data = self.data()?;
        let context = self.journal().context();
        if !context.fold().cumulative_initialization {
            return Err(SourceJournalError::Binding);
        }
        let binding = context
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?
            .1
            .wait();
        let held = self.journal().hold()?;
        let scope = &held.registration().expected_facts().scope;
        let settlement = match (data.observation, data.failure) {
            (Some(observation), None) => {
                let checked = bind_owned_wait_observation_v8(binding, scope, &observation)
                    .map_err(|_| SourceJournalError::Binding)?;
                ObserveSettlementV8::Observed {
                    observation: crate::interpreter::resumable::checkpoint::channel_json(
                        &observation,
                    ),
                    observation_digest: checked.ordinary_digest().into(),
                }
            }
            (None, Some(failure)) => ObserveSettlementV8::Failed {
                status: failure_status(&failure)?,
            },
            _ => return Err(SourceJournalError::Binding),
        };
        Ok(EntryV8::Owned(
            journal_model::OwnedBodyV8::OwnedObserveSettled {
                turn: self.turn(),
                reservation: self.reservation(),
                state_digest: wire::record_argument_digest(&data.state),
                consumed: data.consumed,
                settlement,
            },
        ))
    }
}
fn failure_status(failure: &OwnedFrameFailure) -> Result<serde_json::Value, SourceJournalError> {
    let (tag, language) = match failure {
        OwnedFrameFailure::Language(s) => (
            "language_failure",
            serde_json::from_str(&s.to_json()).map_err(|_| SourceJournalError::Binding)?,
        ),
        OwnedFrameFailure::FuelExhausted => ("fuel_exhausted", serde_json::Value::Null),
        OwnedFrameFailure::CallDepthExceeded => ("call_depth_exceeded", serde_json::Value::Null),
        OwnedFrameFailure::HostAbandoned => ("host_abandoned", serde_json::Value::Null),
        OwnedFrameFailure::EvaluationRejected => ("evaluation_rejected", serde_json::Value::Null),
        _ => return Err(SourceJournalError::Binding),
    };
    Ok(serde_json::json!({"failure":tag,"language_status":language}))
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn select_observe_settlement_v8<
    'j,
>(
    owner: LiveObserveSettlementOwnerV8<'j>,
) -> Result<LiveOwnedObserveSettlementAppendV8<'j>, LiveObserveSettlementFailureV8<'j>> {
    let selected = (|| {
        owner.guard_at(owner.sequence(), owner.bytes())?;
        owner.selected_settlement()
    })();
    match selected {
        Ok(selected) => Ok(LiveOwnedObserveSettlementAppendV8 {
            owner,
            selected,
            acks: Vec::new(),
        }),
        Err(error) => {
            owner.journal().quarantine();
            Err(LiveObserveSettlementFailureV8::Selection { owner, error })
        }
    }
}
impl<'j> LiveOwnedObserveSettlementAppendV8<'j> {
    fn selected_from_owner(&self) -> Result<EntryV8, SourceJournalError> {
        if self.acks.is_empty() {
            self.owner.selected_settlement()
        } else {
            self.owner.selected_turn_observed()
        }
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected(&self) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.acks
            .last()
            .map_or_else(|| self.owner.sequence(), |a| a.session.sequence())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.acks
            .last()
            .map_or_else(|| self.owner.bytes(), |a| a.session.acknowledged_bytes())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.owner.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let r = (|| {
            self.owner
                .guard_at(self.sequence(), self.acknowledged_bytes())?;
            if self.selected_from_owner()? != self.selected {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        r.inspect_err(|_| self.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedObserveSettlementAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(self.fixed_permit())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_permit(
        &self,
    ) -> FixedOwnedObserveSettlementAppendPermitV8<'_, 'j> {
        FixedOwnedObserveSettlementAppendPermitV8 { owner: self }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedObserveSettlementSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let r = (|| {
            witness.validate_predecessor(
                self.owner.journal(),
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_current_session(session)?;
            self.owner
                .guard_at(session.sequence(), session.acknowledged_bytes())?;
            if self.selected_from_owner()? != self.selected {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_current_session(session)
        })();
        r.inspect_err(|_| self.owner.journal().quarantine())
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedObserveSettlementAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveOwnedObserveSettlementAppendV8<'j>,
}
impl FixedOwnedObserveSettlementAppendPermitV8<'_, '_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        self.owner.selected()
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
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(journal)
            || !inventory.belongs_to_context(journal.context())
            || inventory.sequence() != self.owner.sequence()
            || inventory.acknowledged_bytes() != self.owner.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        inventory.validate_observe_settlement_prefix(&self.owner.selected)?;
        match &self.owner.owner {
            LiveObserveSettlementOwnerV8::Initial(_) => journal.validate_initial_observe_registry(),
            LiveObserveSettlementOwnerV8::Continued(c) => {
                c.validate_append_prefix(journal, inventory, &self.owner.selected)
            }
            LiveObserveSettlementOwnerV8::Later(c) => {
                c.validate_append_prefix(journal, inventory, &self.owner.selected)
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedObserveSettlementSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        match &self.owner.owner {
            LiveObserveSettlementOwnerV8::Initial(_) => {
                witness.validate_against_acknowledged_session(session)
            }
            LiveObserveSettlementOwnerV8::Continued(c) => c.advance_registry(witness, session),
            LiveObserveSettlementOwnerV8::Later(c) => c.advance_registry(witness, session),
        }
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_observe_settlement_v8<
    'j,
>(
    obligation: LiveOwnedObserveSettlementAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedObserveSettlementSuccessorV8<'j>,
) -> Result<LiveSettledObserveV8<'j>, LiveObserveSettlementFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveObserveSettlementFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    let LiveOwnedObserveSettlementAppendV8 {
        owner, mut acks, ..
    } = obligation;
    acks.push(ObserveSettlementAckV8 { session, witness });
    Ok(LiveSettledObserveV8 { owner, acks })
}

impl<'j> LiveSettledObserveV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_turn_observed(
        self,
    ) -> Result<LiveOwnedObserveSettlementAppendV8<'j>, LiveObserveSettlementFailureV8<'j>> {
        // Failed Observe has no next-observation route; retain its exact cause
        // and container for the separately admitted incurred cleanup producer.
        if self.owner.data().is_ok_and(|data| data.failure.is_some()) {
            return Err(LiveObserveSettlementFailureV8::Settled {
                owner: self,
                error: SourceJournalError::Order,
            });
        }
        let selected = (|| {
            if self.acks.len() != 1 {
                return Err(SourceJournalError::Order);
            }
            let current = self.acks.last().ok_or(SourceJournalError::Order)?;
            current.witness.validate_current_session(&current.session)?;
            self.owner.guard_at(
                current.session.sequence(),
                current.session.acknowledged_bytes(),
            )?;
            self.owner.selected_turn_observed()
        })();
        match selected {
            Ok(selected) => Ok(LiveOwnedObserveSettlementAppendV8 {
                owner: self.owner,
                selected,
                acks: self.acks,
            }),
            Err(error) => {
                self.owner.journal().quarantine();
                Err(LiveObserveSettlementFailureV8::Settled { owner: self, error })
            }
        }
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveObserveSettlementActorFailureV8<
    'j,
> {
    Source(LiveObserveSettlementFailureV8<'j>),
    Append(crate::live_invocation::source_journal::owned_wait_v8::append::observe_settlement::LiveOwnedObserveSettlementAppendFailureV8<'j>),
    Failed(LiveSettledObserveV8<'j>),
}
pub(super) fn settle_initial_observe_v8<'j>(
    owner: LiveObserveOutcomeV8,
    journal: &'j SourceOwnedWaitJournalV8,
    session: AppendSessionV8<'j>,
    held: HeldOwnedWaitStoreV8<'j>,
    reservation: u32,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
) -> Result<ObservedLiveOwnedRunV8<'j>, LiveObserveSettlementActorFailureV8<'j>> {
    let owner = LiveObserveSettlementOwnerV8::Initial(InitialObserveSettlementV8 {
        owner,
        held,
        journal,
        session,
        reservation,
        cancellation,
    });
    let obligation =
        select_observe_settlement_v8(owner).map_err(LiveObserveSettlementActorFailureV8::Source)?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveObserveSettlementActorFailureV8::Source(
                LiveObserveSettlementFailureV8::Selection {
                    owner: obligation.owner,
                    error,
                },
            ))
        }
    };
    let settled = session
        .append_owned_observe_settlement(obligation)
        .map_err(LiveObserveSettlementActorFailureV8::Append)?
        .advance_observe_settlement()
        .map_err(LiveObserveSettlementActorFailureV8::Source)?;
    let failed = match settled.owner.data() {
        Ok(data) => data.failure.is_some(),
        Err(error) => {
            return Err(LiveObserveSettlementActorFailureV8::Source(
                LiveObserveSettlementFailureV8::Settled {
                    owner: settled,
                    error,
                },
            ))
        }
    };
    if failed {
        return Err(LiveObserveSettlementActorFailureV8::Failed(settled));
    }
    let obligation = settled
        .prepare_turn_observed()
        .map_err(LiveObserveSettlementActorFailureV8::Source)?;
    let session = match journal.begin_session() {
        Ok(s) => s,
        Err(error) => {
            return Err(LiveObserveSettlementActorFailureV8::Source(
                LiveObserveSettlementFailureV8::Selection {
                    owner: obligation.owner,
                    error,
                },
            ))
        }
    };
    let settled = session
        .append_owned_observe_settlement(obligation)
        .map_err(LiveObserveSettlementActorFailureV8::Append)?
        .advance_observe_settlement()
        .map_err(LiveObserveSettlementActorFailureV8::Source)?;
    let checked = (|| {
        if settled.acks.len() != 2 {
            return Err(SourceJournalError::Order);
        }
        let current = settled.acks.last().ok_or(SourceJournalError::Order)?;
        current.witness.validate_current_session(&current.session)?;
        settled.owner.guard_at(
            current.session.sequence(),
            current.session.acknowledged_bytes(),
        )?;
        let LiveObserveSettlementOwnerV8::Initial(i) = &settled.owner else {
            return Err(SourceJournalError::Binding);
        };
        let LiveObserveOutcomeV8::Observed(o) = &i.owner else {
            return Err(SourceJournalError::Binding);
        };
        let binding = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?
            .1
            .wait();
        bind_owned_wait_observation_v8(
            binding,
            &i.held.registration().expected_facts().scope,
            o.observation(),
        )
        .map_err(|_| SourceJournalError::Binding)
    })();
    let observation = match checked {
        Ok(x) => x,
        Err(error) => {
            return Err(LiveObserveSettlementActorFailureV8::Source(
                LiveObserveSettlementFailureV8::Settled {
                    owner: settled,
                    error,
                },
            ))
        }
    };
    let LiveSettledObserveV8 { owner, mut acks } = settled;
    let LiveObserveSettlementOwnerV8::Initial(initial) = owner else {
        unreachable!("closed initial producer")
    };
    let LiveObserveOutcomeV8::Observed(owner) = initial.owner else {
        unreachable!("checked observed phase")
    };
    let ack = acks.pop().expect("exact TurnObserved ACK");
    Ok(ObservedLiveOwnedRunV8 {
        owner,
        session: ack.session,
        held: initial.held,
        journal,
        observation,
        reservation,
        observed: u32::try_from(ack.witness.sequence() - 1).expect("bounded journal"),
        cancellation,
    })
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::test_initial_observe_entry_v8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod carry;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use carry::{
    LiveContinuedWaitV8, LiveTurnCarryFailureV8,
};

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod failed_state;
