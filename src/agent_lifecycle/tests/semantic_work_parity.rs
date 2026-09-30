//! Agent Stage Semantic Work v1 cross-backend gate (#293).
//!
//! One stage call runs on the interpreter, native C11 `-O0` and `-O2`, and
//! Core Wasm through the sealed metered dispatch. Every backend must report
//! the same semantic fuel total and outcome on success, on a checked failure
//! before and inside the loop, on budget exhaustion at a frame entry and at a
//! loop-body entry, and at the exact-limit boundary. The compiled backends
//! must also report the same ordered performed-finalizer sequence; the
//! interpreter performs no plan finalizer and reports none. Instruction
//! counts are never compared. This is local evidence only: it requires the
//! held `clang` and `node` fixtures and makes no hosted or sanitizer claim.

use super::*;
use crate::interpreter::retained_call::{
    prepare_retained_call, PreparedRetainedCall, RetainedField, RetainedRecord, SemanticWork,
};

const SOURCE: &str = r#"module test.stage_semantic_work;

@id("test.stage_semantic_work.Task")
record Task {
    @id("test.stage_semantic_work.Task.objective") objective: Bytes,
    @id("test.stage_semantic_work.Task.budget") budget: i64,
}

@id("test.stage_semantic_work.Summary")
record Summary {
    @id("test.stage_semantic_work.Summary.tag") tag: Bytes,
    @id("test.stage_semantic_work.Summary.total") total: i64,
    @id("test.stage_semantic_work.Summary.count") count: i64,
}

@id("test.stage_semantic_work.measure")
fn measure(label: own Bytes, divisor: i64) -> i64
{
    let view = bytes_as_slice(label);
    if byte_len(view) > 1usize { 10 / divisor } else { 0 }
}

@id("test.stage_semantic_work.weigh")
fn weigh(index: i64, pivot: i64) -> i64
{
    100 / (pivot - index)
}

@id("test.stage_semantic_work.run")
fn run(task: own Task, divisor: i64, pivot: i64) -> Summary
{
    let seed = [83u8, 87u8];
    let scratch = bytes_copy(array_as_slice(seed));
    let base = measure(bytes_copy(array_as_slice(seed)), divisor);
    let mut index = 0;
    let mut total = base;
    let rounds = task.budget;
    while index < rounds {
        total = total + weigh(index, pivot);
        index = index + 1;
        0
    }
    Summary { tag: bytes_copy(array_as_slice(seed)), total: total, count: index }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const ENTRY: &str = "test.stage_semantic_work.run";
const STEPS: usize = 100_000;

fn program() -> hir::ResolvedProgram {
    let checked = crate::check(SOURCE, std::path::Path::new("stage-semantic-work.spx"))
        .expect("semantic work fixture checks");
    let program = hir::resolve(&checked).expect("semantic work fixture resolves");
    hir::validate(&program).expect("semantic work fixture validates");
    program
}

fn prepared(program: &hir::ResolvedProgram) -> PreparedRetainedCall {
    prepare_retained_call(program, ENTRY).expect("semantic work stage prepares")
}

fn arguments(budget: i64, divisor: i64, pivot: i64) -> Vec<RetainedValue> {
    vec![
        RetainedValue::Record(RetainedRecord {
            record: hir::DeclarationId::new("test.stage_semantic_work.Task"),
            fields: vec![
                RetainedField {
                    field: hir::DeclarationId::new("test.stage_semantic_work.Task.objective"),
                    value: RetainedValue::Bytes(vec![1, 2, 3]),
                },
                RetainedField {
                    field: hir::DeclarationId::new("test.stage_semantic_work.Task.budget"),
                    value: RetainedValue::I64(budget),
                },
            ],
        }),
        RetainedValue::I64(divisor),
        RetainedValue::I64(pivot),
    ]
}

fn legs(
    host: &authorization::NativeStageHost,
) -> Vec<(&'static str, authorization::StageBackend<'_>)> {
    legs_on(host, SOURCE)
}

fn legs_on<'a>(
    host: &'a authorization::NativeStageHost,
    source: &'a str,
) -> Vec<(&'static str, authorization::StageBackend<'a>)> {
    vec![
        ("native-O0", native_backend(host)),
        ("native-O2", native_o2_backend(host)),
        ("core-wasm", authorization::StageBackend::Wasm { source }),
    ]
}

fn metered(
    backend: authorization::StageBackend<'_>,
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    limit: u64,
) -> RetainedCallEvaluation {
    authorization::dispatch_on_metered(backend, program, prepared, arguments, STEPS, limit, None)
        .unwrap_or_else(|errors| panic!("metered dispatch refused: {errors:?}"))
}

fn work(evaluation: &RetainedCallEvaluation) -> &SemanticWork {
    evaluation
        .semantic_work
        .as_ref()
        .expect("every metered backend reports semantic work")
}

/// The comparable projection of one metered evaluation: outcome, boundary
/// copy-out events, and semantic fuel. Instruction counts are excluded.
fn comparable(
    evaluation: &RetainedCallEvaluation,
) -> (
    &RetainedCallOutcome,
    &[crate::interpreter::OwnedDataCleanupEvent],
    u64,
    bool,
) {
    let work = work(evaluation);
    (
        &evaluation.outcome,
        &evaluation.cleanup_events,
        work.fuel_used,
        work.exhausted,
    )
}

struct Case {
    name: &'static str,
    budget: i64,
    divisor: i64,
    pivot: i64,
    limit: u64,
    fuel: u64,
    exhausted: bool,
}

const CASES: &[Case] = &[
    Case {
        name: "success",
        budget: 4,
        divisor: 1,
        pivot: 100,
        limit: 1_000,
        fuel: 10,
        exhausted: false,
    },
    Case {
        name: "success-at-exact-limit",
        budget: 4,
        divisor: 1,
        pivot: 100,
        limit: 10,
        fuel: 10,
        exhausted: false,
    },
    Case {
        name: "failure-before-loop",
        budget: 4,
        divisor: 0,
        pivot: 100,
        limit: 1_000,
        fuel: 2,
        exhausted: false,
    },
    Case {
        name: "failure-mid-loop",
        budget: 4,
        divisor: 1,
        pivot: 2,
        limit: 1_000,
        fuel: 8,
        exhausted: false,
    },
    Case {
        name: "exhausted-at-helper-entry-with-owned-argument",
        budget: 4,
        divisor: 1,
        pivot: 100,
        limit: 1,
        fuel: 1,
        exhausted: true,
    },
    Case {
        name: "exhausted-mid-loop-at-call-entry",
        budget: 4,
        divisor: 1,
        pivot: 100,
        limit: 5,
        fuel: 5,
        exhausted: true,
    },
    Case {
        name: "exhausted-mid-loop-at-body-entry",
        budget: 4,
        divisor: 1,
        pivot: 100,
        limit: 6,
        fuel: 6,
        exhausted: true,
    },
    Case {
        name: "exhausted-at-last-call-entry",
        budget: 4,
        divisor: 1,
        pivot: 100,
        limit: 9,
        fuel: 9,
        exhausted: true,
    },
];

fn assert_expected_outcome(case: &Case, evaluation: &RetainedCallEvaluation) {
    let work = work(evaluation);
    assert_eq!(work.fuel_used, case.fuel, "{}: semantic fuel", case.name);
    assert_eq!(work.exhausted, case.exhausted, "{}: exhaustion", case.name);
    assert_eq!(
        work.fuel_limit,
        Some(case.limit),
        "{}: admitted limit",
        case.name
    );
    match (&evaluation.outcome, case) {
        (
            RetainedCallOutcome::FuelExhausted,
            Case {
                exhausted: true, ..
            },
        ) => {}
        (
            RetainedCallOutcome::LanguageFailure(status),
            Case {
                exhausted: false,
                divisor,
                pivot,
                ..
            },
        ) if *divisor == 0 || *pivot < case.budget => {
            assert_eq!(
                status,
                &crate::runtime_status::normalize_arithmetic(
                    crate::cleanup_plan::StatusCase::DivisionByZero
                ),
                "{}",
                case.name
            );
        }
        (
            RetainedCallOutcome::Returned(_),
            Case {
                exhausted: false,
                divisor: 1,
                pivot: 100,
                ..
            },
        ) => {}
        (outcome, _) => panic!("{}: unexpected outcome {outcome:?}", case.name),
    }
}

/// The finalizers every compiled backend must perform for this fixture:
/// `measure`'s owned label plus `run`'s scratch and unmoved task objective,
/// on every path the fixture reaches. Order is the canonical execution order
/// and is compared across backends rather than predicted here.
fn assert_finalizer_inventory(
    case: &Case,
    events: &[crate::interpreter::retained_call::SemanticCleanupEvent],
) {
    let functions = events
        .iter()
        .map(|event| event.function.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        functions,
        [
            "test.stage_semantic_work.measure",
            "test.stage_semantic_work.run",
            "test.stage_semantic_work.run",
        ],
        "{}: performed finalizer inventory",
        case.name
    );
    assert_ne!(
        events[1], events[2],
        "{}: two distinct run slots",
        case.name
    );
}

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn semantic_fuel_and_finalizer_events_agree_across_every_stage_backend() {
    let host = native_stage_host().expect("semantic work parity requires held clang");
    let program = program();
    let prepared = prepared(&program);
    for case in CASES {
        let args = arguments(case.budget, case.divisor, case.pivot);
        let expected = metered(
            authorization::StageBackend::Interpreter,
            &program,
            &prepared,
            &args,
            case.limit,
        );
        assert_expected_outcome(case, &expected);
        assert_eq!(
            work(&expected).finalizer_events,
            None,
            "{}: the interpreter performs no plan finalizer",
            case.name
        );
        let mut compiled_events = None;
        for (leg, backend) in legs(&host) {
            let actual = metered(backend, &program, &prepared, &args, case.limit);
            assert_eq!(
                comparable(&actual),
                comparable(&expected),
                "{}: {leg} semantic work",
                case.name
            );
            assert_eq!(
                actual.steps_used, 0,
                "{}: {leg} reports no instruction count",
                case.name
            );
            let events = work(&actual)
                .finalizer_events
                .clone()
                .unwrap_or_else(|| panic!("{}: {leg} reports performed finalizers", case.name));
            assert_finalizer_inventory(case, &events);
            match &compiled_events {
                None => compiled_events = Some(events),
                Some(first) => assert_eq!(&events, first, "{}: {leg} finalizer order", case.name),
            }
        }
    }
}

// Issue #293 P2-1: a recursive metered function must refuse admission at the
// same call-depth ceiling on every backend (`interpreter::MAX_CALL_DEPTH`,
// native's `SPX_MAX_CALL_DEPTH`, and Core Wasm's `call_admission` module),
// rather than diverging into fuel exhaustion or an uncontrolled host-engine
// stack trap. `recurse` is metered by the same profile as `measure`/`weigh`/
// `run` above; it carries no owned data, so cleanup events stay empty on
// every leg and every path.
const DEPTH_SOURCE: &str = r#"module test.stage_call_depth;

@id("test.stage_call_depth.Step")
record Step {
    @id("test.stage_call_depth.Step.total") total: i64,
}

@id("test.stage_call_depth.recurse")
fn recurse(depth: i64) -> Step
{
    if depth <= 0 {
        Step { total: 0 }
    } else {
        let inner = recurse(depth - 1);
        Step { total: 1 + inner.total }
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const DEPTH_ENTRY: &str = "test.stage_call_depth.recurse";
/// Exceeds every backend's fixed call-depth ceiling (256).
const DEEP_RECURSION: i64 = 300;

fn depth_program() -> hir::ResolvedProgram {
    let checked = crate::check(DEPTH_SOURCE, std::path::Path::new("stage-call-depth.spx"))
        .expect("call-depth fixture checks");
    let program = hir::resolve(&checked).expect("call-depth fixture resolves");
    hir::validate(&program).expect("call-depth fixture validates");
    program
}

fn depth_prepared(program: &hir::ResolvedProgram) -> PreparedRetainedCall {
    prepare_retained_call(program, DEPTH_ENTRY).expect("call-depth stage prepares")
}

/// The native-only legs of [`legs_on`]. Core Wasm's *stage executor* builds a
/// replay-verified owned-data npm package through
/// `project::derive_public_api_descriptor`, which refuses any recursive
/// selected closure outright (`"public API selected closure must be
/// acyclic"`) -- a pre-existing, unrelated restriction of that packaging
/// format, not of Core Wasm lowering itself. The same call-depth admission
/// this test exercises for the interpreter and native C11 is proven directly
/// against the compiled Core Wasm bytecode instead, bypassing that
/// packaging, by
/// `wasm::aggregate::tests::call_depth_admission_refuses_the_same_ceiling_every_backend_shares`.
fn native_legs_on<'a>(
    host: &'a authorization::NativeStageHost,
) -> Vec<(&'static str, authorization::StageBackend<'a>)> {
    vec![
        ("native-O0", native_backend(host)),
        ("native-O2", native_o2_backend(host)),
    ]
}

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn call_depth_admission_agrees_across_every_stage_backend() {
    let host = native_stage_host().expect("call depth parity requires held clang");
    let program = depth_program();
    let prepared = depth_prepared(&program);

    // Depth alone is exhausted first: fuel is ample (recursing to 300 charges
    // at most 300 units, and refusal at the 257th frame charges exactly 256).
    let args = vec![RetainedValue::I64(DEEP_RECURSION)];
    let expected = metered(
        authorization::StageBackend::Interpreter,
        &program,
        &prepared,
        &args,
        100_000,
    );
    assert_eq!(
        expected.outcome,
        RetainedCallOutcome::CallDepthExceeded,
        "depth-exceeded-before-fuel: interpreter outcome"
    );
    assert_eq!(
        work(&expected).fuel_used,
        256,
        "depth-exceeded-before-fuel: interpreter charges every admitted frame, no more"
    );
    assert!(
        !work(&expected).exhausted,
        "depth-exceeded-before-fuel: refusal is a depth ceiling, not fuel exhaustion"
    );
    for (leg, backend) in native_legs_on(&host) {
        let actual = metered(backend, &program, &prepared, &args, 100_000);
        assert_eq!(
            comparable(&actual),
            comparable(&expected),
            "depth-exceeded-before-fuel: {leg} semantic work"
        );
    }

    // Fuel is exhausted first: the limit is far below the depth ceiling, so
    // every backend must settle as ordinary fuel exhaustion, never depth.
    let expected = metered(
        authorization::StageBackend::Interpreter,
        &program,
        &prepared,
        &args,
        10,
    );
    assert_eq!(
        expected.outcome,
        RetainedCallOutcome::FuelExhausted,
        "fuel-exhausted-before-depth: interpreter outcome"
    );
    assert!(
        work(&expected).exhausted,
        "fuel-exhausted-before-depth: fuel settles this call before depth can"
    );
    for (leg, backend) in native_legs_on(&host) {
        let actual = metered(backend, &program, &prepared, &args, 10);
        assert_eq!(
            comparable(&actual),
            comparable(&expected),
            "fuel-exhausted-before-depth: {leg} semantic work"
        );
    }
}

#[test]
fn metered_dispatch_refuses_invalid_limits_unmetered_profiles_and_cancellation_before_target_work()
{
    let program = program();
    let prepared = prepared(&program);
    let args = arguments(4, 1, 100);
    let host = native_stage_host();
    let mut backends = vec![("interpreter", authorization::StageBackend::Interpreter)];
    if let Some(host) = host.as_ref() {
        backends.extend(legs(host));
    }
    let message = |result: Result<RetainedCallEvaluation, Vec<Diagnostic>>| match result {
        Ok(evaluation) => panic!("metered dispatch must refuse: {:?}", evaluation.outcome),
        Err(errors) => errors
            .iter()
            .map(|error| error.message.clone())
            .collect::<Vec<_>>()
            .join("; "),
    };
    for (leg, backend) in &backends {
        for (limit, field) in [
            (0, "semantic_work.fuel_limit"),
            (1_000_001, "semantic_work.fuel_limit"),
        ] {
            let refused = message(authorization::dispatch_on_metered(
                *backend, &program, &prepared, &args, STEPS, limit, None,
            ));
            assert!(refused.contains(field), "{leg}: {refused}");
        }
        let cancellation = crate::agent_runtime::AgentCancellation::new();
        cancellation.cancel();
        let refused = message(authorization::dispatch_on_metered(
            *backend,
            &program,
            &prepared,
            &args,
            STEPS,
            10,
            Some(&cancellation),
        ));
        assert!(
            refused.contains("stage_executor.cancelled"),
            "{leg}: {refused}"
        );
    }

    // A function value is outside the metered profile: its invocation would
    // charge on the interpreter but has no identical compiled charge point.
    let source = SOURCE.replace(
        "    100 / (pivot - index)\n",
        "    let divide = pick;\n    divide(100, pivot - index)\n",
    )
    .replace(
        "@id(\"test.stage_semantic_work.run\")",
        "@id(\"test.stage_semantic_work.pick\")\nfn pick(left: i64, right: i64) -> i64 { left / right }\n\n@id(\"test.stage_semantic_work.run\")",
    );
    let checked = crate::check(
        &source,
        std::path::Path::new("stage-semantic-work-value.spx"),
    )
    .expect("function-value fixture checks");
    let valued = hir::resolve(&checked).expect("function-value fixture resolves");
    hir::validate(&valued).expect("function-value fixture validates");
    let refused = authorization::StageSemanticProfile::admit(&valued, ENTRY, 10)
        .expect_err("a function value is outside the metered profile");
    assert!(
        refused
            .message
            .contains("semantic_work.profile.function_value"),
        "{}",
        refused.message
    );
    authorization::StageSemanticProfile::admit(&program, ENTRY, 10)
        .expect("the direct-call and while-loop fixture is inside the metered profile");
}
