//! Turn-one Reduce reservation after actual continued cleanup settlement.
//! The ACK holder evaluates the real reducer but does not write a Step row.
use super::*;
use crate::live_invocation::source_journal::SourceStageRole;
use crate::resumable_effects::owned_frame::v2::{compile_owned_reduce_v2, CheckedOwnedReduceV2};

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedReduceReservationAppendV8<
    'j,
> {
    owner: SettledContinuedDecisionCleanupV8<'j>,
    plan: CheckedOwnedReduceV2,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ContinuedReduceReservationRejectionV8<
    'j,
> {
    _owner: SettledContinuedDecisionCleanupV8<'j>,
    error: SourceJournalError,
}
impl<'j> SettledContinuedDecisionCleanupV8<'j> {
    fn continued_reduce_row(&self) -> Result<(CheckedOwnedReduceV2, EntryV8), SourceJournalError> {
        self.validate_live()?;
        if !self.outcome_minted()? {
            return Err(SourceJournalError::Binding);
        }
        let journal = self.owner.owner.owner.journal();
        let (_, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan =
            compile_owned_reduce_v2(execution.wait()).map_err(|_| SourceJournalError::Binding)?;
        let EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
            turn, attempt, ..
        }) = self.witness.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        if *turn == 0
            || *attempt != 0
            || Some(execution.evaluation_fuel())
                != journal.context().ordinary().max_steps_per_stage()
            || plan.binding() != execution.wait().binding()
            || !plan.helper().same_helper(execution.wait().helper())
        {
            return Err(SourceJournalError::Binding);
        }
        Ok((
            plan,
            EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn: *turn,
                attempt: Some(*attempt),
                role: SourceStageRole::Reduce,
                fuel: execution.evaluation_fuel(),
            }),
        ))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_continued_reduce(
        self,
    ) -> Result<LiveContinuedReduceReservationAppendV8<'j>, ContinuedReduceReservationRejectionV8<'j>>
    {
        match self.continued_reduce_row() {
            Ok((plan, selected)) => Ok(LiveContinuedReduceReservationAppendV8 {
                owner: self,
                plan,
                selected,
            }),
            Err(error) => {
                self.owner.owner.owner.journal().quarantine();
                Err(ContinuedReduceReservationRejectionV8 {
                    _owner: self,
                    error,
                })
            }
        }
    }
}
impl<'j> LiveContinuedReduceReservationAppendV8<'j> {
    fn journal(&self) -> &SourceOwnedWaitJournalV8 {
        self.owner.owner.owner.owner.journal()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.session.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.session.acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.owner.validate_live()?;
            let (plan, selected) = self.owner.continued_reduce_row()?;
            if plan.binding() != self.plan.binding()
                || !plan.helper().same_helper(self.plan.helper())
                || selected != self.selected
            {
                return Err(SourceJournalError::Binding);
            }
            self.owner.validate_live()
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinuedReduceReservationAppendPermitV8<'_, 'j>, SourceJournalError>
    {
        self.validate_live()?;
        Ok(FixedOwnedContinuedReduceReservationAppendPermitV8 { owner: self })
    }
    fn hold(&self) -> Result<&ProspectiveOwnedReduceHoldV8<'_>, SourceJournalError> {
        self.owner.owner.owner.owner.hold()
    }
    fn accounting(&self) -> &TargetAccounting {
        self.owner.accounting()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_reduce_successor(
        &self,
        witness: &crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedContinuedReduceSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        witness.validate_predecessor(
            self.journal(),
            self.sequence(),
            self.acknowledged_bytes(),
            &self.selected,
        )?;
        witness.validate_current_session(session)?;
        self.hold()?.validate_continued_spent_reduce_guard(
            self.journal(),
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        let (_, execution) = self
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if self.plan.binding() != execution.wait().binding()
            || !self.plan.helper().same_helper(execution.wait().helper())
            || !matches!(
                &self.selected,
                EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                    turn,
                    attempt: Some(0),
                    role: SourceStageRole::Reduce,
                    fuel,
                }) if *turn > 0 && *fuel == execution.evaluation_fuel()
            )
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedContinuedReduceReservationAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveContinuedReduceReservationAppendV8<'j>,
}
impl FixedOwnedContinuedReduceReservationAppendPermitV8<'_, '_> {
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
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(journal)
            || inventory.sequence() != self.owner.sequence()
            || inventory.acknowledged_bytes() != self.owner.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        self.owner.hold()?.validate_continued_reduce_append_prefix(
            journal,
            inventory,
            self.owner.selected_row(),
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedContinuedReduceSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .hold()?
            .advance_continued_reduce_ack(witness, session)
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedReduceReservedV8<'j> {
    owner: LiveContinuedReduceReservationAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedContinuedReduceSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedReduceAdvanceFailureV8<
    'j,
> {
    Before { _owner: LiveContinuedReduceReservationAppendV8<'j>, _session: AppendSessionV8<'j>, _witness: crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedContinuedReduceSuccessorV8<'j>, error: SourceJournalError },
    After { _owner: LiveContinuedReduceReservedV8<'j>, error: SourceJournalError },
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_continued_reduce_v8<
    'j,
>(
    owner: LiveContinuedReduceReservationAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedContinuedReduceSuccessorV8<'j>,
) -> Result<LiveContinuedReduceReservedV8<'j>, LiveContinuedReduceAdvanceFailureV8<'j>> {
    if let Err(error) = owner.validate_reduce_successor(&witness, &session) {
        return Err(LiveContinuedReduceAdvanceFailureV8::Before {
            _owner: owner,
            _session: session,
            _witness: witness,
            error,
        });
    }
    let reserved = LiveContinuedReduceReservedV8 {
        owner,
        session,
        witness,
    };
    if let Err(error) = reserved.validate_live() {
        return Err(LiveContinuedReduceAdvanceFailureV8::After {
            _owner: reserved,
            error,
        });
    }
    Ok(reserved)
}
impl LiveContinuedReduceReservedV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .validate_reduce_successor(&self.witness, &self.session)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        self.owner.accounting()
    }
}

use crate::interpreter::resumable::owned_frame::registered_stage::effect::OwnedEffectInputsV8;
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::{
    evaluate_live_executed_owned_reduce_v2, CheckedLiveOwnedReduceStageFactsV8,
    LiveOwnedReduceCleanupFailureV8, LiveOwnedReduceCleanupGuardV8, LiveReduceEvaluationFailureV8,
    LiveReduceEvaluationGuardV8, OwnedReduceCleanupOriginV8, StagedExecutedOwnedReduceV2,
};

impl LiveContinuedReduceReservedV8<'_> {
    fn validate_evaluation_current(&self) -> Result<(), SourceJournalError> {
        let result = (|| {
            let journal = self.owner.journal();
            self.witness.validate_predecessor(
                journal,
                self.owner.sequence(),
                self.owner.acknowledged_bytes(),
                &self.owner.selected,
            )?;
            self.witness.validate_current_session(&self.session)?;
            self.owner.hold()?.validate_continued_spent_reduce_guard(
                journal,
                self.session.sequence(),
                self.session.acknowledged_bytes(),
            )?;
            self.owner.owner.validate_spent_reduce_context(
                self.session.sequence(),
                self.session.acknowledged_bytes(),
                false,
                false,
            )?;
            self.witness.validate_current_session(&self.session)
        })();
        result.inspect_err(|_| self.owner.journal().quarantine())
    }
}
struct ContinuedReduceEvaluationPermitV8<'p, 'j> {
    reserved: &'p LiveContinuedReduceReservedV8<'j>,
}
impl LiveReduceEvaluationGuardV8 for ContinuedReduceEvaluationPermitV8<'_, '_> {
    fn validate_current(&self) -> Result<(), SourceJournalError> {
        self.reserved.validate_evaluation_current()
    }
    fn fuel(&self) -> Result<usize, SourceJournalError> {
        let (_, execution) = self
            .reserved
            .owner
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let EntryV8::Ordinary(SourceJournalEntry::StageReservation {
            turn,
            attempt: Some(0),
            role: SourceStageRole::Reduce,
            fuel,
        }) = &self.reserved.owner.selected
        else {
            return Err(SourceJournalError::Binding);
        };
        if *turn == 0 || *fuel != execution.evaluation_fuel() {
            return Err(SourceJournalError::Binding);
        }
        Ok(*fuel)
    }
    fn validate_plan(&self, plan: &CheckedOwnedReduceV2) -> Result<(), SourceJournalError> {
        self.validate_current()?;
        let (_, execution) = self
            .reserved
            .owner
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if plan.binding() != execution.wait().binding()
            || !plan.helper().same_helper(execution.wait().helper())
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedEvaluatedReduceV8<
    'j,
> {
    staged: StagedExecutedOwnedReduceV2<'j>,
    reserved: LiveContinuedReduceReservedV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedReduceEvaluationFailureV8<
    'j,
> {
    Before {
        _owner: LiveContinuedReduceReservedV8<'j>,
        error: SourceJournalError,
    },
    Evaluation {
        _owner: LiveReduceEvaluationFailureV8<'j>,
        _reserved: LiveContinuedReduceReservedV8<'j>,
    },
    After {
        _owner: LiveContinuedEvaluatedReduceV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveContinuedReduceReservedV8<'j> {
    /// Only this consuming ACK holder can move the genuine Outcome into the
    /// existing reducer. Its selected row alone has no such authority.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn evaluate(
        mut self,
    ) -> Result<LiveContinuedEvaluatedReduceV8<'j>, LiveContinuedReduceEvaluationFailureV8<'j>>
    {
        if let Err(error) = self.validate_live() {
            return Err(LiveContinuedReduceEvaluationFailureV8::Before {
                _owner: self,
                error,
            });
        }
        let executed = match self.owner.owner.take_reduce_outcome() {
            Ok(owner) => owner,
            Err(error) => {
                self.owner.journal().quarantine();
                return Err(LiveContinuedReduceEvaluationFailureV8::Before {
                    _owner: self,
                    error,
                });
            }
        };
        let permit = ContinuedReduceEvaluationPermitV8 { reserved: &self };
        let staged =
            match evaluate_live_executed_owned_reduce_v2(executed, &self.owner.plan, &permit) {
                Ok(staged) => staged,
                Err(owner) => {
                    self.owner.journal().quarantine();
                    return Err(LiveContinuedReduceEvaluationFailureV8::Evaluation {
                        _owner: owner,
                        _reserved: self,
                    });
                }
            };
        let evaluated = LiveContinuedEvaluatedReduceV8 {
            staged,
            reserved: self,
        };
        if let Err(error) = evaluated.validate_live() {
            return Err(LiveContinuedReduceEvaluationFailureV8::After {
                _owner: evaluated,
                error,
            });
        }
        Ok(evaluated)
    }
}
impl LiveContinuedEvaluatedReduceV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.reserved.validate_evaluation_current()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        self.reserved.accounting()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn stage_facts(
        &self,
    ) -> Result<CheckedLiveOwnedReduceStageFactsV8, SourceJournalError> {
        self.validate_live()?;
        let (_, execution) = self
            .reserved
            .owner
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let facts = self
            .staged
            .live_stage_facts(execution.wait())
            .map_err(|_| SourceJournalError::Binding)?;
        self.validate_live()?;
        Ok(facts)
    }
}

/// Retains the actual turn-one evaluator and its spent hold across the first
/// Step ACK. No cleanup or result-move authority is inferred from the row.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedStagedStepV8<'j> {
    staged: Option<StagedExecutedOwnedReduceV2<'j>>,
    released: Option<crate::interpreter::resumable::owned_frame::registered_stage::reduce::ExecutedOwnedReduceSettledV2<'j>>,
    ready: Option<crate::interpreter::resumable::owned_frame::registered_stage::reduce::ReadyExecutedOwnedStepV2<'j>>,
    held: Option<crate::interpreter::resumable::owned_frame::registered_stage::reduce::HeldExecutedOwnedStepV2<'j>>,
    receipt: Option<serde_json::Value>,
    reserved: LiveContinuedReduceReservedV8<'j>,
    facts: CheckedLiveOwnedReduceStageFactsV8,
    ack: Option<crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedStepSuccessorV8<'j>>,
    session: Option<AppendSessionV8<'j>>,
    cleanup_ack: Option<(AppendSessionV8<'j>, crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedStepSuccessorV8<'j>)>,
    receipt_ack: Option<(AppendSessionV8<'j>, crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedStepSuccessorV8<'j>)>,
    transfer_ack: Option<(AppendSessionV8<'j>, crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedStepSuccessorV8<'j>)>,
    completed_ack: Option<(AppendSessionV8<'j>, crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedStepSuccessorV8<'j>)>,
    transition_ack: Option<(AppendSessionV8<'j>, crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedStepSuccessorV8<'j>)>,
    terminal_ack: Option<(AppendSessionV8<'j>, crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedStepSuccessorV8<'j>)>,
}

impl<'j> LiveContinuedEvaluatedReduceV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_step(
        self,
    ) -> Result<crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveOwnedStepAppendV8<'j>, (Self, SourceJournalError)>{
        let checked = (|| {
            let facts = self.stage_facts()?;
            let step = facts.step().ok_or(SourceJournalError::Binding)?;
            let journal = self.reserved.owner.journal();
            let held = journal.hold()?;
            let scope = &held.registration().expected_facts().scope;
            let scope = serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()});
            let EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn,
                attempt: Some(attempt),
                role: SourceStageRole::Reduce,
                ..
            }) = &self.reserved.owner.selected
            else {
                return Err(SourceJournalError::Binding);
            };
            if *turn == 0 || *attempt != 0 {
                return Err(SourceJournalError::Binding);
            }
            let reservation = u32::try_from(
                self.reserved
                    .session
                    .sequence()
                    .checked_sub(1)
                    .ok_or(SourceJournalError::Order)?,
            )
            .map_err(|_| SourceJournalError::Capacity)?;
            let plan = self.reserved.owner.plan.binding();
            let digest = crate::live_invocation::source_journal::owned_wait_v8::reduce_wire::recipe_digest(
                crate::live_invocation::source_journal::owned_wait_v8::reduce_wire::ReduceRecipeV8::Step,
                &serde_json::json!({"scope":scope,"binding":plan,"plan":plan,"turn":turn,"attempt":attempt,"stage_reservation":reservation,"step":step}),
            )?;
            let row = EntryV8::Owned(OwnedBodyV8::OwnedReduceStaged {
                turn: *turn,
                attempt: *attempt,
                plan: plan.into(),
                stage_reservation: reservation,
                effect_cleanup_settled: facts.effect_settled(),
                step: step.clone(),
                step_digest: digest,
                consumed: u64::try_from(facts.consumed())
                    .map_err(|_| SourceJournalError::Capacity)?,
            });
            Ok((facts, row))
        })();
        let (facts, selected) = match checked {
            Ok(pair) => pair,
            Err(error) => {
                self.reserved.owner.journal().quarantine();
                return Err((self, error));
            }
        };
        let LiveContinuedEvaluatedReduceV8 { staged, reserved } = self;
        Ok(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveOwnedStepAppendV8::continued(
            LiveContinuedStagedStepV8 { staged: Some(staged), released: None, ready: None, held: None, receipt: None, reserved, facts, ack: None, session: None, cleanup_ack: None, receipt_ack: None, transfer_ack: None, completed_ack: None, transition_ack: None, terminal_ack: None }, selected,
        ))
    }
}

impl<'j> LiveContinuedStagedStepV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn journal(
        &self,
    ) -> &SourceOwnedWaitJournalV8 {
        self.reserved.owner.journal()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn cursor(
        &self,
    ) -> (usize, usize) {
        if let Some((session, _)) = &self.terminal_ack {
            return (session.sequence(), session.acknowledged_bytes());
        }
        if let Some((session, _)) = &self.transition_ack {
            return (session.sequence(), session.acknowledged_bytes());
        }
        if let Some((session, _)) = &self.completed_ack {
            return (session.sequence(), session.acknowledged_bytes());
        }
        if let Some((session, _)) = &self.transfer_ack {
            return (session.sequence(), session.acknowledged_bytes());
        }
        if let Some((session, _)) = &self.receipt_ack {
            return (session.sequence(), session.acknowledged_bytes());
        }
        if let Some((session, _)) = &self.cleanup_ack {
            return (session.sequence(), session.acknowledged_bytes());
        }
        self.session.as_ref().map_or(
            (
                self.reserved.session.sequence(),
                self.reserved.session.acknowledged_bytes(),
            ),
            |s| (s.sequence(), s.acknowledged_bytes()),
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            if let Some((s, w)) = self
                .terminal_ack
                .as_ref()
                .or(self.transition_ack.as_ref())
                .or(self.completed_ack.as_ref())
                .or(self.transfer_ack.as_ref())
            {
                w.validate_current_session(s)?;
                self.reserved.owner.owner.validate_spent_reduce_context(
                    s.sequence(),
                    s.acknowledged_bytes(),
                    true,
                    false,
                )
            } else if let Some((s, w)) = &self.receipt_ack {
                w.validate_current_session(s)?;
                let EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupSettled { receipt, .. }) =
                    w.selected_row()
                else {
                    return Err(SourceJournalError::Binding);
                };
                if receipt != &self.receipt()? {
                    return Err(SourceJournalError::Binding);
                }
                self.reserved.owner.owner.validate_spent_reduce_context(
                    s.sequence(),
                    s.acknowledged_bytes(),
                    true,
                    true,
                )
            } else if let Some((s, w)) = &self.cleanup_ack {
                w.validate_current_session(s)?;
                self.reserved.owner.owner.validate_spent_reduce_context(
                    s.sequence(),
                    s.acknowledged_bytes(),
                    true,
                    true,
                )
            } else if let (Some(w), Some(s)) = (&self.ack, &self.session) {
                w.validate_current_session(s)?;
                self.reserved.owner.owner.validate_spent_reduce_context(
                    s.sequence(),
                    s.acknowledged_bytes(),
                    true,
                    false,
                )
            } else {
                self.reserved.validate_evaluation_current()
            }
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_new_prefix(
        &self,
        session: &AppendSessionV8<'_>,
        incurred: bool,
    ) -> Result<(), SourceJournalError> {
        self.reserved.owner.owner.validate_spent_reduce_context(
            session.sequence(),
            session.acknowledged_bytes(),
            true,
            incurred,
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn hold(
        &self,
    ) -> Result<&ProspectiveOwnedReduceHoldV8<'_>, SourceJournalError> {
        self.reserved.owner.hold()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledge(
        mut self,
        session: AppendSessionV8<'j>,
        witness: crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedStepSuccessorV8<'j>,
        selected: &EntryV8,
    ) -> Result<Self, (Self, SourceJournalError)> {
        if self.terminal_ack.is_some() {
            self.journal().quarantine();
            return Err((self, SourceJournalError::Order));
        }
        if self.ack.is_none()
            && self.session.is_none()
            && matches!(
                selected,
                EntryV8::Owned(OwnedBodyV8::OwnedReduceStaged { .. })
            )
        {
            self.ack = Some(witness);
            self.session = Some(session);
        } else if self.receipt_ack.is_some()
            && self.transfer_ack.is_none()
            && self.ready.is_some()
            && matches!(
                selected,
                EntryV8::Owned(OwnedBodyV8::OwnedStepTransferReserved { .. })
            )
        {
            self.transfer_ack = Some((session, witness));
        } else if self.transfer_ack.is_some()
            && self.completed_ack.is_none()
            && self.held.is_some()
            && matches!(
                selected,
                EntryV8::Owned(OwnedBodyV8::OwnedStepTransferCompleted { .. })
            )
        {
            self.completed_ack = Some((session, witness));
        } else if self.completed_ack.is_some()
            && self.transition_ack.is_none()
            && self.held.is_some()
            && matches!(
                selected,
                EntryV8::Ordinary(SourceJournalEntry::Transition { .. })
            )
        {
            self.transition_ack = Some((session, witness));
        } else if self.transition_ack.is_some()
            && self.terminal_ack.is_none()
            && self.held.is_some()
            && matches!(
                selected,
                EntryV8::Ordinary(SourceJournalEntry::TerminalSnapshot { .. })
            )
        {
            self.terminal_ack = Some((session, witness));
        } else if self.ack.is_some()
            && self.session.is_some()
            && self.cleanup_ack.is_some()
            && self.receipt_ack.is_none()
            && self.released.is_some()
            && matches!(
                selected,
                EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupSettled { .. })
            )
        {
            self.receipt_ack = Some((session, witness));
        } else if self.ack.is_some()
            && self.session.is_some()
            && self.cleanup_ack.is_none()
            && self.staged.is_some()
            && matches!(
                selected,
                EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupStarted { .. })
            )
        {
            self.cleanup_ack = Some((session, witness));
        } else {
            self.journal().quarantine();
            return Err((self, SourceJournalError::Order));
        }
        if let Err(error) = self.validate_live() {
            self.journal().quarantine();
            return Err((self, error));
        }
        Ok(self)
    }

    /// Selects only the compiler's active cleanup vector while retaining the
    /// genuine staged reducer. Physical cleanup awaits this row's ACK.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_cleanup(
        self,
    ) -> Result<crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveOwnedStepAppendV8<'j>, (Self, SourceJournalError)>{
        let selected = (|| {
            self.validate_live()?;
            let staged = self.ack.as_ref().ok_or(SourceJournalError::Order)?;
            if self.cleanup_ack.is_some() {
                return Err(SourceJournalError::Order);
            }
            let facts = &self.facts;
            let staged_sequence = u32::try_from(
                staged
                    .sequence()
                    .checked_sub(1)
                    .ok_or(SourceJournalError::Order)?,
            )
            .map_err(|_| SourceJournalError::Capacity)?;
            let reservation = u32::try_from(
                self.reserved
                    .session
                    .sequence()
                    .checked_sub(1)
                    .ok_or(SourceJournalError::Order)?,
            )
            .map_err(|_| SourceJournalError::Capacity)?;
            let EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn,
                attempt: Some(attempt),
                role: SourceStageRole::Reduce,
                ..
            }) = &self.reserved.owner.selected
            else {
                return Err(SourceJournalError::Binding);
            };
            let (_, execution) = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = execution.wait().binding();
            let held = self.journal().hold()?;
            let s = &held.registration().expected_facts().scope;
            let scope = serde_json::json!({"program_root":s.program_root(),"invocation":s.invocation_id(),"policy_epoch":s.policy_epoch()});
            let raw = facts
                .cleanup_basis(Some(staged_sequence))
                .map_err(|_| SourceJournalError::Binding)?;
            let digest = crate::live_invocation::source_journal::owned_wait_v8::reduce_wire::recipe_digest(
                crate::live_invocation::source_journal::owned_wait_v8::reduce_wire::ReduceRecipeV8::Basis,
                &serde_json::json!({"scope":scope,"binding":plan,"plan":plan,"turn":turn,"attempt":attempt,"stage_reservation":reservation,"basis":raw}),
            )?;
            let basis = serde_json::from_value(raw).map_err(|_| SourceJournalError::Binding)?;
            Ok(EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupStarted {
                turn: *turn,
                attempt: *attempt,
                plan: plan.into(),
                stage_reservation: reservation,
                effect_cleanup_settled: facts.effect_settled(),
                basis,
                basis_digest: digest,
                consumed: u64::try_from(facts.consumed())
                    .map_err(|_| SourceJournalError::Capacity)?,
                operations: facts.operations().clone(),
            }))
        })();
        match selected {
            Ok(row) => Ok(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveOwnedStepAppendV8::continued(self, row)),
            Err(error) => {
                self.journal().quarantine();
                Err((self, error))
            }
        }
    }

    fn cleanup_origin(&self) -> Result<OwnedReduceCleanupOriginV8, SourceJournalError> {
        let (session, witness) = self
            .cleanup_ack
            .as_ref()
            .ok_or(SourceJournalError::Binding)?;
        if !matches!(
            witness.selected_row(),
            EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupStarted { .. })
        ) {
            return Err(SourceJournalError::Binding);
        }
        let started = u32::try_from(
            session
                .sequence()
                .checked_sub(1)
                .ok_or(SourceJournalError::Order)?,
        )
        .map_err(|_| SourceJournalError::Capacity)?;
        Ok(OwnedReduceCleanupOriginV8::Observed { started })
    }
    fn receipt(&self) -> Result<serde_json::Value, SourceJournalError> {
        if let Some(receipt) = &self.receipt {
            return Ok(receipt.clone());
        }
        match self.released.as_ref().ok_or(SourceJournalError::Binding)? {
            crate::interpreter::resumable::owned_frame::registered_stage::reduce::ExecutedOwnedReduceSettledV2::Ready(ready) => ready.live_receipt_v8(),
            crate::interpreter::resumable::owned_frame::registered_stage::reduce::ExecutedOwnedReduceSettledV2::Failed(failed) => failed.live_receipt_v8(),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn release(
        mut self,
        observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
    ) -> Result<Self, LiveContinuedStepCleanupFailureV8<'j>> {
        if let Err(error) = self
            .validate_live()
            .and_then(|_| self.cleanup_origin().map(|_| ()))
        {
            return Err(LiveContinuedStepCleanupFailureV8::Before { owner: self, error });
        }
        let Some(staged) = self.staged.take() else {
            self.journal().quarantine();
            return Err(LiveContinuedStepCleanupFailureV8::Before {
                owner: self,
                error: SourceJournalError::Order,
            });
        };
        let result = {
            let permit = ContinuedStepCleanupPermitV8 { owner: &self };
            crate::interpreter::resumable::owned_frame::registered_stage::reduce::settle_live_owned_reduce_v8(staged, &permit, observe)
        };
        match result {
            Ok(released) => {
                self.released = Some(released);
                if let Err(error) = self.validate_live() {
                    return Err(LiveContinuedStepCleanupFailureV8::After { owner: self, error });
                }
                Ok(self)
            }
            Err(owner) => {
                self.journal().quarantine();
                Err(LiveContinuedStepCleanupFailureV8::Engine {
                    reached: self,
                    owner,
                })
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_receipt(
        self,
    ) -> Result<crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveOwnedStepAppendV8<'j>, (Self, SourceJournalError)>{
        let selected = (|| {
            self.validate_live()?;
            let OwnedReduceCleanupOriginV8::Observed { started } = self.cleanup_origin()? else {
                return Err(SourceJournalError::Binding);
            };
            let EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn,
                attempt: Some(attempt),
                role: SourceStageRole::Reduce,
                ..
            }) = &self.reserved.owner.selected
            else {
                return Err(SourceJournalError::Binding);
            };
            Ok(EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupSettled {
                turn: *turn,
                attempt: *attempt,
                started,
                receipt: self.receipt()?,
            }))
        })();
        match selected {
            Ok(row) => Ok(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveOwnedStepAppendV8::continued(self, row)),
            Err(error) => {
                self.journal().quarantine();
                Err((self, error))
            }
        }
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedStepCleanupFailureV8<
    'j,
> {
    Before {
        owner: LiveContinuedStagedStepV8<'j>,
        error: SourceJournalError,
    },
    Engine {
        reached: LiveContinuedStagedStepV8<'j>,
        owner: LiveOwnedReduceCleanupFailureV8<'j>,
    },
    After {
        owner: LiveContinuedStagedStepV8<'j>,
        error: SourceJournalError,
    },
}

struct ContinuedStepCleanupPermitV8<'a, 'j> {
    owner: &'a LiveContinuedStagedStepV8<'j>,
}
impl LiveOwnedReduceCleanupGuardV8 for ContinuedStepCleanupPermitV8<'_, '_> {
    fn cleanup_origin(&self) -> Result<OwnedReduceCleanupOriginV8, SourceJournalError> {
        self.owner.cleanup_origin()
    }
    fn validate_cleanup_current(&self) -> Result<(), SourceJournalError> {
        self.owner.validate_live()?;
        self.owner.cleanup_origin().map(|_| ())
    }
    fn validate_staged(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
        facts: &CheckedLiveOwnedReduceStageFactsV8,
    ) -> Result<(), SourceJournalError> {
        self.validate_cleanup_current()?;
        let (runtime, execution) = self
            .owner
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let EntryV8::Ordinary(SourceJournalEntry::StageReservation {
            turn,
            attempt: Some(attempt),
            role: SourceStageRole::Reduce,
            ..
        }) = &self.owner.reserved.owner.selected
        else {
            return Err(SourceJournalError::Binding);
        };
        let (sequence, bytes) = self.owner.cursor();
        let staged_sequence = u32::try_from(
            self.owner
                .session
                .as_ref()
                .ok_or(SourceJournalError::Binding)?
                .sequence()
                .checked_sub(1)
                .ok_or(SourceJournalError::Order)?,
        )
        .map_err(|_| SourceJournalError::Capacity)?;
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(execution, inputs.execution)
            || inputs.turn != *turn
            || inputs.attempt != *attempt
            || facts.allowance() != self.owner.facts.allowance()
            || facts.consumed() != self.owner.facts.consumed()
            || facts.effect_settled() != self.owner.facts.effect_settled()
            || facts.step() != self.owner.facts.step()
            || facts.operations() != self.owner.facts.operations()
            || facts.active_flags() != self.owner.facts.active_flags()
            || facts
                .cleanup_basis(Some(staged_sequence))
                .map_err(|_| SourceJournalError::Binding)?
                != self
                    .owner
                    .facts
                    .cleanup_basis(Some(staged_sequence))
                    .map_err(|_| SourceJournalError::Binding)?
        {
            return Err(SourceJournalError::Binding);
        }
        inputs.store.validate_prefix(sequence, bytes)?;
        self.validate_cleanup_current()
    }
}

mod step_transfer;
mod terminal_report;
