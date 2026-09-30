//! Actual State cleanup after a complete failed Decision receipt ACK.
//! Every owner precedes its same-ledger/same-hold lineage. No Outcome or Reduce.
use super::*;
use crate::cleanup_plan::FinalizeAction;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
    release_live_observer_failed_state_v8, LiveObserverFailedStateReleaseFailureV8,
    ReleasedObserverFailedStateV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedObserverFailedStateSuccessorV8;
use crate::live_invocation::source_journal::{SourceStopReason, SourceStopStatus};
use crate::resumable_effects::owned_frame::v2::CheckedOwnedFrameHelperV2;
struct ObserverAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedObserverFailedStateSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ObserverStateLineageV8<'j> {
    cleanup: CleanupLineageV8<'j>,
    state: Value,
    operations: Value,
    failure: OwnedEffectFailureV8,
    acks: Vec<ObserverAckV8<'j>>,
    seal: ObserverTerminalSealV8<'j>,
}
fn selection(
    cleanup: &CleanupLineageV8<'_>,
) -> Result<Option<SourceEffectFailure>, SourceJournalError> {
    match cleanup.recorded.facts.ordinary() {
        SourceJournalEntry::EffectObserved {
            turn: 0,
            attempt: 0,
            ..
        } => Ok(None),
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
            Ok(Some(*reason))
        }
        _ => Err(SourceJournalError::Binding),
    }
}
fn clock_guard(
    cleanup: &CleanupLineageV8<'_>,
    seal: &ObserverTerminalSealV8<'_>,
) -> Result<(), SourceJournalError> {
    let l = &cleanup.recorded.intent;
    let ordinary = l.journal.context().ordinary();
    let guard = || {
        seal.validate_guard()?;
        if l.cancellation.is_cancelled() {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    };
    guard()?;
    let domain = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| l.clock.clock_domain()))
        .map_err(|_| SourceJournalError::Poisoned)?;
    guard()?;
    if domain != ordinary.clock_domain() {
        return Err(SourceJournalError::Binding);
    }
    let now = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| l.clock.now_millis()))
        .map_err(|_| SourceJournalError::Poisoned)?;
    guard()?;
    if now < ordinary.initial_millis() || now >= ordinary.deadline_millis() {
        return Err(SourceJournalError::Time);
    }
    Ok(())
}
fn guard(
    cleanup: &CleanupLineageV8<'_>,
    seal: &ObserverTerminalSealV8<'_>,
    incurred: bool,
) -> Result<(), SourceJournalError> {
    let journal = cleanup.recorded.intent.journal;
    let result = (|| {
        seal.validate_guard()?;
        selection(cleanup)?;
        let (runtime, e) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan = plan_owned_effect_v8(
            runtime,
            e,
            &journal.context().registration().expected_facts().scope,
            &cleanup.recorded.intent.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !cleanup
            .recorded
            .intent
            .policy
            .allows(plan.operation().effect_id())
        {
            return Err(SourceJournalError::Binding);
        }
        if !incurred {
            clock_guard(cleanup, seal)?;
        }
        seal.validate_guard()
    })();
    result.inspect_err(|_| journal.quarantine())
}
impl ObserverStateLineageV8<'_> {
    fn current(&self) -> Result<&ObserverAckV8<'_>, SourceJournalError> {
        self.acks.last().ok_or(SourceJournalError::Order)
    }
    fn validate(&self, incurred: bool) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.current()?
                .witness
                .validate_against_acknowledged_session(&self.current()?.session)?;
            guard(&self.cleanup, &self.seal, incurred)
        })();
        result.inspect_err(|_| self.cleanup.recorded.intent.journal.quarantine())
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveStartedObserverStateV8<'j>
{
    pending: PendingOwnedEffectReceiptV8<'j>,
    accounting: TargetAccounting,
    lineage: ObserverStateLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveReleasedObserverStateV8<'j>
{
    released: ReleasedObserverFailedStateV8<'j>,
    accounting: TargetAccounting,
    lineage: ObserverStateLineageV8<'j>,
}
enum OwnerV8<'j> {
    Pending(LiveFailedOwnedEffectV8<'j>),
    Released(LiveReleasedObserverStateV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveObserverStateAppendV8<'j> {
    owner: OwnerV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveObserverStateAcknowledgedV8<
    'j,
> {
    Started(LiveStartedObserverStateV8<'j>),
    Released(LiveReleasedObserverStateV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveObserverStateFailureV8<'j> {
    Pending {
        owner: LiveFailedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    Started {
        owner: LiveStartedObserverStateV8<'j>,
        error: SourceJournalError,
    },
    Release {
        owner: LiveObserverFailedStateReleaseFailureV8<'j>,
        accounting: TargetAccounting,
        lineage: ObserverStateLineageV8<'j>,
    },
    Released {
        owner: LiveReleasedObserverStateV8<'j>,
        error: SourceJournalError,
    },
    Before {
        owner: LiveObserverStateAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedObserverFailedStateSuccessorV8<'j>,
        error: SourceJournalError,
    },
}
fn started(owner: &LiveFailedOwnedEffectV8<'_>) -> Result<EntryV8, SourceJournalError> {
    owner.validate_observer_state_intent()?;
    let failure = selection(&owner.lineage)?;
    if failure.is_some() && owner.pending.live_failed_state_reason_v8() != failure
        || failure.is_none()
            && owner.pending.failure() != Some(OwnedEffectFailureV8::ObservationFailed)
    {
        return Err(SourceJournalError::Binding);
    }
    let state = owner.pending.live_observer_failed_state_facts_v8()?;
    let l = &owner.lineage;
    let e = l
        .recorded
        .intent
        .journal
        .context()
        .ready_runtime()
        .ok_or(SourceJournalError::Binding)?
        .1;
    let operations = crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
        &e.wait().helper().liveness().result_disposal,
    )
    .map_err(|_| SourceJournalError::Binding)?;
    Ok(EntryV8::Owned(
        OwnedBodyV8::OwnedEffectObserverFailureStateCleanupStarted {
            turn: 0,
            attempt: 0,
            plan: e.wait().binding().into(),
            settlement: true_seq(l.recorded.settlement.session.sequence())?,
            recorded: true_seq(l.recorded.recorded.session.sequence())?,
            decision_cleanup_settled: true_seq(l.current().session.sequence())?,
            decision_receipt_digest: wire::recipe_digest(
                wire::RecipeV8::Receipt,
                owner.pending.receipt(),
            )?,
            cause: "decision_observation_failed".into(),
            selected_effect_failure: failure.map(|f| f.as_str().into()),
            state_digest: wire::record_argument_digest(&state),
            operations,
        },
    ))
}
impl<'j> LiveFailedOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_observer_state(
        self,
    ) -> Result<LiveObserverStateAppendV8<'j>, LiveObserverStateFailureV8<'j>> {
        match started(&self) {
            Ok(selected) => Ok(LiveObserverStateAppendV8 {
                owner: OwnerV8::Pending(self),
                selected,
            }),
            Err(error) => {
                self.lineage.recorded.intent.journal.quarantine();
                Err(LiveObserverStateFailureV8::Pending { owner: self, error })
            }
        }
    }
}
pub(crate) struct LiveObserverFailedStateCleanupPermitV8<'p, 'j> {
    lineage: &'p ObserverStateLineageV8<'j>,
}
impl LiveObserverFailedStateCleanupPermitV8<'_, '_> {
    pub(crate) fn validate_cleanup_current(&self) -> Result<(), SourceJournalError> {
        if !matches!(
            self.lineage.current()?.witness.selected_row(),
            EntryV8::Owned(OwnedBodyV8::OwnedEffectObserverFailureStateCleanupStarted { .. })
        ) {
            return Err(SourceJournalError::Order);
        }
        self.lineage.validate(true)
    }
    pub(crate) fn validate_actual_state(
        &self,
        helper: &CheckedOwnedFrameHelperV2,
        state: &Value,
    ) -> Result<(), SourceJournalError> {
        let e = self
            .lineage
            .cleanup
            .recorded
            .intent
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?
            .1;
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
        self.validate_cleanup_current()
    }
    pub(crate) fn validate_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let l = &self.lineage.cleanup.recorded.intent;
        let (runtime, e) = l
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(e, inputs.execution)
            || !std::ptr::eq(l.policy, inputs.policy)
            || !std::ptr::eq(l.cancellation, inputs.cancellation)
            || !inputs.store.belongs_to(l.journal)
            || inputs.turn != 0
            || inputs.attempt != 0
            || inputs.proposal.carrier() != l.proposal.carrier()
            || inputs.proposal.ordinary_digest() != l.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        self.validate_cleanup_current()
    }
}
impl<'j> LiveStartedObserverStateV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn release(
        self,
        observe: impl FnMut(&FinalizeAction),
    ) -> Result<LiveReleasedObserverStateV8<'j>, LiveObserverStateFailureV8<'j>> {
        if let Err(error) = self.lineage.validate(true) {
            return Err(LiveObserverStateFailureV8::Started { owner: self, error });
        }
        let Self {
            pending,
            accounting,
            lineage,
        } = self;
        match release_live_observer_failed_state_v8(
            pending,
            &LiveObserverFailedStateCleanupPermitV8 { lineage: &lineage },
            observe,
        ) {
            Ok(released) => Ok(LiveReleasedObserverStateV8 {
                released,
                accounting,
                lineage,
            }),
            Err(owner) => {
                lineage.cleanup.recorded.intent.journal.quarantine();
                Err(LiveObserverStateFailureV8::Release {
                    owner,
                    accounting,
                    lineage,
                })
            }
        }
    }
}
impl<'j> LiveReleasedObserverStateV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_receipt(
        self,
    ) -> Result<LiveObserverStateAppendV8<'j>, LiveObserverStateFailureV8<'j>> {
        let result = (|| {
            self.lineage.validate(true)?;
            if self.released.failure() != self.lineage.failure {
                return Err(SourceJournalError::Binding);
            }
            let ack = self.lineage.current()?;
            if !matches!(
                ack.witness.selected_row(),
                EntryV8::Owned(OwnedBodyV8::OwnedEffectObserverFailureStateCleanupStarted { .. })
            ) {
                return Err(SourceJournalError::Order);
            }
            crate::resumable_effects::owned_frame::v2::validate_owned_wait_observed_receipt_v8(
                &self.lineage.operations,
                self.released.receipt(),
            )
            .map_err(|_| SourceJournalError::Binding)?;
            Ok(EntryV8::Owned(
                OwnedBodyV8::OwnedEffectObserverFailureStateCleanupSettled {
                    turn: 0,
                    attempt: 0,
                    started: true_seq(ack.session.sequence())?,
                    receipt: self.released.receipt().clone(),
                },
            ))
        })();
        match result {
            Ok(selected) => Ok(LiveObserverStateAppendV8 {
                owner: OwnerV8::Released(self),
                selected,
            }),
            Err(error) => {
                self.lineage.cleanup.recorded.intent.journal.quarantine();
                Err(LiveObserverStateFailureV8::Released { owner: self, error })
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_stop(
        self,
    ) -> Result<LiveObserverStateAppendV8<'j>, LiveObserverStateFailureV8<'j>> {
        let result = (|| {
            self.lineage.validate(false)?;
            if self.released.failure() != self.lineage.failure {
                return Err(SourceJournalError::Binding);
            }
            if !matches!(self.lineage.current()?.witness.selected_row(),EntryV8::Owned(OwnedBodyV8::OwnedEffectObserverFailureStateCleanupSettled{receipt,..})if receipt==self.released.receipt()&&receipt["settlement"]=="completed")
            {
                return Err(SourceJournalError::Order);
            }
            let failed = selection(&self.lineage.cleanup)?.is_some();
            Ok(EntryV8::Ordinary(SourceJournalEntry::Stop {
                turn: Some(0),
                attempt: Some(0),
                status: if failed {
                    SourceStopStatus::EffectFailed
                } else {
                    SourceStopStatus::Rejected
                },
                reason: if failed {
                    SourceStopReason::EffectFailed
                } else {
                    SourceStopReason::StageRefused
                },
            }))
        })();
        match result {
            Ok(selected) => Ok(LiveObserverStateAppendV8 {
                owner: OwnerV8::Released(self),
                selected,
            }),
            Err(error) => {
                self.lineage.cleanup.recorded.intent.journal.quarantine();
                Err(LiveObserverStateFailureV8::Released { owner: self, error })
            }
        }
    }
}
impl<'j> LiveObserverStateAppendV8<'j> {
    fn cleanup(&self) -> &CleanupLineageV8<'j> {
        match &self.owner {
            OwnerV8::Pending(o) => &o.lineage,
            OwnerV8::Released(o) => &o.lineage.cleanup,
        }
    }
    fn seal(&self) -> Result<&ObserverTerminalSealV8<'j>, SourceJournalError> {
        match &self.owner {
            OwnerV8::Pending(o) => o.observer_seal.as_ref().ok_or(SourceJournalError::Binding),
            OwnerV8::Released(o) => Ok(&o.lineage.seal),
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
            OwnerV8::Pending(o) => o.lineage.current().session.sequence(),
            OwnerV8::Released(o) => o.lineage.current().expect("actual ACK").session.sequence(),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        match &self.owner {
            OwnerV8::Pending(o) => o.lineage.current().session.acknowledged_bytes(),
            OwnerV8::Released(o) => o
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn begin_session(
        &self,
    ) -> Result<AppendSessionV8<'j>, SourceJournalError> {
        self.validate_live()?;
        self.seal()?.recover_session()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| match &self.owner {
            OwnerV8::Pending(o) => {
                if started(o)? != self.selected {
                    return Err(SourceJournalError::Binding);
                }
                Ok(())
            }
            OwnerV8::Released(o) => {
                let receipt = matches!(&self.selected,EntryV8::Owned(OwnedBodyV8::OwnedEffectObserverFailureStateCleanupSettled{turn:0,attempt:0,started,receipt})if Some(*started)==true_seq(o.lineage.current()?.session.sequence()).ok()&&receipt==o.released.receipt());
                let expected = selection(&o.lineage.cleanup)?.is_some();
                let stop = matches!(&self.selected,EntryV8::Ordinary(SourceJournalEntry::Stop{turn:Some(0),attempt:Some(0),status,reason})if (*status,*reason)==if expected{(SourceStopStatus::EffectFailed,SourceStopReason::EffectFailed)}else{(SourceStopStatus::Rejected,SourceStopReason::StageRefused)});
                if !receipt && !stop || stop && o.released.receipt()["settlement"] != "completed" {
                    return Err(SourceJournalError::Order);
                }
                o.lineage.validate(receipt)
            }
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedObserverStateAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedObserverStateAppendPermitV8 { owner: self })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedObserverFailedStateSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        witness.validate_predecessor(
            self.journal(),
            self.sequence(),
            self.acknowledged_bytes(),
            &self.selected,
        )?;
        witness.validate_against_acknowledged_session(session)?;
        let incurred = matches!(
            self.selected,
            EntryV8::Owned(
                OwnedBodyV8::OwnedEffectObserverFailureStateCleanupStarted { .. }
                    | OwnedBodyV8::OwnedEffectObserverFailureStateCleanupSettled { .. }
            )
        );
        guard(self.cleanup(), self.seal()?, incurred)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedObserverStateAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveObserverStateAppendV8<'j>,
}
impl FixedObserverStateAppendPermitV8<'_, '_> {
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
            .seal()?
            .validate_append_prefix(journal, inventory, &self.owner.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_physical_guard(
        &self,
        lease: &crate::resumable_effects::owned_frame::SourceOwnedWaitLeaseV8,
    ) -> Result<(), SourceJournalError> {
        self.owner.seal()?.validate_physical_guard(lease)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedObserverFailedStateSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner.seal()?.advance_ack(witness, session)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_observer_state_v8<
    'j,
>(
    owner: LiveObserverStateAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedObserverFailedStateSuccessorV8<'j>,
) -> Result<LiveObserverStateAcknowledgedV8<'j>, LiveObserverStateFailureV8<'j>> {
    if let Err(error) = owner.validate_successor(&witness, &session) {
        owner.journal().quarantine();
        return Err(LiveObserverStateFailureV8::Before {
            owner,
            session,
            witness,
            error,
        });
    }
    match owner.owner {
        OwnerV8::Pending(mut o) => {
            let state = match o.pending.live_observer_failed_state_facts_v8() {
                Ok(s) => s,
                Err(error) => {
                    o.lineage.recorded.intent.journal.quarantine();
                    return Err(LiveObserverStateFailureV8::Pending { owner: o, error });
                }
            };
            let EntryV8::Owned(OwnedBodyV8::OwnedEffectObserverFailureStateCleanupStarted {
                operations,
                ..
            }) = owner.selected
            else {
                unreachable!("closed selected row")
            };
            let seal = o
                .observer_seal
                .take()
                .expect("checked live failed receipt seal");
            let failure = o.pending.failure().expect("checked sticky actual failure");
            let mut acks = Vec::with_capacity(3);
            acks.push(ObserverAckV8 { session, witness });
            Ok(LiveObserverStateAcknowledgedV8::Started(
                LiveStartedObserverStateV8 {
                    pending: o.pending,
                    accounting: o.accounting,
                    lineage: ObserverStateLineageV8 {
                        cleanup: o.lineage,
                        state,
                        operations,
                        failure,
                        acks,
                        seal,
                    },
                },
            ))
        }
        OwnerV8::Released(mut o) => {
            if matches!(&owner.selected,EntryV8::Owned(OwnedBodyV8::OwnedEffectObserverFailureStateCleanupSettled{receipt,..})if receipt["settlement"]!="completed")
            {
                o.lineage.cleanup.recorded.intent.journal.quarantine();
            }
            o.lineage.acks.push(ObserverAckV8 { session, witness });
            Ok(LiveObserverStateAcknowledgedV8::Released(o))
        }
    }
}
#[cfg(test)]
impl LiveObserverStateAppendV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn owner_accounting_for_test(
        &self,
    ) -> &TargetAccounting {
        match &self.owner {
            OwnerV8::Pending(o) => &o.accounting,
            OwnerV8::Released(o) => &o.accounting,
        }
    }
}
#[cfg(test)]
impl LiveStartedObserverStateV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn lineage_guard_for_test(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.lineage.validate(true)
    }
}
