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
use crate::hir::{self, ResolvedFunction, ResolvedProgram, ValueId};
use crate::resumable_effects::lowering::{
    self, ResumableScalar, ResumableStateId, ResumableSuspensionBinding, SequentialResumablePlan,
};

use crate::conformance::NormalizedStatus;
use crate::interpreter::prepared::PreparedCancellation;
use crate::interpreter::{ArgumentValue, Evaluator, Flow, FunctionLookup, Value};

use super::{
    admitted_resolved_functions, argument_error, argument_of, channel_of,
    channel_to_resumable_scalar, max_byte_allocation, option_error, scan_closure, selection_error,
    typed_resume_channel_value, value_of_channel, ChannelYieldRecord, ResumableChannelContinuation,
    ResumableChannelValue, Resumption, EVALUATION_STACK_BYTES, MAX_STEPS_LIMIT,
    REASON_AUTOMATIC_IDENTITY, REASON_NOT_RESUMABLE, REASON_OUTSIDE_PROFILE,
    REASON_UNSUPPORTED_CALLEE, REQUEST_DRIFT, SUSPENDED_AT_YIELD, SUSPENSION_MISMATCH,
};

/// The aggregate whole-function boundary has its own admitted carrier. The
/// legacy `Admitted` surface remains scalar-only so unrelated interpreter
/// APIs cannot acquire aggregate arguments by accident.
pub(super) struct ChannelAdmitted<'p> {
    pub(super) entry: &'p ResolvedFunction,
    pub(super) yields: &'p hir::ResolvedYieldsClause,
    pub(super) bound: Vec<(ValueId, Value)>,
    pub(super) arguments: Vec<ResumableScalar>,
    pub(super) admitted: std::collections::BTreeMap<&'p str, &'p ResolvedFunction>,
}

/// Check the complete Copy-only response carrier without evaluating source.
/// Durable callers use this before acknowledging a host observation.
pub(crate) fn valid_copy_channel_response(
    program: &ResolvedProgram,
    function_id: &str,
    supplied: &ResumableChannelValue,
) -> bool {
    valid_copy_channel_boundary(program, function_id, supplied, false)
}

/// Validate the complete checked Copy request without evaluator entry or an
/// owned Bytes reconstruction. It shares the response boundary's conversions.
pub(crate) fn valid_copy_channel_request(
    program: &ResolvedProgram,
    function_id: &str,
    supplied: &ResumableChannelValue,
) -> bool {
    valid_copy_channel_boundary(program, function_id, supplied, true)
}

fn valid_copy_channel_boundary(
    program: &ResolvedProgram,
    function_id: &str,
    supplied: &ResumableChannelValue,
    request: bool,
) -> bool {
    if matches!(
        supplied,
        ResumableChannelValue::RecordBytes { .. } | ResumableChannelValue::VariantBytes { .. }
    ) {
        return false;
    }
    let Some(yields) = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == function_id)
        .and_then(|function| function.yields.as_ref())
    else {
        return false;
    };
    let ty = if request {
        &yields.request_type
    } else {
        &yields.response_type
    };
    if hir::yield_aggregate::has_bytes_leaf(&program.declarations, ty) {
        return false;
    }
    if !hir::is_scalar_resolved_type(ty)
        && hir::yield_aggregate::bounded_aggregate_refusal(&program.declarations, ty).is_err()
    {
        return false;
    }
    let mut allocation = 0;
    value_of_channel(&program.declarations, ty, supplied, &mut allocation).is_some()
}

pub(super) fn admit_channel_entry<'p>(
    program: &'p hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ResumableChannelValue],
    max_steps: usize,
    allow_aggregate_boundary: bool,
) -> Result<ChannelAdmitted<'p>, Vec<Diagnostic>> {
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "resumable-effect evaluation max_steps must be between 1 and {MAX_STEPS_LIMIT}"
        ))]);
    }
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == function_id)
        .ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("resumable entry `{function_id}` is absent from the function index"),
            )]
        })?;
    if !program
        .declarations
        .declaration(&entry.id)
        .is_some_and(|declaration| declaration.identity_origin == hir::IdentityOrigin::Explicit)
    {
        return Err(vec![selection_error(
            REASON_AUTOMATIC_IDENTITY,
            format!("resumable entry `{function_id}` does not have an explicit stable identity"),
        )]);
    }
    let yields = entry.yields.as_ref().ok_or_else(|| vec![selection_error(REASON_NOT_RESUMABLE,
        format!("`{function_id}` declares no `yields` clause; this lane runs only functions that can suspend"))])?;
    // Existing v6 channels permit a bounded owned-Bytes request. Whole
    // function inputs/results and answers remain Copy-only until they have a
    // carried-owned-state cleanup protocol.
    let request_type = |ty: &hir::ResolvedType| {
        hir::is_scalar_resolved_type(ty)
            || (matches!(ty, hir::ResolvedType::Nominal { .. })
                && hir::yield_aggregate::bounded_aggregate_refusal(&program.declarations, ty)
                    .is_ok())
    };
    let copy_type = |ty: &hir::ResolvedType| {
        request_type(ty) && !hir::yield_aggregate::has_bytes_leaf(&program.declarations, ty)
    };
    if !entry.effects.is_empty()
        || !request_type(&yields.request_type)
        || !copy_type(&yields.response_type)
        || !(if allow_aggregate_boundary {
            copy_type(&entry.return_type)
        } else {
            hir::is_scalar_resolved_type(&entry.return_type)
        })
        || entry.params.iter().any(|parameter| {
            parameter.ownership != hir::OwnershipMode::Value
                || if allow_aggregate_boundary {
                    !copy_type(&parameter.ty)
                } else {
                    !hir::is_scalar_resolved_type(&parameter.ty)
                }
        })
    {
        return Err(vec![selection_error(
            REASON_OUTSIDE_PROFILE,
            format!("resumable entry `{function_id}` is outside the bounded Copy channel profile"),
        )]);
    }
    if entry.params.len() != arguments.len() {
        return Err(vec![argument_error(format!(
            "`{}` takes {} argument(s); {} were supplied",
            entry.name,
            entry.params.len(),
            arguments.len()
        ))]);
    }
    let mut allocation = 0;
    let mut bound = Vec::with_capacity(arguments.len());
    let mut scalar_arguments = Vec::with_capacity(arguments.len());
    for (index, (parameter, argument)) in entry.params.iter().zip(arguments).enumerate() {
        let value = value_of_channel(
            &program.declarations,
            &parameter.ty,
            argument,
            &mut allocation,
        )
        .ok_or_else(|| {
            vec![argument_error(format!(
                "argument {index} of `{}` does not have the declared bounded Copy parameter type",
                entry.name
            ))]
        })?;
        let scalar = channel_to_resumable_scalar(argument).ok_or_else(|| {
            vec![argument_error(format!(
                "argument {index} of `{}` is outside the bounded Copy channel profile",
                entry.name
            ))]
        })?;
        bound.push((parameter.id.clone(), value));
        scalar_arguments.push(scalar);
    }
    let mut admitted = admitted_resolved_functions(program);
    admitted.insert(entry.id.as_str(), entry);
    scan_closure(function_id, &admitted, program)?;
    hir::validate(program).map_err(|error| vec![error])?;
    Ok(ChannelAdmitted {
        entry,
        yields,
        bound,
        arguments: scalar_arguments,
        admitted,
    })
}

pub(super) fn run_worker_values<T: Send>(
    program: &hir::ResolvedProgram,
    admitted: &std::collections::BTreeMap<&str, &ResolvedFunction>,
    entry: &ResolvedFunction,
    bound: Vec<(ValueId, Value)>,
    resumption: Resumption,
    max_steps: usize,
    settle: impl FnOnce(Result<Value, Flow>, &mut Resumption) -> T + Send,
) -> Result<(T, usize), Vec<Diagnostic>> {
    let closure_functions =
        crate::interpreter::closures::checked_functions(program).map_err(|error| vec![error])?;
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("semaprax-resumable-evaluate".to_owned())
            .stack_size(EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, move || {
                let mut evaluator = Evaluator::new_prepared(
                    FunctionLookup::Borrowed(admitted),
                    closure_functions,
                    &program.declarations,
                    max_steps,
                    0,
                    PreparedCancellation::Never,
                );
                if let Resumption::Replay {
                    carried,
                    expected,
                    answers,
                    ..
                } = &resumption
                {
                    evaluator.next_byte_allocation = carried
                        .values()
                        .chain(expected)
                        .chain(answers)
                        .map(max_byte_allocation)
                        .max()
                        .unwrap_or(0);
                }
                evaluator.resumption = resumption;
                let settled = evaluator.evaluate_entry_values(entry, bound);
                let step = settle(settled, &mut evaluator.resumption);
                (step, evaluator.steps)
            })
            .map_err(|error| {
                vec![option_error(format!(
                    "resumable-effect evaluation thread failed to start: {error}"
                ))]
            })?;
        worker.join().map_err(|_| {
            vec![option_error(
                "resumable-effect evaluation thread panicked".to_owned(),
            )]
        })
    })
}

/// The legacy bounded request/response channel outcome. Its ordinary function
/// boundary remains scalar, preserving the existing public API.
#[derive(Clone, Debug, PartialEq)]
pub enum SequentialChannelResumableStep {
    Suspended {
        continuation: ResumableChannelContinuation,
    },
    Completed {
        state: ResumableStateId,
        result: ArgumentValue,
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

/// The distinct whole-function aggregate carrier. It is used only by the
/// `_with_arguments` APIs, keeping legacy scalar callers source-compatible.
#[derive(Clone, Debug, PartialEq)]
pub enum SequentialChannelArgumentsStep {
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
pub struct SequentialChannelArgumentsEvaluation {
    pub step: SequentialChannelArgumentsStep,
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
    into_legacy(evaluate_channel_resumable(
        program,
        function_id,
        &arguments,
        None,
        max_steps,
        false,
    )?)
}

/// Run the sequential channel lane with its bounded Copy whole-function
/// boundary. This is intentionally a new API: `ArgumentValue` remains the
/// scalar-only boundary for every existing interpreter entry point.
pub fn run_sequential_channel_resumable_effect_with_arguments(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ResumableChannelValue],
    max_steps: usize,
) -> Result<SequentialChannelArgumentsEvaluation, Vec<Diagnostic>> {
    into_arguments(evaluate_channel_resumable(
        program,
        function_id,
        arguments,
        None,
        max_steps,
        true,
    )?)
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
    into_legacy(evaluate_channel_resumable(
        program,
        function_id,
        &arguments,
        Some((continuation.clone(), answer.clone())),
        max_steps,
        false,
    )?)
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
) -> Result<SequentialChannelArgumentsEvaluation, Vec<Diagnostic>> {
    into_arguments(evaluate_channel_resumable(
        program,
        function_id,
        arguments,
        Some((continuation.clone(), answer.clone())),
        max_steps,
        true,
    )?)
}

fn evaluate_channel_resumable(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ResumableChannelValue],
    resume: Option<(ResumableChannelContinuation, ResumableChannelValue)>,
    max_steps: usize,
    allow_aggregate_boundary: bool,
) -> Result<ChannelEvaluation, Vec<Diagnostic>> {
    let ChannelAdmitted {
        entry,
        yields,
        bound,
        admitted,
        arguments: scalar_arguments,
    } = admit_channel_entry(
        program,
        function_id,
        arguments,
        max_steps,
        allow_aggregate_boundary,
    )?;
    let plan = if allow_aggregate_boundary {
        lowering::lower_sequential_with_arguments(program, entry)
    } else {
        lowering::lower_sequential(program, entry)
    }
    .map_err(|error| vec![error])?;
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
    Ok(ChannelEvaluation {
        step,
        steps_used,
        max_steps,
    })
}

struct ChannelEvaluation {
    step: ChannelStep,
    steps_used: usize,
    max_steps: usize,
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

fn into_legacy(
    evaluation: ChannelEvaluation,
) -> Result<SequentialChannelResumableEvaluation, Vec<Diagnostic>> {
    Ok(SequentialChannelResumableEvaluation {
        step: match evaluation.step {
            ChannelStep::Suspended { continuation } => {
                SequentialChannelResumableStep::Suspended { continuation }
            }
            ChannelStep::Completed {
                state,
                result: ResumableChannelValue::Scalar(result),
            } => SequentialChannelResumableStep::Completed { state, result },
            ChannelStep::Completed { .. } => {
                return Err(vec![Diagnostic::io(
                    "SPX-F102",
                    "legacy sequential channel entry requires a scalar function result",
                )])
            }
            ChannelStep::LanguageFailure(status) => {
                SequentialChannelResumableStep::LanguageFailure(status)
            }
            ChannelStep::FuelExhausted => SequentialChannelResumableStep::FuelExhausted,
            ChannelStep::CallDepthExceeded => SequentialChannelResumableStep::CallDepthExceeded,
            ChannelStep::GuardError(detail) => SequentialChannelResumableStep::GuardError(detail),
        },
        steps_used: evaluation.steps_used,
        max_steps: evaluation.max_steps,
    })
}

fn into_arguments(
    evaluation: ChannelEvaluation,
) -> Result<SequentialChannelArgumentsEvaluation, Vec<Diagnostic>> {
    Ok(SequentialChannelArgumentsEvaluation {
        step: match evaluation.step {
            ChannelStep::Suspended { continuation } => {
                SequentialChannelArgumentsStep::Suspended { continuation }
            }
            ChannelStep::Completed { state, result } => {
                SequentialChannelArgumentsStep::Completed { state, result }
            }
            ChannelStep::LanguageFailure(status) => {
                SequentialChannelArgumentsStep::LanguageFailure(status)
            }
            ChannelStep::FuelExhausted => SequentialChannelArgumentsStep::FuelExhausted,
            ChannelStep::CallDepthExceeded => SequentialChannelArgumentsStep::CallDepthExceeded,
            ChannelStep::GuardError(detail) => SequentialChannelArgumentsStep::GuardError(detail),
        },
        steps_used: evaluation.steps_used,
        max_steps: evaluation.max_steps,
    })
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
