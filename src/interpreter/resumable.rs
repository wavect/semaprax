//! Resumable Effects v1 (issue #204): interpreter suspend/resume execution
//! of one `yields`-declaring function.
//!
//! `24c1d166` admitted the `yield` slice across parser, canonical formatter,
//! resolver/HIR, verifier, semantic graph, and both backends, but no engine
//! executed it: native refused with `SPX-B116`, Wasm with `SPX-W126`, and the
//! interpreter refused at its own admission gate. This module is the first
//! public compiler engine that runs it. Ordinary native and Wasm emission
//! still refuses with those stable codes; compiler-private parity runners use
//! `resumable_effects::lowering`'s independently validated yield-free
//! projections instead of weakening either ordinary emitter's admission.
//!
//! # The execution model
//!
//! `hir::resolve_yield` already states it: a suspension is resumed by
//! *re-executing the function from its entry* with the resume value
//! substituted at the yield site. That is sound here, and only here, because
//! the admitted slice forecloses every way a re-execution could differ from
//! or duplicate the original prefix:
//!
//! - a `yields`-declaring function may declare no `uses` effects
//!   (`SPX-T302`), so the replayed prefix contacts no host and can
//!   redispatch nothing;
//! - every parameter and intermediate value is an admitted Copy scalar
//!   (`SPX-T301`/`SPX-T303`), so nothing owned is live across the
//!   suspension and the replay allocates and frees nothing;
//! - `parser::yields` admits direct top-level sites and, for the
//!   control-dependent lane in [`control`], structured-control sites inside
//!   `if`/`else` branches and `while` bodies (`SPX-T297`/`SPX-T298`); the
//!   sequential lane narrows to one to eight direct sites, and the control
//!   lane replays each suspension at exactly its recorded site;
//! - `resumable_effects::lowering` rejects a call closure that reaches any
//!   other `yields`-declaring function. The ordered replay proof is therefore
//!   over the complete reachable computation, not merely the selected
//!   function's source body.
//!
//! Replay is therefore a *re-use* of an already-computed prefix, not a
//! second, possibly divergent execution -- and this module proves that
//! rather than assuming it. On every resume the replayed prefix recomputes
//! the request and it must equal the request the suspension recorded;
//! disagreement is refused as drift (`SPX-F114`) instead of being papered
//! over. This is the same discipline `resumable_effects::core`'s
//! `RequestDrift` enforces at the Rust reference level, now applied to real
//! `.spx` source.
//!
//! Request equality is not a continuation identity: two argument vectors can
//! compute the same request and different suffix results. Each suspension
//! therefore carries a domain-separated binding over the exact checked HIR,
//! the lowering plan and yield-site identities, and the bit-exact original
//! scalar arguments. Resume refuses a mismatched state or binding with
//! `SPX-F115`. The binding commits to the replay inputs; it grants no effect
//! authority.
//!
//! # Typed resume
//!
//! A resume value is checked against the function's *declared* response type
//! (`yields Request -> Response`) before the program is ever entered, and a
//! mismatch is refused with `SPX-F113`. The recorded request is checked the
//! same way against the declared request type. A resumed computation is
//! therefore checked, not trusted: no interpreter lane can hand a `bool`
//! answer to a suspension that declared `yields i64 -> i64`.
//!
//! # No authority
//!
//! This lane opens no file, spawns no process, and contacts no network: a
//! suspension is proof data about what the program asked for, never
//! permission to satisfy it. Who answers a request, and whether they were
//! entitled to, is the caller's -- `resumable_effects::capability`'s --
//! concern, outside this module entirely.

use crate::conformance::NormalizedStatus;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ExpressionId, ResolvedExpr, ResolvedFunction, ResolvedType, ValueId};
use crate::resumable_effects::lowering::{
    self, ResumableScalar, ResumableStateId, ResumableSuspensionBinding, SequentialResumablePlan,
};
use std::collections::BTreeMap;

use super::prepared::PreparedCancellation;
use super::{
    admitted_resolved_functions, argument_error, option_error, resolved_signature_is_admitted,
    scan_closure, selection_error, ArgumentValue, Environment, Evaluator, Flow, FunctionLookup,
    OwnedRecordValue, OwnedVariantValue, Value, EVALUATION_STACK_BYTES, MAX_STEPS_LIMIT,
    REASON_AUTOMATIC_IDENTITY, REASON_UNSUPPORTED_CALLEE,
};

/// The function named for this lane declares no `yields` clause.
const REASON_NOT_RESUMABLE: &str = "not_a_resumable_effect_function";
/// The function is outside the interpreter's admitted scalar profile.
const REASON_OUTSIDE_PROFILE: &str = "outside_resumable_effect_profile";

/// A resume value's type disagrees with the declared `yields` signature.
const RESUME_TYPE_MISMATCH: &str = "SPX-F113";
/// The replayed prefix recomputed a different request than the suspension
/// recorded. Fails closed: no resumed value is produced.
const REQUEST_DRIFT: &str = "SPX-F114";
/// The caller presented a continuation state or invocation binding that was
/// not produced by this exact checked program and exact argument vector.
const SUSPENSION_MISMATCH: &str = "SPX-F115";

/// The `Flow::Guard` detail a fresh suspension travels on. It never escapes
/// this module: [`evaluate_resumable`] converts it into
/// the legacy or sequential public step using the request parked in
/// [`Resumption`].
pub(super) const SUSPENDED_AT_YIELD: &str = "resumable-effect invocation suspended at its `yield`";
/// The `Flow::Guard` detail every *ordinary* interpreter lane keeps for a
/// `yield`. Those lanes refuse a `yields`-declaring function at admission
/// long before its body is evaluated, so this is an unreachable-in-practice
/// refusal that stays explicit rather than becoming a silent fallthrough.
const YIELD_REFUSED: &str = "`yield` is not yet evaluated by the interpreter";
/// A fresh/replayed invocation reached another site after it had already
/// parked. The sequential lane parks exactly once per call, so this remains a
/// guard rather than an alternate continuation path.
const SECOND_YIELD: &str =
    "a resumable-effect invocation reached a second `yield` after suspension";

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ResumableYieldRecord {
    request: ArgumentValue,
    answer: ArgumentValue,
}

/// An opaque Copy-scalar continuation for a sequential top-level `yield`
/// program. It is proof data only: it grants neither authority to answer a
/// request nor a public continuation ABI. The public ambient-authority-free
/// checkpoint envelope uses a crate-private structural codec to reconstruct this only
/// after independently re-deriving its checked program, site, argument binding,
/// and typed history. Request claims are verified by the ordinary
/// deterministic replay during resume.
#[derive(Clone, Debug, PartialEq)]
pub struct ResumableContinuation {
    state: ResumableStateId,
    binding: ResumableSuspensionBinding,
    request: ArgumentValue,
    history: Vec<ResumableYieldRecord>,
}

impl ResumableContinuation {
    pub fn state(&self) -> &ResumableStateId {
        &self.state
    }

    pub fn binding(&self) -> &ResumableSuspensionBinding {
        &self.binding
    }

    pub fn request(&self) -> &ArgumentValue {
        &self.request
    }

    pub(crate) fn history(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ArgumentValue, &ArgumentValue)> {
        self.history
            .iter()
            .map(|record| (&record.request, &record.answer))
    }
}

/// Issue #296 R20: [`ResumableYieldRecord`] widened to
/// [`ResumableChannelValue`]. Defined here, alongside
/// [`ResumableContinuation`] and its own record type, rather than in
/// `channel.rs`, so every descendant of this module (`channel`, `checkpoint`,
/// and a future durable-journal bridge) can construct and destructure it
/// directly, exactly as they already do for the scalar carrier.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ChannelYieldRecord {
    request: ResumableChannelValue,
    answer: ResumableChannelValue,
}

/// [`ResumableContinuation`]'s own shape and proof-data guarantees, widened
/// to [`ResumableChannelValue`]: it grants no authority to answer a request,
/// and decoding it independently re-derives the checked program, site,
/// argument binding, and typed history rather than trusting stored bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct ResumableChannelContinuation {
    state: ResumableStateId,
    binding: ResumableSuspensionBinding,
    request: ResumableChannelValue,
    history: Vec<ChannelYieldRecord>,
}

impl ResumableChannelContinuation {
    pub fn state(&self) -> &ResumableStateId {
        &self.state
    }

    pub fn binding(&self) -> &ResumableSuspensionBinding {
        &self.binding
    }

    pub fn request(&self) -> &ResumableChannelValue {
        &self.request
    }

    pub(crate) fn history(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ResumableChannelValue, &ResumableChannelValue)> {
        self.history
            .iter()
            .map(|record| (&record.request, &record.answer))
    }
}

/// How one `Evaluator` treats the ordered top-level `yield` sites its function
/// may contain.
pub(super) enum Resumption {
    /// Every ordinary interpreter lane. `yield` is refused outright.
    Refused,
    /// A fresh resumable invocation: the first `yield` parks its request
    /// here and suspends.
    Fresh {
        parked: Option<Value>,
        parked_site: Option<ExpressionId>,
        /// Issue #296, spec section 11.6: the frame's own bindings at the
        /// moment of park, snapshotted so the control lane can read the
        /// owned `Bytes` locals it proves live at this site. Unused by the
        /// legacy and sequential lanes.
        parked_environment: Option<Vec<(ValueId, Value)>>,
    },
    /// A replayed resumable invocation replays every completed request in
    /// order, then consumes one new answer. If another direct site is
    /// reached, it parks it for the next explicit continuation call.
    Replay {
        expected: Vec<Value>,
        answers: Vec<Value>,
        observed: usize,
        parked: Option<Value>,
        history: Vec<ResumableYieldRecord>,
        /// Control lane only: the exact site of each replayed suspension.
        sites: Option<Vec<ExpressionId>>,
        parked_site: Option<ExpressionId>,
        parked_environment: Option<Vec<(ValueId, Value)>>,
        /// Issue #296, spec section 11.6: the owned `Bytes` locals carried
        /// into this resume, keyed by the exact `let`-bound storage they
        /// substitute. A `Let` statement whose binding identity is a key
        /// here binds the carried value directly and removes the entry,
        /// rather than re-evaluating (and so recomputing) its own value
        /// expression; every other lane's map is empty and every ordinary
        /// `Let` is unaffected.
        carried: BTreeMap<ValueId, Value>,
    },
}

enum ResumeInput {
    Legacy {
        state: ResumableStateId,
        binding: ResumableSuspensionBinding,
        request: ArgumentValue,
        answer: ArgumentValue,
    },
    Sequential {
        continuation: ResumableContinuation,
        answer: ArgumentValue,
    },
}

impl Evaluator<'_> {
    /// A `let` statement's value: a control-lane resume that carries this
    /// exact `let`-bound owned `Bytes` value (issue #296, spec section 11.6)
    /// substitutes it directly instead of recomputing (and so
    /// reallocating) its defining expression, and removes it from the map
    /// so a later dynamic occurrence of the same static binding (inside a
    /// loop body) still evaluates normally. Every other lane's map is
    /// always empty, so this is a no-op there.
    pub(super) fn resolve_let_value(
        &mut self,
        binding_id: &hir::ValueId,
        value: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        let carried = match &mut self.resumption {
            Resumption::Replay { carried, .. } => carried.remove(binding_id),
            _ => None,
        };
        match carried {
            Some(value) => Ok(value),
            None => self.evaluate(value, environment, depth),
        }
    }
}

/// The ordered `yield` sites' whole runtime behaviour, in one place.
pub(super) fn settle_yield(
    state: &mut Resumption,
    site: &ExpressionId,
    request: Value,
    environment: &Environment,
) -> Result<Value, Flow> {
    match state {
        Resumption::Refused => Err(Flow::Guard(YIELD_REFUSED)),
        Resumption::Fresh {
            parked,
            parked_site,
            parked_environment,
        } => {
            if parked.is_some() {
                return Err(Flow::Guard(SECOND_YIELD));
            }
            *parked = Some(request);
            *parked_site = Some(site.clone());
            *parked_environment = Some(snapshot_environment(environment));
            Err(Flow::Guard(SUSPENDED_AT_YIELD))
        }
        Resumption::Replay {
            expected,
            answers,
            observed,
            parked,
            sites,
            parked_site,
            parked_environment,
            ..
        } => {
            if *observed == expected.len() {
                if parked.is_some() {
                    return Err(Flow::Guard(SECOND_YIELD));
                }
                *parked = Some(request);
                *parked_site = Some(site.clone());
                *parked_environment = Some(snapshot_environment(environment));
                return Err(Flow::Guard(SUSPENDED_AT_YIELD));
            }
            // Control lane: replay must reach exactly the recorded site.
            if sites
                .as_ref()
                .is_some_and(|sites| sites.get(*observed) != Some(site))
            {
                return Err(Flow::Guard(REQUEST_DRIFT));
            }
            let Some(expected) = expected.get(*observed) else {
                return Err(Flow::Guard(SECOND_YIELD));
            };
            if !scalar_values_equal(&request, expected) {
                return Err(Flow::Guard(REQUEST_DRIFT));
            }
            let Some(answer) = answers.get(*observed) else {
                return Err(Flow::Guard(SECOND_YIELD));
            };
            *observed += 1;
            clone_scalar(answer).ok_or(Flow::Guard("resume answer is not an admitted scalar"))
        }
    }
}

/// One settled outcome of starting or resuming a resumable-effect function.
#[derive(Clone, Debug, PartialEq)]
pub enum ResumableStep {
    /// The function reached its `yield` and produced this request. Nothing
    /// was dispatched: the caller owns deciding whether and how to answer.
    Suspended {
        state: ResumableStateId,
        binding: ResumableSuspensionBinding,
        request: ArgumentValue,
    },
    /// The function ran to its result -- on a resume, past the yield site.
    Completed {
        state: ResumableStateId,
        result: ArgumentValue,
    },
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    /// An impossible state after the caller's HIR validation.
    GuardError(String),
}

/// Deterministic facts from one resumable-effect start or resume.
#[derive(Clone, Debug, PartialEq)]
pub struct ResumableEvaluation {
    pub step: ResumableStep,
    pub steps_used: usize,
    pub max_steps: usize,
}

/// One settled outcome from the explicitly multi-site resumable lane.
#[derive(Clone, Debug, PartialEq)]
pub enum SequentialResumableStep {
    /// The function reached its next direct top-level `yield`.
    Suspended {
        continuation: ResumableContinuation,
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

/// Deterministic facts from one explicitly multi-site start or resume.
#[derive(Clone, Debug, PartialEq)]
pub struct SequentialResumableEvaluation {
    pub step: SequentialResumableStep,
    pub steps_used: usize,
    pub max_steps: usize,
}

#[derive(Clone, Debug, PartialEq)]
enum EvaluatedStep {
    Suspended {
        state: ResumableStateId,
        binding: ResumableSuspensionBinding,
        request: ArgumentValue,
    },
    SequentialSuspended {
        continuation: ResumableContinuation,
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

struct Evaluated {
    step: EvaluatedStep,
    steps_used: usize,
    max_steps: usize,
}

/// Run `function_id` until its single suspension, or to completion.
pub fn run_resumable_effect(
    program: &hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    max_steps: usize,
) -> Result<ResumableEvaluation, Vec<Diagnostic>> {
    into_legacy(evaluate_resumable(
        program,
        function_id,
        arguments,
        None,
        false,
        max_steps,
    )?)
}

/// Resume a suspension of `function_id` by replaying its prefix under the
/// exact `arguments` that produced it and substituting `answer` at the yield
/// site.
///
/// `request` is the request the suspension recorded. It is not trusted: the
/// replayed prefix recomputes its own request and the two must agree
/// (`SPX-F114`). `answer` must have the declared response type
/// (`SPX-F113`). `state` and `binding` must identify this exact checked
/// program, yield site, and bit-exact original argument vector (`SPX-F115`).
pub fn resume_resumable_effect(
    program: &hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    state: &ResumableStateId,
    binding: &ResumableSuspensionBinding,
    request: &ArgumentValue,
    answer: &ArgumentValue,
    max_steps: usize,
) -> Result<ResumableEvaluation, Vec<Diagnostic>> {
    into_legacy(evaluate_resumable(
        program,
        function_id,
        arguments,
        Some(ResumeInput::Legacy {
            state: state.clone(),
            binding: binding.clone(),
            request: request.clone(),
            answer: answer.clone(),
        }),
        false,
        max_steps,
    )?)
}

/// Run a function with multiple direct sequential `yield` sites until its
/// first suspension. The legacy one-site API remains source-compatible and
/// deliberately refuses this lane.
pub fn run_sequential_resumable_effect(
    program: &hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    max_steps: usize,
) -> Result<SequentialResumableEvaluation, Vec<Diagnostic>> {
    Ok(into_sequential(evaluate_resumable(
        program,
        function_id,
        arguments,
        None,
        true,
        max_steps,
    )?))
}

/// Resume an opaque sequential continuation. It supplies the previous
/// request/answer trace solely so replay can re-check it under the exact
/// current program. The ambient-authority-free public checkpoint envelope can recover
/// this carrier, but recovery itself never supplies an answer or dispatches.
pub fn resume_sequential_resumable_effect(
    program: &hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    continuation: &ResumableContinuation,
    answer: &ArgumentValue,
    max_steps: usize,
) -> Result<SequentialResumableEvaluation, Vec<Diagnostic>> {
    Ok(into_sequential(evaluate_resumable(
        program,
        function_id,
        arguments,
        Some(ResumeInput::Sequential {
            continuation: continuation.clone(),
            answer: answer.clone(),
        }),
        true,
        max_steps,
    )?))
}

fn evaluate_resumable(
    program: &hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    resume: Option<ResumeInput>,
    sequential: bool,
    max_steps: usize,
) -> Result<Evaluated, Vec<Diagnostic>> {
    let Admitted {
        entry,
        yields,
        bound,
        admitted,
    } = admit_entry(program, function_id, arguments, max_steps)?;
    let plan = lowering::lower_sequential(program, entry).map_err(|error| vec![error])?;
    if sequential == (plan.suspensions.len() == 1) {
        let detail = if sequential {
            format!(
                "sequential resumable entry `{function_id}` has only one yield site; use the legacy one-site API"
            )
        } else {
            format!(
                "resumable entry `{function_id}` has multiple yield sites; use the explicit sequential API"
            )
        };
        return Err(vec![Diagnostic::io(SUSPENSION_MISMATCH, detail)]);
    }
    let scalar_arguments = resumable_scalars(arguments).ok_or_else(|| {
        vec![argument_error(
            "resumable invocation contains a non-scalar argument".to_owned(),
        )]
    })?;
    let (resumption, next_binding) = match resume {
        None => (
            Resumption::Fresh {
                parked: None,
                parked_site: None,
                parked_environment: None,
            },
            Some(plan.suspension_binding(&scalar_arguments)),
        ),
        Some(ResumeInput::Legacy {
            state,
            binding,
            request,
            answer,
        }) => {
            if plan.suspensions.len() != 1 {
                return Err(vec![Diagnostic::io(
                    SUSPENSION_MISMATCH,
                    format!(
                        "resuming `{function_id}` has multiple yield sites; use its opaque sequential continuation"
                    ),
                )]);
            }
            let expected_binding = plan.suspension_binding(&scalar_arguments);
            if state != plan.suspensions[0].state.id || binding != expected_binding {
                return Err(vec![Diagnostic::io(
                    SUSPENSION_MISMATCH,
                    format!(
                        "resuming `{function_id}` presented a suspension state or invocation binding that does not match this exact checked program, yield site, and argument vector"
                    ),
                )]);
            }
            let request = typed_resume_value(&yields.request_type, &request, "request")?;
            let answer_value = typed_resume_value(&yields.response_type, &answer, "answer")?;
            (
                Resumption::Replay {
                    expected: vec![request],
                    answers: vec![answer_value],
                    observed: 0,
                    parked: None,
                    history: Vec::new(),
                    sites: None,
                    parked_site: None,
                    parked_environment: None,
                    carried: BTreeMap::new(),
                },
                None,
            )
        }
        Some(ResumeInput::Sequential {
            continuation,
            answer,
        }) => {
            let Some(index) = plan.suspension_index(&continuation.state) else {
                return Err(vec![Diagnostic::io(
                    SUSPENSION_MISMATCH,
                    "sequential continuation state does not belong to this exact checked plan",
                )]);
            };
            if plan.suspensions.len() == 1 || continuation.history.len() != index {
                return Err(vec![Diagnostic::io(
                    SUSPENSION_MISMATCH,
                    "sequential continuation history length does not match its suspension state",
                )]);
            }
            let prior_answers = continuation
                .history
                .iter()
                .map(|record| record.answer.clone())
                .collect::<Vec<_>>();
            let prior_scalars = resumable_scalars(&prior_answers).ok_or_else(|| {
                vec![Diagnostic::io(
                    SUSPENSION_MISMATCH,
                    "sequential continuation carries a non-scalar answer history",
                )]
            })?;
            let expected_binding = plan
                .suspension_binding_at(index, &scalar_arguments, &prior_scalars)
                .map_err(|error| vec![error])?;
            if continuation.binding != expected_binding {
                return Err(vec![Diagnostic::io(
                    SUSPENSION_MISMATCH,
                    "sequential continuation binding does not match this exact program, site, arguments, and prior answer bits",
                )]);
            }
            let mut expected = Vec::with_capacity(index + 1);
            let mut answers = Vec::with_capacity(index + 1);
            for record in &continuation.history {
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
            }
            expected.push(typed_resume_value(
                &yields.request_type,
                &continuation.request,
                "request",
            )?);
            answers.push(typed_resume_value(
                &yields.response_type,
                &answer,
                "answer",
            )?);
            let mut history = continuation.history.clone();
            history.push(ResumableYieldRecord {
                request: continuation.request,
                answer,
            });
            let next_index = index + 1;
            let next_binding = plan
                .suspensions
                .get(next_index)
                .map(|_| {
                    let mut answer_scalars = prior_scalars;
                    answer_scalars.push(
                        resumable_scalars(&[history.last().expect("just appended").answer.clone()])
                            .expect("typed sequential answer is scalar")
                            .pop()
                            .expect("one scalar answer"),
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
                    history,
                    sites: None,
                    parked_site: None,
                    parked_environment: None,
                    carried: BTreeMap::new(),
                },
                next_binding,
            )
        }
    };
    let (step, steps_used) = run_worker(
        program,
        &admitted,
        entry,
        &bound,
        resumption,
        max_steps,
        |settled, resumption| settle_step(settled, resumption, &plan, next_binding),
    )?;
    let evaluated = Evaluated {
        step,
        steps_used,
        max_steps,
    };

    if evaluated.step == EvaluatedStep::GuardError(REQUEST_DRIFT.to_owned()) {
        return Err(vec![Diagnostic::io(
            REQUEST_DRIFT,
            format!(
                "resuming `{function_id}` replayed its prefix and recomputed a different request \
                 than the suspension recorded; the resume is refused rather than answered"
            ),
        )]);
    }
    Ok(evaluated)
}

/// The exact admitted entry facts every resumable lane shares.
pub(super) struct Admitted<'p> {
    pub(super) entry: &'p ResolvedFunction,
    pub(super) yields: &'p hir::ResolvedYieldsClause,
    pub(super) bound: Vec<(String, ArgumentValue)>,
    pub(super) admitted: std::collections::BTreeMap<&'p str, &'p ResolvedFunction>,
}

/// Select, identity-check and bind a resumable entry before any lowering.
pub(super) fn admit_entry<'p>(
    program: &'p hir::ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    max_steps: usize,
) -> Result<Admitted<'p>, Vec<Diagnostic>> {
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
    let yields = entry.yields.as_ref().ok_or_else(|| {
        vec![selection_error(
            REASON_NOT_RESUMABLE,
            format!("`{function_id}` declares no `yields` clause; this lane runs only functions that can suspend"),
        )]
    })?;
    if !resolved_signature_is_admitted(entry, &program.declarations) {
        return Err(vec![selection_error(
            REASON_OUTSIDE_PROFILE,
            format!("resumable entry `{function_id}` is outside the interpreter profile"),
        )]);
    }
    let bound = bind_scalar_arguments(entry, arguments)?;
    let admitted = admitted_resolved_functions(program);
    scan_closure(function_id, &admitted, program)?;
    hir::validate(program).map_err(|error| vec![error])?;
    Ok(Admitted {
        entry,
        yields,
        bound,
        admitted,
    })
}

pub(super) fn max_byte_allocation(value: &Value) -> u32 {
    match value {
        Value::Bytes(bytes) => bytes.allocation,
        Value::Record(record) => record
            .fields
            .values()
            .map(max_byte_allocation)
            .max()
            .unwrap_or(0),
        Value::Variant(variant) => variant
            .fields
            .values()
            .map(max_byte_allocation)
            .max()
            .unwrap_or(0),
        _ => 0,
    }
}

/// Evaluate one segment on the bounded-stack worker and settle it there.
pub(super) fn run_worker<T: Send>(
    program: &hir::ResolvedProgram,
    admitted: &std::collections::BTreeMap<&str, &ResolvedFunction>,
    entry: &ResolvedFunction,
    bound: &[(String, ArgumentValue)],
    resumption: Resumption,
    max_steps: usize,
    settle: impl FnOnce(Result<Value, Flow>, &mut Resumption) -> T + Send,
) -> Result<(T, usize), Vec<Diagnostic>> {
    let closure_functions =
        super::closures::checked_functions(program).map_err(|error| vec![error])?;

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
                // Issue #296, spec section 11.6: a carried owned `Bytes`
                // value is injected directly (never evaluated by an
                // allocating operation this evaluator's own counter would
                // charge), so the counter starts pre-charged for exactly as
                // many carried values as this resume substitutes. Every
                // other lane's `carried` map is empty and this is a no-op.
                if let Resumption::Replay {
                    carried,
                    expected,
                    answers,
                    ..
                } = &resumption
                {
                    evaluator.next_byte_allocation = carried
                        .values()
                        .chain(expected.iter())
                        .chain(answers.iter())
                        .map(max_byte_allocation)
                        .max()
                        .unwrap_or(0);
                }
                evaluator.resumption = resumption;
                let settled = evaluator.evaluate_entry(entry, bound);
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

fn into_legacy(evaluated: Evaluated) -> Result<ResumableEvaluation, Vec<Diagnostic>> {
    let step = match evaluated.step {
        EvaluatedStep::Suspended {
            state,
            binding,
            request,
        } => ResumableStep::Suspended {
            state,
            binding,
            request,
        },
        EvaluatedStep::Completed { state, result } => ResumableStep::Completed { state, result },
        EvaluatedStep::LanguageFailure(status) => ResumableStep::LanguageFailure(status),
        EvaluatedStep::FuelExhausted => ResumableStep::FuelExhausted,
        EvaluatedStep::CallDepthExceeded => ResumableStep::CallDepthExceeded,
        EvaluatedStep::GuardError(detail) => ResumableStep::GuardError(detail),
        EvaluatedStep::SequentialSuspended { .. } => {
            return Err(vec![Diagnostic::io(
                SUSPENSION_MISMATCH,
                "a sequential suspension escaped through the legacy one-site API",
            )]);
        }
    };
    Ok(ResumableEvaluation {
        step,
        steps_used: evaluated.steps_used,
        max_steps: evaluated.max_steps,
    })
}

fn into_sequential(evaluated: Evaluated) -> SequentialResumableEvaluation {
    let step = match evaluated.step {
        EvaluatedStep::SequentialSuspended { continuation } => {
            SequentialResumableStep::Suspended { continuation }
        }
        EvaluatedStep::Completed { state, result } => {
            SequentialResumableStep::Completed { state, result }
        }
        EvaluatedStep::LanguageFailure(status) => SequentialResumableStep::LanguageFailure(status),
        EvaluatedStep::FuelExhausted => SequentialResumableStep::FuelExhausted,
        EvaluatedStep::CallDepthExceeded => SequentialResumableStep::CallDepthExceeded,
        EvaluatedStep::GuardError(detail) => SequentialResumableStep::GuardError(detail),
        EvaluatedStep::Suspended { .. } => SequentialResumableStep::GuardError(
            "a legacy one-site suspension escaped through the sequential API".to_owned(),
        ),
    };
    SequentialResumableEvaluation {
        step,
        steps_used: evaluated.steps_used,
        max_steps: evaluated.max_steps,
    }
}

/// Turn the evaluator's settled `Result` into one closed step, reading the
/// parked request for the suspension case.
fn settle_step(
    settled: Result<Value, Flow>,
    resumption: &mut Resumption,
    plan: &SequentialResumablePlan,
    binding: Option<ResumableSuspensionBinding>,
) -> EvaluatedStep {
    match settled {
        Ok(value) => match argument_of(&value) {
            Some(result) => EvaluatedStep::Completed {
                state: plan.complete.id.clone(),
                result,
            },
            None => EvaluatedStep::GuardError(
                "resumable-effect entry returned a non-scalar value".to_owned(),
            ),
        },
        Err(Flow::Guard(SUSPENDED_AT_YIELD)) => {
            let Some(binding) = binding else {
                return EvaluatedStep::GuardError(
                    "a suspension escaped without its exact invocation binding".to_owned(),
                );
            };
            match resumption {
                Resumption::Fresh {
                    parked: Some(request),
                    ..
                } => {
                    let Some(request) = argument_of(request) else {
                        return EvaluatedStep::GuardError(
                            "`yield` produced a non-scalar request".to_owned(),
                        );
                    };
                    if plan.suspensions.len() == 1 {
                        EvaluatedStep::Suspended {
                            state: plan.suspensions[0].state.id.clone(),
                            binding,
                            request,
                        }
                    } else {
                        EvaluatedStep::SequentialSuspended {
                            continuation: ResumableContinuation {
                                state: plan.suspensions[0].state.id.clone(),
                                binding,
                                request,
                                history: Vec::new(),
                            },
                        }
                    }
                }
                Resumption::Replay {
                    parked: Some(request),
                    history,
                    ..
                } => {
                    let index = history.len();
                    let Some(suspension) = plan.suspensions.get(index) else {
                        return EvaluatedStep::GuardError(
                            "a replay parked beyond the plan's final suspension".to_owned(),
                        );
                    };
                    let Some(request) = argument_of(request) else {
                        return EvaluatedStep::GuardError(
                            "`yield` produced a non-scalar request".to_owned(),
                        );
                    };
                    EvaluatedStep::SequentialSuspended {
                        continuation: ResumableContinuation {
                            state: suspension.state.id.clone(),
                            binding,
                            request,
                            history: history.clone(),
                        },
                    }
                }
                _ => EvaluatedStep::GuardError(
                    "a suspension escaped without parking its request".to_owned(),
                ),
            }
        }
        Err(Flow::Failure(status)) => EvaluatedStep::LanguageFailure(status),
        Err(Flow::Exhausted) => EvaluatedStep::FuelExhausted,
        Err(Flow::DepthExceeded) => EvaluatedStep::CallDepthExceeded,
        Err(Flow::Guard(detail)) => EvaluatedStep::GuardError(detail.to_owned()),
        Err(Flow::Residual(_)) => EvaluatedStep::GuardError(
            "owned postfix `?` residual escaped its function frame".to_owned(),
        ),
        Err(Flow::Cancelled { .. }) => {
            EvaluatedStep::GuardError("unexpected cancellation in resumable evaluation".to_owned())
        }
        Err(Flow::Utf8MaterializationLimitExceeded { .. }) => EvaluatedStep::GuardError(
            "unexpected UTF-8 materialization limit in resumable evaluation".to_owned(),
        ),
    }
}

fn resumable_scalars(arguments: &[ArgumentValue]) -> Option<Vec<ResumableScalar>> {
    arguments.iter().map(resumable_scalar_of).collect()
}

/// The [`ResumableScalar`] one admitted Copy-scalar `ArgumentValue` denotes,
/// or `None` for a borrowed view the resumable profile never admits.
fn resumable_scalar_of(argument: &ArgumentValue) -> Option<ResumableScalar> {
    Some(match argument {
        ArgumentValue::Int(value) => ResumableScalar::I64(*value),
        ArgumentValue::Int32(value) => ResumableScalar::I32(*value),
        ArgumentValue::Uint8(value) => ResumableScalar::U8(*value),
        ArgumentValue::Usize(value) => ResumableScalar::Usize(*value),
        ArgumentValue::Char(value) => ResumableScalar::Char(*value),
        ArgumentValue::Float32(value) => ResumableScalar::F32(value.to_bits()),
        ArgumentValue::Float64(value) => ResumableScalar::F64(value.to_bits()),
        ArgumentValue::Bool(value) => ResumableScalar::Bool(*value),
        _ => return None,
    })
}

/// Bind the caller's arguments positionally, refusing an arity or type
/// disagreement with the declared parameters before anything runs.
fn bind_scalar_arguments(
    entry: &ResolvedFunction,
    arguments: &[ArgumentValue],
) -> Result<Vec<(String, ArgumentValue)>, Vec<Diagnostic>> {
    if entry.params.len() != arguments.len() {
        return Err(vec![argument_error(format!(
            "`{}` takes {} argument(s); {} were supplied",
            entry.name,
            entry.params.len(),
            arguments.len()
        ))]);
    }
    let mut bound = Vec::with_capacity(arguments.len());
    for (index, (param, argument)) in entry.params.iter().zip(arguments.iter()).enumerate() {
        if scalar_of(&param.ty, argument).is_none() {
            return Err(vec![argument_error(format!(
                "argument {index} of `{}` is not an admitted scalar of the declared parameter type",
                entry.name
            ))]);
        }
        bound.push((param.id.as_str().to_owned(), argument.clone()));
    }
    Ok(bound)
}

/// Check one resume value against its declared type before the program runs.
fn typed_resume_value(
    declared: &ResolvedType,
    supplied: &ArgumentValue,
    role: &str,
) -> Result<Value, Vec<Diagnostic>> {
    scalar_of(declared, supplied).ok_or_else(|| {
        vec![Diagnostic::io(
            RESUME_TYPE_MISMATCH,
            format!(
                "resume {role} `{}` does not have the declared `yields` {role} type",
                supplied.type_text()
            ),
        )]
    })
}

/// The exact scalar `Value` an `ArgumentValue` denotes at `declared`, or
/// `None` when the two disagree. Deliberately total and exact: no widening,
/// no coercion, no signed/unsigned reinterpretation.
fn scalar_of(declared: &ResolvedType, supplied: &ArgumentValue) -> Option<Value> {
    Some(match (declared, supplied) {
        (ResolvedType::I64, ArgumentValue::Int(value)) => Value::Int(*value),
        (ResolvedType::I32, ArgumentValue::Int32(value)) => Value::Int32(*value),
        (ResolvedType::U8, ArgumentValue::Uint8(value)) => Value::Uint8(*value),
        (ResolvedType::Usize, ArgumentValue::Usize(value)) => Value::Usize(*value),
        (ResolvedType::Char, ArgumentValue::Char(value)) => Value::Char(*value),
        (ResolvedType::F32, ArgumentValue::Float32(value)) => Value::Float32(*value),
        (ResolvedType::F64, ArgumentValue::Float64(value)) => Value::Float64(*value),
        (ResolvedType::Bool, ArgumentValue::Bool(value)) => Value::Bool(*value),
        _ => return None,
    })
}

/// The `ArgumentValue` one scalar `Value` denotes. `None` for every
/// non-scalar carrier, which the admitted slice forecloses.
fn argument_of(value: &Value) -> Option<ArgumentValue> {
    Some(match value {
        Value::Int(inner) => ArgumentValue::Int(*inner),
        Value::Int32(inner) => ArgumentValue::Int32(*inner),
        Value::Uint8(inner) => ArgumentValue::Uint8(*inner),
        Value::Usize(inner) => ArgumentValue::Usize(*inner),
        Value::Char(inner) => ArgumentValue::Char(*inner),
        Value::Float32(inner) => ArgumentValue::Float32(*inner),
        Value::Float64(inner) => ArgumentValue::Float64(*inner),
        Value::Bool(inner) => ArgumentValue::Bool(*inner),
        _ => return None,
    })
}

/// Issue #296 R20: a record or variant channel value clones the same way
/// [`super::Evaluator::clone_value`] already does for any other reachable
/// aggregate alias -- an `Arc::clone`, never a deep field-by-field copy.
/// This is sound only because the admitted bounded-aggregate shape is always
/// fully Copy (every field is itself a Copy scalar, never an owned or
/// borrowed carrier), exactly like the scalars this function already
/// cloned, so no unique-ownership or borrow invariant is ever shared this
/// way.
fn clone_scalar(value: &Value) -> Option<Value> {
    match value {
        Value::Record(record) => Some(Value::Record(std::sync::Arc::clone(record))),
        Value::Variant(variant) => Some(Value::Variant(std::sync::Arc::clone(variant))),
        other => argument_of(other).and_then(|argument| match argument {
            ArgumentValue::Int(inner) => Some(Value::Int(inner)),
            ArgumentValue::Int32(inner) => Some(Value::Int32(inner)),
            ArgumentValue::Uint8(inner) => Some(Value::Uint8(inner)),
            ArgumentValue::Usize(inner) => Some(Value::Usize(inner)),
            ArgumentValue::Char(inner) => Some(Value::Char(inner)),
            ArgumentValue::Float32(inner) => Some(Value::Float32(inner)),
            ArgumentValue::Float64(inner) => Some(Value::Float64(inner)),
            ArgumentValue::Bool(inner) => Some(Value::Bool(inner)),
            _ => None,
        }),
    }
}

/// Issue #296, spec section 11.6: every binding of `environment` this
/// admitted profile can hold -- an admitted Copy scalar, or (only in the
/// control lane's owned-Bytes profile) an owned `Bytes` value -- cloned into
/// an owned snapshot at the moment of park. A binding this cannot clone is
/// simply absent from the snapshot rather than a hard failure: the admitted
/// profile never puts one there, so an absent entry only ever means "not
/// carried", never a wrong or missing carried value.
fn snapshot_environment(environment: &Environment) -> Vec<(ValueId, Value)> {
    environment
        .iter()
        .filter_map(|(id, value)| clone_environment_value(value).map(|value| (id.clone(), value)))
        .collect()
}

fn clone_environment_value(value: &Value) -> Option<Value> {
    match value {
        Value::Bytes(inner) => Some(Value::Bytes(inner.clone())),
        other => clone_scalar(other),
    }
}

/// Exact scalar identity for the drift check. Floats compare by bits, not by
/// IEEE equality, so a replayed `NaN` request agrees with the recorded one
/// and `-0.0` never silently passes for `0.0`. Issue #296 R20: a record or
/// variant channel value recurses field by field in canonical declared
/// order, still by bits for any nested float, rather than falling through to
/// `Value`'s own derived `PartialEq` (which would compare a nested `NaN`
/// field by IEEE equality and spuriously report drift for a bit-identical
/// replay).
fn scalar_values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Float32(left), Value::Float32(right)) => left.to_bits() == right.to_bits(),
        (Value::Float64(left), Value::Float64(right)) => left.to_bits() == right.to_bits(),
        (Value::Bytes(left), Value::Bytes(right)) => left.bytes == right.bytes,
        (Value::Record(left), Value::Record(right)) => {
            left.record == right.record
                && left.fields.len() == right.fields.len()
                && left.fields.iter().zip(right.fields.iter()).all(
                    |((left_id, left), (right_id, right))| {
                        left_id == right_id && scalar_values_equal(left, right)
                    },
                )
        }
        (Value::Variant(left), Value::Variant(right)) => {
            left.ty == right.ty
                && left.variant == right.variant
                && left.case == right.case
                && left.fields.len() == right.fields.len()
                && left.fields.iter().zip(right.fields.iter()).all(
                    |((left_id, left), (right_id, right))| {
                        left_id == right_id && scalar_values_equal(left, right)
                    },
                )
        }
        (left, right) => argument_of(left).is_some() && left == right,
    }
}

/// One bounded Copy-scalar `yields` request or response value (issue #296
/// R20): either a bare admitted scalar, or a value of the shape
/// `hir::yield_aggregate::bounded_aggregate_refusal` admits -- a record's
/// own fields, or a variant's selected case's own fields, in canonical
/// declared field order, one level deep by construction. `channel.rs`'s
/// sequential-channel evaluation is the only lane that ever produces or
/// consumes `Record`/`Variant`; every other lane's request/answer stays
/// `Scalar` (an admitted `ArgumentValue`), the exact profile this type
/// otherwise matches.
#[derive(Clone, Debug, PartialEq)]
pub enum ResumableChannelValue {
    Scalar(ArgumentValue),
    Record {
        declaration: hir::DeclarationId,
        fields: Vec<ArgumentValue>,
    },
    Variant {
        declaration: hir::DeclarationId,
        case: hir::DeclarationId,
        fields: Vec<ArgumentValue>,
    },
    RecordBytes {
        declaration: hir::DeclarationId,
        fields: Vec<channel_bytes::ChannelField>,
    },
    VariantBytes {
        declaration: hir::DeclarationId,
        case: hir::DeclarationId,
        fields: Vec<channel_bytes::ChannelField>,
    },
}

pub use channel_bytes::ChannelField;

impl From<ArgumentValue> for ResumableChannelValue {
    fn from(value: ArgumentValue) -> Self {
        Self::Scalar(value)
    }
}

/// The [`ResumableChannelValue`] a runtime `Value` denotes, in the
/// declaration index's own canonical field order. `None` for a carrier this
/// admitted profile never puts at a yield boundary (a borrow, a resource, an
/// owned `Bytes`/`Vec`/`Box`, or a field that is itself one of those).
pub(super) fn channel_of(
    declarations: &hir::DeclarationIndex,
    value: &Value,
) -> Option<ResumableChannelValue> {
    match value {
        Value::Record(_) | Value::Variant(_) => channel_bytes::channel_of(declarations, value),
        /*Value::Record(record) => {
            let canonical = declarations.record_fields(&record.record)?;
            let fields = canonical
                .iter()
                .map(|field| record.fields.get(&field.id).and_then(argument_of))
                .collect::<Option<Vec<_>>>()?;
            Some(ResumableChannelValue::Record {
                declaration: record.record.clone(),
                fields,
            })
        }
        Value::Variant(variant) => {
            let canonical = declarations.case_fields(&variant.case)?;
            let fields = canonical
                .iter()
                .map(|field| variant.fields.get(&field.id).and_then(argument_of))
                .collect::<Option<Vec<_>>>()?;
            Some(ResumableChannelValue::Variant {
                declaration: variant.variant.clone(),
                case: variant.case.clone(),
                fields,
            })
        }*/
        other => argument_of(other).map(ResumableChannelValue::Scalar),
    }
}

/// The exact runtime `Value` a [`ResumableChannelValue`] denotes at
/// `declared`, or `None` when the two disagree: a wrong declaration or case
/// identity, a field-count mismatch, or a field outside the admitted scalar
/// profile. Deliberately total and exact, mirroring [`scalar_of`]: no
/// coercion, and the declared type's own identity -- not merely its field
/// count -- must match, so a channel value manufactured for one nominal type
/// can never be replayed in as another.
pub(super) fn value_of_channel(
    declarations: &hir::DeclarationIndex,
    declared: &ResolvedType,
    supplied: &ResumableChannelValue,
    allocation: &mut u32,
) -> Option<Value> {
    match supplied {
        ResumableChannelValue::Scalar(argument) => scalar_of(declared, argument),
        ResumableChannelValue::Record {
            declaration,
            fields,
        } => {
            let ResolvedType::Nominal {
                declaration: expected,
                arguments,
            } = declared
            else {
                return None;
            };
            if expected != declaration || !arguments.is_empty() {
                return None;
            }
            let canonical = declarations.record_fields(declaration)?;
            if canonical.len() != fields.len() {
                return None;
            }
            let mut built = std::collections::BTreeMap::new();
            for (field, argument) in canonical.iter().zip(fields) {
                built.insert(field.id.clone(), scalar_of(&field.ty, argument)?);
            }
            Some(Value::Record(std::sync::Arc::new(OwnedRecordValue {
                record: declaration.clone(),
                fields: built,
            })))
        }
        ResumableChannelValue::Variant {
            declaration,
            case,
            fields,
        } => {
            let ResolvedType::Nominal {
                declaration: expected,
                arguments,
            } = declared
            else {
                return None;
            };
            if expected != declaration || !arguments.is_empty() {
                return None;
            }
            let cases = declarations.variant_cases(declaration)?;
            if !cases.iter().any(|candidate| &candidate.id == case) {
                return None;
            }
            let canonical = declarations.case_fields(case)?;
            if canonical.len() != fields.len() {
                return None;
            }
            let mut built = std::collections::BTreeMap::new();
            for (field, argument) in canonical.iter().zip(fields) {
                built.insert(field.id.clone(), scalar_of(&field.ty, argument)?);
            }
            Some(Value::Variant(std::sync::Arc::new(OwnedVariantValue {
                ty: declared.clone(),
                variant: declaration.clone(),
                case: case.clone(),
                fields: built,
            })))
        }
        owned @ (ResumableChannelValue::RecordBytes { .. }
        | ResumableChannelValue::VariantBytes { .. }) => {
            channel_bytes::value_of(declarations, declared, owned, allocation)
        }
    }
}

/// The [`ResumableScalar`] one [`ResumableChannelValue`] denotes, for the
/// suspension binding hash. `None` when a field is outside the admitted
/// scalar profile (unreachable for a value this module itself produced, but
/// never assumed).
fn channel_to_resumable_scalar(value: &ResumableChannelValue) -> Option<ResumableScalar> {
    Some(match value {
        ResumableChannelValue::Scalar(scalar) => resumable_scalar_of(scalar)?,
        ResumableChannelValue::Record { fields, .. } => ResumableScalar::Record(
            fields
                .iter()
                .map(resumable_scalar_of)
                .collect::<Option<Vec<_>>>()?,
        ),
        ResumableChannelValue::Variant { case, fields, .. } => ResumableScalar::Variant {
            case: case.as_str().to_owned(),
            fields: fields
                .iter()
                .map(resumable_scalar_of)
                .collect::<Option<Vec<_>>>()?,
        },
        owned @ (ResumableChannelValue::RecordBytes { .. }
        | ResumableChannelValue::VariantBytes { .. }) => channel_bytes::binding(owned)?,
    })
}

/// Check one resume channel value against its declared type before the
/// program runs. Mirrors [`typed_resume_value`], widened to
/// [`ResumableChannelValue`].
fn typed_resume_channel_value(
    declarations: &hir::DeclarationIndex,
    declared: &ResolvedType,
    supplied: &ResumableChannelValue,
    role: &str,
    allocation: &mut u32,
) -> Result<Value, Vec<Diagnostic>> {
    value_of_channel(declarations, declared, supplied, allocation).ok_or_else(|| {
        vec![Diagnostic::io(
            RESUME_TYPE_MISMATCH,
            format!("resume {role} does not have the declared `yields` {role} type"),
        )]
    })
}

pub mod channel;
pub(crate) mod channel_bytes;
/// Inner closed recovery bytes for the admitted scalar sequential lane. The
/// public scoped envelope is `resumable_effects::source_checkpoint`; keeping
/// this structural layer crate-private prevents bypassing its external scope.
pub(crate) mod checkpoint;
pub mod control;
pub(crate) mod owned_frame;
#[cfg(test)]
mod tests;
