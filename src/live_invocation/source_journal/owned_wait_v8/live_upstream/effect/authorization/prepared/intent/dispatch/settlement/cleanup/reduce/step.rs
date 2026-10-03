//! Actual evaluated Reduce and Step holders retain the original owner first,
//! actual target ledger, and the same spent hold last. Inert rows mint no owner.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::OwnedEffectInputsV8;
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::{
    CheckedLiveOwnedReduceStageFactsV8, ExecutedOwnedReduceSettledV2, HeldExecutedOwnedStepV2,
    OwnedReduceCleanupOriginV8, ReadyExecutedOwnedStepV2, StagedExecutedOwnedReduceV2,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedStepSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::{
    model::OwnedBodyV8,
    reduce_wire::{self, ReduceRecipeV8},
};
use serde_json::{json, Value};

struct StepAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedStepSuccessorV8<'j>,
}
struct StepLineageV8<'j> {
    reduce: ReduceLineageV8<'j>,
    facts: CheckedLiveOwnedReduceStageFactsV8,
    acks: Vec<StepAckV8<'j>>,
}
impl<'j> StepLineageV8<'j> {
    fn origin(&self) -> &super::super::super::super::super::activation::IntentLineageV8<'j> {
        &self.reduce.cleanup.recorded.intent
    }
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.origin().journal
    }
    fn current(&self) -> Result<&StepAckV8<'j>, SourceJournalError> {
        self.acks.last().ok_or(SourceJournalError::Binding)
    }
    fn validate_current(&self, incurred: bool) -> Result<(), SourceJournalError> {
        if self.acks.is_empty() {
            return self.reduce.validate_current();
        }
        let result = (|| {
            let ack = self.current()?;
            let origin = self.origin();
            let journal = self.journal();
            ack.witness.validate_current_session(&ack.session)?;
            origin.hold.validate_step_guard(
                journal,
                ack.session.sequence(),
                ack.session.acknowledged_bytes(),
            )?;
            let held = journal.hold()?;
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
            origin.hold.validate_step_guard(
                journal,
                ack.session.sequence(),
                ack.session.acknowledged_bytes(),
            )?;
            ack.witness.validate_current_session(&ack.session)
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
    fn matches_inputs(&self, inputs: &OwnedEffectInputsV8<'_>) -> Result<(), SourceJournalError> {
        let o = self.origin();
        let (runtime, execution) = self
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(execution, inputs.execution)
            || !std::ptr::eq(o.policy, inputs.policy)
            || !std::ptr::eq(o.cancellation, inputs.cancellation)
            || inputs.turn != 0
            || inputs.attempt != 0
            || inputs.proposal.carrier() != o.proposal.carrier()
            || inputs.proposal.ordinary_digest() != o.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        let ack = self.current()?;
        inputs
            .store
            .validate_prefix(ack.session.sequence(), ack.session.acknowledged_bytes())
    }
    fn true_seq(ack: &StepAckV8<'_>) -> Result<u32, SourceJournalError> {
        u32::try_from(
            ack.session
                .sequence()
                .checked_sub(1)
                .ok_or(SourceJournalError::Order)?,
        )
        .map_err(|_| SourceJournalError::Capacity)
    }
    fn staged_ack(&self) -> Result<&StepAckV8<'j>, SourceJournalError> {
        self.acks
            .iter()
            .find(|a| {
                matches!(
                    a.witness.selected_row(),
                    EntryV8::Owned(OwnedBodyV8::OwnedReduceStaged { .. })
                )
            })
            .ok_or(SourceJournalError::Binding)
    }
    fn started_ack(&self) -> Option<&StepAckV8<'j>> {
        self.acks.iter().find(|a| {
            matches!(
                a.witness.selected_row(),
                EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupStarted { .. })
            )
        })
    }
    fn cleanup_origin(&self) -> Result<OwnedReduceCleanupOriginV8, SourceJournalError> {
        if let Some(a) = self.started_ack() {
            return Ok(OwnedReduceCleanupOriginV8::Observed {
                started: Self::true_seq(a)?,
            });
        }
        let staged = Self::true_seq(self.staged_ack()?)?;
        let (_, execution) = self
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan =
            compile_owned_reduce_v2(execution.wait()).map_err(|_| SourceJournalError::Binding)?;
        let basis = self
            .facts
            .cleanup_basis(Some(staged))
            .map_err(|_| SourceJournalError::Binding)?;
        let checked = crate::resumable_effects::owned_frame::v2::validate_owned_reduce_cleanup_v8(
            &plan,
            &basis,
            self.facts.operations(),
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if self.facts.step().is_none()
            || !checked
                .active_operations()
                .as_array()
                .is_some_and(|v| v.is_empty())
            || !compiler_empty_eligible(
                &plan,
                self.facts.step().ok_or(SourceJournalError::Binding)?,
                staged,
            )?
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(OwnedReduceCleanupOriginV8::CompilerEmpty {
            staged: Self::true_seq(self.staged_ack()?)?,
        })
    }
}

fn compiler_empty_eligible(
    plan: &crate::resumable_effects::owned_frame::v2::CheckedOwnedReduceV2,
    step: &Value,
    staged: u32,
) -> Result<bool, SourceJournalError> {
    let matching = plan
        .transfers()
        .cases
        .iter()
        .filter(|c| step["case"] == c.case.as_str())
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return Err(SourceJournalError::Binding);
    }
    let operations = crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
        &plan.transfers().completion_cleanup,
    )
    .map_err(|_| SourceJournalError::Binding)?;
    for c in matching {
        let basis = json!({"kind":"success","staged":staged,"constructor":c.constructor.as_str(),"case":c.case.as_str(),"active_flags":c.completion_live_flags.iter().map(|f|f.0).collect::<Vec<_>>()});
        let checked = crate::resumable_effects::owned_frame::v2::validate_owned_reduce_cleanup_v8(
            plan,
            &basis,
            &operations,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !checked
            .active_operations()
            .as_array()
            .is_some_and(|v| v.is_empty())
        {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(crate) struct LiveOwnedReduceCleanupPermitV8<'p, 'j> {
    lineage: &'p StepLineageV8<'j>,
}
impl LiveOwnedReduceCleanupPermitV8<'_, '_> {
    pub(crate) fn cleanup_origin(&self) -> Result<OwnedReduceCleanupOriginV8, SourceJournalError> {
        self.lineage.cleanup_origin()
    }
    pub(crate) fn validate_cleanup_current(&self) -> Result<(), SourceJournalError> {
        self.lineage.validate_current(matches!(
            self.cleanup_origin()?,
            OwnedReduceCleanupOriginV8::Observed { .. }
        ))
    }
    pub(crate) fn validate_staged(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
        facts: &CheckedLiveOwnedReduceStageFactsV8,
    ) -> Result<(), SourceJournalError> {
        self.validate_cleanup_current()?;
        self.lineage.matches_inputs(inputs)?;
        let actual = &self.lineage.facts;
        let staged = if facts.step().is_some() {
            Some(StepLineageV8::true_seq(self.lineage.staged_ack()?)?)
        } else {
            None
        };
        if facts.allowance() != actual.allowance()
            || facts.consumed() != actual.consumed()
            || facts.effect_settled() != actual.effect_settled()
            || facts.step() != actual.step()
            || facts.operations() != actual.operations()
            || facts.active_flags() != actual.active_flags()
            || facts
                .cleanup_basis(staged)
                .map_err(|_| SourceJournalError::Binding)?
                != actual
                    .cleanup_basis(staged)
                    .map_err(|_| SourceJournalError::Binding)?
        {
            return Err(SourceJournalError::Binding);
        }
        self.validate_cleanup_current()
    }
}
impl crate::interpreter::resumable::owned_frame::registered_stage::reduce::LiveOwnedReduceCleanupGuardV8
    for LiveOwnedReduceCleanupPermitV8<'_, '_>
{
    fn cleanup_origin(&self) -> Result<OwnedReduceCleanupOriginV8, SourceJournalError> {
        LiveOwnedReduceCleanupPermitV8::cleanup_origin(self)
    }
    fn validate_cleanup_current(&self) -> Result<(), SourceJournalError> {
        LiveOwnedReduceCleanupPermitV8::validate_cleanup_current(self)
    }
    fn validate_staged(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
        facts: &CheckedLiveOwnedReduceStageFactsV8,
    ) -> Result<(), SourceJournalError> {
        LiveOwnedReduceCleanupPermitV8::validate_staged(self, inputs, facts)
    }
}
pub(crate) struct LiveOwnedStepTransferPermitV8<'p, 'j> {
    lineage: &'p StepLineageV8<'j>,
}
impl LiveOwnedStepTransferPermitV8<'_, '_> {
    pub(crate) fn validate_transfer_current(&self) -> Result<(), SourceJournalError> {
        self.lineage.validate_current(false)
    }
    pub(crate) fn transfer_reserved(&self) -> Result<u32, SourceJournalError> {
        let ack = self.lineage.current()?;
        if !matches!(
            ack.witness.selected_row(),
            EntryV8::Owned(OwnedBodyV8::OwnedStepTransferReserved { .. })
        ) {
            return Err(SourceJournalError::Binding);
        }
        StepLineageV8::true_seq(ack)
    }
    pub(crate) fn validate_ready(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
        receipt: &Value,
        origin: OwnedReduceCleanupOriginV8,
    ) -> Result<(), SourceJournalError> {
        self.validate_transfer_current()?;
        self.lineage.matches_inputs(inputs)?;
        if origin != self.lineage.cleanup_origin()? || receipt["settlement"] != "completed" {
            return Err(SourceJournalError::Binding);
        }
        match origin {
            OwnedReduceCleanupOriginV8::Observed { .. } => {
                let ack = self
                    .lineage
                    .acks
                    .iter()
                    .find(|a| {
                        matches!(
                            a.witness.selected_row(),
                            EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupSettled { .. })
                        )
                    })
                    .ok_or(SourceJournalError::Binding)?;
                let EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupSettled {
                    receipt: recorded,
                    ..
                }) = ack.witness.selected_row()
                else {
                    return Err(SourceJournalError::Binding);
                };
                if recorded != receipt {
                    return Err(SourceJournalError::Binding);
                }
            }
            OwnedReduceCleanupOriginV8::CompilerEmpty { .. } => {
                if receipt["operations"] != json!([]) {
                    return Err(SourceJournalError::Binding);
                }
            }
        }
        self.validate_transfer_current()
    }
}
impl crate::interpreter::resumable::owned_frame::registered_stage::reduce::LiveOwnedStepTransferGuardV8
    for LiveOwnedStepTransferPermitV8<'_, '_>
{
    fn validate_transfer_current(&self) -> Result<(), SourceJournalError> {
        LiveOwnedStepTransferPermitV8::validate_transfer_current(self)
    }
    fn transfer_reserved(&self) -> Result<u32, SourceJournalError> {
        LiveOwnedStepTransferPermitV8::transfer_reserved(self)
    }
    fn validate_ready(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
        receipt: &Value,
        origin: OwnedReduceCleanupOriginV8,
    ) -> Result<(), SourceJournalError> {
        LiveOwnedStepTransferPermitV8::validate_ready(self, inputs, receipt, origin)
    }
}

// Source holders remain private; their only construction will consume the
// actual evaluated core owner and actual fixed ACK envelopes in this child.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveStagedStepV8<'j> {
    staged: StagedExecutedOwnedReduceV2<'j>,
    accounting: TargetAccounting,
    lineage: StepLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveReleasedStepV8<'j> {
    released: ExecutedOwnedReduceSettledV2<'j>,
    accounting: TargetAccounting,
    lineage: StepLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveReadyStepV8<'j> {
    ready: ReadyExecutedOwnedStepV2<'j>,
    accounting: TargetAccounting,
    lineage: StepLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveMovedStepV8<'j> {
    held: HeldExecutedOwnedStepV2<'j>,
    accounting: TargetAccounting,
    lineage: StepLineageV8<'j>,
}
fn scope_of(lineage: &StepLineageV8<'_>) -> Result<Value, SourceJournalError> {
    let held = lineage.journal().hold()?;
    let scope = &held.registration().expected_facts().scope;
    Ok(
        json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()}),
    )
}
fn original_reservation(lineage: &StepLineageV8<'_>) -> Result<u32, SourceJournalError> {
    u32::try_from(
        lineage
            .reduce
            .session
            .sequence()
            .checked_sub(1)
            .ok_or(SourceJournalError::Order)?,
    )
    .map_err(|_| SourceJournalError::Capacity)
}
fn selected_cleanup(lineage: &StepLineageV8<'_>) -> Result<EntryV8, SourceJournalError> {
    let (_, e) = lineage
        .journal()
        .context()
        .ready_runtime()
        .ok_or(SourceJournalError::Binding)?;
    let plan = e.wait().binding();
    let reservation = original_reservation(lineage)?;
    let staged = if lineage.facts.step().is_some() {
        Some(StepLineageV8::true_seq(lineage.staged_ack()?)?)
    } else {
        None
    };
    let raw = lineage
        .facts
        .cleanup_basis(staged)
        .map_err(|_| SourceJournalError::Binding)?;
    let digest = reduce_wire::recipe_digest(
        ReduceRecipeV8::Basis,
        &json!({"scope":scope_of(lineage)?,"binding":plan,"plan":plan,"turn":0,"attempt":0,"stage_reservation":reservation,"basis":raw}),
    )?;
    let basis = serde_json::from_value(raw).map_err(|_| SourceJournalError::Binding)?;
    Ok(EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupStarted {
        turn: 0,
        attempt: 0,
        plan: plan.into(),
        stage_reservation: reservation,
        effect_cleanup_settled: lineage.facts.effect_settled(),
        basis,
        basis_digest: digest,
        consumed: u64::try_from(lineage.facts.consumed())
            .map_err(|_| SourceJournalError::Capacity)?,
        operations: lineage.facts.operations().clone(),
    }))
}
enum StepAppendOwnerV8<'j> {
    Staged(LiveStagedStepV8<'j>),
    Released(LiveReleasedStepV8<'j>),
    Ready(LiveReadyStepV8<'j>),
    Moved(LiveMovedStepV8<'j>),
    Continued(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedStagedStepV8<'j>),
}
impl<'j> StepAppendOwnerV8<'j> {
    fn lineage(&self) -> &StepLineageV8<'j> {
        match self {
            Self::Staged(o) => &o.lineage,
            Self::Released(o) => &o.lineage,
            Self::Ready(o) => &o.lineage,
            Self::Moved(o) => &o.lineage,
            Self::Continued(_) => unreachable!("continued Step has its own held lineage"),
        }
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedStepAppendV8<'j> {
    owner: StepAppendOwnerV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveStepAdvanceFailureV8<'j> {
    Evaluated {
        _owner: LiveEvaluatedOwnedReduceV8<'j>,
        error: SourceJournalError,
    },
    Selection {
        _owner: LiveStagedStepV8<'j>,
        error: SourceJournalError,
    },
    Before {
        _owner: LiveOwnedStepAppendV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveEvaluatedOwnedReduceV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_step(
        self,
    ) -> Result<LiveOwnedStepAppendV8<'j>, LiveStepAdvanceFailureV8<'j>> {
        let facts = match self.stage_facts() {
            Ok(f) => f,
            Err(error) => {
                return Err(LiveStepAdvanceFailureV8::Evaluated {
                    _owner: self,
                    error,
                })
            }
        };
        let checked = (|| {
            let (_, e) = self
                .lineage
                .cleanup
                .recorded
                .intent
                .journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = e.wait().binding();
            let reservation = u32::try_from(
                self.lineage
                    .session
                    .sequence()
                    .checked_sub(1)
                    .ok_or(SourceJournalError::Order)?,
            )
            .map_err(|_| SourceJournalError::Capacity)?;
            if let Some(step) = facts.step() {
                let held = self.lineage.cleanup.recorded.intent.journal.hold()?;
                let s = &held.registration().expected_facts().scope;
                let scope = json!({"program_root":s.program_root(),"invocation":s.invocation_id(),"policy_epoch":s.policy_epoch()});
                let digest = reduce_wire::recipe_digest(
                    ReduceRecipeV8::Step,
                    &json!({"scope":scope,"binding":plan,"plan":plan,"turn":0,"attempt":0,"stage_reservation":reservation,"step":step}),
                )?;
                Ok(Some(EntryV8::Owned(OwnedBodyV8::OwnedReduceStaged {
                    turn: 0,
                    attempt: 0,
                    plan: plan.into(),
                    stage_reservation: reservation,
                    effect_cleanup_settled: facts.effect_settled(),
                    step: step.clone(),
                    step_digest: digest,
                    consumed: u64::try_from(facts.consumed())
                        .map_err(|_| SourceJournalError::Capacity)?,
                })))
            } else {
                Ok(None)
            }
        })();
        let selected = match checked {
            Ok(s) => s,
            Err(error) => {
                return Err(LiveStepAdvanceFailureV8::Evaluated {
                    _owner: self,
                    error,
                })
            }
        };
        let LiveEvaluatedOwnedReduceV8 {
            staged,
            accounting,
            lineage,
        } = self;
        let owner = LiveStagedStepV8 {
            staged,
            accounting,
            lineage: StepLineageV8 {
                reduce: lineage,
                facts,
                acks: Vec::with_capacity(7),
            },
        };
        let selected = match selected {
            Some(row) => row,
            None => match selected_cleanup(&owner.lineage) {
                Ok(row) => row,
                Err(error) => {
                    return Err(LiveStepAdvanceFailureV8::Selection {
                        _owner: owner,
                        error,
                    })
                }
            },
        };
        Ok(LiveOwnedStepAppendV8 {
            owner: StepAppendOwnerV8::Staged(owner),
            selected,
        })
    }
}

impl<'j> LiveOwnedStepAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued(
        owner: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedStagedStepV8<'j>,
        selected: EntryV8,
    ) -> Self {
        Self {
            owner: StepAppendOwnerV8::Continued(owner),
            selected,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        j: &SourceOwnedWaitJournalV8,
    ) -> bool {
        match &self.owner {
            StepAppendOwnerV8::Continued(o) => std::ptr::eq(o.journal(), j),
            _ => std::ptr::eq(self.owner.lineage().journal(), j),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        match &self.owner {
            StepAppendOwnerV8::Continued(o) => o.cursor().0,
            _ => self.owner.lineage().cursor().0,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        match &self.owner {
            StepAppendOwnerV8::Continued(o) => o.cursor().1,
            _ => self.owner.lineage().cursor().1,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        match &self.owner {
            StepAppendOwnerV8::Continued(o) => o.validate_live(),
            _ => self
                .owner
                .lineage()
                .validate_current(self.owner.lineage().incurred()),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedStepAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedStepAppendPermitV8 { obligation: self })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_step_successor(
        &self,
        w: &VerifiedOwnedStepSuccessorV8<'_>,
        s: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        w.validate_against_acknowledged_session(s)?;
        let journal = match &self.owner {
            StepAppendOwnerV8::Continued(o) => o.journal(),
            _ => self.owner.lineage().journal(),
        };
        w.validate_predecessor(
            journal,
            self.sequence(),
            self.acknowledged_bytes(),
            &self.selected,
        )?;
        // Immediately after Started and receipt ACK, cancellation cannot
        // invalidate already incurred release/recording. Full guard resumes
        // before selecting a transfer or Stop and after the actual field move.
        let incurred = matches!(
            &self.selected,
            EntryV8::Owned(
                OwnedBodyV8::OwnedReduceCleanupStarted { .. }
                    | OwnedBodyV8::OwnedReduceCleanupSettled { .. }
            )
        );
        match &self.owner {
            StepAppendOwnerV8::Continued(o) => o.validate_new_prefix(s, incurred),
            _ => self.owner.lineage().validate_new_prefix(s, incurred),
        }
    }
}
pub(crate) struct FixedOwnedStepAppendPermitV8<'p, 'j> {
    obligation: &'p LiveOwnedStepAppendV8<'j>,
}
impl FixedOwnedStepAppendPermitV8<'_, '_> {
    pub(crate) fn validate_preflight(
        &self,
        j: &SourceOwnedWaitJournalV8,
    ) -> Result<(), SourceJournalError> {
        if !self.obligation.belongs_to(j) {
            return Err(SourceJournalError::Binding);
        }
        self.obligation.validate_live()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        self.obligation.selected_row()
    }
    pub(crate) fn sequence(&self) -> usize {
        self.obligation.sequence()
    }
    pub(crate) fn acknowledged_bytes(&self) -> usize {
        self.obligation.acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        j: &SourceOwnedWaitJournalV8,
        i: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<'_>,
    ) -> Result<(), SourceJournalError> {
        match &self.obligation.owner {
            StepAppendOwnerV8::Continued(o) => {
                o.hold()?
                    .validate_step_append_prefix(j, i, &self.obligation.selected)
            }
            _ => self
                .obligation
                .owner
                .lineage()
                .origin()
                .hold
                .validate_step_append_prefix(j, i, &self.obligation.selected),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        w: &VerifiedOwnedStepSuccessorV8<'_>,
        s: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        match &self.obligation.owner {
            StepAppendOwnerV8::Continued(o) => o.hold()?.advance_step_ack(w, s),
            _ => self
                .obligation
                .owner
                .lineage()
                .origin()
                .hold
                .advance_step_ack(w, s),
        }
    }
}
impl<'j> StepLineageV8<'j> {
    fn cursor(&self) -> (usize, usize) {
        match self.acks.last() {
            Some(a) => (a.session.sequence(), a.session.acknowledged_bytes()),
            None => (
                self.reduce.session.sequence(),
                self.reduce.session.acknowledged_bytes(),
            ),
        }
    }
    fn incurred(&self) -> bool {
        self.acks.last().is_some_and(|a| {
            matches!(
                a.witness.selected_row(),
                EntryV8::Owned(
                    OwnedBodyV8::OwnedReduceCleanupStarted { .. }
                        | OwnedBodyV8::OwnedReduceCleanupSettled { .. }
                )
            )
        })
    }
    fn validate_new_prefix(
        &self,
        s: &AppendSessionV8<'_>,
        incurred: bool,
    ) -> Result<(), SourceJournalError> {
        let o = self.origin();
        o.hold
            .validate_step_guard(self.journal(), s.sequence(), s.acknowledged_bytes())?;
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
            &o.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !o.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        if !incurred {
            let b = self.journal().context().ordinary();
            check_clock_v8(
                &held,
                s.sequence(),
                s.acknowledged_bytes(),
                o.cancellation,
                o.clock,
                b.clock_domain(),
                b.initial_millis(),
                b.deadline_millis(),
            )?;
        }
        o.hold
            .validate_step_guard(self.journal(), s.sequence(), s.acknowledged_bytes())
    }
    fn mapped_step(&self)->Result<crate::live_invocation::source_journal::owned_wait_v8::reduce_inventory::CheckedReduceStepV8,SourceJournalError>{
        let (_, e) = self
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan = compile_owned_reduce_v2(e.wait()).map_err(|_| SourceJournalError::Binding)?;
        let a = self.staged_ack()?;
        let EntryV8::Owned(OwnedBodyV8::OwnedReduceStaged {
            step, step_digest, ..
        }) = a.witness.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        crate::live_invocation::source_journal::owned_wait_v8::reduce_inventory::checked_step(
            &plan,
            &scope_of(self)?,
            0,
            0,
            original_reservation(self)?,
            step,
            step_digest,
        )
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveStepAcknowledgedV8<'j> {
    Staged(LiveStagedStepV8<'j>),
    Released(LiveReleasedStepV8<'j>),
    Ready(LiveReadyStepV8<'j>),
    Moved(LiveMovedStepV8<'j>),
    Continued(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedStagedStepV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_step_v8<'j>(
    mut obligation: LiveOwnedStepAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedStepSuccessorV8<'j>,
) -> Result<LiveStepAcknowledgedV8<'j>, LiveStepAdvanceFailureV8<'j>> {
    if let Err(error) = obligation.validate_step_successor(&witness, &session) {
        match &obligation.owner {
            StepAppendOwnerV8::Continued(o) => o.journal().quarantine(),
            _ => obligation.owner.lineage().journal().quarantine(),
        }
        return Err(LiveStepAdvanceFailureV8::Before {
            _owner: obligation,
            error,
        });
    }
    if matches!(&obligation.owner, StepAppendOwnerV8::Continued(_)) {
        let StepAppendOwnerV8::Continued(owner) = obligation.owner else {
            unreachable!()
        };
        return owner
            .acknowledge(session, witness, &obligation.selected)
            .map(LiveStepAcknowledgedV8::Continued)
            .map_err(|(owner, error)| LiveStepAdvanceFailureV8::Before {
                _owner: LiveOwnedStepAppendV8::continued(owner, obligation.selected),
                error,
            });
    }
    let lineage = match &mut obligation.owner {
        StepAppendOwnerV8::Staged(o) => &mut o.lineage,
        StepAppendOwnerV8::Released(o) => &mut o.lineage,
        StepAppendOwnerV8::Ready(o) => &mut o.lineage,
        StepAppendOwnerV8::Moved(o) => &mut o.lineage,
        StepAppendOwnerV8::Continued(_) => unreachable!(),
    };
    if lineage.acks.len() == lineage.acks.capacity() {
        lineage.journal().quarantine();
        return Err(LiveStepAdvanceFailureV8::Before {
            _owner: obligation,
            error: SourceJournalError::Capacity,
        });
    }
    lineage.acks.push(StepAckV8 { session, witness });
    Ok(match obligation.owner {
        StepAppendOwnerV8::Staged(o) => LiveStepAcknowledgedV8::Staged(o),
        StepAppendOwnerV8::Released(o) => LiveStepAcknowledgedV8::Released(o),
        StepAppendOwnerV8::Ready(o) => LiveStepAcknowledgedV8::Ready(o),
        StepAppendOwnerV8::Moved(o) => LiveStepAcknowledgedV8::Moved(o),
        StepAppendOwnerV8::Continued(_) => unreachable!(),
    })
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveStepReleaseFailureV8<'j>{
    owner:crate::interpreter::resumable::owned_frame::registered_stage::reduce::LiveOwnedReduceCleanupFailureV8<'j>,
    accounting:TargetAccounting,
    lineage:StepLineageV8<'j>,
}
impl<'j> LiveStagedStepV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_cleanup(
        self,
    ) -> Result<LiveOwnedStepAppendV8<'j>, (Self, SourceJournalError)> {
        let row = (|| {
            self.lineage.validate_current(false)?;
            selected_cleanup(&self.lineage)
        })();
        match row {
            Ok(selected) => Ok(LiveOwnedStepAppendV8 {
                owner: StepAppendOwnerV8::Staged(self),
                selected,
            }),
            Err(e) => Err((self, e)),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn release(
        self,
        observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
    ) -> Result<LiveReleasedStepV8<'j>, LiveStepReleaseFailureV8<'j>> {
        let Self {
            staged,
            accounting,
            lineage,
        } = self;
        let permit = LiveOwnedReduceCleanupPermitV8 { lineage: &lineage };
        match crate::interpreter::resumable::owned_frame::registered_stage::reduce::settle_live_owned_reduce_v8(staged,&permit,observe){
            Ok(released)=>Ok(LiveReleasedStepV8{released,accounting,lineage}),
            Err(owner)=>{lineage.journal().quarantine();Err(LiveStepReleaseFailureV8{owner,accounting,lineage})}
        }
    }
}
impl<'j> LiveReleasedStepV8<'j> {
    fn receipt(&self) -> Result<Value, SourceJournalError> {
        match &self.released {
            ExecutedOwnedReduceSettledV2::Ready(r) => r.live_receipt_v8(),
            ExecutedOwnedReduceSettledV2::Failed(f) => f.live_receipt_v8(),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_receipt(
        self,
    ) -> Result<LiveOwnedStepAppendV8<'j>, (Self, SourceJournalError)> {
        let row = (|| {
            self.lineage.validate_current(true)?;
            let started = self
                .lineage
                .started_ack()
                .ok_or(SourceJournalError::Binding)?;
            let receipt = self.receipt()?;
            Ok(EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupSettled {
                turn: 0,
                attempt: 0,
                started: StepLineageV8::true_seq(started)?,
                receipt,
            }))
        })();
        match row {
            Ok(selected) => Ok(LiveOwnedStepAppendV8 {
                owner: StepAppendOwnerV8::Released(self),
                selected,
            }),
            Err(e) => Err((self, e)),
        }
    }
    /// No row is invented for compiler-empty success. The same actual Ready
    /// owner is consumed only after its complete active receipt is checked.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn into_ready(
        self,
    ) -> Result<LiveReadyStepV8<'j>, (Self, SourceJournalError)> {
        let result = (|| {
            self.lineage.validate_current(false)?;
            let receipt = self.receipt()?;
            if receipt["settlement"] != "completed"
                || !matches!(&self.released, ExecutedOwnedReduceSettledV2::Ready(_))
            {
                return Err(SourceJournalError::Binding);
            }
            match self.lineage.cleanup_origin()? {
                OwnedReduceCleanupOriginV8::Observed { .. } => {
                    let ack = self.lineage.current()?;
                    let EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupSettled {
                        receipt: recorded,
                        ..
                    }) = ack.witness.selected_row()
                    else {
                        return Err(SourceJournalError::Binding);
                    };
                    if recorded != &receipt {
                        return Err(SourceJournalError::Binding);
                    }
                }
                OwnedReduceCleanupOriginV8::CompilerEmpty { .. } => {
                    if receipt["operations"] != json!([]) {
                        return Err(SourceJournalError::Binding);
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.lineage.journal().quarantine();
            return Err((self, e));
        }
        let Self {
            released,
            accounting,
            lineage,
        } = self;
        let ExecutedOwnedReduceSettledV2::Ready(ready) = released else {
            unreachable!("checked Ready owner")
        };
        Ok(LiveReadyStepV8 {
            ready,
            accounting,
            lineage,
        })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_stop(
        self,
    ) -> Result<LiveOwnedStepAppendV8<'j>, (Self, SourceJournalError)> {
        let row = (|| {
            self.lineage.validate_current(false)?;
            let ExecutedOwnedReduceSettledV2::Failed(f) = &self.released else {
                return Err(SourceJournalError::Binding);
            };
            let receipt = f.live_receipt_v8()?;
            if !f.observations_succeeded() || receipt["settlement"] != "completed" {
                return Err(SourceJournalError::Binding);
            }
            let ack = self.lineage.current()?;
            let EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupSettled {
                receipt: recorded, ..
            }) = ack.witness.selected_row()
            else {
                return Err(SourceJournalError::Binding);
            };
            if recorded != &receipt {
                return Err(SourceJournalError::Binding);
            }
            let budget=matches!(f.failure(),crate::interpreter::resumable::owned_frame::OwnedFrameFailure::FuelExhausted|crate::interpreter::resumable::owned_frame::OwnedFrameFailure::CallDepthExceeded);
            Ok(EntryV8::Ordinary(SourceJournalEntry::Stop {
                turn: Some(0),
                attempt: Some(0),
                status: if budget {
                    crate::live_invocation::source_journal::SourceStopStatus::BudgetExhausted
                } else {
                    crate::live_invocation::source_journal::SourceStopStatus::Rejected
                },
                reason: if budget {
                    crate::live_invocation::source_journal::SourceStopReason::BudgetExhausted
                } else {
                    crate::live_invocation::source_journal::SourceStopReason::StageRefused
                },
            }))
        })();
        match row {
            Ok(selected) => Ok(LiveOwnedStepAppendV8 {
                owner: StepAppendOwnerV8::Released(self),
                selected,
            }),
            Err(e) => {
                self.lineage.journal().quarantine();
                Err((self, e))
            }
        }
    }
}
impl<'j> LiveReadyStepV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_transfer(
        self,
    ) -> Result<LiveOwnedStepAppendV8<'j>, (Self, SourceJournalError)> {
        let row = (|| {
            self.lineage.validate_current(false)?;
            let step = self.lineage.mapped_step()?;
            let cleanup=match self.lineage.cleanup_origin()?{
                OwnedReduceCleanupOriginV8::CompilerEmpty{..}=>crate::live_invocation::source_journal::owned_wait_v8::reduce_model::ReduceCleanupV8::CompilerEmpty{},
                OwnedReduceCleanupOriginV8::Observed{started}=>{
                    let ack=self.lineage.current()?;
                    if !matches!(ack.witness.selected_row(),EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupSettled{..})){return Err(SourceJournalError::Binding);}
                    crate::live_invocation::source_journal::owned_wait_v8::reduce_model::ReduceCleanupV8::Observed{started,settled:StepLineageV8::true_seq(ack)?}
                }
            };
            let (_, e) = self
                .lineage
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            Ok(EntryV8::Owned(OwnedBodyV8::OwnedStepTransferReserved {
                turn: 0,
                attempt: 0,
                plan: e.wait().binding().into(),
                stage_reservation: original_reservation(&self.lineage)?,
                staged: StepLineageV8::true_seq(self.lineage.staged_ack()?)?,
                cleanup,
                case: step.case().into(),
            }))
        })();
        match row {
            Ok(selected) => Ok(LiveOwnedStepAppendV8 {
                owner: StepAppendOwnerV8::Ready(self),
                selected,
            }),
            Err(e) => Err((self, e)),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn move_fields(
        self,
    ) -> Result<LiveMovedStepV8<'j>, LiveStepMoveFailureV8<'j>> {
        let Self {
            ready,
            accounting,
            lineage,
        } = self;
        let permit = LiveOwnedStepTransferPermitV8 { lineage: &lineage };
        match crate::interpreter::resumable::owned_frame::registered_stage::reduce::consume_live_owned_step_v8(ready,&permit){
            Ok(held)=>Ok(LiveMovedStepV8{held,accounting,lineage}),
            Err(owner)=>{lineage.journal().quarantine();Err(LiveStepMoveFailureV8{owner,accounting,lineage})}
        }
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveStepMoveFailureV8<'j>{
    owner:crate::interpreter::resumable::owned_frame::registered_stage::reduce::LiveOwnedStepTransferFailureV8<'j>,
    accounting:TargetAccounting,
    lineage:StepLineageV8<'j>,
}
impl<'j> LiveMovedStepV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_completed(
        self,
    ) -> Result<LiveOwnedStepAppendV8<'j>, (Self, SourceJournalError)> {
        let row = (|| {
            self.lineage.validate_current(false)?;
            let step = self.lineage.mapped_step()?;
            let target = self.held.live_target_v8()?;
            step.matches_target(&target)?;
            let ack = self.lineage.current()?;
            if !matches!(
                ack.witness.selected_row(),
                EntryV8::Owned(OwnedBodyV8::OwnedStepTransferReserved { .. })
            ) {
                return Err(SourceJournalError::Binding);
            }
            let reserved = StepLineageV8::true_seq(ack)?;
            if self.held.causal_refs() != (self.lineage.facts.effect_settled(), reserved) {
                return Err(SourceJournalError::Binding);
            }
            let (_, e) = self
                .lineage
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan =
                compile_owned_reduce_v2(e.wait()).map_err(|_| SourceJournalError::Binding)?;
            let transfer_digest =
                step.transfer_digest(&scope_of(&self.lineage)?, &plan, 0, 0, reserved)?;
            let target = serde_json::from_value(target).map_err(|_| SourceJournalError::Binding)?;
            Ok(EntryV8::Owned(OwnedBodyV8::OwnedStepTransferCompleted {
                turn: 0,
                attempt: 0,
                reserved,
                target,
                transfer_digest,
            }))
        })();
        match row {
            Ok(selected) => Ok(LiveOwnedStepAppendV8 {
                owner: StepAppendOwnerV8::Moved(self),
                selected,
            }),
            Err(e) => {
                self.lineage.journal().quarantine();
                Err((self, e))
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_transition(
        self,
    ) -> Result<LiveOwnedStepAppendV8<'j>, (Self, SourceJournalError)> {
        let row = (|| {
            self.lineage.validate_current(false)?;
            let step = self.lineage.mapped_step()?;
            let target = self.held.live_target_v8()?;
            step.matches_target(&target)?;
            let ack = self.lineage.current()?;
            let EntryV8::Owned(OwnedBodyV8::OwnedStepTransferCompleted {
                target: recorded, ..
            }) = ack.witness.selected_row()
            else {
                return Err(SourceJournalError::Binding);
            };
            if serde_json::to_value(recorded).map_err(|_| SourceJournalError::Binding)? != target {
                return Err(SourceJournalError::Binding);
            }
            let (_, e) = self
                .lineage
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan =
                compile_owned_reduce_v2(e.wait()).map_err(|_| SourceJournalError::Binding)?;
            let bytes = step.ordinary_carrier_bytes(&plan)?;
            use crate::live_invocation::source_journal::SourceTransitionCase as C;
            let case = match self.held.kind() {
                "continue" => C::Continue,
                "suspend" => C::Suspend,
                "complete" => C::Complete,
                "fail" => C::Fail,
                _ => return Err(SourceJournalError::Binding),
            };
            Ok(EntryV8::Ordinary(SourceJournalEntry::Transition {
                turn: 0,
                attempt: 0,
                case,
                carrier_digest: crate::live_invocation::identity::digest(
                    b"semaprax.agent-step.value.v2\0",
                    &bytes,
                ),
            }))
        })();
        match row {
            Ok(selected) => Ok(LiveOwnedStepAppendV8 {
                owner: StepAppendOwnerV8::Moved(self),
                selected,
            }),
            Err(e) => {
                self.lineage.journal().quarantine();
                Err((self, e))
            }
        }
    }
}

impl<'j> LiveMovedStepV8<'j> {
    /// The consuming turn driver derives its store from the physical owner.
    /// Callers cannot pair an owner with a different registered journal.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn journal(
        &self,
    ) -> &'j SourceOwnedWaitJournalV8 {
        self.lineage.journal()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn kind(&self) -> &'static str {
        self.held.kind()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        &self.accounting
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.lineage.validate_current(false)?;
        self.lineage
            .mapped_step()?
            .matches_target(&self.held.live_target_v8()?)
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod r#continue;
