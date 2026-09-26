//! The two compiler-owned execution lanes the durable driver runs: the
//! direct sequential plan (v2 envelope) and the control-dependent plan (v3
//! envelope). The lane is fixed by the checked signature, never by stored
//! bytes.

use super::DurableFailure;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::interpreter::resumable::control::{
    resume_control_resumable_effect, run_control_resumable_effect, ControlContinuation,
    ControlResumableStep,
};
use crate::interpreter::resumable::{
    resume_sequential_resumable_effect, run_sequential_resumable_effect, ResumableContinuation,
    SequentialResumableStep,
};
use crate::interpreter::ArgumentValue;
use crate::resumable_effects::source_checkpoint::{
    decode_source_checkpoint_v2, decode_source_checkpoint_v3, encode_source_checkpoint_v2,
    encode_source_checkpoint_v3, SourceCheckpointError, SourceCheckpointKey, SourceCheckpointScope,
};

#[derive(Clone, Debug)]
pub(super) enum Carrier {
    Sequential(ResumableContinuation),
    Control(ControlContinuation),
}

impl Carrier {
    pub(super) fn request(&self) -> &ArgumentValue {
        match self {
            Self::Sequential(continuation) => continuation.request(),
            Self::Control(continuation) => continuation.request(),
        }
    }

    /// The dynamic suspension ordinal: how many sites settled before it.
    pub(super) fn site(&self) -> u32 {
        let settled = match self {
            Self::Sequential(continuation) => continuation.history().len(),
            Self::Control(continuation) => continuation.history().len(),
        };
        u32::try_from(settled).expect("bounded suspension history")
    }
}

pub(super) enum LaneStep {
    Suspended(Carrier),
    Completed(ArgumentValue),
    Failed(DurableFailure),
}

fn sequential(step: SequentialResumableStep) -> LaneStep {
    match step {
        SequentialResumableStep::Suspended { continuation } => {
            LaneStep::Suspended(Carrier::Sequential(continuation))
        }
        SequentialResumableStep::Completed { result, .. } => LaneStep::Completed(result),
        SequentialResumableStep::LanguageFailure(_) => {
            LaneStep::Failed(DurableFailure::LanguageFailure)
        }
        SequentialResumableStep::FuelExhausted => LaneStep::Failed(DurableFailure::FuelExhausted),
        SequentialResumableStep::CallDepthExceeded => {
            LaneStep::Failed(DurableFailure::CallDepthExceeded)
        }
        SequentialResumableStep::GuardError(_) => {
            LaneStep::Failed(DurableFailure::EvaluationRejected)
        }
    }
}

fn control(step: ControlResumableStep) -> LaneStep {
    match step {
        ControlResumableStep::Suspended { continuation } => {
            LaneStep::Suspended(Carrier::Control(continuation))
        }
        ControlResumableStep::Completed { result, .. } => LaneStep::Completed(result),
        ControlResumableStep::LanguageFailure(_) => {
            LaneStep::Failed(DurableFailure::LanguageFailure)
        }
        ControlResumableStep::FuelExhausted => LaneStep::Failed(DurableFailure::FuelExhausted),
        ControlResumableStep::CallDepthExceeded => {
            LaneStep::Failed(DurableFailure::CallDepthExceeded)
        }
        ControlResumableStep::SuspensionBoundExceeded => {
            LaneStep::Failed(DurableFailure::SuspensionBoundExceeded)
        }
        ControlResumableStep::GuardError(_) => LaneStep::Failed(DurableFailure::EvaluationRejected),
    }
}

pub(super) fn start(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    max_steps: usize,
    control_dependent: bool,
) -> Result<LaneStep, Vec<Diagnostic>> {
    if control_dependent {
        run_control_resumable_effect(program, function_id, arguments, max_steps)
            .map(|evaluation| control(evaluation.step))
    } else {
        run_sequential_resumable_effect(program, function_id, arguments, max_steps)
            .map(|evaluation| sequential(evaluation.step))
    }
}

pub(super) fn resume(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    carrier: &Carrier,
    answer: &ArgumentValue,
    max_steps: usize,
) -> Result<LaneStep, Vec<Diagnostic>> {
    match carrier {
        Carrier::Sequential(continuation) => resume_sequential_resumable_effect(
            program,
            function_id,
            arguments,
            continuation,
            answer,
            max_steps,
        )
        .map(|evaluation| sequential(evaluation.step)),
        Carrier::Control(continuation) => resume_control_resumable_effect(
            program,
            function_id,
            arguments,
            continuation,
            answer,
            max_steps,
        )
        .map(|evaluation| control(evaluation.step)),
    }
}

pub(super) fn encode(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    carrier: &Carrier,
) -> Result<Vec<u8>, SourceCheckpointError> {
    match carrier {
        Carrier::Sequential(continuation) => {
            encode_source_checkpoint_v2(program, key, scope, function_id, arguments, continuation)
        }
        Carrier::Control(continuation) => {
            encode_source_checkpoint_v3(program, key, scope, function_id, arguments, continuation)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn decode(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    bytes: &[u8],
    control_dependent: bool,
) -> Result<Carrier, SourceCheckpointError> {
    if control_dependent {
        decode_source_checkpoint_v3(program, key, scope, function_id, arguments, bytes)
            .map(Carrier::Control)
    } else {
        decode_source_checkpoint_v2(program, key, scope, function_id, arguments, bytes)
            .map(Carrier::Sequential)
    }
}
