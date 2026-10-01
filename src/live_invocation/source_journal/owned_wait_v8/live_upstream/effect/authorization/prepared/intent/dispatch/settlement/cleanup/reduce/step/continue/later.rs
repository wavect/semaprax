//! Later Continue handoff. The original turn's lineage remains sealed inside
//! the actual continued Step; these owners stop at the next physical Observe.
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod settlement;
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::HeldExecutedOwnedStepV2;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::ProspectiveOwnedReduceHoldV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedStagedStepV8;
use crate::live_invocation::source_journal::owned_wait_v8::wire;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LaterContinueLineageV8<'j> {
    source: Box<LiveContinuedStagedStepV8<'j>>,
    journal: &'j SourceOwnedWaitJournalV8,
    proposal: CheckedOwnedWaitProposalV8,
    policy: &'j crate::resumable_effects::CapabilityPolicy,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
    turn: u32,
    acks: Vec<ContinueAckV8<'j>>,
}
impl<'j> LaterContinueLineageV8<'j> {
    fn current(&self) -> Result<&AppendSessionV8<'j>, SourceJournalError> {
        self.acks
            .last()
            .map(|ack| &ack.session)
            .map(Ok)
            .unwrap_or_else(|| self.source.continue_session())
    }
    fn hold(&self) -> Result<&ProspectiveOwnedReduceHoldV8<'_>, SourceJournalError> {
        self.source.hold()
    }
    fn guard(&self) -> Result<(), SourceJournalError> {
        let result = (|| {
            if self.acks.is_empty() {
                self.source.validate_live()?;
                return self.source.continue_transition().map(|_| ());
            }
            let session = self.current()?;
            self.acks
                .last()
                .ok_or(SourceJournalError::Order)?
                .witness
                .validate_current_session(session)?;
            self.hold()?.validate_continue_guard(
                self.journal,
                session.sequence(),
                session.acknowledged_bytes(),
            )?;
            let held = self.journal.hold()?;
            let (runtime, execution) = self
                .journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = plan_owned_effect_v8(
                runtime,
                execution,
                &held.registration().expected_facts().scope,
                &self.proposal,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            if !self.policy.allows(plan.operation().effect_id()) {
                return Err(SourceJournalError::Binding);
            }
            let ordinary = self.journal.context().ordinary();
            check_clock_v8(
                &held,
                session.sequence(),
                session.acknowledged_bytes(),
                self.cancellation,
                self.source.continued_model_origin()?.1,
                ordinary.clock_domain(),
                ordinary.initial_millis(),
                ordinary.deadline_millis(),
            )?;
            self.hold()?.validate_continue_guard(
                self.journal,
                session.sequence(),
                session.acknowledged_bytes(),
            )?;
            self.acks
                .last()
                .ok_or(SourceJournalError::Order)?
                .witness
                .validate_current_session(session)
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterContinueStateV8<'j> {
    held: HeldExecutedOwnedStepV2<'j>,
    accounting: TargetAccounting,
    lineage: LaterContinueLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterContinueAppendV8<'j> {
    owner: LiveLaterContinueStateV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveLaterContinueFailureV8<'j> {
    Before {
        owner: LiveContinuedStagedStepV8<'j>,
        error: SourceJournalError,
    },
    State {
        owner: LiveLaterContinueStateV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveLaterContinueAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinueSuccessorV8<'j>,
        error: SourceJournalError,
    },
    Observe {
        owner: LiveContinuedObserveFailureV8<'j>,
        accounting: TargetAccounting,
        lineage: LaterContinueLineageV8<'j>,
    },
    After {
        owner: LiveLaterObservedContinueV8<'j>,
        error: SourceJournalError,
    },
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterObservedContinueV8<'j>
{
    outcome: ContinuedOwnedObserveV2<'j>,
    accounting: TargetAccounting,
    lineage: LaterContinueLineageV8<'j>,
    consumed: usize,
}
impl LiveLaterObservedContinueV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn is_observed(&self) -> bool {
        matches!(self.outcome, ContinuedOwnedObserveV2::Observed(_))
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
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveLaterContinueAcknowledgedV8<
    'j,
> {
    State(LiveLaterContinueStateV8<'j>),
    Observed(LiveLaterObservedContinueV8<'j>),
}
impl<'j> LiveContinuedStagedStepV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_later_continue(
        mut self,
    ) -> Result<LiveLaterContinueAppendV8<'j>, LiveLaterContinueFailureV8<'j>> {
        let selected = (|| {
            self.validate_live()?;
            let (_, old_turn) = self.continue_transition()?;
            if old_turn != 1 {
                return Err(SourceJournalError::Binding);
            }
            let turn = old_turn
                .checked_add(1)
                .ok_or(SourceJournalError::Capacity)?;
            let target = self.continue_target()?;
            if target["kind"] != "continue" {
                return Err(SourceJournalError::Binding);
            }
            let state = target
                .get("state")
                .ok_or(SourceJournalError::Binding)?
                .clone();
            let inputs = self.continue_inputs()?;
            let (_, execution) = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            if turn >= execution.ordinary().max_iterations()
                || inputs.turn != old_turn
                || inputs.attempt != 0
            {
                return Err(SourceJournalError::Binding);
            }
            let (journal, _, accounting) = self.continued_model_origin()?;
            Ok((
                turn,
                journal,
                accounting,
                inputs.proposal.clone(),
                inputs.policy,
                inputs.cancellation,
                EntryV8::Owned(OwnedBodyV8::OwnedStateCommitted {
                    turn,
                    argument_digest: wire::record_argument_digest(&state),
                    state,
                    cleanup_plan_digest: self
                        .journal()
                        .context()
                        .fold()
                        .cleanup_plan_digest
                        .clone(),
                }),
            ))
        })();
        let (turn, journal, accounting, proposal, policy, cancellation, selected) = match selected {
            Ok(x) => x,
            Err(error) => return Err(LiveLaterContinueFailureV8::Before { owner: self, error }),
        };
        let held = match self.take_continue_held() {
            Ok(held) => held,
            Err(error) => return Err(LiveLaterContinueFailureV8::Before { owner: self, error }),
        };
        Ok(LiveLaterContinueAppendV8 {
            owner: LiveLaterContinueStateV8 {
                held,
                accounting,
                lineage: LaterContinueLineageV8 {
                    source: Box::new(self),
                    journal,
                    proposal,
                    policy,
                    cancellation,
                    turn,
                    acks: Vec::new(),
                },
            },
            selected,
        })
    }
}
impl<'j> LiveLaterContinueStateV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_observe(
        self,
    ) -> Result<LiveLaterContinueAppendV8<'j>, LiveLaterContinueFailureV8<'j>> {
        let selected = (|| {
            self.lineage.guard()?;
            if self.lineage.acks.len() != 1 {
                return Err(SourceJournalError::Order);
            }
            let fuel = self
                .lineage
                .journal
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
            Ok(selected) => Ok(LiveLaterContinueAppendV8 {
                owner: self,
                selected,
            }),
            Err(error) => Err(LiveLaterContinueFailureV8::State { owner: self, error }),
        }
    }
}
impl<'j> LiveLaterContinueAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.owner.lineage.journal, journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner
            .lineage
            .current()
            .expect("live lineage")
            .sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner
            .lineage
            .current()
            .expect("live lineage")
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
                        .journal
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
                        .journal
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
        Ok(FixedOwnedContinueAppendPermitV8 {
            owner: ContinuePermitOwnerV8::Later(self),
        })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        self.owner.lineage.hold()?.validate_continue_append_prefix(
            journal,
            inventory,
            &self.selected,
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinueSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .lineage
            .hold()?
            .advance_continue_ack(witness, session)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinueSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            witness.validate_predecessor(
                self.owner.lineage.journal,
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_current_session(session)?;
            self.owner.lineage.hold()?.validate_continue_guard(
                self.owner.lineage.journal,
                session.sequence(),
                session.acknowledged_bytes(),
            )?;
            let held = self.owner.lineage.journal.hold()?;
            let (runtime, execution) = self
                .owner
                .lineage
                .journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = plan_owned_effect_v8(
                runtime,
                execution,
                &held.registration().expected_facts().scope,
                &self.owner.lineage.proposal,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            if !self
                .owner
                .lineage
                .policy
                .allows(plan.operation().effect_id())
            {
                return Err(SourceJournalError::Binding);
            }
            let ordinary = self.owner.lineage.journal.context().ordinary();
            check_clock_v8(
                &held,
                session.sequence(),
                session.acknowledged_bytes(),
                self.owner.lineage.cancellation,
                self.owner.lineage.source.continued_model_origin()?.1,
                ordinary.clock_domain(),
                ordinary.initial_millis(),
                ordinary.deadline_millis(),
            )?;
            self.owner.held.live_target_v8()?;
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| self.owner.lineage.journal.quarantine())
    }
}

struct LaterObservePermitV8<'p, 'j> {
    lineage: &'p LaterContinueLineageV8<'j>,
}
impl LiveContinueObserveGuardV8 for LaterObservePermitV8<'_, '_> {
    fn validate_guard(&self) -> Result<(), SourceJournalError> {
        self.lineage.guard()
    }
    fn turn(&self) -> u32 {
        self.lineage.turn
    }
    fn fuel(&self) -> Result<usize, SourceJournalError> {
        self.lineage
            .journal
            .context()
            .ordinary()
            .max_steps_per_stage()
            .ok_or(SourceJournalError::Binding)
    }
    fn causal_refs(&self) -> Result<(u32, u32), SourceJournalError> {
        if self.lineage.acks.len() != 2 {
            return Err(SourceJournalError::Order);
        }
        let transition = self.lineage.source.continue_transition_seq()?;
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
    fn matches_inputs(&self, inputs: &OwnedEffectInputsV8<'_>) -> Result<(), SourceJournalError> {
        let (runtime, execution) = self
            .lineage
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(execution, inputs.execution)
            || !std::ptr::eq(self.lineage.policy, inputs.policy)
            || !std::ptr::eq(self.lineage.cancellation, inputs.cancellation)
            || inputs.turn.checked_add(1) != Some(self.lineage.turn)
            || inputs.attempt != 0
            || inputs.proposal.carrier() != self.lineage.proposal.carrier()
            || inputs.proposal.ordinary_digest() != self.lineage.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        self.lineage.guard()
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_later_continue_v8<
    'j,
>(
    obligation: LiveLaterContinueAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinueSuccessorV8<'j>,
) -> Result<LiveLaterContinueAcknowledgedV8<'j>, LiveLaterContinueFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveLaterContinueFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    let LiveLaterContinueAppendV8 { mut owner, .. } = obligation;
    owner.lineage.acks.push(ContinueAckV8 { session, witness });
    if owner.lineage.acks.len() == 1 {
        return Ok(LiveLaterContinueAcknowledgedV8::State(owner));
    }
    let LiveLaterContinueStateV8 {
        held,
        accounting,
        lineage,
    } = owner;
    let result = observe_live_continued_state_v8(held, &LaterObservePermitV8 { lineage: &lineage });
    match result {
        Err(owner) => Err(LiveLaterContinueFailureV8::Observe {
            owner,
            accounting,
            lineage,
        }),
        Ok((outcome, consumed)) => {
            let actual = LiveLaterObservedContinueV8 {
                outcome,
                accounting,
                lineage,
                consumed,
            };
            if let Err(error) = actual.lineage.guard() {
                return Err(LiveLaterContinueFailureV8::After {
                    owner: actual,
                    error,
                });
            }
            Ok(LiveLaterContinueAcknowledgedV8::Observed(actual))
        }
    }
}
