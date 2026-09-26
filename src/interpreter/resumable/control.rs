//! Control-dependent resumable lane (issue #296).
//!
//! Executes functions whose `yield` sites sit inside `if`/`else` branches and
//! `while` bodies, lowered by `resumable_effects::lowering::control`. The
//! execution model is the sequential lane's: resume replays the pure prefix
//! from entry, re-checking every recorded request, and additionally requires
//! each replayed suspension to occur at exactly its recorded static site. The
//! branch taken and the loop iteration reached are therefore recomputed, never
//! read from the continuation, and a continuation cannot steer control.

use super::{
    admit_entry, argument_of, resumable_scalars, run_worker, typed_resume_value, ArgumentValue,
    Diagnostic, Flow, Resumption, Value, REQUEST_DRIFT, SUSPENDED_AT_YIELD, SUSPENSION_MISMATCH,
};
use crate::conformance::NormalizedStatus;
use crate::hir::{self, ValueId};
use crate::interpreter::OwnedBytesValue;
use crate::resumable_effects::lowering::control::{
    lower_control, ControlResumablePlan, MAX_CONTROL_SUSPENSIONS,
};
use crate::resumable_effects::lowering::{
    ResumableScalar, ResumableStateId, ResumableSuspensionBinding,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// One settled suspension: its static site state, request and answer.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlYieldRecord {
    site: ResumableStateId,
    request: ArgumentValue,
    answer: ArgumentValue,
}

impl ControlYieldRecord {
    pub fn site(&self) -> &ResumableStateId {
        &self.site
    }

    pub fn request(&self) -> &ArgumentValue {
        &self.request
    }

    pub fn answer(&self) -> &ArgumentValue {
        &self.answer
    }
}

/// An opaque continuation of a control-dependent invocation. Proof data only:
/// it grants no authority and cannot choose a branch or loop count.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlContinuation {
    state: ResumableStateId,
    binding: ResumableSuspensionBinding,
    request: ArgumentValue,
    history: Vec<ControlYieldRecord>,
    /// Issue #296, spec section 11.6: the owned `Bytes` locals live at this
    /// exact site, in the plan's own cleanup-inventory order, carried by
    /// value rather than left for a resume to recompute. Empty for every
    /// continuation of a plan `carries_owned_bytes` is `false` for.
    carried: Vec<(ValueId, Vec<u8>)>,
}

impl ControlContinuation {
    pub fn state(&self) -> &ResumableStateId {
        &self.state
    }

    pub fn binding(&self) -> &ResumableSuspensionBinding {
        &self.binding
    }

    pub fn request(&self) -> &ArgumentValue {
        &self.request
    }

    pub fn history(&self) -> &[ControlYieldRecord] {
        &self.history
    }

    pub(crate) fn carried(&self) -> &[(ValueId, Vec<u8>)] {
        &self.carried
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ControlResumableStep {
    Suspended {
        continuation: ControlContinuation,
    },
    Completed {
        state: ResumableStateId,
        result: ArgumentValue,
    },
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    /// A further suspension would exceed [`MAX_CONTROL_SUSPENSIONS`].
    SuspensionBoundExceeded,
    GuardError(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControlResumableEvaluation {
    pub step: ControlResumableStep,
    pub steps_used: usize,
    pub max_steps: usize,
}

/// Run a control-dependent `yields` function to its first suspension.
pub fn run_control_resumable_effect(
    program: &hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    max_steps: usize,
) -> Result<ControlResumableEvaluation, Vec<Diagnostic>> {
    evaluate(program, function_id, arguments, None, max_steps)
}

/// Resume a control continuation with one typed answer.
pub fn resume_control_resumable_effect(
    program: &hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    continuation: &ControlContinuation,
    answer: &ArgumentValue,
    max_steps: usize,
) -> Result<ControlResumableEvaluation, Vec<Diagnostic>> {
    evaluate(
        program,
        function_id,
        arguments,
        Some((continuation, answer)),
        max_steps,
    )
}

fn mismatch(detail: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(SUSPENSION_MISMATCH, detail)]
}

fn scalars_of(values: &[ArgumentValue]) -> Result<Vec<ResumableScalar>, Vec<Diagnostic>> {
    resumable_scalars(values).ok_or_else(|| mismatch("control history carries a non-scalar value"))
}

/// Settled `(site index, answer bits)` pairs of a history, checked against
/// the plan.
fn indexed_history(
    plan: &ControlResumablePlan,
    history: &[ControlYieldRecord],
) -> Result<Vec<(usize, ResumableScalar)>, Vec<Diagnostic>> {
    let answers = history
        .iter()
        .map(|record| record.answer.clone())
        .collect::<Vec<_>>();
    let answers = scalars_of(&answers)?;
    history
        .iter()
        .zip(answers)
        .map(|(record, answer)| {
            plan.site_of_state(&record.site)
                .map(|site| (site, answer))
                .ok_or_else(|| mismatch("control history names a site outside this exact plan"))
        })
        .collect()
}

/// The exact bytes of every `ValueId` `carried` names, read from the frame
/// snapshot taken at the moment of park, in `carried`'s own order. `Err` when
/// a required local is absent from the snapshot or is not an owned `Bytes`
/// value -- unreachable once `hir::resolve` has admitted the function, since
/// every site's `carried` list is exactly the whole-storage owned `Bytes`
/// locals `cleanup_plan::admit_owned_bytes_profile` proved live there, but
/// checked rather than assumed.
fn carried_bytes_of(
    carried: &[ValueId],
    environment: &[(ValueId, Value)],
) -> Result<Vec<(ValueId, Vec<u8>)>, String> {
    carried
        .iter()
        .map(|value_id| {
            let value = environment
                .iter()
                .find(|(id, _)| id == value_id)
                .map(|(_, value)| value)
                .ok_or_else(|| {
                    "a carried owned Bytes local is absent from the parked frame".to_owned()
                })?;
            match value {
                Value::Bytes(inner) => Ok((value_id.clone(), inner.bytes.to_vec())),
                _ => Err("a carried owned Bytes local is not an owned Bytes value".to_owned()),
            }
        })
        .collect()
}

fn evaluate(
    program: &hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    resume: Option<(&ControlContinuation, &ArgumentValue)>,
    max_steps: usize,
) -> Result<ControlResumableEvaluation, Vec<Diagnostic>> {
    let admitted = admit_entry(program, function_id, arguments, max_steps)?;
    let plan = lower_control(program, admitted.entry).map_err(|error| vec![error])?;
    let scalar_arguments = scalars_of(arguments)?;
    let yields = admitted.yields;
    let (resumption, settled_history) = match resume {
        None => (
            Resumption::Fresh {
                parked: None,
                parked_site: None,
                parked_environment: None,
            },
            Vec::new(),
        ),
        Some((continuation, answer)) => {
            let site = plan
                .site_of_state(&continuation.state)
                .ok_or_else(|| mismatch("control continuation state is not a site of this plan"))?;
            let indexed = indexed_history(&plan, &continuation.history)?;
            if continuation.carried.len() != plan.sites[site].carried.len()
                || continuation
                    .carried
                    .iter()
                    .zip(&plan.sites[site].carried)
                    .any(|((carried_id, _), expected_id)| carried_id != expected_id)
            {
                return Err(mismatch(
                    "control continuation's carried owned values do not match this exact site",
                ));
            }
            let carried_bytes = continuation
                .carried
                .iter()
                .map(|(_, bytes)| bytes.clone())
                .collect::<Vec<_>>();
            let expected_binding = plan
                .binding(site, &scalar_arguments, &indexed, &carried_bytes)
                .map_err(|error| vec![error])?;
            if continuation.binding != expected_binding {
                return Err(mismatch(
                    "control continuation binding does not match this exact program, site, arguments, and settled history",
                ));
            }
            let mut expected = Vec::with_capacity(continuation.history.len() + 1);
            let mut answers = Vec::with_capacity(continuation.history.len() + 1);
            let mut sites = Vec::with_capacity(continuation.history.len() + 1);
            for (record, (index, _)) in continuation.history.iter().zip(&indexed) {
                expected.push(typed_resume_value(
                    &yields.request_type,
                    &record.request,
                    "historical request",
                )?);
                answers.push(typed_resume_value(
                    &yields.response_type,
                    &record.answer,
                    "historical answer",
                )?);
                sites.push(plan.sites[*index].expression.clone());
            }
            expected.push(typed_resume_value(
                &yields.request_type,
                &continuation.request,
                "request",
            )?);
            answers.push(typed_resume_value(&yields.response_type, answer, "answer")?);
            sites.push(plan.sites[site].expression.clone());
            let mut settled = continuation.history.clone();
            settled.push(ControlYieldRecord {
                site: continuation.state.clone(),
                request: continuation.request.clone(),
                answer: answer.clone(),
            });
            let mut carried_values = BTreeMap::new();
            for (index, (value_id, bytes)) in continuation.carried.iter().enumerate() {
                carried_values.insert(
                    value_id.clone(),
                    Value::Bytes(OwnedBytesValue {
                        allocation: index as u32 + 1,
                        bytes: Arc::from(bytes.as_slice()),
                    }),
                );
            }
            (
                Resumption::Replay {
                    expected,
                    answers,
                    observed: 0,
                    parked: None,
                    history: Vec::new(),
                    sites: Some(sites),
                    parked_site: None,
                    parked_environment: None,
                    carried: carried_values,
                },
                settled,
            )
        }
    };
    let arguments_for_binding = scalar_arguments.clone();
    let (step, steps_used) = run_worker(
        program,
        &admitted.admitted,
        admitted.entry,
        &admitted.bound,
        resumption,
        max_steps,
        |settled, resumption| {
            settle(
                settled,
                resumption,
                &plan,
                &arguments_for_binding,
                settled_history,
            )
        },
    )?;
    if step == ControlResumableStep::GuardError(REQUEST_DRIFT.to_owned()) {
        return Err(vec![Diagnostic::io(
            REQUEST_DRIFT,
            format!(
                "resuming `{function_id}` replayed its prefix and reached a different request or \
                 yield site than the continuation recorded; the resume is refused"
            ),
        )]);
    }
    Ok(ControlResumableEvaluation {
        step,
        steps_used,
        max_steps,
    })
}

fn settle(
    settled: Result<Value, Flow>,
    resumption: &mut Resumption,
    plan: &ControlResumablePlan,
    arguments: &[ResumableScalar],
    history: Vec<ControlYieldRecord>,
) -> ControlResumableStep {
    match settled {
        Ok(value) => {
            if replay_left_history_unconsumed(resumption) {
                return ControlResumableStep::GuardError(REQUEST_DRIFT.to_owned());
            }
            match argument_of(&value) {
                Some(result) => ControlResumableStep::Completed {
                    state: plan.complete.id.clone(),
                    result,
                },
                None => ControlResumableStep::GuardError(
                    "resumable-effect entry returned a non-scalar value".to_owned(),
                ),
            }
        }
        Err(Flow::Guard(SUSPENDED_AT_YIELD)) => {
            let (parked, parked_site, parked_environment) = match resumption {
                Resumption::Fresh {
                    parked,
                    parked_site,
                    parked_environment,
                }
                | Resumption::Replay {
                    parked,
                    parked_site,
                    parked_environment,
                    ..
                } => (parked.take(), parked_site.take(), parked_environment.take()),
                Resumption::Refused => (None, None, None),
            };
            let (Some(request), Some(site)) = (parked, parked_site) else {
                return ControlResumableStep::GuardError(
                    "a suspension escaped without parking its request and site".to_owned(),
                );
            };
            if history.len() >= MAX_CONTROL_SUSPENSIONS {
                return ControlResumableStep::SuspensionBoundExceeded;
            }
            let Some(index) = plan.site_of_expression(&site) else {
                return ControlResumableStep::GuardError(
                    "a suspension parked at a site outside the plan".to_owned(),
                );
            };
            let Some(request) = argument_of(&request) else {
                return ControlResumableStep::GuardError(
                    "`yield` produced a non-scalar request".to_owned(),
                );
            };
            let indexed = match indexed_history(plan, &history) {
                Ok(indexed) => indexed,
                Err(_) => {
                    return ControlResumableStep::GuardError(
                        "settled control history left the plan".to_owned(),
                    )
                }
            };
            let environment = parked_environment.unwrap_or_default();
            let carried = match carried_bytes_of(&plan.sites[index].carried, &environment) {
                Ok(carried) => carried,
                Err(message) => return ControlResumableStep::GuardError(message),
            };
            let carried_bytes = carried
                .iter()
                .map(|(_, bytes)| bytes.clone())
                .collect::<Vec<_>>();
            match plan.binding(index, arguments, &indexed, &carried_bytes) {
                Ok(binding) => ControlResumableStep::Suspended {
                    continuation: ControlContinuation {
                        state: plan.sites[index].state.id.clone(),
                        binding,
                        request,
                        history,
                        carried,
                    },
                },
                Err(error) => ControlResumableStep::GuardError(error.message),
            }
        }
        Err(Flow::Failure(status)) => {
            if replay_left_history_unconsumed(resumption) {
                return ControlResumableStep::GuardError(REQUEST_DRIFT.to_owned());
            }
            ControlResumableStep::LanguageFailure(status)
        }
        Err(Flow::Exhausted) => ControlResumableStep::FuelExhausted,
        Err(Flow::DepthExceeded) => ControlResumableStep::CallDepthExceeded,
        Err(Flow::Guard(detail)) => ControlResumableStep::GuardError(detail.to_owned()),
        Err(_) => ControlResumableStep::GuardError(
            "unexpected control flow escaped a control resumable invocation".to_owned(),
        ),
    }
}

/// True when a replayed segment finished without consuming every expected
/// history record: the resumed run diverged from its continuation without
/// tripping a per-site request check, so completion or failure is refused as
/// drift (`SPX-F114`) rather than reported. A parked suspension always
/// implies full consumption, so only the completion and failure arms consult
/// this. Fuel and depth exhaustion stay honest resource reports.
fn replay_left_history_unconsumed(resumption: &Resumption) -> bool {
    matches!(
        resumption,
        Resumption::Replay {
            expected,
            observed,
            ..
        } if observed != &expected.len()
    )
}

/// Rebuild a continuation from authenticated checkpoint fields. The plan,
/// every site, and the binding are re-derived from the current program and
/// arguments; requests remain claims that the next resume replay checks.
pub(crate) fn rebuild_control_continuation(
    program: &hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    state: &str,
    binding: [u8; 32],
    request: ArgumentValue,
    history: Vec<(String, ArgumentValue, ArgumentValue)>,
    carried: Vec<(String, Vec<u8>)>,
) -> Result<ControlContinuation, Vec<Diagnostic>> {
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == function_id)
        .ok_or_else(|| mismatch("control checkpoint names an absent function"))?;
    let plan = lower_control(program, entry).map_err(|error| vec![error])?;
    let yields = entry
        .yields
        .as_ref()
        .ok_or_else(|| mismatch("control checkpoint function has no `yields` clause"))?;
    let lookup = |text: &str| {
        plan.sites
            .iter()
            .find(|site| site.state.id.as_str() == text)
            .map(|site| site.state.id.clone())
            .ok_or_else(|| mismatch("control checkpoint names a site outside this exact plan"))
    };
    let mut records = Vec::with_capacity(history.len());
    for (site, historical_request, historical_answer) in history {
        typed_resume_value(
            &yields.request_type,
            &historical_request,
            "historical request",
        )?;
        typed_resume_value(
            &yields.response_type,
            &historical_answer,
            "historical answer",
        )?;
        records.push(ControlYieldRecord {
            site: lookup(&site)?,
            request: historical_request,
            answer: historical_answer,
        });
    }
    typed_resume_value(&yields.request_type, &request, "request")?;
    let state = lookup(state)?;
    let site = plan.site_of_state(&state).expect("looked-up site");
    let indexed = indexed_history(&plan, &records)?;
    if carried.len() != plan.sites[site].carried.len()
        || carried
            .iter()
            .zip(&plan.sites[site].carried)
            .any(|((carried_id, _), expected_id)| carried_id.as_str() != expected_id.as_str())
    {
        return Err(mismatch(
            "control checkpoint's carried owned values do not match this exact site",
        ));
    }
    let carried_ids = plan.sites[site].carried.clone();
    let carried_bytes = carried
        .iter()
        .map(|(_, bytes)| bytes.clone())
        .collect::<Vec<_>>();
    let expected = plan
        .binding(site, &scalars_of(arguments)?, &indexed, &carried_bytes)
        .map_err(|error| vec![error])?;
    if *expected.as_bytes() != binding {
        return Err(mismatch(
            "control checkpoint binding does not match this exact program, arguments, and history",
        ));
    }
    Ok(ControlContinuation {
        state,
        binding: expected,
        request,
        history: records,
        carried: carried_ids.into_iter().zip(carried_bytes).collect(),
    })
}

#[cfg(test)]
mod tests;
