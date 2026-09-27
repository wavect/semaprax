//! The two compiler-owned execution lanes the durable driver runs: the
//! direct sequential plan (v2 envelope) and the control-dependent plan (v3
//! envelope). The lane is fixed by the checked signature, never by stored
//! bytes.

use super::DurableFailure;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::interpreter::resumable::channel::{
    resume_sequential_channel_resumable_effect, run_sequential_channel_resumable_effect,
    SequentialChannelResumableStep,
};
use crate::interpreter::resumable::control::{
    resume_control_resumable_effect, run_control_resumable_effect, ControlContinuation,
    ControlResumableStep,
};
use crate::interpreter::resumable::{
    resume_sequential_resumable_effect, run_sequential_resumable_effect,
    ResumableChannelContinuation, ResumableChannelValue, ResumableContinuation,
    SequentialResumableStep,
};
use crate::interpreter::ArgumentValue;
use crate::resumable_effects::source_checkpoint::{
    decode_source_checkpoint_v2, decode_source_checkpoint_v3, decode_source_checkpoint_v4,
    decode_source_checkpoint_v5, encode_source_checkpoint_v2, encode_source_checkpoint_v3,
    encode_source_checkpoint_v4, encode_source_checkpoint_v5, SourceCheckpointError,
    SourceCheckpointKey, SourceCheckpointScope,
};

#[derive(Clone, Debug)]
pub(super) enum Carrier {
    Sequential(ResumableContinuation),
    Control(ControlContinuation),
    /// Issue #296 R20: the bounded record/variant channel, sequential
    /// placement only (never paired with `Control`: an aggregate channel is
    /// admitted only for the direct top-level placement).
    SequentialChannel(ResumableChannelContinuation),
}

impl Carrier {
    /// Owned rather than borrowed (unlike the scalar-only lanes'
    /// `.request()`): a `Sequential`/`Control` carrier's own request is a
    /// bare `ArgumentValue`, so producing the shared, wider
    /// `ResumableChannelValue` this driver's request/answer channel now
    /// uses means wrapping it, which cannot return a borrow of the original.
    pub(super) fn request(&self) -> ResumableChannelValue {
        match self {
            Self::Sequential(continuation) => {
                ResumableChannelValue::Scalar(continuation.request().clone())
            }
            Self::Control(continuation) => {
                ResumableChannelValue::Scalar(continuation.request().clone())
            }
            Self::SequentialChannel(continuation) => continuation.request().clone(),
        }
    }

    /// The dynamic suspension ordinal: how many sites settled before it.
    pub(super) fn site(&self) -> u32 {
        let settled = match self {
            Self::Sequential(continuation) => continuation.history().len(),
            Self::Control(continuation) => continuation.history().len(),
            Self::SequentialChannel(continuation) => continuation.history().len(),
        };
        u32::try_from(settled).expect("bounded suspension history")
    }

    /// The exact bytes of every owned value this carrier carries at its
    /// current site, in cleanup-inventory order. Empty for a sequential
    /// (scalar or channel) carrier or a non-carrying control plan.
    pub(super) fn carried_bytes(&self) -> Vec<Vec<u8>> {
        match self {
            Self::Sequential(_) | Self::SequentialChannel(_) => Vec::new(),
            Self::Control(continuation) => continuation
                .carried()
                .iter()
                .map(|(_, bytes)| bytes.clone())
                .collect(),
        }
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

fn channel(step: SequentialChannelResumableStep) -> LaneStep {
    match step {
        SequentialChannelResumableStep::Suspended { continuation } => {
            LaneStep::Suspended(Carrier::SequentialChannel(continuation))
        }
        SequentialChannelResumableStep::Completed { result, .. } => LaneStep::Completed(result),
        SequentialChannelResumableStep::LanguageFailure(_) => {
            LaneStep::Failed(DurableFailure::LanguageFailure)
        }
        SequentialChannelResumableStep::FuelExhausted => {
            LaneStep::Failed(DurableFailure::FuelExhausted)
        }
        SequentialChannelResumableStep::CallDepthExceeded => {
            LaneStep::Failed(DurableFailure::CallDepthExceeded)
        }
        SequentialChannelResumableStep::GuardError(_) => {
            LaneStep::Failed(DurableFailure::EvaluationRejected)
        }
    }
}

/// The `ArgumentValue` a channel value denotes when it is only ever supposed
/// to be `Scalar` -- the `Sequential`/`Control` lanes' own answer, which the
/// durable driver always threads through as a [`ResumableChannelValue`] now
/// but which those two lanes' own interpreter entry points still take as a
/// bare scalar. `None` for a genuinely aggregate value, which those two
/// lanes can never actually produce or accept.
fn require_scalar_answer(answer: &ResumableChannelValue) -> Option<ArgumentValue> {
    match answer {
        ResumableChannelValue::Scalar(value) => Some(value.clone()),
        ResumableChannelValue::Record { .. } | ResumableChannelValue::Variant { .. } => None,
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

/// `aggregate_channel` and `control_dependent` are never both true: an
/// aggregate channel is admitted only for the direct top-level (sequential)
/// placement (`hir::resolve_yield`, `resumable_effects::lowering::control::
/// check_resumable_profile`'s `allow_aggregate` gate), so the checked
/// signature this driver derives before ever calling `start` can never set
/// both.
pub(super) fn start(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    max_steps: usize,
    control_dependent: bool,
    aggregate_channel: bool,
) -> Result<LaneStep, Vec<Diagnostic>> {
    if control_dependent {
        run_control_resumable_effect(program, function_id, arguments, max_steps)
            .map(|evaluation| control(evaluation.step))
    } else if aggregate_channel {
        run_sequential_channel_resumable_effect(program, function_id, arguments, max_steps)
            .map(|evaluation| channel(evaluation.step))
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
    answer: &ResumableChannelValue,
    max_steps: usize,
) -> Result<LaneStep, Vec<Diagnostic>> {
    match carrier {
        Carrier::Sequential(continuation) => {
            let answer = require_scalar_answer(answer).ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-F113",
                    "sequential lane answer is not an admitted scalar",
                )]
            })?;
            resume_sequential_resumable_effect(
                program,
                function_id,
                arguments,
                continuation,
                &answer,
                max_steps,
            )
            .map(|evaluation| sequential(evaluation.step))
        }
        Carrier::Control(continuation) => {
            let answer = require_scalar_answer(answer).ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-F113",
                    "control lane answer is not an admitted scalar",
                )]
            })?;
            resume_control_resumable_effect(
                program,
                function_id,
                arguments,
                continuation,
                &answer,
                max_steps,
            )
            .map(|evaluation| control(evaluation.step))
        }
        Carrier::SequentialChannel(continuation) => resume_sequential_channel_resumable_effect(
            program,
            function_id,
            arguments,
            continuation,
            answer,
            max_steps,
        )
        .map(|evaluation| channel(evaluation.step)),
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
        Carrier::Control(continuation) if continuation.carried().is_empty() => {
            encode_source_checkpoint_v3(program, key, scope, function_id, arguments, continuation)
        }
        Carrier::Control(continuation) => {
            encode_source_checkpoint_v4(program, key, scope, function_id, arguments, continuation)
        }
        Carrier::SequentialChannel(continuation) => {
            encode_source_checkpoint_v5(program, key, scope, function_id, arguments, continuation)
        }
    }
}

/// The lane, and within it the exact envelope schema, is fixed by the
/// checked signature and never by stored bytes: `control_dependent`,
/// `aggregate_channel`, and `carries_owned_bytes` are all derived from the
/// signature before this ever inspects `bytes`.
#[allow(clippy::too_many_arguments)]
pub(super) fn decode(
    program: &ResolvedProgram,
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    function_id: &str,
    arguments: &[ArgumentValue],
    bytes: &[u8],
    control_dependent: bool,
    carries_owned_bytes: bool,
    aggregate_channel: bool,
) -> Result<Carrier, SourceCheckpointError> {
    match (control_dependent, carries_owned_bytes, aggregate_channel) {
        (true, true, _) => {
            decode_source_checkpoint_v4(program, key, scope, function_id, arguments, bytes)
                .map(Carrier::Control)
        }
        (true, false, _) => {
            decode_source_checkpoint_v3(program, key, scope, function_id, arguments, bytes)
                .map(Carrier::Control)
        }
        (false, _, true) => {
            decode_source_checkpoint_v5(program, key, scope, function_id, arguments, bytes)
                .map(Carrier::SequentialChannel)
        }
        (false, _, false) => {
            decode_source_checkpoint_v2(program, key, scope, function_id, arguments, bytes)
                .map(Carrier::Sequential)
        }
    }
}
