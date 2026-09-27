//! Issue #296 R20: the bounded Copy-scalar record/variant `yields`
//! request/response channel, run to completion on the interpreter.
//!
//! This is the direct top-level (sequential) lane's own public API, exactly
//! parallel to [`super::run_sequential_resumable_effect`]/
//! [`super::resume_sequential_resumable_effect`] and their
//! [`super::ResumableContinuation`], widened from `ArgumentValue` to
//! [`super::ResumableChannelValue`]. It deliberately reuses this crate's
//! existing sequential state machine unchanged --
//! [`super::Resumption`], [`super::settle_yield`] and [`super::run_worker`]
//! already operate on the interpreter's own `Value`, which already
//! represents a record or variant; only the *boundary* conversions this
//! module calls (`super::channel_of`, `super::value_of_channel`,
//! `super::channel_to_resumable_scalar`) are new. No `ArgumentValue`-typed
//! public surface elsewhere in this crate is touched: a scalar-channel
//! caller keeps using the unwidened legacy/sequential API exactly as before.
//!
//! The control-dependent lane (`super::control`) has no counterpart here: an
//! aggregate channel is admitted only for the direct top-level placement
//! (`hir::resolve_yield`, `resumable_effects::lowering::control::
//! check_resumable_profile`'s `allow_aggregate` gate), so this module's
//! plan is always [`crate::resumable_effects::lowering::SequentialResumablePlan`].

use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedProgram};
use crate::resumable_effects::lowering::{
    self, ResumableStateId, ResumableSuspensionBinding, SequentialResumablePlan,
};

use crate::conformance::NormalizedStatus;
use crate::interpreter::{ArgumentValue, Flow, Value};

use super::{
    admit_channel_entry, argument_of, channel_of, channel_to_resumable_scalar, run_worker_values,
    typed_resume_channel_value, ChannelAdmitted, ChannelYieldRecord, ResumableChannelContinuation,
    ResumableChannelValue, Resumption, REQUEST_DRIFT, SUSPENDED_AT_YIELD, SUSPENSION_MISMATCH,
};

/// One settled outcome from a bounded record/variant-channel start or resume.
/// The whole-function channel entry point returns the same checked bounded
/// Copy value it accepts at its parameter boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum SequentialChannelResumableStep {
    Suspended {
        continuation: ResumableChannelContinuation,
    },
    Completed {
        state: ResumableStateId,
        result: ResumableChannelValue,
    },
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    GuardError(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SequentialChannelResumableEvaluation {
    pub step: SequentialChannelResumableStep,
    pub steps_used: usize,
    pub max_steps: usize,
}

/// Run a bounded record/variant-channel function until its first
/// suspension, or to completion.
pub fn run_sequential_channel_resumable_effect(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    max_steps: usize,
) -> Result<SequentialChannelResumableEvaluation, Vec<Diagnostic>> {
    let arguments = arguments
        .iter()
        .cloned()
        .map(ResumableChannelValue::Scalar)
        .collect::<Vec<_>>();
    evaluate_channel_resumable(program, function_id, &arguments, None, max_steps)
}

/// Run the sequential channel lane with its bounded Copy whole-function
/// boundary. This is intentionally a new API: `ArgumentValue` remains the
/// scalar-only boundary for every existing interpreter entry point.
pub fn run_sequential_channel_resumable_effect_with_arguments(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ResumableChannelValue],
    max_steps: usize,
) -> Result<SequentialChannelResumableEvaluation, Vec<Diagnostic>> {
    evaluate_channel_resumable(program, function_id, arguments, None, max_steps)
}

/// Resume an opaque channel continuation with an answer of the declared
/// (record or variant) response type. `request`/the recorded history are not
/// trusted: the replayed prefix recomputes its own requests and every one of
/// them must agree (`SPX-F114`), exactly as the scalar sequential lane
/// already requires.
pub fn resume_sequential_channel_resumable_effect(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    continuation: &ResumableChannelContinuation,
    answer: &ResumableChannelValue,
    max_steps: usize,
) -> Result<SequentialChannelResumableEvaluation, Vec<Diagnostic>> {
    let arguments = arguments
        .iter()
        .cloned()
        .map(ResumableChannelValue::Scalar)
        .collect::<Vec<_>>();
    evaluate_channel_resumable(
        program,
        function_id,
        &arguments,
        Some((continuation.clone(), answer.clone())),
        max_steps,
    )
}

/// Resume an aggregate whole-function channel invocation. The supplied
/// arguments are replay-bound into the continuation binding before the
/// answer is accepted, so a changed aggregate argument fails closed.
pub fn resume_sequential_channel_resumable_effect_with_arguments(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ResumableChannelValue],
    continuation: &ResumableChannelContinuation,
    answer: &ResumableChannelValue,
    max_steps: usize,
) -> Result<SequentialChannelResumableEvaluation, Vec<Diagnostic>> {
    evaluate_channel_resumable(
        program,
        function_id,
        arguments,
        Some((continuation.clone(), answer.clone())),
        max_steps,
    )
}

fn evaluate_channel_resumable(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ResumableChannelValue],
    resume: Option<(ResumableChannelContinuation, ResumableChannelValue)>,
    max_steps: usize,
) -> Result<SequentialChannelResumableEvaluation, Vec<Diagnostic>> {
    let ChannelAdmitted {
        entry,
        yields,
        bound,
        admitted,
        arguments: scalar_arguments,
    } = admit_channel_entry(program, function_id, arguments, max_steps)?;
    let plan = lowering::lower_sequential(program, entry).map_err(|error| vec![error])?;
    let declarations = &program.declarations;

    let (resumption, next_binding, channel_history) = match resume {
        None => (
            Resumption::Fresh {
                parked: None,
                parked_site: None,
                parked_environment: None,
            },
            Some(plan.suspension_binding(&scalar_arguments)),
            Vec::new(),
        ),
        Some((continuation, answer)) => {
            let Some(index) = plan.suspension_index(&continuation.state) else {
                return Err(vec![Diagnostic::io(
                    SUSPENSION_MISMATCH,
                    "channel continuation state does not belong to this exact checked plan",
                )]);
            };
            if continuation.history.len() != index {
                return Err(vec![Diagnostic::io(
                    SUSPENSION_MISMATCH,
                    "channel continuation history length does not match its suspension state",
                )]);
            }
            let prior_scalars = continuation
                .history
                .iter()
                .map(|record| channel_to_resumable_scalar(&record.answer))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    vec![Diagnostic::io(
                        SUSPENSION_MISMATCH,
                        "channel continuation carries an answer outside the admitted profile",
                    )]
                })?;
            let expected_binding = plan
                .suspension_binding_at(index, &scalar_arguments, &prior_scalars)
                .map_err(|error| vec![error])?;
            if continuation.binding != expected_binding {
                return Err(vec![Diagnostic::io(
                    SUSPENSION_MISMATCH,
                    "channel continuation binding does not match this exact program, site, \
                     arguments, and prior answer bits",
                )]);
            }
            let mut expected = Vec::with_capacity(index + 1);
            let mut answers = Vec::with_capacity(index + 1);
            let mut injected_allocations = 0_u32;
            for record in &continuation.history {
                expected.push(typed_resume_channel_value(
                    declarations,
                    &yields.request_type,
                    &record.request,
                    "historical request",
                    &mut injected_allocations,
                )?);
                answers.push(typed_resume_channel_value(
                    declarations,
                    &yields.response_type,
                    &record.answer,
                    "historical answer",
                    &mut injected_allocations,
                )?);
            }
            expected.push(typed_resume_channel_value(
                declarations,
                &yields.request_type,
                &continuation.request,
                "request",
                &mut injected_allocations,
            )?);
            answers.push(typed_resume_channel_value(
                declarations,
                &yields.response_type,
                &answer,
                "answer",
                &mut injected_allocations,
            )?);
            let mut channel_history = continuation.history.clone();
            channel_history.push(ChannelYieldRecord {
                request: continuation.request.clone(),
                answer: answer.clone(),
            });
            let next_index = index + 1;
            let next_binding = plan
                .suspensions
                .get(next_index)
                .map(|_| {
                    let mut answer_scalars = prior_scalars;
                    answer_scalars.push(
                        channel_to_resumable_scalar(
                            &channel_history.last().expect("just appended").answer,
                        )
                        .expect("typed channel answer is within the admitted profile"),
                    );
                    plan.suspension_binding_at(next_index, &scalar_arguments, &answer_scalars)
                })
                .transpose()
                .map_err(|error| vec![error])?;
            (
                Resumption::Replay {
                    expected,
                    answers,
                    observed: 0,
                    parked: None,
                    history: Vec::new(),
                    sites: None,
                    parked_site: None,
                    parked_environment: None,
                    carried: std::collections::BTreeMap::new(),
                },
                next_binding,
                channel_history,
            )
        }
    };

    let (step, steps_used) = run_worker_values(
        program,
        &admitted,
        entry,
        bound,
        resumption,
        max_steps,
        |settled, resumption| {
            settle_channel_step(
                settled,
                resumption,
                &plan,
                next_binding,
                declarations,
                &channel_history,
            )
        },
    )?;

    if step == ChannelStep::GuardError(REQUEST_DRIFT.to_owned()) {
        return Err(vec![Diagnostic::io(
            REQUEST_DRIFT,
            format!(
                "resuming `{function_id}` replayed its prefix and recomputed a different \
                 request than the suspension recorded; the resume is refused rather than \
                 answered"
            ),
        )]);
    }
    Ok(SequentialChannelResumableEvaluation {
        step: into_public(step),
        steps_used,
        max_steps,
    })
}

#[derive(Clone, Debug, PartialEq)]
enum ChannelStep {
    Suspended {
        continuation: ResumableChannelContinuation,
    },
    Completed {
        state: ResumableStateId,
        result: ResumableChannelValue,
    },
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    GuardError(String),
}

fn into_public(step: ChannelStep) -> SequentialChannelResumableStep {
    match step {
        ChannelStep::Suspended { continuation } => {
            SequentialChannelResumableStep::Suspended { continuation }
        }
        ChannelStep::Completed { state, result } => {
            SequentialChannelResumableStep::Completed { state, result }
        }
        ChannelStep::LanguageFailure(status) => {
            SequentialChannelResumableStep::LanguageFailure(status)
        }
        ChannelStep::FuelExhausted => SequentialChannelResumableStep::FuelExhausted,
        ChannelStep::CallDepthExceeded => SequentialChannelResumableStep::CallDepthExceeded,
        ChannelStep::GuardError(detail) => SequentialChannelResumableStep::GuardError(detail),
    }
}

/// Turn the evaluator's settled `Result` into one closed step, mirroring
/// `super::settle_step` widened to [`ResumableChannelValue`]. `channel_history`
/// is the exact settled request/answer history this call's own resume
/// already checked and typed; `Resumption::Replay`'s own (unused here)
/// `history` field stays `ArgumentValue`-typed and empty for every channel
/// evaluation, so this reads the caller-supplied vector instead of that
/// field.
#[allow(clippy::too_many_arguments)]
fn settle_channel_step(
    settled: Result<Value, Flow>,
    resumption: &mut Resumption,
    plan: &SequentialResumablePlan,
    binding: Option<ResumableSuspensionBinding>,
    declarations: &hir::DeclarationIndex,
    channel_history: &[ChannelYieldRecord],
) -> ChannelStep {
    match settled {
        Ok(value) => match channel_of(declarations, &value) {
            Some(result) => ChannelStep::Completed {
                state: plan.complete.id.clone(),
                result,
            },
            None => ChannelStep::GuardError(
                "resumable-effect entry returned a non-scalar value".to_owned(),
            ),
        },
        Err(Flow::Guard(SUSPENDED_AT_YIELD)) => {
            let Some(binding) = binding else {
                return ChannelStep::GuardError(
                    "a suspension escaped without its exact invocation binding".to_owned(),
                );
            };
            match resumption {
                Resumption::Fresh {
                    parked: Some(request),
                    ..
                } => {
                    let Some(request) = channel_of(declarations, request) else {
                        return ChannelStep::GuardError(
                            "`yield` produced a value outside the admitted channel profile"
                                .to_owned(),
                        );
                    };
                    ChannelStep::Suspended {
                        continuation: ResumableChannelContinuation {
                            state: plan.suspensions[0].state.id.clone(),
                            binding,
                            request,
                            history: Vec::new(),
                        },
                    }
                }
                Resumption::Replay {
                    parked: Some(request),
                    ..
                } => {
                    let index = channel_history.len();
                    let Some(suspension) = plan.suspensions.get(index) else {
                        return ChannelStep::GuardError(
                            "a replay parked beyond the plan's final suspension".to_owned(),
                        );
                    };
                    let Some(request) = channel_of(declarations, request) else {
                        return ChannelStep::GuardError(
                            "`yield` produced a value outside the admitted channel profile"
                                .to_owned(),
                        );
                    };
                    ChannelStep::Suspended {
                        continuation: ResumableChannelContinuation {
                            state: suspension.state.id.clone(),
                            binding,
                            request,
                            history: channel_history.to_vec(),
                        },
                    }
                }
                _ => ChannelStep::GuardError(
                    "a suspension escaped without parking its request".to_owned(),
                ),
            }
        }
        Err(Flow::Failure(status)) => ChannelStep::LanguageFailure(status),
        Err(Flow::Exhausted) => ChannelStep::FuelExhausted,
        Err(Flow::DepthExceeded) => ChannelStep::CallDepthExceeded,
        Err(Flow::Guard(detail)) => ChannelStep::GuardError(detail.to_owned()),
        Err(Flow::Residual(_)) => ChannelStep::GuardError(
            "owned postfix `?` residual escaped its function frame".to_owned(),
        ),
        Err(Flow::Cancelled { .. }) => {
            ChannelStep::GuardError("unexpected cancellation in resumable evaluation".to_owned())
        }
        Err(Flow::Utf8MaterializationLimitExceeded { .. }) => ChannelStep::GuardError(
            "unexpected UTF-8 materialization limit in resumable evaluation".to_owned(),
        ),
    }
}

#[cfg(test)]
mod tests;
