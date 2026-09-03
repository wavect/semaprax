use std::path::Path;

use crate::hir::{self, ResolvedExprKind};
use crate::parse;

use super::*;

const SOURCE: &str = r#"module test.trace;

@id("app.main")
fn main() -> i64 { 42 }
"#;

fn fixture_ids() -> (DeclarationId, ExpressionId) {
    let parsed = parse(SOURCE, Path::new("trace.spx")).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    let function = &resolved.functions[0];
    let ResolvedExprKind::Block { .. } = &function.body.kind else {
        panic!("fixture body must be a block")
    };
    (function.id.clone(), function.body.id.clone())
}

#[test]
fn compiler_status_codes_and_canonical_json_are_stable() {
    let expected = [
        (StatusCase::AddOverflow, 1),
        (StatusCase::SubOverflow, 2),
        (StatusCase::MulOverflow, 3),
        (StatusCase::DivisionByZero, 4),
        (StatusCase::DivisionOverflow, 5),
        (StatusCase::RemainderByZero, 6),
        (StatusCase::RemainderOverflow, 7),
        (StatusCase::NegationOverflow, 8),
    ];
    for (case, code) in expected {
        let status = NormalizedStatus::arithmetic(case);
        assert_eq!(status.code(), code);
        assert_eq!(status.class(), StatusClass::Arithmetic);
        assert_eq!(status.retryability(), Retryability::Known(false));
    }

    assert_eq!(
            NormalizedStatus::contract(ContractPhase::Requires).to_json(),
            "{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":1,\"class\":\"contract\",\"retryable\":false}"
        );
    assert_eq!(
        NormalizedStatus::contract(ContractPhase::Ensures).code(),
        CONTRACT_ENSURES_FALSE_CODE
    );
    assert_eq!(
        NormalizedStatus::try_new("", 1, StatusClass::Import, Retryability::Unknown,),
        Err(StatusDefinitionError::EmptyDomain)
    );
    assert_eq!(
        NormalizedStatus::try_new("io.error.v1", 0, StatusClass::Import, Retryability::Unknown,),
        Err(StatusDefinitionError::ZeroCode)
    );
    assert!(NormalizedStatus::try_new(
        "a".repeat(STATUS_DOMAIN_MAX_BYTES_V1),
        1,
        StatusClass::Import,
        Retryability::Unknown,
    )
    .is_ok());
    assert_eq!(
        NormalizedStatus::try_new(
            "é".repeat(STATUS_DOMAIN_MAX_BYTES_V1 / 2 + 1),
            1,
            StatusClass::Import,
            Retryability::Unknown,
        ),
        Err(StatusDefinitionError::DomainTooLong)
    );
    assert_eq!(
        NormalizedStatus::try_new(
            "io\0error.v1",
            1,
            StatusClass::Import,
            Retryability::Unknown,
        ),
        Err(StatusDefinitionError::DomainContainsNul)
    );
}

#[test]
fn external_statuses_cannot_forge_compiler_owned_mappings() {
    for class in [StatusClass::Contract, StatusClass::Arithmetic] {
        assert_eq!(
            NormalizedStatus::try_new("host.error.v1", 7, class, Retryability::Known(false),),
            Err(StatusDefinitionError::CompilerOwnedClass)
        );
    }

    for domain_id in [CONTRACT_STATUS_DOMAIN_V1, ARITHMETIC_STATUS_DOMAIN_V1] {
        for code in [1, 99] {
            assert_eq!(
                NormalizedStatus::try_new(
                    domain_id,
                    code,
                    StatusClass::Adapter,
                    Retryability::Unknown,
                ),
                Err(StatusDefinitionError::CompilerOwnedDomain)
            );
        }
    }

    let messages = [
        (
            StatusDefinitionError::EmptyDomain,
            "status domain identity cannot be empty",
        ),
        (
            StatusDefinitionError::DomainTooLong,
            "status domain identity cannot exceed 255 UTF-8 bytes",
        ),
        (
            StatusDefinitionError::DomainContainsNul,
            "status domain identity cannot contain NUL",
        ),
        (
            StatusDefinitionError::ZeroCode,
            "status code zero is reserved for success",
        ),
        (
            StatusDefinitionError::CompilerOwnedClass,
            "contract and arithmetic status classes are reserved for compiler-owned mappings",
        ),
        (
            StatusDefinitionError::CompilerOwnedDomain,
            "compiler-owned status domains cannot be constructed by external producers",
        ),
    ];
    for (error, message) in messages {
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn scalar_success_trace_has_an_exact_canonical_projection() {
    let (function, body) = fixture_ids();
    let trace = ConformanceTrace::new(
        "scalar-success",
        function.clone(),
        vec![TraceEvent {
            function,
            invocation: InvocationPath::default(),
            event: TraceEventKind::ResultCommit {
                source: CleanupResultSource::Scalar { expression: body },
            },
        }],
        TraceOutcome::Success {
            result: TraceResult::I64(42),
        },
    );
    let expected = "{\"schema\":\"semaprax.conformance-trace.v1\",\"scenario_id\":\"scalar-success\",\"root_function\":\"app.main\",\"events\":[{\"kind\":\"result_commit\",\"function\":\"app.main\",\"invocation\":[],\"source\":{\"kind\":\"scalar\",\"expression\":\"declaration:8:app.main:expression:4:body\"}}],\"outcome\":{\"kind\":\"success\",\"selected_source\":null,\"status\":null,\"result_published\":true,\"result\":{\"kind\":\"i64\",\"value\":\"42\"}}}";
    let json = trace.to_json();
    assert_eq!(trace.schema(), CONFORMANCE_TRACE_SCHEMA_V1);
    assert_eq!(json, expected);
    assert_eq!(json, trace.to_json());
    assert!(!json.contains("status_token"));
    assert!(!json.contains("handle"));
    assert!(!json.contains("pointer"));
    assert_eq!(
        trace_result_json(&TraceResult::Owned {
            type_id: DeclarationId::new("token.type"),
        }),
        "{\"kind\":\"owned\",\"type_id\":\"token.type\"}"
    );
    assert_eq!(
        trace_result_json(&TraceResult::Bytes),
        "{\"kind\":\"bytes\"}"
    );
}

#[test]
fn nested_failure_selection_and_callable_import_failure_have_exact_json() {
    let (function, expression) = fixture_ids();
    let selected_source = StatusSourceId {
        expression: expression.clone(),
        lane: StatusLane::OperationFailure,
    };
    let status = NormalizedStatus::try_new(
        "io.error.v1",
        17,
        StatusClass::Import,
        Retryability::Unknown,
    )
    .unwrap();
    let invocation = InvocationPath(vec![expression.clone()]);
    let import_id = DeclarationId::new("io.read");
    let trace = ConformanceTrace::new(
        "nested-import-failure",
        function.clone(),
        vec![
            TraceEvent {
                function: function.clone(),
                invocation: invocation.clone(),
                event: TraceEventKind::ImportBegin {
                    site: ImportSite::Call {
                        expression: expression.clone(),
                    },
                    import_id: import_id.clone(),
                },
            },
            TraceEvent {
                function: function.clone(),
                invocation: invocation.clone(),
                event: TraceEventKind::CallImportEnd {
                    expression: expression.clone(),
                    import_id,
                    outcome: OperationOutcome::Failure(status.clone()),
                },
            },
            TraceEvent {
                function,
                invocation,
                event: TraceEventKind::SelectFailure {
                    source: selected_source.clone(),
                    status: status.clone(),
                },
            },
        ],
        TraceOutcome::Failure {
            selected_source,
            status,
        },
    );
    let expected = "{\"schema\":\"semaprax.conformance-trace.v1\",\"scenario_id\":\"nested-import-failure\",\"root_function\":\"app.main\",\"events\":[{\"kind\":\"import_begin\",\"function\":\"app.main\",\"invocation\":[\"declaration:8:app.main:expression:4:body\"],\"site\":{\"kind\":\"call\",\"expression\":\"declaration:8:app.main:expression:4:body\"},\"import_id\":\"io.read\"},{\"kind\":\"import_end\",\"function\":\"app.main\",\"invocation\":[\"declaration:8:app.main:expression:4:body\"],\"site\":{\"kind\":\"call\",\"expression\":\"declaration:8:app.main:expression:4:body\"},\"import_id\":\"io.read\",\"outcome\":{\"kind\":\"failure\",\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"io.error.v1\",\"code\":17,\"class\":\"import\",\"retryable\":\"unknown\"}}},{\"kind\":\"select_failure\",\"function\":\"app.main\",\"invocation\":[\"declaration:8:app.main:expression:4:body\"],\"source\":{\"kind\":\"status_source_id\",\"expression\":\"declaration:8:app.main:expression:4:body\",\"lane\":\"operation_failure\"},\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"io.error.v1\",\"code\":17,\"class\":\"import\",\"retryable\":\"unknown\"}}],\"outcome\":{\"kind\":\"failure\",\"selected_source\":{\"kind\":\"status_source_id\",\"expression\":\"declaration:8:app.main:expression:4:body\",\"lane\":\"operation_failure\"},\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"io.error.v1\",\"code\":17,\"class\":\"import\",\"retryable\":\"unknown\"},\"result_published\":false,\"result\":null}}";
    assert_eq!(trace.to_json(), expected);
}

#[test]
fn event_order_and_json_escaping_are_preserved_without_sorting() {
    let (function, body) = fixture_ids();
    let temporary = CleanupPlace {
        storage: StorageId::Temporary(body.clone()),
        projections: vec![DeclarationId::new("field.\"quoted\"")],
    };
    let lifecycle = DeclarationId::new("token.drop");
    let import = DeclarationId::new("host.finalize");
    let site = ImportSite::Finalizer {
        source: temporary.clone(),
        lifecycle_id: lifecycle.clone(),
    };
    let events = vec![
        TraceEvent {
            function: function.clone(),
            invocation: InvocationPath::default(),
            event: TraceEventKind::FinalizeBegin {
                source: temporary.clone(),
                lifecycle_id: lifecycle.clone(),
                guard_flag: LivenessFlagId(7),
                binding_import: Some(import.clone()),
            },
        },
        TraceEvent {
            function: function.clone(),
            invocation: InvocationPath::default(),
            event: TraceEventKind::ImportBegin {
                site: site.clone(),
                import_id: import.clone(),
            },
        },
        TraceEvent {
            function: function.clone(),
            invocation: InvocationPath::default(),
            event: TraceEventKind::FinalizerImportEnd {
                source: temporary.clone(),
                lifecycle_id: lifecycle.clone(),
                import_id: import.clone(),
            },
        },
        TraceEvent {
            function: function.clone(),
            invocation: InvocationPath::default(),
            event: TraceEventKind::FinalizeEnd {
                source: temporary,
                lifecycle_id: lifecycle,
                guard_flag: LivenessFlagId(7),
                binding_import: Some(import),
            },
        },
    ];
    let trace = ConformanceTrace::new(
        "quote \" and newline\n",
        function,
        events,
        TraceOutcome::Failure {
            selected_source: StatusSourceId {
                expression: body,
                lane: StatusLane::ContractFalse,
            },
            status: NormalizedStatus::contract(ContractPhase::Ensures),
        },
    );
    let json = trace.to_json();
    let begin = json.find("\"kind\":\"finalize_begin\"").unwrap();
    let import_begin = json.find("\"kind\":\"import_begin\"").unwrap();
    let import_end = json.find("\"kind\":\"import_end\"").unwrap();
    let end = json.find("\"kind\":\"finalize_end\"").unwrap();
    assert!(begin < import_begin && import_begin < import_end && import_end < end);
    assert!(json.contains("quote \\\" and newline\\n"));
    assert!(json.contains("field.\\\"quoted\\\""));
    assert_eq!(json, trace.to_json());
}
