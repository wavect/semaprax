//! One-use turn-one Step transfer from the actual cleanup receipt owner.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::{
    HeldExecutedOwnedStepV2, LiveOwnedStepTransferFailureV8, LiveOwnedStepTransferGuardV8,
    ReadyExecutedOwnedStepV2,
};
use crate::resumable_effects::owned_frame::v2::compile_owned_reduce_v2;

type StepAppend<'j> = crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveOwnedStepAppendV8<'j>;

fn true_seq(sequence: usize) -> Result<u32, SourceJournalError> {
    u32::try_from(sequence.checked_sub(1).ok_or(SourceJournalError::Order)?)
        .map_err(|_| SourceJournalError::Capacity)
}

impl<'j> LiveContinuedStagedStepV8<'j> {
    fn coordinates(&self) -> Result<(u32, u32), SourceJournalError> {
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
        Ok((*turn, *attempt))
    }
    fn checked_step(&self) -> Result<crate::live_invocation::source_journal::owned_wait_v8::reduce_inventory::CheckedReduceStepV8, SourceJournalError>{
        let (_, execution) = self
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan =
            compile_owned_reduce_v2(execution.wait()).map_err(|_| SourceJournalError::Binding)?;
        let staged = self.ack.as_ref().ok_or(SourceJournalError::Binding)?;
        let EntryV8::Owned(OwnedBodyV8::OwnedReduceStaged {
            step, step_digest, ..
        }) = staged.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        let held = self.journal().hold()?;
        let scope = &held.registration().expected_facts().scope;
        let scope = serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()});
        let (turn, attempt) = self.coordinates()?;
        crate::live_invocation::source_journal::owned_wait_v8::reduce_inventory::checked_step(
            &plan,
            &scope,
            turn,
            attempt,
            true_seq(self.reserved.session.sequence())?,
            step,
            step_digest,
        )
    }
    /// The receipt is matched to the actual released owner before it yields
    /// the one-use ReadyStep. No row or failed settlement remints a result.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn into_ready(
        mut self,
    ) -> Result<Self, (Self, SourceJournalError)> {
        let checked = (|| {
            self.validate_live()?;
            let (sequence, bytes) = self.cursor();
            self.reserved
                .owner
                .owner
                .validate_spent_reduce_context(sequence, bytes, true, false)?;
            let (_, witness) = self
                .receipt_ack
                .as_ref()
                .ok_or(SourceJournalError::Binding)?;
            let EntryV8::Owned(OwnedBodyV8::OwnedReduceCleanupSettled { receipt, .. }) =
                witness.selected_row()
            else {
                return Err(SourceJournalError::Binding);
            };
            if receipt != &self.receipt()? || receipt["settlement"] != "completed" {
                return Err(SourceJournalError::Binding);
            }
            self.checked_step()?;
            Ok(receipt.clone())
        })();
        let receipt = match checked {
            Ok(receipt) => receipt,
            Err(error) => {
                self.journal().quarantine();
                return Err((self, error));
            }
        };
        let Some(released) = self.released.take() else {
            self.journal().quarantine();
            return Err((self, SourceJournalError::Binding));
        };
        match released {
            crate::interpreter::resumable::owned_frame::registered_stage::reduce::ExecutedOwnedReduceSettledV2::Ready(ready) => {
                self.ready = Some(ready);
                self.receipt = Some(receipt);
                if let Err(error) = self.validate_live() {
                    return Err((self, error));
                }
                Ok(self)
            }
            failed => {
                self.released = Some(failed);
                self.journal().quarantine();
                Err((self, SourceJournalError::Binding))
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_transfer(
        self,
    ) -> Result<StepAppend<'j>, (Self, SourceJournalError)> {
        let selected = (|| {
            self.validate_live()?;
            let (sequence, bytes) = self.cursor();
            self.reserved
                .owner
                .owner
                .validate_spent_reduce_context(sequence, bytes, true, false)?;
            if self.ready.is_none() || self.transfer_ack.is_some() {
                return Err(SourceJournalError::Order);
            }
            let step = self.checked_step()?;
            let (turn, attempt) = self.coordinates()?;
            let (_, execution) = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let OwnedReduceCleanupOriginV8::Observed { started } = self.cleanup_origin()? else {
                return Err(SourceJournalError::Binding);
            };
            let settled = true_seq(
                self.receipt_ack
                    .as_ref()
                    .ok_or(SourceJournalError::Binding)?
                    .0
                    .sequence(),
            )?;
            Ok(EntryV8::Owned(OwnedBodyV8::OwnedStepTransferReserved {
                turn, attempt,
                plan: execution.wait().binding().into(),
                stage_reservation: true_seq(self.reserved.session.sequence())?,
                staged: true_seq(self.session.as_ref().ok_or(SourceJournalError::Binding)?.sequence())?,
                cleanup: crate::live_invocation::source_journal::owned_wait_v8::reduce_model::ReduceCleanupV8::Observed { started, settled },
                case: step.case().into(),
            }))
        })();
        match selected {
            Ok(row) => Ok(StepAppend::continued(self, row)),
            Err(error) => {
                self.journal().quarantine();
                Err((self, error))
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn move_fields(
        mut self,
    ) -> Result<Self, LiveContinuedStepMoveFailureV8<'j>> {
        if let Err(error) = self.validate_live() {
            return Err(LiveContinuedStepMoveFailureV8::Before { owner: self, error });
        }
        let Some(ready) = self.ready.take() else {
            self.journal().quarantine();
            return Err(LiveContinuedStepMoveFailureV8::Before {
                owner: self,
                error: SourceJournalError::Order,
            });
        };
        let result = {
            let permit = ContinuedStepTransferPermitV8 { owner: &self };
            crate::interpreter::resumable::owned_frame::registered_stage::reduce::consume_live_owned_step_v8(ready, &permit)
        };
        match result {
            Ok(held) => {
                self.held = Some(held);
                if let Err(error) = self.validate_live() {
                    return Err(LiveContinuedStepMoveFailureV8::After { owner: self, error });
                }
                Ok(self)
            }
            Err(owner) => {
                self.journal().quarantine();
                Err(LiveContinuedStepMoveFailureV8::Engine {
                    reached: self,
                    owner,
                })
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_completed(
        self,
    ) -> Result<StepAppend<'j>, (Self, SourceJournalError)> {
        let selected = (|| {
            self.validate_live()?;
            let step = self.checked_step()?;
            let held = self.held.as_ref().ok_or(SourceJournalError::Binding)?;
            let target = held.live_target_v8()?;
            step.matches_target(&target)?;
            let (session, witness) = self
                .transfer_ack
                .as_ref()
                .ok_or(SourceJournalError::Binding)?;
            if !matches!(
                witness.selected_row(),
                EntryV8::Owned(OwnedBodyV8::OwnedStepTransferReserved { .. })
            ) {
                return Err(SourceJournalError::Binding);
            }
            let reserved = true_seq(session.sequence())?;
            if held.causal_refs() != (self.facts.effect_settled(), reserved) {
                return Err(SourceJournalError::Binding);
            }
            let (_, execution) = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = compile_owned_reduce_v2(execution.wait())
                .map_err(|_| SourceJournalError::Binding)?;
            let registration = self.journal().hold()?;
            let s = &registration.registration().expected_facts().scope;
            let scope = serde_json::json!({"program_root":s.program_root(),"invocation":s.invocation_id(),"policy_epoch":s.policy_epoch()});
            let (turn, attempt) = self.coordinates()?;
            let transfer_digest = step.transfer_digest(&scope, &plan, turn, attempt, reserved)?;
            Ok(EntryV8::Owned(OwnedBodyV8::OwnedStepTransferCompleted {
                turn,
                attempt,
                reserved,
                target: serde_json::from_value(target).map_err(|_| SourceJournalError::Binding)?,
                transfer_digest,
            }))
        })();
        match selected {
            Ok(row) => Ok(StepAppend::continued(self, row)),
            Err(error) => {
                self.journal().quarantine();
                Err((self, error))
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_transition(
        self,
    ) -> Result<StepAppend<'j>, (Self, SourceJournalError)> {
        let selected = (|| {
            self.validate_live()?;
            let step = self.checked_step()?;
            let held = self.held.as_ref().ok_or(SourceJournalError::Binding)?;
            let target = held.live_target_v8()?;
            step.matches_target(&target)?;
            let (_, witness) = self
                .completed_ack
                .as_ref()
                .ok_or(SourceJournalError::Binding)?;
            let EntryV8::Owned(OwnedBodyV8::OwnedStepTransferCompleted {
                target: recorded, ..
            }) = witness.selected_row()
            else {
                return Err(SourceJournalError::Binding);
            };
            if serde_json::to_value(recorded).map_err(|_| SourceJournalError::Binding)? != target {
                return Err(SourceJournalError::Binding);
            }
            let (_, execution) = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = compile_owned_reduce_v2(execution.wait())
                .map_err(|_| SourceJournalError::Binding)?;
            let bytes = step.ordinary_carrier_bytes(&plan)?;
            let case = match held.kind() {
                "continue" => {
                    crate::live_invocation::source_journal::SourceTransitionCase::Continue
                }
                "suspend" => crate::live_invocation::source_journal::SourceTransitionCase::Suspend,
                "complete" => {
                    crate::live_invocation::source_journal::SourceTransitionCase::Complete
                }
                "fail" => crate::live_invocation::source_journal::SourceTransitionCase::Fail,
                _ => return Err(SourceJournalError::Binding),
            };
            let (turn, attempt) = self.coordinates()?;
            Ok(EntryV8::Ordinary(SourceJournalEntry::Transition {
                turn,
                attempt,
                case,
                carrier_digest: crate::live_invocation::identity::digest(
                    b"semaprax.agent-step.value.v2\0",
                    &bytes,
                ),
            }))
        })();
        match selected {
            Ok(row) => Ok(StepAppend::continued(self, row)),
            Err(error) => {
                self.journal().quarantine();
                Err((self, error))
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn kind(
        &self,
    ) -> Option<&'static str> {
        self.held.as_ref().map(HeldExecutedOwnedStepV2::kind)
    }
    /// Build the canonical terminal row from the authenticated prefix and the
    /// live mapped Step. Evidence input is descriptive; it grants no append.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_terminal(
        self,
        input: crate::live_invocation::source_journal::SourceTerminalEvidenceInput,
    ) -> Result<StepAppend<'j>, (Self, SourceJournalError)> {
        let selected = (|| {
            self.validate_live()?;
            if self.terminal_ack.is_some() {
                return Err(SourceJournalError::Order);
            }
            let (_, transition) = self
                .transition_ack
                .as_ref()
                .ok_or(SourceJournalError::Order)?;
            let EntryV8::Ordinary(SourceJournalEntry::Transition {
                turn,
                attempt,
                case,
                carrier_digest,
            }) = transition.selected_row()
            else {
                return Err(SourceJournalError::Binding);
            };
            let (actual_turn, actual_attempt) = self.coordinates()?;
            if (*turn, *attempt) != (actual_turn, actual_attempt) {
                return Err(SourceJournalError::Binding);
            }
            let status = match case {
                crate::live_invocation::source_journal::SourceTransitionCase::Complete => {
                    crate::live_invocation::source_journal::SourceTerminalStatus::Complete
                }
                crate::live_invocation::source_journal::SourceTransitionCase::Suspend => {
                    crate::live_invocation::source_journal::SourceTerminalStatus::Suspend
                }
                crate::live_invocation::source_journal::SourceTransitionCase::Fail => {
                    crate::live_invocation::source_journal::SourceTerminalStatus::Fail
                }
                crate::live_invocation::source_journal::SourceTransitionCase::Continue => {
                    return Err(SourceJournalError::Order)
                }
            };
            let step = self.checked_step()?;
            let held = self.held.as_ref().ok_or(SourceJournalError::Binding)?;
            step.matches_target(&held.live_target_v8()?)?;
            let (_, execution) = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = compile_owned_reduce_v2(execution.wait())
                .map_err(|_| SourceJournalError::Binding)?;
            let carrier = step.ordinary_carrier_bytes(&plan)?;
            if crate::live_invocation::identity::digest(b"semaprax.agent-step.value.v2\0", &carrier)
                != *carrier_digest
            {
                return Err(SourceJournalError::Binding);
            }
            self.journal()
                .begin_session()?
                .terminal_entry(actual_turn, status, carrier, input)
        })();
        match selected {
            Ok(row) => Ok(StepAppend::continued(self, row)),
            Err(error) => {
                self.journal().quarantine();
                Err((self, error))
            }
        }
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedStepMoveFailureV8<
    'j,
> {
    Before {
        owner: LiveContinuedStagedStepV8<'j>,
        error: SourceJournalError,
    },
    Engine {
        reached: LiveContinuedStagedStepV8<'j>,
        owner: LiveOwnedStepTransferFailureV8<'j>,
    },
    After {
        owner: LiveContinuedStagedStepV8<'j>,
        error: SourceJournalError,
    },
}

struct ContinuedStepTransferPermitV8<'a, 'j> {
    owner: &'a LiveContinuedStagedStepV8<'j>,
}
impl LiveOwnedStepTransferGuardV8 for ContinuedStepTransferPermitV8<'_, '_> {
    fn validate_transfer_current(&self) -> Result<(), SourceJournalError> {
        self.owner.validate_live()?;
        let (_, witness) = self
            .owner
            .transfer_ack
            .as_ref()
            .ok_or(SourceJournalError::Binding)?;
        if !matches!(
            witness.selected_row(),
            EntryV8::Owned(OwnedBodyV8::OwnedStepTransferReserved { .. })
        ) {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    fn transfer_reserved(&self) -> Result<u32, SourceJournalError> {
        self.validate_transfer_current()?;
        true_seq(
            self.owner
                .transfer_ack
                .as_ref()
                .ok_or(SourceJournalError::Binding)?
                .0
                .sequence(),
        )
    }
    fn validate_ready(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
        receipt: &serde_json::Value,
        origin: OwnedReduceCleanupOriginV8,
    ) -> Result<(), SourceJournalError> {
        self.validate_transfer_current()?;
        let (runtime, execution) = self
            .owner
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let (turn, attempt) = self.owner.coordinates()?;
        let (sequence, bytes) = self.owner.cursor();
        if !std::ptr::eq(inputs.runtime, runtime)
            || !std::ptr::eq(inputs.execution, execution)
            || (inputs.turn, inputs.attempt) != (turn, attempt)
            || origin != self.owner.cleanup_origin()?
            || receipt != &self.owner.receipt()?
            || receipt["settlement"] != "completed"
        {
            return Err(SourceJournalError::Binding);
        }
        inputs.store.validate_prefix(sequence, bytes)?;
        self.validate_transfer_current()
    }
}
