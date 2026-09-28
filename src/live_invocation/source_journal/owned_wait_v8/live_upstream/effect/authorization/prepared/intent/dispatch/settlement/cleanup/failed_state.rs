//! Actual failed-target State cleanup. Observed Decision failure is excluded;
//! the separate quarantined observer route is not admitted by this producer.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
    release_live_failed_effect_state_v8, LiveFailedEffectStateReleaseFailureV8,
    ReleasedFailedEffectStateV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedFailedEffectStateSuccessorV8;
use crate::live_invocation::source_journal::{
    SourceEffectFailure, SourceStopReason, SourceStopStatus,
};
use crate::resumable_effects::owned_frame::v2::CheckedOwnedFrameHelperV2;

struct FailedStateAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedFailedEffectStateSuccessorV8<'j>,
}
struct FailedStateLineageV8<'j> {
    cleanup: CleanupLineageV8<'j>,
    state: Value,
    operations: Value,
    acks: Vec<FailedStateAckV8<'j>>,
}
impl<'j> FailedStateLineageV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.cleanup.recorded.intent.journal
    }
    fn current(&self) -> Result<&FailedStateAckV8<'j>, SourceJournalError> {
        self.acks.last().ok_or(SourceJournalError::Order)
    }
    fn validate_current(&self, incurred: bool) -> Result<(), SourceJournalError> {
        if self.acks.is_empty() {
            return if incurred {
                self.cleanup.validate_cleanup()
            } else {
                self.cleanup.validate_outcome()
            };
        }
        let result = (|| {
            let ack = self.current()?;
            let origin = &self.cleanup.recorded.intent;
            let journal = self.journal();
            ack.witness.validate_current_session(&ack.session)?;
            origin.hold.validate_failed_state_guard(
                journal,
                ack.session.sequence(),
                ack.session.acknowledged_bytes(),
            )?;
            let held = journal.hold()?;
            held.validate_prefix(ack.session.sequence(), ack.session.acknowledged_bytes())?;
            let (runtime, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = plan_owned_effect_v8(
                runtime,
                execution,
                &held.registration().expected_facts().scope,
                &origin.proposal,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            if !origin.policy.allows(plan.operation().effect_id()) {
                return Err(SourceJournalError::Binding);
            }
            if !incurred {
                let ordinary = journal.context().ordinary();
                check_clock_v8(
                    &held,
                    ack.session.sequence(),
                    ack.session.acknowledged_bytes(),
                    origin.cancellation,
                    origin.clock,
                    ordinary.clock_domain(),
                    ordinary.initial_millis(),
                    ordinary.deadline_millis(),
                )?;
            }
            held.validate_prefix(ack.session.sequence(), ack.session.acknowledged_bytes())?;
            origin.hold.validate_failed_state_guard(
                journal,
                ack.session.sequence(),
                ack.session.acknowledged_bytes(),
            )?;
            ack.witness.validate_current_session(&ack.session)
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
    fn selected_failure(
        &self,
    ) -> Result<crate::live_invocation::source_journal::SourceEffectFailure, SourceJournalError>
    {
        match self.cleanup.recorded.facts.ordinary() {
            SourceJournalEntry::EffectFailed {
                turn: 0,
                attempt: 0,
                reason,
                ..
            } if matches!(
                reason,
                SourceEffectFailure::HandlerFailed | SourceEffectFailure::ResultLimit
            ) =>
            {
                Ok(*reason)
            }
            _ => Err(SourceJournalError::Binding),
        }
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveStartedFailedEffectStateV8<
    'j,
> {
    pending: PendingOwnedEffectReceiptV8<'j>,
    accounting: TargetAccounting,
    lineage: FailedStateLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveReleasedFailedEffectStateV8<
    'j,
> {
    released: ReleasedFailedEffectStateV8<'j>,
    accounting: TargetAccounting,
    lineage: FailedStateLineageV8<'j>,
}
enum FailedStateOwnerV8<'j> {
    Failed(LiveFailedOwnedEffectV8<'j>),
    Released(LiveReleasedFailedEffectStateV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveFailedEffectStateAppendV8<
    'j,
> {
    owner: FailedStateOwnerV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveFailedEffectStateAcknowledgedV8<
    'j,
> {
    Started(LiveStartedFailedEffectStateV8<'j>),
    Released(LiveReleasedFailedEffectStateV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveFailedEffectStateFailureV8<
    'j,
> {
    Failed {
        _owner: LiveFailedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    Started {
        _owner: LiveStartedFailedEffectStateV8<'j>,
        error: SourceJournalError,
    },
    Release {
        _owner: LiveFailedEffectStateReleaseFailureV8<'j>,
        _accounting: TargetAccounting,
        _lineage: FailedStateLineageV8<'j>,
    },
    Released {
        _owner: LiveReleasedFailedEffectStateV8<'j>,
        error: SourceJournalError,
    },
    Before {
        _owner: LiveFailedEffectStateAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedFailedEffectStateSuccessorV8<'j>,
        error: SourceJournalError,
    },
}
fn started(owner: &LiveFailedOwnedEffectV8<'_>) -> Result<EntryV8, SourceJournalError> {
    owner.lineage.validate_outcome()?;
    let state = owner.pending.live_failed_state_facts_v8()?;
    let l = &owner.lineage;
    let journal = l.recorded.intent.journal;
    let (_, e) = journal
        .context()
        .ready_runtime()
        .ok_or(SourceJournalError::Binding)?;
    let reason = match l.recorded.facts.ordinary() {
        SourceJournalEntry::EffectFailed {
            turn: 0,
            attempt: 0,
            reason,
            ..
        } if matches!(
            reason,
            SourceEffectFailure::HandlerFailed | SourceEffectFailure::ResultLimit
        ) =>
        {
            *reason
        }
        _ => return Err(SourceJournalError::Binding),
    };
    if owner.pending.live_failed_state_reason_v8() != Some(reason) {
        return Err(SourceJournalError::Binding);
    }
    if owner.pending.receipt()["settlement"] != "completed" || l.settled.is_none() {
        return Err(SourceJournalError::Binding);
    }
    let operations = crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
        &e.wait().helper().liveness().result_disposal,
    )
    .map_err(|_| SourceJournalError::Binding)?;
    let row = EntryV8::Owned(OwnedBodyV8::OwnedEffectFailureStateCleanupStarted {
        turn: 0,
        attempt: 0,
        plan: e.wait().binding().into(),
        settlement: true_seq(l.recorded.settlement.session.sequence())?,
        recorded: true_seq(l.recorded.recorded.session.sequence())?,
        decision_cleanup_settled: true_seq(l.current().session.sequence())?,
        effect_failure: reason.as_str().into(),
        state_digest: wire::record_argument_digest(&state),
        operations,
    });
    owner.lineage.validate_outcome()?;
    Ok(row)
}
impl<'j> LiveFailedOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_failed_state(
        self,
    ) -> Result<LiveFailedEffectStateAppendV8<'j>, LiveFailedEffectStateFailureV8<'j>> {
        match started(&self) {
            Ok(selected) => Ok(LiveFailedEffectStateAppendV8 {
                owner: FailedStateOwnerV8::Failed(self),
                selected,
            }),
            Err(error) => {
                self.lineage.recorded.intent.journal.quarantine();
                Err(LiveFailedEffectStateFailureV8::Failed {
                    _owner: self,
                    error,
                })
            }
        }
    }
}
/// Only an actual Started owner supplies this borrowed, nonconstructible permit.
pub(crate) struct LiveFailedEffectStateCleanupPermitV8<'p, 'j> {
    lineage: &'p FailedStateLineageV8<'j>,
}
impl LiveFailedEffectStateCleanupPermitV8<'_, '_> {
    pub(crate) fn validate_cleanup_current(&self) -> Result<(), SourceJournalError> {
        if !matches!(
            self.lineage.current()?.witness.selected_row(),
            EntryV8::Owned(OwnedBodyV8::OwnedEffectFailureStateCleanupStarted { .. })
        ) {
            return Err(SourceJournalError::Order);
        }
        self.lineage.validate_current(true)
    }
    pub(crate) fn validate_actual_state(
        &self,
        helper: &CheckedOwnedFrameHelperV2,
        state: &Value,
    ) -> Result<(), SourceJournalError> {
        let (_, e) = self
            .lineage
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if !helper.same_helper(e.wait().helper())
            || state != &self.lineage.state
            || crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
                &helper.liveness().result_disposal,
            )
            .map_err(|_| SourceJournalError::Binding)?
                != self.lineage.operations
        {
            return Err(SourceJournalError::Binding);
        }
        self.lineage.selected_failure()?;
        self.validate_cleanup_current()
    }
    pub(crate) fn validate_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let origin = &self.lineage.cleanup.recorded.intent;
        let journal = self.lineage.journal();
        let (runtime, e) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(e, inputs.execution)
            || !std::ptr::eq(origin.policy, inputs.policy)
            || !std::ptr::eq(origin.cancellation, inputs.cancellation)
            || inputs.turn != 0
            || inputs.attempt != 0
            || inputs.proposal.carrier() != origin.proposal.carrier()
            || inputs.proposal.ordinary_digest() != origin.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        let ack = self.lineage.current()?;
        inputs
            .store
            .validate_prefix(ack.session.sequence(), ack.session.acknowledged_bytes())?;
        self.validate_cleanup_current()
    }
}
impl<'j> LiveStartedFailedEffectStateV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn release(
        self,
        observe: impl FnMut(&FinalizeAction),
    ) -> Result<LiveReleasedFailedEffectStateV8<'j>, LiveFailedEffectStateFailureV8<'j>> {
        if let Err(error) = self.lineage.validate_current(true) {
            return Err(LiveFailedEffectStateFailureV8::Started {
                _owner: self,
                error,
            });
        }
        let Self {
            pending,
            accounting,
            lineage,
        } = self;
        match release_live_failed_effect_state_v8(
            pending,
            &LiveFailedEffectStateCleanupPermitV8 { lineage: &lineage },
            observe,
        ) {
            Ok(released) => Ok(LiveReleasedFailedEffectStateV8 {
                released,
                accounting,
                lineage,
            }),
            Err(error) => {
                lineage.journal().quarantine();
                Err(LiveFailedEffectStateFailureV8::Release {
                    _owner: error,
                    _accounting: accounting,
                    _lineage: lineage,
                })
            }
        }
    }
}
impl<'j> LiveReleasedFailedEffectStateV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_receipt(
        self,
    ) -> Result<LiveFailedEffectStateAppendV8<'j>, LiveFailedEffectStateFailureV8<'j>> {
        let selected = (|| {
            self.lineage.validate_current(true)?;
            let ack = self.lineage.current()?;
            if !matches!(
                ack.witness.selected_row(),
                EntryV8::Owned(OwnedBodyV8::OwnedEffectFailureStateCleanupStarted { .. })
            ) {
                return Err(SourceJournalError::Order);
            }
            crate::resumable_effects::owned_frame::v2::validate_owned_wait_observed_receipt_v8(
                &self.lineage.operations,
                self.released.receipt(),
            )
            .map_err(|_| SourceJournalError::Binding)?;
            Ok(EntryV8::Owned(
                OwnedBodyV8::OwnedEffectFailureStateCleanupSettled {
                    turn: 0,
                    attempt: 0,
                    started: true_seq(ack.session.sequence())?,
                    receipt: self.released.receipt().clone(),
                },
            ))
        })();
        match selected {
            Ok(selected) => Ok(LiveFailedEffectStateAppendV8 {
                owner: FailedStateOwnerV8::Released(self),
                selected,
            }),
            Err(error) => {
                self.lineage.journal().quarantine();
                Err(LiveFailedEffectStateFailureV8::Released {
                    _owner: self,
                    error,
                })
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_stop(
        self,
    ) -> Result<LiveFailedEffectStateAppendV8<'j>, LiveFailedEffectStateFailureV8<'j>> {
        let selected = (|| {
            self.lineage.validate_current(false)?;
            let ack = self.lineage.current()?;
            if !matches!(ack.witness.selected_row(),EntryV8::Owned(OwnedBodyV8::OwnedEffectFailureStateCleanupSettled{receipt,..}) if receipt==self.released.receipt()&&receipt["settlement"]=="completed")
            {
                return Err(SourceJournalError::Order);
            }
            self.lineage.selected_failure()?;
            Ok(EntryV8::Ordinary(SourceJournalEntry::Stop {
                turn: Some(0),
                attempt: Some(0),
                status: SourceStopStatus::EffectFailed,
                reason: SourceStopReason::EffectFailed,
            }))
        })();
        match selected {
            Ok(selected) => Ok(LiveFailedEffectStateAppendV8 {
                owner: FailedStateOwnerV8::Released(self),
                selected,
            }),
            Err(error) => {
                self.lineage.journal().quarantine();
                Err(LiveFailedEffectStateFailureV8::Released {
                    _owner: self,
                    error,
                })
            }
        }
    }
}
impl<'j> LiveFailedEffectStateAppendV8<'j> {
    fn cleanup(&self) -> &CleanupLineageV8<'j> {
        match &self.owner {
            FailedStateOwnerV8::Failed(o) => &o.lineage,
            FailedStateOwnerV8::Released(o) => &o.lineage.cleanup,
        }
    }
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.cleanup().recorded.intent.journal
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        match &self.owner {
            FailedStateOwnerV8::Failed(o) => o.lineage.current().session.sequence(),
            FailedStateOwnerV8::Released(o) => {
                o.lineage.current().expect("actual ACK").session.sequence()
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        match &self.owner {
            FailedStateOwnerV8::Failed(o) => o.lineage.current().session.acknowledged_bytes(),
            FailedStateOwnerV8::Released(o) => o
                .lineage
                .current()
                .expect("actual ACK")
                .session
                .acknowledged_bytes(),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = match &self.owner {
            FailedStateOwnerV8::Failed(o) => {
                if started(o)? != self.selected {
                    Err(SourceJournalError::Binding)
                } else {
                    Ok(())
                }
            }
            FailedStateOwnerV8::Released(o) => {
                let receipt = matches!(&self.selected,EntryV8::Owned(OwnedBodyV8::OwnedEffectFailureStateCleanupSettled{turn:0,attempt:0,started,receipt})if Some(*started)==true_seq(o.lineage.current()?.session.sequence()).ok()&&receipt==o.released.receipt());
                let stop = matches!(
                    &self.selected,
                    EntryV8::Ordinary(SourceJournalEntry::Stop {
                        turn: Some(0),
                        attempt: Some(0),
                        status: SourceStopStatus::EffectFailed,
                        reason: SourceStopReason::EffectFailed
                    })
                );
                if !receipt && !stop {
                    return Err(SourceJournalError::Binding);
                }
                if stop && o.released.receipt()["settlement"] != "completed" {
                    return Err(SourceJournalError::Order);
                }
                o.lineage.validate_current(receipt)
            }
        };
        result.inspect_err(|_| self.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedFailedEffectStateAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedFailedEffectStateAppendPermitV8 { owner: self })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedFailedEffectStateSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        witness.validate_predecessor(
            self.journal(),
            self.sequence(),
            self.acknowledged_bytes(),
            &self.selected,
        )?;
        witness.validate_against_acknowledged_session(session)?;
        self.cleanup()
            .recorded
            .intent
            .hold
            .validate_failed_state_guard(
                self.journal(),
                session.sequence(),
                session.acknowledged_bytes(),
            )?;
        // The Started/Settled receipt phases immediately use the incurred guard.
        let incurred = matches!(
            self.selected,
            EntryV8::Owned(
                OwnedBodyV8::OwnedEffectFailureStateCleanupStarted { .. }
                    | OwnedBodyV8::OwnedEffectFailureStateCleanupSettled { .. }
            )
        );
        if !incurred {
            let origin = &self.cleanup().recorded.intent;
            let held = self.journal().hold()?;
            let ordinary = self.journal().context().ordinary();
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
        let origin = &self.cleanup().recorded.intent;
        let held = self.journal().hold()?;
        let (runtime, e) = self
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan = plan_owned_effect_v8(
            runtime,
            e,
            &held.registration().expected_facts().scope,
            &origin.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !origin.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_predecessor(
            self.journal(),
            self.sequence(),
            self.acknowledged_bytes(),
            &self.selected,
        )
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedFailedEffectStateAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveFailedEffectStateAppendV8<'j>,
}
impl FixedFailedEffectStateAppendPermitV8<'_, '_> {
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
            .cleanup()
            .recorded
            .intent
            .hold
            .validate_failed_state_append_prefix(journal, inventory, &self.owner.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedFailedEffectStateSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .cleanup()
            .recorded
            .intent
            .hold
            .advance_failed_state_ack(witness, session)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_failed_state_v8<
    'j,
>(
    owner: LiveFailedEffectStateAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedFailedEffectStateSuccessorV8<'j>,
) -> Result<LiveFailedEffectStateAcknowledgedV8<'j>, LiveFailedEffectStateFailureV8<'j>> {
    if let Err(error) = owner.validate_successor(&witness, &session) {
        owner.journal().quarantine();
        return Err(LiveFailedEffectStateFailureV8::Before {
            _owner: owner,
            _session: session,
            _witness: witness,
            error,
        });
    }
    match owner.owner {
        FailedStateOwnerV8::Failed(o) => {
            let state = match o.pending.live_failed_state_facts_v8() {
                Ok(state) => state,
                Err(error) => {
                    o.lineage.recorded.intent.journal.quarantine();
                    return Err(LiveFailedEffectStateFailureV8::Failed { _owner: o, error });
                }
            };
            let EntryV8::Owned(OwnedBodyV8::OwnedEffectFailureStateCleanupStarted {
                operations,
                ..
            }) = owner.selected
            else {
                unreachable!("closed actual selection")
            };
            let mut acks = Vec::with_capacity(3);
            acks.push(FailedStateAckV8 { session, witness });
            Ok(LiveFailedEffectStateAcknowledgedV8::Started(
                LiveStartedFailedEffectStateV8 {
                    pending: o.pending,
                    accounting: o.accounting,
                    lineage: FailedStateLineageV8 {
                        cleanup: o.lineage,
                        state,
                        operations,
                        acks,
                    },
                },
            ))
        }
        FailedStateOwnerV8::Released(mut o) => {
            if matches!(&owner.selected, EntryV8::Owned(OwnedBodyV8::OwnedEffectFailureStateCleanupSettled { receipt, .. }) if receipt["settlement"] != "completed")
            {
                o.lineage.journal().quarantine();
            }
            o.lineage.acks.push(FailedStateAckV8 { session, witness });
            Ok(LiveFailedEffectStateAcknowledgedV8::Released(o))
        }
    }
}
