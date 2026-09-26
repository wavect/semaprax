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
    vec![
        ("native-O0", native_backend(host)),
        ("native-O2", native_o2_backend(host)),
        (
            "core-wasm",
            authorization::StageBackend::Wasm { source: SOURCE },
        ),
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
