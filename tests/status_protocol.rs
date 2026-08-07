use std::path::Path;

use semaprax::cleanup_plan::{ContractPhase, ExitContinuation, StatusCase};
use semaprax::conformance::{
    ConformanceTrace, InvocationPath, NormalizedStatus, Retryability, StatusClass, TraceEvent,
    TraceEventKind, TraceOutcome, TraceResult, ARITHMETIC_STATUS_DOMAIN_V1,
    CONTRACT_ENSURES_FALSE_CODE, CONTRACT_REQUIRES_FALSE_CODE, CONTRACT_STATUS_DOMAIN_V1,
    NORMALIZED_STATUS_SCHEMA_V1,
};
use semaprax::hir::{self, DeclarationId};
use semaprax::parse;
use semaprax::runtime_status::{
    normalize_arithmetic, normalize_contract, StatusArena, StatusArenaError, StatusContextId,
    StatusToken,
};

#[test]
fn tokens_are_immutable_one_based_and_context_local() {
    let mut left = StatusArena::new(StatusContextId::new(101), 3).unwrap();
    let mut right = StatusArena::new(StatusContextId::new(202), 1).unwrap();

    assert_eq!(StatusToken::SUCCESS.raw(), 0);
    assert!(StatusToken::SUCCESS.is_success());
    assert_eq!(
        left.resolve_local(StatusToken::SUCCESS),
        Err(StatusArenaError::SuccessHasNoRecord)
    );

    let first = left.record_arithmetic(StatusCase::AddOverflow).unwrap();
    let first_value = left.resolve(first).unwrap().clone();
    let second = left.record_contract(ContractPhase::Ensures).unwrap();
    let right_first = right.record_contract(ContractPhase::Requires).unwrap();

    assert_eq!((first.raw(), second.raw()), (1, 2));
    assert_eq!(right_first.raw(), 1, "indices are local to each context");
    assert_eq!(left.resolve(first), Ok(&first_value));
    assert_eq!(
        right.resolve(first),
        Err(StatusArenaError::WrongContext {
            expected: StatusContextId::new(202),
            actual: StatusContextId::new(101),
        })
    );
}

#[test]
fn duplicate_context_nonces_do_not_alias_distinct_arenas() {
    let mut first = StatusArena::new(StatusContextId::new(101), 1).unwrap();
    let mut second = StatusArena::new(StatusContextId::new(101), 1).unwrap();
    let token = first.record_arithmetic(StatusCase::AddOverflow).unwrap();
    let _other = second.record_contract(ContractPhase::Ensures).unwrap();

    assert_eq!(
        second.resolve(token),
        Err(StatusArenaError::ForeignArena {
            context: StatusContextId::new(101),
        })
    );
}

#[test]
fn arena_exhaustion_is_non_mutating_and_not_a_language_status() {
    let mut arena = StatusArena::new(StatusContextId::new(303), 1).unwrap();
    let token = arena.record_arithmetic(StatusCase::DivisionByZero).unwrap();
    let before = arena.resolve(token).unwrap().clone();

    assert_eq!(
        arena.record_contract(ContractPhase::Requires),
        Err(StatusArenaError::Exhausted { capacity: 1 })
    );
    assert_eq!(arena.len(), 1);
    assert_eq!(arena.resolve(token), Ok(&before));
    assert_eq!(before.class(), StatusClass::Arithmetic);
}

#[test]
fn compiler_failures_use_the_exact_versioned_status_mapping() {
    let arithmetic = [
        (StatusCase::AddOverflow, 1),
        (StatusCase::SubOverflow, 2),
        (StatusCase::MulOverflow, 3),
        (StatusCase::DivisionByZero, 4),
        (StatusCase::DivisionOverflow, 5),
        (StatusCase::RemainderByZero, 6),
        (StatusCase::RemainderOverflow, 7),
        (StatusCase::NegationOverflow, 8),
    ];
    for (case, code) in arithmetic {
        let status = normalize_arithmetic(case);
        assert_eq!(status.schema(), NORMALIZED_STATUS_SCHEMA_V1);
        assert_eq!(status.domain_id(), ARITHMETIC_STATUS_DOMAIN_V1);
        assert_eq!(status.code(), code);
        assert_eq!(status.class(), StatusClass::Arithmetic);
        assert_eq!(status.retryability(), Retryability::Known(false));
    }

    let contract = [
        (ContractPhase::Requires, CONTRACT_REQUIRES_FALSE_CODE),
        (ContractPhase::Ensures, CONTRACT_ENSURES_FALSE_CODE),
    ];
    for (phase, code) in contract {
        let status = normalize_contract(phase);
        assert_eq!(status.schema(), NORMALIZED_STATUS_SCHEMA_V1);
        assert_eq!(status.domain_id(), CONTRACT_STATUS_DOMAIN_V1);
        assert_eq!(status.code(), code);
        assert_eq!(status.class(), StatusClass::Contract);
        assert_eq!(status.retryability(), Retryability::Known(false));
    }
}

#[test]
fn canonical_trace_is_deterministic_and_excludes_physical_runtime_data() {
    let source = r#"
module test.status_trace;
@id("app.main")
fn main() -> i64 { 42 }
"#;
    let parsed = parse(source, Path::new("status-trace.spx")).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let result_source = function
        .cleanup_plan
        .exits
        .iter()
        .find_map(|exit| match &exit.continuation {
            ExitContinuation::CommitResult { source } => Some(source.clone()),
            _ => None,
        })
        .unwrap();
    let trace = ConformanceTrace::new(
        "status.trace.success",
        function.id.clone(),
        vec![TraceEvent {
            function: function.id.clone(),
            invocation: InvocationPath::default(),
            event: TraceEventKind::ResultCommit {
                source: result_source,
            },
        }],
        TraceOutcome::Success {
            result: TraceResult::I64(42),
        },
    );

    let first = trace.to_json();
    let second = trace.to_json();
    assert_eq!(first, second);
    assert!(first.contains(r#""schema":"semaprax.conformance-trace.v1""#));
    assert!(first.contains(r#""kind":"result_commit""#));
    for forbidden in [
        "uintptr",
        "pointer",
        "payload",
        "handle",
        "address",
        "status_token",
    ] {
        assert!(
            !first.contains(forbidden),
            "canonical trace leaked physical field `{forbidden}`: {first}"
        );
    }
}

#[test]
fn arbitrary_imported_status_round_trips_through_arena_and_canonical_json() {
    let imported = NormalizedStatus::try_new(
        "io.error.v1",
        42,
        StatusClass::Import,
        Retryability::Unknown,
    )
    .unwrap();
    let expected_json = imported.to_json();
    let mut arena = StatusArena::new(StatusContextId::new(404), 1).unwrap();
    let token = arena.record(imported.clone()).unwrap();
    let resolved = arena.resolve(token).unwrap();

    assert_eq!(resolved, &imported);
    assert_eq!(resolved.to_json(), expected_json);
    assert_eq!(
        expected_json,
        r#"{"schema":"semaprax.status.v1","domain_id":"io.error.v1","code":42,"class":"import","retryable":"unknown"}"#
    );
}

#[test]
fn declaration_identity_remains_semantic_while_context_identity_does_not_serialize() {
    let trace = ConformanceTrace::new(
        "status.trace.identity",
        DeclarationId::new("app.main"),
        Vec::new(),
        TraceOutcome::Success {
            result: TraceResult::Owned {
                type_id: DeclarationId::new("platform.token"),
            },
        },
    );
    let json = trace.to_json();
    assert!(json.contains(r#""root_function":"app.main""#));
    assert!(json.contains(r#""type_id":"platform.token""#));
    assert!(!json.contains("context"));
    assert!(!json.contains("nonce"));
}
