//! Fresh, synchronous driving of the admitted checked-source scalar lane.
//!
//! The caller injects both the handler and an explicit capability policy. The
//! selected function's persistent identity is the capability to authorize.
//! The compiler derives the channel signature; handlers cannot supply tags.
//! Each authored suspension dispatches at most once within this invocation.
//! Returned evidence is inert and is not accepted as recovery or retry input.
//! This API makes no crash-recovery or cross-invocation exactly-once claim.
//!
//! Cancellation is cooperative at evaluation and dispatch boundaries. Fuel
//! counts actual interpreter work, including pure-prefix replay. Handler work
//! itself must be bounded by its injector; interpreter fuel cannot preempt it.

use super::capability::CapabilityPolicy;
use super::core::EffectHandler;
pub use super::lowering::ResumableScalar;
use super::signature::EffectTag;
use super::source_signature::{
    derive_source_effect_signature, SourceEffectSignature, SOURCE_TYPE_SHAPE_PREFIX,
};
use crate::conformance::NormalizedStatus;
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedProgram, ResolvedType};
use crate::interpreter::resumable::{self, ResumableStep, SequentialResumableStep};
use crate::interpreter::{ArgumentValue, MAX_STEPS_LIMIT};

/// Explicit resource limits. Byte counts use one type tag plus the scalar's
/// fixed-width payload (bool/u8: 2, i32/char/f32: 5, i64/usize/f64: 9).
/// `max_total_bytes` covers requests and reserves the declared answer width
/// before dispatch, including failed or refused answers. It is a channel
/// accounting limit, not a bound on memory allocated inside an injected host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceDriverBudget {
    pub max_calls: u32,
    pub max_steps_per_segment: usize,
    pub max_total_steps: usize,
    pub max_request_bytes: usize,
    pub max_total_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceDriverStatus {
    Completed(ResumableScalar),
    LanguageFailure(NormalizedStatus),
    Cancelled,
    CallBudgetExhausted,
    FuelExhausted,
    RequestBudgetExhausted,
    TotalByteBudgetExhausted,
    CapabilityDenied,
    RequestTypeMismatch,
    AnswerTypeMismatch,
    HandlerFailed,
    CallDepthExceeded,
    EvaluationRejected,
}

/// One attempted host call. Missing `answer` means the call failed or its
/// answer was refused. Host error strings are deliberately not retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDispatchRecord {
    pub request: ResumableScalar,
    pub answer: Option<ResumableScalar>,
}

/// At most eight dispatch records, in source order, bound to the exact checked
/// plan by `signature`. Float values retain their exact bits. These facts grant
/// no authority and are not an authenticated checkpoint or portable wire.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDriverRun {
    pub signature: SourceEffectSignature,
    pub status: SourceDriverStatus,
    pub calls: u32,
    pub steps_used: usize,
    pub bytes_reserved: usize,
    pub dispatches: Vec<SourceDispatchRecord>,
}

enum Pending {
    Single {
        state: super::ResumableStateId,
        binding: super::ResumableSuspensionBinding,
        request: ArgumentValue,
    },
    Sequential(resumable::ResumableContinuation),
}

enum Segment {
    Pending(Pending),
    Terminal(SourceDriverStatus),
}

/// Drive a fresh checked source invocation through one to eight scalar yields.
/// Admission errors return before dispatch; all later failures return bounded
/// accounting in `SourceDriverRun`. There is no retry or recovery entry point.
#[allow(clippy::too_many_arguments)]
pub fn run_source_resumable(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    policy: &CapabilityPolicy,
    budget: SourceDriverBudget,
    handler: &mut dyn EffectHandler<ArgumentValue, ArgumentValue>,
    cancelled: &dyn Fn() -> bool,
) -> Result<SourceDriverRun, Vec<Diagnostic>> {
    if !(1..=MAX_STEPS_LIMIT).contains(&budget.max_steps_per_segment) {
        return Err(vec![Diagnostic::io(
            "SPX-F101",
            "source driver segment fuel must be within the interpreter limit",
        )]);
    }
    let signature = derive_source_effect_signature(program, function_id).map_err(|e| vec![e])?;
    let answer_width = shape_width(signature.answer_shape()).expect("checked scalar signature");
    let mut run = SourceDriverRun {
        signature,
        status: SourceDriverStatus::EvaluationRejected,
        calls: 0,
        steps_used: 0,
        bytes_reserved: 0,
        dispatches: Vec::new(),
    };
    macro_rules! stop {
        ($status:expr) => {{
            run.status = $status;
            return Ok(run);
        }};
    }
    let mut resume = None;
    loop {
        if cancelled() {
            stop!(SourceDriverStatus::Cancelled);
        }
        let remaining = budget.max_total_steps.saturating_sub(run.steps_used);
        let fuel = remaining.min(budget.max_steps_per_segment);
        if fuel == 0 {
            stop!(SourceDriverStatus::FuelExhausted);
        }
        let evaluated = evaluate(
            program,
            function_id,
            arguments,
            resume.take(),
            fuel,
            run.signature.yield_count() == 1,
        );
        let (segment, steps) = match evaluated {
            Ok(value) => value,
            Err(_) => stop!(SourceDriverStatus::EvaluationRejected),
        };
        run.steps_used += steps;
        let pending = match segment {
            Segment::Terminal(status) => stop!(status),
            Segment::Pending(pending) => pending,
        };
        // A selected language failure above stays sticky even if cancellation
        // was raised during evaluation. Pending work checks it before dispatch.
        if cancelled() {
            stop!(SourceDriverStatus::Cancelled);
        }
        if run.calls >= budget.max_calls {
            stop!(SourceDriverStatus::CallBudgetExhausted);
        }
        if run.steps_used >= budget.max_total_steps {
            stop!(SourceDriverStatus::FuelExhausted);
        }
        let request = match &pending {
            Pending::Single { request, .. } => request,
            Pending::Sequential(continuation) => continuation.request(),
        };
        let Some((request_bits, request_type, width)) = scalar(request) else {
            stop!(SourceDriverStatus::RequestTypeMismatch);
        };
        let request_tag = tag(function_id, &request_type);
        if run.signature.table().check_request(&request_tag).is_err() {
            stop!(SourceDriverStatus::RequestTypeMismatch);
        }
        if width > budget.max_request_bytes {
            stop!(SourceDriverStatus::RequestBudgetExhausted);
        }
        let required = width + answer_width;
        if required > budget.max_total_bytes.saturating_sub(run.bytes_reserved) {
            stop!(SourceDriverStatus::TotalByteBudgetExhausted);
        }
        if !policy.allows(function_id) {
            stop!(SourceDriverStatus::CapabilityDenied);
        }
        // Reserve before the one physical boundary; refusals after this point
        // retain both the attempted call and its channel reservation.
        run.bytes_reserved += required;
        run.calls += 1;
        run.dispatches.push(SourceDispatchRecord {
            request: request_bits,
            answer: None,
        });
        let answer = match handler.dispatch(request) {
            Ok(answer) => answer,
            Err(_) => stop!(SourceDriverStatus::HandlerFailed),
        };
        let Some((answer_bits, answer_type, _)) = scalar(&answer) else {
            stop!(SourceDriverStatus::AnswerTypeMismatch);
        };
        if run
            .signature
            .table()
            .check_answer(&request_tag, &tag(function_id, &answer_type))
            .is_err()
        {
            stop!(SourceDriverStatus::AnswerTypeMismatch);
        }
        run.dispatches
            .last_mut()
            .expect("one dispatched call")
            .answer = Some(answer_bits);
        resume = Some((pending, answer));
    }
}

pub(super) fn tag(function_id: &str, ty: &ResolvedType) -> EffectTag {
    EffectTag::new(
        function_id,
        format!("{SOURCE_TYPE_SHAPE_PREFIX}{}", ty.identity_key()),
    )
}

fn shape_width(shape: &str) -> Option<usize> {
    match shape.strip_prefix(SOURCE_TYPE_SHAPE_PREFIX)? {
        "bool" | "u8" => Some(2),
        "i32" | "char" | "f32" => Some(5),
        "i64" | "usize" | "f64" => Some(9),
        _ => None,
    }
}

pub(super) fn scalar(value: &ArgumentValue) -> Option<(ResumableScalar, ResolvedType, usize)> {
    Some(match value {
        ArgumentValue::Int(v) => (ResumableScalar::I64(*v), ResolvedType::I64, 9),
        ArgumentValue::Int32(v) => (ResumableScalar::I32(*v), ResolvedType::I32, 5),
        ArgumentValue::Uint8(v) => (ResumableScalar::U8(*v), ResolvedType::U8, 2),
        ArgumentValue::Usize(v) => (ResumableScalar::Usize(*v), ResolvedType::Usize, 9),
        ArgumentValue::Char(v) if char::from_u32(*v).is_some() => {
            (ResumableScalar::Char(*v), ResolvedType::Char, 5)
        }
        ArgumentValue::Float32(v) => (ResumableScalar::F32(v.to_bits()), ResolvedType::F32, 5),
        ArgumentValue::Float64(v) => (ResumableScalar::F64(v.to_bits()), ResolvedType::F64, 9),
        ArgumentValue::Bool(v) => (ResumableScalar::Bool(*v), ResolvedType::Bool, 2),
        _ => return None,
    })
}

fn completed(result: ArgumentValue) -> Segment {
    Segment::Terminal(match scalar(&result) {
        Some((bits, _, _)) => SourceDriverStatus::Completed(bits),
        None => SourceDriverStatus::EvaluationRejected,
    })
}

fn evaluate(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    resume: Option<(Pending, ArgumentValue)>,
    fuel: usize,
    single: bool,
) -> Result<(Segment, usize), Vec<Diagnostic>> {
    if single {
        let evaluated = match resume {
            None => resumable::run_resumable_effect(program, function_id, arguments, fuel)?,
            Some((
                Pending::Single {
                    state,
                    binding,
                    request,
                },
                answer,
            )) => resumable::resume_resumable_effect(
                program,
                function_id,
                arguments,
                &state,
                &binding,
                &request,
                &answer,
                fuel,
            )?,
            _ => unreachable!("driver retains the compiler-selected lane"),
        };
        let segment = match evaluated.step {
            ResumableStep::Suspended {
                state,
                binding,
                request,
            } => Segment::Pending(Pending::Single {
                state,
                binding,
                request,
            }),
            ResumableStep::Completed { result, .. } => completed(result),
            ResumableStep::LanguageFailure(status) => {
                Segment::Terminal(SourceDriverStatus::LanguageFailure(status))
            }
            ResumableStep::FuelExhausted => Segment::Terminal(SourceDriverStatus::FuelExhausted),
            ResumableStep::CallDepthExceeded => {
                Segment::Terminal(SourceDriverStatus::CallDepthExceeded)
            }
            ResumableStep::GuardError(_) => {
                Segment::Terminal(SourceDriverStatus::EvaluationRejected)
            }
        };
        Ok((segment, evaluated.steps_used))
    } else {
        let evaluated = match resume {
            None => {
                resumable::run_sequential_resumable_effect(program, function_id, arguments, fuel)?
            }
            Some((Pending::Sequential(continuation), answer)) => {
                resumable::resume_sequential_resumable_effect(
                    program,
                    function_id,
                    arguments,
                    &continuation,
                    &answer,
                    fuel,
                )?
            }
            _ => unreachable!("driver retains the compiler-selected lane"),
        };
        let segment = match evaluated.step {
            SequentialResumableStep::Suspended { continuation } => {
                Segment::Pending(Pending::Sequential(continuation))
            }
            SequentialResumableStep::Completed { result, .. } => completed(result),
            SequentialResumableStep::LanguageFailure(status) => {
                Segment::Terminal(SourceDriverStatus::LanguageFailure(status))
            }
            SequentialResumableStep::FuelExhausted => {
                Segment::Terminal(SourceDriverStatus::FuelExhausted)
            }
            SequentialResumableStep::CallDepthExceeded => {
                Segment::Terminal(SourceDriverStatus::CallDepthExceeded)
            }
            SequentialResumableStep::GuardError(_) => {
                Segment::Terminal(SourceDriverStatus::EvaluationRejected)
            }
        };
        Ok((segment, evaluated.steps_used))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const SOURCE: &str = r#"
module test.source_driver;
@id("app.ask")
fn ask(seed: i64) -> bool yields i64 -> bool {
    let first = yield seed + 1;
    let second = yield seed + 2;
    first && second
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program(source: &str) -> ResolvedProgram {
        crate::hir::resolve(&crate::parse(source, "source-driver.spx").unwrap()).unwrap()
    }

    fn budget() -> SourceDriverBudget {
        SourceDriverBudget {
            max_calls: 8,
            max_steps_per_segment: 1000,
            max_total_steps: 10_000,
            max_request_bytes: 9,
            max_total_bytes: 144,
        }
    }

    fn policy() -> CapabilityPolicy {
        CapabilityPolicy::new(vec!["app.ask".into()]).unwrap()
    }

    struct Host {
        requests: Vec<ArgumentValue>,
        answer: ArgumentValue,
        fail: bool,
    }

    impl EffectHandler<ArgumentValue, ArgumentValue> for Host {
        fn dispatch(&mut self, request: &ArgumentValue) -> Result<ArgumentValue, String> {
            self.requests.push(request.clone());
            if self.fail {
                Err("host error is not retained in evidence".into())
            } else {
                Ok(self.answer.clone())
            }
        }
    }

    fn host() -> Host {
        Host {
            requests: Vec::new(),
            answer: ArgumentValue::Bool(true),
            fail: false,
        }
    }

    fn drive(
        source: &ResolvedProgram,
        limits: SourceDriverBudget,
        host: &mut Host,
    ) -> SourceDriverRun {
        run_source_resumable(
            source,
            "app.ask",
            &[ArgumentValue::Int(7)],
            &policy(),
            limits,
            host,
            &|| false,
        )
        .unwrap()
    }

    #[test]
    fn source_drives_each_site_once_and_accounts_for_replay_deterministically() {
        let source = program(SOURCE);
        let mut first_host = host();
        let first = drive(&source, budget(), &mut first_host);
        assert_eq!(
            first.status,
            SourceDriverStatus::Completed(ResumableScalar::Bool(true))
        );
        assert_eq!(
            first_host.requests,
            [ArgumentValue::Int(8), ArgumentValue::Int(9)]
        );
        assert_eq!(first.calls, 2);
        assert_eq!(first.bytes_reserved, 22);
        assert_eq!(first.dispatches.len(), 2);
        assert!(first.steps_used > 0);
        assert_eq!(first, drive(&source, budget(), &mut host()));
        let exact = SourceDriverBudget {
            max_calls: 2,
            max_total_steps: first.steps_used,
            max_total_bytes: 22,
            ..budget()
        };
        assert_eq!(drive(&source, exact, &mut host()), first);
        let less = SourceDriverBudget {
            max_total_steps: first.steps_used - 1,
            ..exact
        };
        let refused = drive(&source, less, &mut host());
        assert_eq!(refused.status, SourceDriverStatus::FuelExhausted);
        assert!(refused.steps_used <= less.max_total_steps);
    }

    #[test]
    fn cancellation_and_every_predispatch_budget_refuse_without_host_calls() {
        let source = program(SOURCE);
        for (limits, status) in [
            (
                SourceDriverBudget {
                    max_calls: 0,
                    ..budget()
                },
                SourceDriverStatus::CallBudgetExhausted,
            ),
            (
                SourceDriverBudget {
                    max_total_steps: 0,
                    ..budget()
                },
                SourceDriverStatus::FuelExhausted,
            ),
            (
                SourceDriverBudget {
                    max_steps_per_segment: 1,
                    ..budget()
                },
                SourceDriverStatus::FuelExhausted,
            ),
            (
                SourceDriverBudget {
                    max_request_bytes: 8,
                    ..budget()
                },
                SourceDriverStatus::RequestBudgetExhausted,
            ),
            (
                SourceDriverBudget {
                    max_total_bytes: 10,
                    ..budget()
                },
                SourceDriverStatus::TotalByteBudgetExhausted,
            ),
        ] {
            let mut host = host();
            let run = drive(&source, limits, &mut host);
            assert_eq!(run.status, status);
            assert_eq!(run.calls, 0);
            assert!(host.requests.is_empty());
        }
        let mut host = host();
        let run = run_source_resumable(
            &source,
            "app.ask",
            &[ArgumentValue::Int(7)],
            &policy(),
            budget(),
            &mut host,
            &|| true,
        )
        .unwrap();
        assert_eq!(run.status, SourceDriverStatus::Cancelled);
        assert!(host.requests.is_empty());
        assert_eq!(run.steps_used, 0);
    }

    #[test]
    fn explicit_policy_and_cumulative_limits_gate_the_next_call() {
        let source = program(SOURCE);
        let mut denied_host = host();
        let denied = run_source_resumable(
            &source,
            "app.ask",
            &[ArgumentValue::Int(7)],
            &CapabilityPolicy::none(),
            budget(),
            &mut denied_host,
            &|| false,
        )
        .unwrap();
        assert_eq!(denied.status, SourceDriverStatus::CapabilityDenied);
        assert!(denied_host.requests.is_empty());
        for (limits, expected) in [
            (
                SourceDriverBudget {
                    max_calls: 1,
                    ..budget()
                },
                SourceDriverStatus::CallBudgetExhausted,
            ),
            (
                SourceDriverBudget {
                    max_total_bytes: 21,
                    ..budget()
                },
                SourceDriverStatus::TotalByteBudgetExhausted,
            ),
        ] {
            let mut host = host();
            let run = drive(&source, limits, &mut host);
            assert_eq!(run.status, expected);
            assert_eq!(run.calls, 1);
            assert_eq!(run.bytes_reserved, 11);
            assert_eq!(host.requests, [ArgumentValue::Int(8)]);
        }
    }

    #[test]
    fn wrong_or_non_scalar_answers_never_reach_the_next_yield() {
        let source = program(SOURCE);
        for answer in [
            ArgumentValue::Int(1),
            ArgumentValue::BorrowedSlice(vec![0; 1024]),
        ] {
            let mut host = Host { answer, ..host() };
            let run = drive(&source, budget(), &mut host);
            assert_eq!(run.status, SourceDriverStatus::AnswerTypeMismatch);
            assert_eq!(run.calls, 1);
            assert_eq!(run.bytes_reserved, 11);
            assert_eq!(run.dispatches[0].answer, None);
            assert_eq!(host.requests.len(), 1);
        }
    }

    #[test]
    fn host_failure_is_sticky_even_when_it_also_cancels() {
        struct FailingHost<'a>(&'a Cell<bool>, usize);
        impl EffectHandler<ArgumentValue, ArgumentValue> for FailingHost<'_> {
            fn dispatch(&mut self, _: &ArgumentValue) -> Result<ArgumentValue, String> {
                self.0.set(true);
                self.1 += 1;
                Err("failed".into())
            }
        }
        let cancellation = Cell::new(false);
        let mut host = FailingHost(&cancellation, 0);
        let run = run_source_resumable(
            &program(SOURCE),
            "app.ask",
            &[ArgumentValue::Int(7)],
            &policy(),
            budget(),
            &mut host,
            &|| cancellation.get(),
        )
        .unwrap();
        assert_eq!(run.status, SourceDriverStatus::HandlerFailed);
        assert_eq!(host.1, 1);
        assert_eq!(run.dispatches[0].answer, None);
    }

    #[test]
    fn cancellation_after_an_answer_prevents_resume_and_next_dispatch() {
        let checks = Cell::new(0);
        let mut host = host();
        let run = run_source_resumable(
            &program(SOURCE),
            "app.ask",
            &[ArgumentValue::Int(7)],
            &policy(),
            budget(),
            &mut host,
            &|| {
                checks.set(checks.get() + 1);
                checks.get() >= 3
            },
        )
        .unwrap();
        assert_eq!(run.status, SourceDriverStatus::Cancelled);
        assert_eq!(run.calls, 1);
        assert_eq!(run.dispatches[0].answer, Some(ResumableScalar::Bool(true)));
    }

    #[test]
    fn single_site_and_float_bits_use_the_same_checked_boundary() {
        let source = program(
            r#"module test.source_driver;
@id("app.ask") fn ask(seed: f64) -> f64 yields f64 -> f64 { yield seed }
@id("app.main") fn main() -> i64 { 0 }
"#,
        );
        let bits = 0x8000000000000000;
        let mut host = Host {
            answer: ArgumentValue::Float64(f64::from_bits(bits)),
            ..host()
        };
        let run = run_source_resumable(
            &source,
            "app.ask",
            &[ArgumentValue::Float64(-0.0)],
            &policy(),
            budget(),
            &mut host,
            &|| false,
        )
        .unwrap();
        assert_eq!(
            run.status,
            SourceDriverStatus::Completed(ResumableScalar::F64(bits))
        );
        assert_eq!(run.calls, 1);
        assert_eq!(run.bytes_reserved, 18);
        assert_eq!(run.dispatches[0].request, ResumableScalar::F64(bits));
    }

    #[test]
    fn suspension_with_no_remaining_fuel_never_calls_the_host() {
        let source = program(SOURCE);
        let stopped = drive(
            &source,
            SourceDriverBudget {
                max_calls: 0,
                ..budget()
            },
            &mut host(),
        );
        let mut host = host();
        let run = drive(
            &source,
            SourceDriverBudget {
                max_total_steps: stopped.steps_used,
                ..budget()
            },
            &mut host,
        );
        assert_eq!(run.status, SourceDriverStatus::FuelExhausted);
        assert_eq!(run.steps_used, stopped.steps_used);
        assert_eq!(run.calls, 0);
        assert!(host.requests.is_empty());
    }

    #[test]
    fn language_failure_stops_before_the_next_physical_call() {
        let source = program(
            r#"module test.source_driver;
@id("app.ask") fn ask(seed: i64) -> i64 yields i64 -> i64 {
    let first = yield seed;
    let second = yield 10 / first;
    second
}
@id("app.main") fn main() -> i64 { 0 }
"#,
        );
        let mut host = Host {
            answer: ArgumentValue::Int(0),
            ..host()
        };
        let run = drive(&source, budget(), &mut host);
        assert!(matches!(run.status, SourceDriverStatus::LanguageFailure(_)));
        assert_eq!(run.calls, 1);
        assert_eq!(run.dispatches[0].answer, Some(ResumableScalar::I64(0)));
        assert_eq!(host.requests, [ArgumentValue::Int(7)]);
    }

    #[test]
    fn malformed_options_or_source_are_refused_before_dispatch() {
        let mut host = host();
        let source = program(SOURCE);
        let errors = run_source_resumable(
            &source,
            "app.ask",
            &[ArgumentValue::Int(7)],
            &policy(),
            SourceDriverBudget {
                max_steps_per_segment: 0,
                ..budget()
            },
            &mut host,
            &|| false,
        )
        .unwrap_err();
        assert_eq!(errors[0].code, "SPX-F101");
        let errors = run_source_resumable(
            &source,
            "app.main",
            &[],
            &policy(),
            budget(),
            &mut host,
            &|| false,
        )
        .unwrap_err();
        assert_eq!(errors[0].code, "SPX-H006");
        assert!(host.requests.is_empty());
    }
}
