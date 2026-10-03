//! The same failed-target State obligation after a continued Model turn.
//! Only the retained physical Pending and exact cleanup ACK can enter this route.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::SettledContinuedDecisionCleanupV8;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct OriginV8<'p, 'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) journal:
        &'j SourceOwnedWaitJournalV8,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) hold:
        &'p ProspectiveOwnedReduceHoldV8<'j>,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) policy: &'j CapabilityPolicy,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) cancellation:
        &'j crate::agent_runtime::AgentCancellation,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) clock:
        &'p dyn crate::live_invocation::SourceInvocationClock,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) proposal:
        &'p CheckedOwnedWaitProposalV8,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) turn: u32,
}

pub(crate) struct ContinuedFailureLineageV8<'j> {
    cleanup: Box<SettledContinuedDecisionCleanupV8<'j>>,
    journal: &'j SourceOwnedWaitJournalV8,
    state: Value,
    operations: Value,
    reason: SourceEffectFailure,
    settlement: u32,
    recorded: u32,
    acks: Vec<FailedStateAckV8<'j>>,
}
enum OwnerV8<'j> {
    Pending(PendingOwnedEffectReceiptV8<'j>),
    Released(ReleasedFailedEffectStateV8<'j>),
    Failed(LiveFailedEffectStateReleaseFailureV8<'j>),
    InFlight,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FailedContinuedStateV8<'j> {
    lineage: ContinuedFailureLineageV8<'j>,
    owner: OwnerV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct StoppedContinuedStateV8<'j> {
    _owner: Box<FailedContinuedStateV8<'j>>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum ContinuedFailureV8<'j> {
    Before(
        Box<SettledContinuedDecisionCleanupV8<'j>>,
        SourceJournalError,
    ),
    State(Box<FailedContinuedStateV8<'j>>, SourceJournalError),
    Append(LiveFailedEffectStateAppendFailureV8<'j>),
    Advance(LiveFailedEffectStateFailureV8<'j>),
    Shape(LiveFailedEffectStateAcknowledgedV8<'j>),
    Session(LiveFailedEffectStateAppendV8<'j>, SourceJournalError),
}
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::LiveFailedEffectStateAppendFailureV8;
type Failure<'j> = Box<ContinuedFailureV8<'j>>;

impl<'j> ContinuedFailureLineageV8<'j> {
    fn origin(&self) -> Result<OriginV8<'_, 'j>, SourceJournalError> {
        self.cleanup.failure_origin()
    }
    fn session(&self) -> &AppendSessionV8<'j> {
        self.acks
            .last()
            .map_or(self.cleanup.failure_session(), |a| &a.session)
    }
    fn validate_at(
        &self,
        session: &AppendSessionV8<'_>,
        incurred: bool,
    ) -> Result<(), SourceJournalError> {
        let origin = self.origin()?;
        let journal = self.journal;
        if !session.belongs_to(journal)
            || !std::ptr::eq(origin.journal, journal)
            || origin.turn == 0
        {
            return Err(SourceJournalError::Binding);
        }
        origin.hold.validate_failed_state_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        let held = journal.hold()?;
        held.validate_prefix(session.sequence(), session.acknowledged_bytes())?;
        let (runtime, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &held.registration().expected_facts().scope,
            origin.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !origin.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        if !incurred {
            let ordinary = journal.context().ordinary();
            check_clock_v8(
                &held,
                session.sequence(),
                session.acknowledged_bytes(),
                origin.cancellation,
                origin.clock,
                ordinary.clock_domain(),
                ordinary.initial_millis(),
                ordinary.deadline_millis(),
            )?;
        }
        held.validate_prefix(session.sequence(), session.acknowledged_bytes())?;
        origin.hold.validate_failed_state_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )
    }
    fn validate_current(&self, incurred: bool) -> Result<(), SourceJournalError> {
        if let Some(ack) = self.acks.last() {
            ack.witness.validate_current_session(&ack.session)?;
        }
        self.validate_at(self.session(), incurred)
    }
    pub(super) fn validate_cleanup_current(&self) -> Result<(), SourceJournalError> {
        let ack = self.acks.last().ok_or(SourceJournalError::Order)?;
        if !matches!(
            ack.witness.selected_row(),
            EntryV8::Owned(OwnedBodyV8::OwnedEffectFailureStateCleanupStarted { .. })
        ) {
            return Err(SourceJournalError::Order);
        }
        self.validate_current(true)
    }
    pub(super) fn validate_actual_state(
        &self,
        helper: &CheckedOwnedFrameHelperV2,
        state: &Value,
    ) -> Result<(), SourceJournalError> {
        let (_, execution) = self
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if !helper.same_helper(execution.wait().helper())
            || state != &self.state
            || crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
                &helper.liveness().result_disposal,
            )
            .map_err(|_| SourceJournalError::Binding)?
                != self.operations
        {
            return Err(SourceJournalError::Binding);
        }
        self.validate_cleanup_current()
    }
    pub(super) fn validate_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let origin = self.origin()?;
        let (runtime, execution) = self
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let held = self.journal.hold()?;
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(execution, inputs.execution)
            || !std::ptr::eq(origin.policy, inputs.policy)
            || !std::ptr::eq(origin.cancellation, inputs.cancellation)
            || !held.same_container(&inputs.store)
            || inputs.turn != origin.turn
            || inputs.attempt != 0
            || inputs.proposal.carrier() != origin.proposal.carrier()
            || inputs.proposal.ordinary_digest() != origin.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        inputs.store.validate_prefix(
            self.session().sequence(),
            self.session().acknowledged_bytes(),
        )?;
        self.validate_cleanup_current()
    }
}
impl<'j> FailedContinuedStateV8<'j> {
    pub(super) fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.lineage.journal
    }
    pub(super) fn session(&self) -> &AppendSessionV8<'j> {
        self.lineage.session()
    }
    pub(super) fn origin(&self) -> Result<OriginV8<'_, 'j>, SourceJournalError> {
        self.lineage.origin()
    }
    fn prepare(
        mut cleanup: Box<SettledContinuedDecisionCleanupV8<'j>>,
    ) -> Result<Box<Self>, Failure<'j>> {
        let facts = (|| {
            if !cleanup.failed_target() {
                return Err(SourceJournalError::Binding);
            }
            let (reason, state, settlement, recorded) = cleanup.failed_state_facts()?;
            let origin = cleanup.failure_origin()?;
            let (_, execution) = origin
                .journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let operations = crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
                &execution.wait().helper().liveness().result_disposal,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            Ok((
                origin.journal,
                reason,
                state,
                settlement,
                recorded,
                operations,
            ))
        })();
        let (journal, reason, state, settlement, recorded, operations) = match facts {
            Ok(facts) => facts,
            Err(error) => return Err(Box::new(ContinuedFailureV8::Before(cleanup, error))),
        };
        let pending = match cleanup.take_failed_state() {
            Ok(pending) => pending,
            Err(error) => return Err(Box::new(ContinuedFailureV8::Before(cleanup, error))),
        };
        Ok(Box::new(Self {
            lineage: ContinuedFailureLineageV8 {
                cleanup,
                journal,
                state,
                operations,
                reason,
                settlement,
                recorded,
                acks: Vec::with_capacity(3),
            },
            owner: OwnerV8::Pending(pending),
        }))
    }
    fn selected(&self) -> Result<EntryV8, SourceJournalError> {
        let origin = self.origin()?;
        let turn = origin.turn;
        match (&self.owner, self.lineage.acks.as_slice()) {
            (OwnerV8::Pending(pending), []) => {
                self.lineage.validate_current(false)?;
                if pending.live_failed_state_reason_v8() != Some(self.lineage.reason)
                    || pending.live_failed_state_facts_v8()? != self.lineage.state
                {
                    return Err(SourceJournalError::Binding);
                }
                let (_, execution) = self
                    .journal()
                    .context()
                    .ready_runtime()
                    .ok_or(SourceJournalError::Binding)?;
                Ok(EntryV8::Owned(
                    OwnedBodyV8::OwnedEffectFailureStateCleanupStarted {
                        turn,
                        attempt: 0,
                        plan: execution.wait().binding().into(),
                        settlement: self.lineage.settlement,
                        recorded: self.lineage.recorded,
                        decision_cleanup_settled: true_seq(
                            self.lineage.cleanup.failure_session().sequence(),
                        )?,
                        effect_failure: self.lineage.reason.as_str().into(),
                        state_digest: wire::record_argument_digest(&self.lineage.state),
                        operations: self.lineage.operations.clone(),
                    },
                ))
            }
            (OwnerV8::Released(released), [started]) => {
                self.lineage.validate_current(true)?;
                crate::resumable_effects::owned_frame::v2::validate_owned_wait_observed_receipt_v8(
                    &self.lineage.operations,
                    released.receipt(),
                )
                .map_err(|_| SourceJournalError::Binding)?;
                Ok(EntryV8::Owned(
                    OwnedBodyV8::OwnedEffectFailureStateCleanupSettled {
                        turn,
                        attempt: 0,
                        started: true_seq(started.session.sequence())?,
                        receipt: released.receipt().clone(),
                    },
                ))
            }
            (OwnerV8::Released(released), [_, settled]) => {
                self.lineage.validate_current(false)?;
                if !matches!(settled.witness.selected_row(), EntryV8::Owned(OwnedBodyV8::OwnedEffectFailureStateCleanupSettled { receipt, .. }) if receipt == released.receipt() && receipt["settlement"] == "completed")
                {
                    return Err(SourceJournalError::Binding);
                }
                Ok(EntryV8::Ordinary(SourceJournalEntry::Stop {
                    turn: Some(turn),
                    attempt: Some(0),
                    status: SourceStopStatus::EffectFailed,
                    reason: SourceStopReason::EffectFailed,
                }))
            }
            _ => Err(SourceJournalError::Order),
        }
    }
    pub(super) fn validate_selected(&self, selected: &EntryV8) -> Result<(), SourceJournalError> {
        if &self.selected()? != selected {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(super) fn validate_successor(
        &self,
        selected: &EntryV8,
        witness: &VerifiedFailedEffectStateSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        witness.validate_predecessor(
            self.journal(),
            self.session().sequence(),
            self.session().acknowledged_bytes(),
            selected,
        )?;
        witness.validate_against_acknowledged_session(session)?;
        self.lineage.validate_at(
            session,
            !matches!(selected, EntryV8::Ordinary(SourceJournalEntry::Stop { .. })),
        )?;
        witness.validate_predecessor(
            self.journal(),
            self.session().sequence(),
            self.session().acknowledged_bytes(),
            selected,
        )
    }
    pub(super) fn acknowledge(
        &mut self,
        session: AppendSessionV8<'j>,
        witness: VerifiedFailedEffectStateSuccessorV8<'j>,
    ) {
        if matches!(witness.selected_row(), EntryV8::Owned(OwnedBodyV8::OwnedEffectFailureStateCleanupSettled { receipt, .. }) if receipt["settlement"] != "completed")
        {
            self.journal().quarantine();
        }
        self.lineage
            .acks
            .push(FailedStateAckV8 { session, witness });
    }
    fn into_append(self: Box<Self>) -> Result<LiveFailedEffectStateAppendV8<'j>, Failure<'j>> {
        match self.selected() {
            Ok(selected) => Ok(LiveFailedEffectStateAppendV8 {
                owner: FailedStateOwnerV8::Continued(self),
                selected,
            }),
            Err(error) => Err(Box::new(ContinuedFailureV8::State(self, error))),
        }
    }
    fn release(
        mut self: Box<Self>,
        observe: impl FnMut(&FinalizeAction),
    ) -> Result<Box<Self>, Failure<'j>> {
        if let Err(error) = self.lineage.validate_cleanup_current() {
            return Err(Box::new(ContinuedFailureV8::State(self, error)));
        }
        let pending = match std::mem::replace(&mut self.owner, OwnerV8::InFlight) {
            OwnerV8::Pending(pending) => pending,
            owner => {
                self.owner = owner;
                return Err(Box::new(ContinuedFailureV8::State(
                    self,
                    SourceJournalError::Order,
                )));
            }
        };
        let result = release_live_failed_effect_state_v8(
            pending,
            &LiveFailedEffectStateCleanupPermitV8::Continued(&self.lineage),
            observe,
        );
        match result {
            Ok(released) => {
                self.owner = OwnerV8::Released(released);
                Ok(self)
            }
            Err(failure) => {
                self.owner = OwnerV8::Failed(failure);
                Err(Box::new(ContinuedFailureV8::State(
                    self,
                    SourceJournalError::Binding,
                )))
            }
        }
    }
}
#[inline(never)]
fn acknowledge<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: Box<FailedContinuedStateV8<'j>>,
) -> Result<Box<FailedContinuedStateV8<'j>>, Failure<'j>> {
    let selected = owner.into_append()?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => return Err(Box::new(ContinuedFailureV8::Session(selected, error))),
    };
    let appended = session
        .append_failed_effect_state(selected)
        .map_err(|e| Box::new(ContinuedFailureV8::Append(e)))?;
    match appended
        .advance_failed_state()
        .map_err(|e| Box::new(ContinuedFailureV8::Advance(e)))?
    {
        LiveFailedEffectStateAcknowledgedV8::Continued(owner) => Ok(owner),
        owner => Err(Box::new(ContinuedFailureV8::Shape(owner))),
    }
}
/// The Started ACK precedes physical release; only acknowledged successful
/// receipt and sticky Stop permit a normal runtime close.
#[inline(never)]
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn stop_failed_continued_state<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cleanup: Box<SettledContinuedDecisionCleanupV8<'j>>,
    observe: impl FnMut(&FinalizeAction),
) -> Result<Box<StoppedContinuedStateV8<'j>>, Failure<'j>> {
    let result = (|| {
        let owner = FailedContinuedStateV8::prepare(cleanup)?;
        let owner = acknowledge(journal, owner)?;
        let owner = owner.release(observe)?;
        let owner = acknowledge(journal, owner)?;
        let owner = acknowledge(journal, owner)?;
        if let Err(error) = owner.lineage.validate_current(false) {
            return Err(Box::new(ContinuedFailureV8::State(owner, error)));
        }
        Ok(Box::new(StoppedContinuedStateV8 { _owner: owner }))
    })();
    result.inspect_err(|_| journal.quarantine())
}
