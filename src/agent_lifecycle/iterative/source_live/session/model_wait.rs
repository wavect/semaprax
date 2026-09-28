//! Pure interpreter wait evaluations share the outer source checkpoint owner.
use super::*;
use crate::agent_lifecycle::iterative::model_wait::{carrier_digest, SourceModelWaitBinding};
use crate::interpreter::resumable::{
    resume_sequential_channel_resumable_effect_with_arguments,
    run_sequential_channel_resumable_effect_with_arguments, ResumableChannelValue,
    SequentialChannelArgumentsStep,
};
use crate::resumable_effects::source_checkpoint::{
    decode_source_checkpoint_v7, encode_source_checkpoint_v7, SourceCheckpointKey,
    SourceCheckpointScope,
};

pub(super) struct ModelWaitContext<'a> {
    pub lifecycle: &'a CompiledIterativeLifecycle,
    pub binding: SourceModelWaitBinding,
    pub key: &'a SourceCheckpointKey,
    active: Option<(u32, u32, Vec<ResumableChannelValue>, Vec<u8>)>,
}

impl<'a> SourceExecutionSession<'a> {
    pub(super) fn with_model_wait(
        mut self,
        lifecycle: &'a CompiledIterativeLifecycle,
        binding: SourceModelWaitBinding,
        key: &'a SourceCheckpointKey,
    ) -> Self {
        self.model_wait = Some(ModelWaitContext {
            lifecycle,
            binding,
            key,
            active: None,
        });
        self
    }

    fn append_wait(&mut self, entry: SourceModelWaitEntryV7) -> Result<(), Vec<Diagnostic>> {
        self.guard()?;
        let now = self.clock.now_millis();
        self.sink
            .preflight_wait_at(&entry, now)
            .map_err(|e| self.journal_failure(e))?;
        self.sink
            .append_wait_at(entry, now)
            .map_err(|e| self.journal_failure(e))
    }

    fn reserve_wait(
        &mut self,
        turn: u32,
        attempt: u32,
        wait: &str,
        phase: SourceModelWaitPhaseV7,
        original: Option<u32>,
        fuel: usize,
    ) -> Result<u32, Vec<Diagnostic>> {
        let seq = self.sink.journal().execution_entries_v7().len() as u32;
        self.append_wait(SourceModelWaitEntryV7::EvaluationReserved {
            turn,
            attempt,
            wait: wait.into(),
            phase,
            replay_of: original,
            fuel,
        })?;
        Ok(seq)
    }

    fn wait_result_failure(&mut self, step: &SequentialChannelArgumentsStep) -> Vec<Diagnostic> {
        self.refuse(
            if matches!(step, SequentialChannelArgumentsStep::FuelExhausted) {
                SourceTerminalStatus::BudgetExhausted
            } else {
                SourceTerminalStatus::Rejected
            },
            "source.model_wait_evaluation",
        )
    }

    pub(super) fn prepare_model_wait(
        &mut self,
        turn: u32,
        attempt: u32,
        observation: &RetainedValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(context) = self.model_wait.as_ref() else {
            return Ok(());
        };
        let lifecycle = context.lifecycle;
        let binding = context.binding.clone();
        let key = context.key;
        let carrier = binding
            .observation_carrier(lifecycle, observation)
            .ok_or_else(|| {
                self.refuse(
                    SourceTerminalStatus::Rejected,
                    "source.model_wait_observation",
                )
            })?;
        let arguments = vec![carrier.clone()];
        let wait = self
            .sink
            .journal()
            .binding()
            .model_wait_id(turn, attempt)
            .map_err(|e| self.journal_failure(e))?;
        let state = self
            .sink
            .journal()
            .wait_state(turn, attempt)
            .map_err(|e| self.journal_failure(e))?;
        let observation_digest =
            carrier_digest(&wait, "observation", &carrier).ok_or_else(|| {
                self.refuse(
                    SourceTerminalStatus::Rejected,
                    "source.model_wait_observation",
                )
            })?;
        let scope = SourceCheckpointScope::new(binding.source_revision(), &wait, 0)
            .map_err(|_| self.journal_failure(SourceJournalError::Binding))?;
        if let Some((
            _,
            SourceModelWaitEntryV7::Prepared {
                observation_digest: prior,
                checkpoint,
                ..
            },
        )) = state.as_ref().and_then(|s| s.prepared.as_ref())
        {
            if prior != &observation_digest {
                return Err(self.journal_failure(SourceJournalError::Binding));
            }
            decode_source_checkpoint_v7(
                &lifecycle.inner.program,
                key,
                &scope,
                binding.wrapper_id(),
                &arguments,
                checkpoint,
            )
            .map_err(|_| self.journal_failure(SourceJournalError::Binding))?;
        }
        let original = state.as_ref().and_then(|s| s.start_reservation);
        let reservation = self.reserve_wait(
            turn,
            attempt,
            &wait,
            SourceModelWaitPhaseV7::Start,
            original,
            binding.evaluation_fuel(),
        )?;
        self.guard()?;
        let evaluation = run_sequential_channel_resumable_effect_with_arguments(
            &lifecycle.inner.program,
            binding.wrapper_id(),
            &arguments,
            binding.evaluation_fuel(),
        )
        .map_err(|_| {
            self.refuse(
                SourceTerminalStatus::Rejected,
                "source.model_wait_evaluation",
            )
        })?;
        let SequentialChannelArgumentsStep::Suspended { continuation } = evaluation.step else {
            return Err(self.wait_result_failure(&evaluation.step));
        };
        if continuation.request() != &carrier {
            return Err(self.journal_failure(SourceJournalError::Binding));
        }
        let checkpoint = encode_source_checkpoint_v7(
            &lifecycle.inner.program,
            key,
            &scope,
            binding.wrapper_id(),
            &arguments,
            &continuation,
        )
        .map_err(|_| self.journal_failure(SourceJournalError::Binding))?;
        let checkpoint_digest = source_model_wait_checkpoint_digest(&checkpoint);
        let closed_seq = if let Some((
            seq,
            SourceModelWaitEntryV7::Prepared {
                checkpoint: prior,
                checkpoint_digest: prior_digest,
                ..
            },
        )) = state.as_ref().and_then(|s| s.prepared.as_ref())
        {
            if prior != &checkpoint || prior_digest != &checkpoint_digest {
                return Err(self.journal_failure(SourceJournalError::Binding));
            }
            *seq
        } else {
            let seq = self.sink.journal().execution_entries_v7().len() as u32;
            self.append_wait(SourceModelWaitEntryV7::Prepared {
                turn,
                attempt,
                wait: wait.clone(),
                reservation: original.unwrap_or(reservation),
                observation_digest,
                checkpoint_digest: checkpoint_digest.clone(),
                checkpoint: checkpoint.clone(),
            })?;
            seq
        };
        if original.is_some() {
            self.append_wait(SourceModelWaitEntryV7::ReplayChecked {
                turn,
                attempt,
                wait,
                reservation,
                original: closed_seq,
                result_digest: checkpoint_digest,
            })?;
        }
        self.model_wait.as_mut().unwrap().active = Some((turn, attempt, arguments, checkpoint));
        Ok(())
    }

    pub(crate) fn model_wait_proposal(
        &mut self,
        turn: usize,
        attempt: usize,
        decoded: &DecodedProposal,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(context) = self.model_wait.as_ref() else {
            return Ok(());
        };
        let lifecycle = context.lifecycle;
        let binding = context.binding.clone();
        let key = context.key;
        let (t, a, arguments, checkpoint) = context
            .active
            .clone()
            .ok_or_else(|| self.journal_failure(SourceJournalError::Order))?;
        if t != turn as u32 || a != attempt as u32 {
            return Err(self.journal_failure(SourceJournalError::Order));
        }
        let carrier = lifecycle
            .proposal_schema()
            .model_wait_carrier(decoded)
            .ok_or_else(|| {
                self.refuse(SourceTerminalStatus::Rejected, "source.model_wait_proposal")
            })?;
        let wait = self
            .sink
            .journal()
            .binding()
            .model_wait_id(t, a)
            .map_err(|e| self.journal_failure(e))?;
        let proposal_digest = carrier_digest(&wait, "proposal", &carrier)
            .ok_or_else(|| self.journal_failure(SourceJournalError::Binding))?;
        let state = self
            .sink
            .journal()
            .wait_state(t, a)
            .map_err(|e| self.journal_failure(e))?;
        let scope = SourceCheckpointScope::new(binding.source_revision(), &wait, 0)
            .map_err(|_| self.journal_failure(SourceJournalError::Binding))?;
        let continuation = decode_source_checkpoint_v7(
            &lifecycle.inner.program,
            key,
            &scope,
            binding.wrapper_id(),
            &arguments,
            &checkpoint,
        )
        .map_err(|_| self.journal_failure(SourceJournalError::Binding))?;
        let original = state.as_ref().and_then(|s| s.resume_reservation);
        let reservation = self.reserve_wait(
            t,
            a,
            &wait,
            SourceModelWaitPhaseV7::Resume,
            original,
            binding.evaluation_fuel(),
        )?;
        self.guard()?;
        let evaluation = resume_sequential_channel_resumable_effect_with_arguments(
            &lifecycle.inner.program,
            binding.wrapper_id(),
            &arguments,
            &continuation,
            &carrier,
            binding.evaluation_fuel(),
        )
        .map_err(|_| {
            self.refuse(
                SourceTerminalStatus::Rejected,
                "source.model_wait_evaluation",
            )
        })?;
        let SequentialChannelArgumentsStep::Completed { result, .. } = evaluation.step else {
            return Err(self.wait_result_failure(&evaluation.step));
        };
        if result != carrier {
            return Err(self.journal_failure(SourceJournalError::Binding));
        }
        let closed_seq = if let Some((
            seq,
            SourceModelWaitEntryV7::Completed {
                proposal_digest: prior,
                ..
            },
        )) = state.as_ref().and_then(|s| s.completed.as_ref())
        {
            if prior != &proposal_digest {
                return Err(self.journal_failure(SourceJournalError::Binding));
            }
            *seq
        } else {
            let seq = self.sink.journal().execution_entries_v7().len() as u32;
            self.append_wait(SourceModelWaitEntryV7::Completed {
                turn: t,
                attempt: a,
                wait: wait.clone(),
                reservation: original.unwrap_or(reservation),
                proposal_digest: proposal_digest.clone(),
            })?;
            seq
        };
        if original.is_some() {
            self.append_wait(SourceModelWaitEntryV7::ReplayChecked {
                turn: t,
                attempt: a,
                wait,
                reservation,
                original: closed_seq,
                result_digest: proposal_digest,
            })?;
        }
        Ok(())
    }
}
