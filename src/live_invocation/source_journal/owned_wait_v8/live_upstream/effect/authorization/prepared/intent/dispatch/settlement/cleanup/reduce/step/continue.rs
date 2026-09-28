//! Closed same-State continuation. The prior Proposal is historical lineage,
//! never a source of a new model grant. Every holder retains the same ledger/hold.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::{
    observe_live_continued_state_v8, ContinuedOwnedObserveV2, LiveContinuedObserveFailureV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedContinueSuccessorV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ContinueLineageV8<'j> {
    step: StepLineageV8<'j>,
    turn: u32,
    acks: Vec<ContinueAckV8<'j>>,
}
struct ContinueAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinueSuccessorV8<'j>,
}
impl<'j> ContinueLineageV8<'j> {
    fn current(&self) -> Result<&AppendSessionV8<'j>, SourceJournalError> {
        match self.acks.last() {
            Some(ack) => Ok(&ack.session),
            None => Ok(&self.step.current()?.session),
        }
    }
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.step.journal()
    }
    fn guard(&self) -> Result<(), SourceJournalError> {
        let result = (|| {
            if self.acks.is_empty() {
                return self.step.validate_current(false);
            }
            let session = self.current()?;
            self.acks
                .last()
                .ok_or(SourceJournalError::Order)?
                .witness
                .validate_current_session(session)?;
            let origin = self.step.origin();
            origin.hold.validate_continue_guard(
                self.journal(),
                session.sequence(),
                session.acknowledged_bytes(),
            )?;
            let held = self.journal().hold()?;
            let (runtime, execution) = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            runtime
                .owned_wait_effects_v8(execution)
                .map_err(|_| SourceJournalError::Binding)?;
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
            origin.hold.validate_continue_guard(
                self.journal(),
                session.sequence(),
                session.acknowledged_bytes(),
            )?;
            self.acks
                .last()
                .ok_or(SourceJournalError::Order)?
                .witness
                .validate_current_session(session)
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
}
/// The physical mapped State precedes its ledger and the same spent hold.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedStateV8<'j> {
    held: HeldExecutedOwnedStepV2<'j>,
    accounting: TargetAccounting,
    lineage: ContinueLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedContinueAppendV8<'j> {
    owner: LiveContinuedStateV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinueFailureV8<'j> {
    Moved {
        owner: LiveMovedStepV8<'j>,
        error: SourceJournalError,
    },
    State {
        owner: LiveContinuedStateV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveOwnedContinueAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinueSuccessorV8<'j>,
        error: SourceJournalError,
    },
    Observe {
        owner: LiveContinuedObserveFailureV8<'j>,
        accounting: TargetAccounting,
        lineage: ContinueLineageV8<'j>,
    },
    After {
        owner: LiveObservedContinueV8<'j>,
        error: SourceJournalError,
    },
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveObservedContinueV8<'j> {
    outcome: ContinuedOwnedObserveV2<'j>,
    accounting: TargetAccounting,
    lineage: ContinueLineageV8<'j>,
    consumed: usize,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinueAcknowledgedV8<'j> {
    State(LiveContinuedStateV8<'j>),
    Observed(LiveObservedContinueV8<'j>),
}
impl LiveObservedContinueV8<'_> {
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_observation(
        &self,
    ) -> &crate::interpreter::resumable::ResumableChannelValue {
        let ContinuedOwnedObserveV2::Observed(observed) = &self.outcome else {
            panic!("actual observation")
        };
        observed.observation()
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn is_observed(&self) -> bool {
        matches!(self.outcome, ContinuedOwnedObserveV2::Observed(_))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn is_failed(&self) -> bool {
        matches!(self.outcome, ContinuedOwnedObserveV2::Failed(_))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn turn(&self) -> u32 {
        self.lineage.turn
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        &self.accounting
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn consumed(&self) -> usize {
        self.consumed
    }
}
impl<'j> LiveMovedStepV8<'j> {
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_observe_oracle(
        &self,
    ) -> (crate::interpreter::resumable::ResumableChannelValue, usize) {
        crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_oracle_v8(&self.held)
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_continue(
        self,
    ) -> Result<LiveOwnedContinueAppendV8<'j>, LiveContinueFailureV8<'j>> {
        let selected = (|| {
            self.validate_live()?;
            if self.held.kind() != "continue" {
                return Err(SourceJournalError::Binding);
            }
            let context = self.lineage.journal().context();
            let fold_context = context.fold();
            let (_, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
            let target = self.held.live_target_v8()?;
            let old_turn = match self.lineage.current()?.witness.selected_row() {
                EntryV8::Ordinary(SourceJournalEntry::Transition {
                    turn,
                    case: crate::live_invocation::source_journal::SourceTransitionCase::Continue,
                    ..
                }) => *turn,
                _ => return Err(SourceJournalError::Order),
            };
            let turn = old_turn
                .checked_add(1)
                .ok_or(SourceJournalError::Capacity)?;
            if !fold_context.cumulative_initialization
                || turn >= execution.ordinary().max_iterations()
                || target["kind"] != "continue"
            {
                return Err(SourceJournalError::Binding);
            }
            let state = target
                .get("state")
                .ok_or(SourceJournalError::Binding)?
                .clone();
            Ok((
                turn,
                EntryV8::Owned(OwnedBodyV8::OwnedStateCommitted {
                    turn,
                    state: state.clone(),
                    argument_digest: wire::record_argument_digest(&state),
                    cleanup_plan_digest: fold_context.cleanup_plan_digest.clone(),
                }),
            ))
        })();
        let (turn, selected) = match selected {
            Ok(x) => x,
            Err(error) => return Err(LiveContinueFailureV8::Moved { owner: self, error }),
        };
        let LiveMovedStepV8 {
            held,
            accounting,
            lineage,
        } = self;
        Ok(LiveOwnedContinueAppendV8 {
            owner: LiveContinuedStateV8 {
                held,
                accounting,
                lineage: ContinueLineageV8 {
                    step: lineage,
                    turn,
                    acks: Vec::new(),
                },
            },
            selected,
        })
    }
}
impl<'j> LiveContinuedStateV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_observe(
        self,
    ) -> Result<LiveOwnedContinueAppendV8<'j>, LiveContinueFailureV8<'j>> {
        let selected = (|| {
            self.lineage.guard()?;
            if self.lineage.acks.len() != 1 {
                return Err(SourceJournalError::Order);
            }
            let fuel = self
                .lineage
                .journal()
                .context()
                .ordinary()
                .max_steps_per_stage()
                .ok_or(SourceJournalError::Binding)?;
            Ok(EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn: self.lineage.turn,
                attempt: None,
                role: crate::live_invocation::source_journal::SourceStageRole::Observe,
                fuel,
            }))
        })();
        match selected {
            Ok(selected) => Ok(LiveOwnedContinueAppendV8 {
                owner: self,
                selected,
            }),
            Err(error) => Err(LiveContinueFailureV8::State { owner: self, error }),
        }
    }
}
impl<'j> LiveOwnedContinueAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.owner.lineage.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner
            .lineage
            .current()
            .expect("actual lineage")
            .sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner
            .lineage
            .current()
            .expect("actual lineage")
            .acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.owner.lineage.guard()?;
        if self.owner.held.kind() != "continue" || self.owner.lineage.acks.len() > 1 {
            return Err(SourceJournalError::Binding);
        }
        let target = self.owner.held.live_target_v8()?;
        match &self.selected {
            EntryV8::Owned(OwnedBodyV8::OwnedStateCommitted {
                turn,
                state,
                argument_digest,
                cleanup_plan_digest,
            }) if self.owner.lineage.acks.is_empty()
                && *turn == self.owner.lineage.turn
                && target.get("state") == Some(state)
                && wire::record_argument_digest(state) == *argument_digest
                && cleanup_plan_digest
                    == &self
                        .owner
                        .lineage
                        .journal()
                        .context()
                        .fold()
                        .cleanup_plan_digest => {}
            EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn,
                attempt: None,
                role: crate::live_invocation::source_journal::SourceStageRole::Observe,
                fuel,
            }) if self.owner.lineage.acks.len() == 1
                && *turn == self.owner.lineage.turn
                && Some(*fuel)
                    == self
                        .owner
                        .lineage
                        .journal()
                        .context()
                        .ordinary()
                        .max_steps_per_stage() => {}
            _ => return Err(SourceJournalError::Binding),
        }
        self.owner.lineage.guard()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinueAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedContinueAppendPermitV8 { owner: self })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continue_successor(
        &self,
        witness: &VerifiedOwnedContinueSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            witness.validate_predecessor(
                self.owner.lineage.journal(),
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_current_session(session)?;
            self.owner
                .lineage
                .step
                .origin()
                .hold
                .validate_continue_guard(
                    self.owner.lineage.journal(),
                    session.sequence(),
                    session.acknowledged_bytes(),
                )?;
            let origin = self.owner.lineage.step.origin();
            let (runtime, execution) = self
                .owner
                .lineage
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let held = self.owner.lineage.journal().hold()?;
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
            let ordinary = self.owner.lineage.journal().context().ordinary();
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
            self.owner.held.live_target_v8()?;
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| self.owner.lineage.journal().quarantine())
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedContinueAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveOwnedContinueAppendV8<'j>,
}
impl FixedOwnedContinueAppendPermitV8<'_, '_> {
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .lineage
            .step
            .origin()
            .hold
            .validate_continue_append_prefix(journal, inventory, &self.owner.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinueSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .lineage
            .step
            .origin()
            .hold
            .advance_continue_ack(witness, session)
    }
}
/// Minted only after the actual StateCommitted and Observe reservation ACKs.
pub(crate) struct LiveContinueObservePermitV8<'p, 'j> {
    lineage: &'p ContinueLineageV8<'j>,
}
impl LiveContinueObservePermitV8<'_, '_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        self.lineage.guard()
    }
    pub(crate) fn turn(&self) -> u32 {
        self.lineage.turn
    }
    pub(crate) fn fuel(&self) -> Result<usize, SourceJournalError> {
        self.lineage
            .journal()
            .context()
            .ordinary()
            .max_steps_per_stage()
            .ok_or(SourceJournalError::Binding)
    }
    pub(crate) fn causal_refs(&self) -> Result<(u32, u32), SourceJournalError> {
        if self.lineage.acks.len() != 2 {
            return Err(SourceJournalError::Order);
        }
        let transition = StepLineageV8::true_seq(self.lineage.step.current()?)?;
        let reservation = u32::try_from(
            self.lineage
                .current()?
                .sequence()
                .checked_sub(1)
                .ok_or(SourceJournalError::Order)?,
        )
        .map_err(|_| SourceJournalError::Capacity)?;
        Ok((transition, reservation))
    }
    pub(crate) fn matches_inputs(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let origin = self.lineage.step.origin();
        let (runtime, execution) = self
            .lineage
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(execution, inputs.execution)
            || !std::ptr::eq(origin.policy, inputs.policy)
            || !std::ptr::eq(origin.cancellation, inputs.cancellation)
            || inputs.turn.checked_add(1) != Some(self.lineage.turn)
            || inputs.proposal.carrier() != origin.proposal.carrier()
            || inputs.proposal.ordinary_digest() != origin.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        // Earlier Proposal is checked historical lineage, never installed as next K.
        self.validate_guard()
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_continue_v8<
    'j,
>(
    obligation: LiveOwnedContinueAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinueSuccessorV8<'j>,
) -> Result<LiveContinueAcknowledgedV8<'j>, LiveContinueFailureV8<'j>> {
    if let Err(error) = obligation.validate_continue_successor(&witness, &session) {
        return Err(LiveContinueFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    let LiveOwnedContinueAppendV8 { mut owner, .. } = obligation;
    owner.lineage.acks.push(ContinueAckV8 { session, witness });
    if owner.lineage.acks.len() == 1 {
        return Ok(LiveContinueAcknowledgedV8::State(owner));
    }
    let LiveContinuedStateV8 {
        held,
        accounting,
        lineage,
    } = owner;
    let result = {
        let permit = LiveContinueObservePermitV8 { lineage: &lineage };
        observe_live_continued_state_v8(held, &permit)
    };
    match result {
        Err(owner) => Err(LiveContinueFailureV8::Observe {
            owner,
            accounting,
            lineage,
        }),
        Ok((outcome, consumed)) => {
            let actual = LiveObservedContinueV8 {
                outcome,
                accounting,
                lineage,
                consumed,
            };
            if let Err(error) = actual.lineage.guard() {
                return Err(LiveContinueFailureV8::After {
                    owner: actual,
                    error,
                });
            }
            Ok(LiveContinueAcknowledgedV8::Observed(actual))
        }
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod settlement;
