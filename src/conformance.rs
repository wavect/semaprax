//! Versioned, target-neutral status and conformance-trace protocol.
//!
//! These types contain semantic identities only. Native pointers, Wasm
//! handles, context-local status tokens, and adapter diagnostic objects are
//! deliberately absent so native and Wasm executions can be compared exactly.

use std::error::Error;
use std::fmt;

use crate::cleanup::LivenessFlagId;
use crate::cleanup_plan::{
    CallArgumentTransfer, CleanupPlace, CleanupResultSource, ContractPhase, StatusCase, StatusLane,
    StatusSourceId, StorageId,
};
use crate::diagnostic::quote_json;
use crate::hir::{DeclarationId, ExpressionId};

pub const NORMALIZED_STATUS_SCHEMA_V1: &str = "semaprax.status.v1";
pub const CONFORMANCE_TRACE_SCHEMA_V1: &str = "semaprax.conformance-trace.v1";
pub const ARITHMETIC_STATUS_DOMAIN_V1: &str = "semaprax.arithmetic.v1";
pub const CONTRACT_STATUS_DOMAIN_V1: &str = "semaprax.contract.v1";
/// Maximum UTF-8 byte length of a status-v1 domain identity.
pub const STATUS_DOMAIN_MAX_BYTES_V1: usize = 255;

pub const CONTRACT_REQUIRES_FALSE_CODE: u32 = 1;
pub const CONTRACT_ENSURES_FALSE_CODE: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StatusClass {
    Contract,
    Arithmetic,
    Import,
    ExplicitClose,
    Adapter,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Retryability {
    Known(bool),
    Unknown,
}

/// Stable semantic failure information stored behind a context-local ABI
/// token. The token and opaque diagnostic detail are intentionally not part of
/// this value or its canonical representation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NormalizedStatus {
    schema: &'static str,
    domain_id: String,
    code: u32,
    class: StatusClass,
    retryable: Retryability,
}

impl NormalizedStatus {
    /// Builds an adapter/import status while preserving the ABI invariant that
    /// token/code zero means success and compiler-owned mappings cannot be
    /// forged by an external producer.
    pub fn try_new(
        domain_id: impl Into<String>,
        code: u32,
        class: StatusClass,
        retryable: Retryability,
    ) -> Result<Self, StatusDefinitionError> {
        let domain_id = domain_id.into();
        if matches!(class, StatusClass::Contract | StatusClass::Arithmetic) {
            return Err(StatusDefinitionError::CompilerOwnedClass);
        }
        if matches!(
            domain_id.as_str(),
            CONTRACT_STATUS_DOMAIN_V1 | ARITHMETIC_STATUS_DOMAIN_V1
        ) {
            return Err(StatusDefinitionError::CompilerOwnedDomain);
        }
        Self::try_new_trusted(domain_id, code, class, retryable)
    }

    fn try_new_trusted(
        domain_id: impl Into<String>,
        code: u32,
        class: StatusClass,
        retryable: Retryability,
    ) -> Result<Self, StatusDefinitionError> {
        let domain_id = domain_id.into();
        if domain_id.is_empty() {
            return Err(StatusDefinitionError::EmptyDomain);
        }
        if domain_id.len() > STATUS_DOMAIN_MAX_BYTES_V1 {
            return Err(StatusDefinitionError::DomainTooLong);
        }
        if domain_id.contains('\0') {
            return Err(StatusDefinitionError::DomainContainsNul);
        }
        if code == 0 {
            return Err(StatusDefinitionError::ZeroCode);
        }
        Ok(Self {
            schema: NORMALIZED_STATUS_SCHEMA_V1,
            domain_id,
            code,
            class,
            retryable,
        })
    }

    pub fn arithmetic(case: StatusCase) -> Self {
        Self::try_new_trusted(
            ARITHMETIC_STATUS_DOMAIN_V1,
            case.code(),
            StatusClass::Arithmetic,
            Retryability::Known(false),
        )
        .expect("compiler arithmetic status constants are valid")
    }

    pub fn contract(phase: ContractPhase) -> Self {
        let code = match phase {
            ContractPhase::Requires => CONTRACT_REQUIRES_FALSE_CODE,
            ContractPhase::Ensures => CONTRACT_ENSURES_FALSE_CODE,
        };
        Self::try_new_trusted(
            CONTRACT_STATUS_DOMAIN_V1,
            code,
            StatusClass::Contract,
            Retryability::Known(false),
        )
        .expect("compiler contract status constants are valid")
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn domain_id(&self) -> &str {
        &self.domain_id
    }

    pub const fn code(&self) -> u32 {
        self.code
    }

    pub const fn class(&self) -> StatusClass {
        self.class
    }

    pub const fn retryability(&self) -> Retryability {
        self.retryable
    }

    pub fn to_json(&self) -> String {
        normalized_status_json(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusDefinitionError {
    EmptyDomain,
    DomainTooLong,
    DomainContainsNul,
    ZeroCode,
    CompilerOwnedClass,
    CompilerOwnedDomain,
}

impl fmt::Display for StatusDefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDomain => formatter.write_str("status domain identity cannot be empty"),
            Self::DomainTooLong => {
                formatter.write_str("status domain identity cannot exceed 255 UTF-8 bytes")
            }
            Self::DomainContainsNul => {
                formatter.write_str("status domain identity cannot contain NUL")
            }
            Self::ZeroCode => formatter.write_str("status code zero is reserved for success"),
            Self::CompilerOwnedClass => formatter.write_str(
                "contract and arithmetic status classes are reserved for compiler-owned mappings",
            ),
            Self::CompilerOwnedDomain => formatter.write_str(
                "compiler-owned status domains cannot be constructed by external producers",
            ),
        }
    }
}

impl Error for StatusDefinitionError {}

/// The semantic call path from the root invocation. Each element is the
/// revision-scoped call expression that entered the next frame.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InvocationPath(pub Vec<ExpressionId>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportSite {
    Call {
        expression: ExpressionId,
    },
    Finalizer {
        source: CleanupPlace,
        lifecycle_id: DeclarationId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationOutcome {
    Success,
    Failure(NormalizedStatus),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraceEventKind {
    Initialize {
        at: ExpressionId,
        destination: CleanupPlace,
    },
    Transfer {
        at: ExpressionId,
        source: CleanupPlace,
        destination: CleanupPlace,
    },
    CallCommit {
        call: ExpressionId,
        callee: DeclarationId,
        arguments: Vec<CallArgumentTransfer>,
    },
    ImportBegin {
        site: ImportSite,
        import_id: DeclarationId,
    },
    /// Completion of an ordinary callable import. Unlike automatic
    /// finalization, this operation may return a normalized failure.
    CallImportEnd {
        expression: ExpressionId,
        import_id: DeclarationId,
        outcome: OperationOutcome,
    },
    /// Completion of an imported automatic finalizer. Success is encoded by
    /// the variant itself, so a language-level finalizer failure is
    /// unrepresentable.
    FinalizerImportEnd {
        source: CleanupPlace,
        lifecycle_id: DeclarationId,
        import_id: DeclarationId,
    },
    /// Records the frame-local, write-once failure selection. Nested callees
    /// therefore retain their exact semantic source even when the caller later
    /// propagates the same normalized status from its call expression.
    SelectFailure {
        source: StatusSourceId,
        status: NormalizedStatus,
    },
    FinalizeBegin {
        source: CleanupPlace,
        lifecycle_id: DeclarationId,
        guard_flag: LivenessFlagId,
        binding_import: Option<DeclarationId>,
    },
    /// Automatic finalization is semantically infallible. A trap or unwind is
    /// an adapter-conformance failure and therefore cannot be represented as a
    /// language-level failure outcome here.
    FinalizeEnd {
        source: CleanupPlace,
        lifecycle_id: DeclarationId,
        guard_flag: LivenessFlagId,
        binding_import: Option<DeclarationId>,
    },
    ResultCommit {
        source: CleanupResultSource,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceEvent {
    pub function: DeclarationId,
    pub invocation: InvocationPath,
    pub event: TraceEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraceResult {
    I64(i64),
    /// Exact checked 32-bit integer payload.
    Int32(i32),
    /// Exact Unicode scalar value payload.
    Char(u32),
    /// Exact IEEE-754 single-precision payload bits.
    F32(u32),
    /// Exact IEEE-754 double-precision payload bits.
    F64(u64),
    Bool(bool),
    Unit,
    /// Only the stable resolved type identity is observable. The physical
    /// resource payload remains target-private.
    Owned {
        type_id: DeclarationId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraceOutcome {
    Success {
        result: TraceResult,
    },
    Failure {
        selected_source: StatusSourceId,
        status: NormalizedStatus,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceTrace {
    schema: &'static str,
    pub scenario_id: String,
    pub root_function: DeclarationId,
    pub events: Vec<TraceEvent>,
    pub outcome: TraceOutcome,
}

impl ConformanceTrace {
    pub fn new(
        scenario_id: impl Into<String>,
        root_function: DeclarationId,
        events: Vec<TraceEvent>,
        outcome: TraceOutcome,
    ) -> Self {
        Self {
            schema: CONFORMANCE_TRACE_SCHEMA_V1,
            scenario_id: scenario_id.into(),
            root_function,
            events,
            outcome,
        }
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    /// Canonical JSON preserves event, invocation-path, projection, and call
    /// argument order exactly. It performs no sorting or repair.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"schema\":{},\"scenario_id\":{},\"root_function\":{},\"events\":{},\"outcome\":{}}}",
            quote_json(CONFORMANCE_TRACE_SCHEMA_V1),
            quote_json(&self.scenario_id),
            quote_json(self.root_function.as_str()),
            array_json(&self.events, trace_event_json),
            trace_outcome_json(&self.outcome),
        )
    }
}

fn normalized_status_json(status: &NormalizedStatus) -> String {
    format!(
        "{{\"schema\":{},\"domain_id\":{},\"code\":{},\"class\":{},\"retryable\":{}}}",
        quote_json(status.schema),
        quote_json(&status.domain_id),
        status.code,
        quote_json(status_class_text(status.class)),
        retryability_json(status.retryable),
    )
}

fn trace_event_json(event: &TraceEvent) -> String {
    let prefix = format!(
        "\"function\":{},\"invocation\":{}",
        quote_json(event.function.as_str()),
        array_json(&event.invocation.0, |expression| quote_json(
            expression.as_str()
        )),
    );
    match &event.event {
        TraceEventKind::Initialize { at, destination } => format!(
            "{{\"kind\":\"initialize\",{prefix},\"at\":{},\"destination\":{}}}",
            quote_json(at.as_str()),
            place_json(destination),
        ),
        TraceEventKind::Transfer {
            at,
            source,
            destination,
        } => format!(
            "{{\"kind\":\"transfer\",{prefix},\"at\":{},\"source\":{},\"destination\":{}}}",
            quote_json(at.as_str()),
            place_json(source),
            place_json(destination),
        ),
        TraceEventKind::CallCommit {
            call,
            callee,
            arguments,
        } => format!(
            "{{\"kind\":\"call_commit\",{prefix},\"call\":{},\"callee\":{},\"arguments\":{}}}",
            quote_json(call.as_str()),
            quote_json(callee.as_str()),
            array_json(arguments, call_argument_json),
        ),
        TraceEventKind::ImportBegin { site, import_id } => format!(
            "{{\"kind\":\"import_begin\",{prefix},\"site\":{},\"import_id\":{}}}",
            import_site_json(site),
            quote_json(import_id.as_str()),
        ),
        TraceEventKind::CallImportEnd {
            expression,
            import_id,
            outcome,
        } => format!(
            "{{\"kind\":\"import_end\",{prefix},\"site\":{},\"import_id\":{},\"outcome\":{}}}",
            import_site_json(&ImportSite::Call {
                expression: expression.clone(),
            }),
            quote_json(import_id.as_str()),
            operation_outcome_json(outcome),
        ),
        TraceEventKind::FinalizerImportEnd {
            source,
            lifecycle_id,
            import_id,
        } => format!(
            "{{\"kind\":\"import_end\",{prefix},\"site\":{},\"import_id\":{},\"outcome\":{{\"kind\":\"success\"}}}}",
            import_site_json(&ImportSite::Finalizer {
                source: source.clone(),
                lifecycle_id: lifecycle_id.clone(),
            }),
            quote_json(import_id.as_str()),
        ),
        TraceEventKind::SelectFailure { source, status } => format!(
            "{{\"kind\":\"select_failure\",{prefix},\"source\":{},\"status\":{}}}",
            status_source_id_json(source),
            normalized_status_json(status),
        ),
        TraceEventKind::FinalizeBegin {
            source,
            lifecycle_id,
            guard_flag,
            binding_import,
        } => finalize_event_json(
            "finalize_begin",
            &prefix,
            source,
            lifecycle_id,
            *guard_flag,
            binding_import.as_ref(),
            false,
        ),
        TraceEventKind::FinalizeEnd {
            source,
            lifecycle_id,
            guard_flag,
            binding_import,
        } => finalize_event_json(
            "finalize_end",
            &prefix,
            source,
            lifecycle_id,
            *guard_flag,
            binding_import.as_ref(),
            true,
        ),
        TraceEventKind::ResultCommit { source } => format!(
            "{{\"kind\":\"result_commit\",{prefix},\"source\":{}}}",
            result_source_json(source),
        ),
    }
}

fn finalize_event_json(
    kind: &str,
    prefix: &str,
    source: &CleanupPlace,
    lifecycle_id: &DeclarationId,
    guard_flag: LivenessFlagId,
    binding_import: Option<&DeclarationId>,
    completed: bool,
) -> String {
    let binding_import =
        binding_import.map_or_else(|| "null".to_owned(), |import| quote_json(import.as_str()));
    let completion = if completed {
        ",\"outcome\":{\"kind\":\"success\"}"
    } else {
        ""
    };
    format!(
        "{{\"kind\":{},{},\"source\":{},\"lifecycle_id\":{},\"guard_flag\":{},\"binding_import\":{}{completion}}}",
        quote_json(kind),
        prefix,
        place_json(source),
        quote_json(lifecycle_id.as_str()),
        guard_flag.0,
        binding_import,
    )
}

fn import_site_json(site: &ImportSite) -> String {
    match site {
        ImportSite::Call { expression } => format!(
            "{{\"kind\":\"call\",\"expression\":{}}}",
            quote_json(expression.as_str())
        ),
        ImportSite::Finalizer {
            source,
            lifecycle_id,
        } => format!(
            "{{\"kind\":\"finalizer\",\"source\":{},\"lifecycle_id\":{}}}",
            place_json(source),
            quote_json(lifecycle_id.as_str())
        ),
    }
}

fn operation_outcome_json(outcome: &OperationOutcome) -> String {
    match outcome {
        OperationOutcome::Success => "{\"kind\":\"success\"}".to_owned(),
        OperationOutcome::Failure(status) => format!(
            "{{\"kind\":\"failure\",\"status\":{}}}",
            normalized_status_json(status)
        ),
    }
}

fn trace_outcome_json(outcome: &TraceOutcome) -> String {
    match outcome {
        TraceOutcome::Success { result } => format!(
            "{{\"kind\":\"success\",\"selected_source\":null,\"status\":null,\"result_published\":true,\"result\":{}}}",
            trace_result_json(result)
        ),
        TraceOutcome::Failure {
            selected_source,
            status,
        } => format!(
            "{{\"kind\":\"failure\",\"selected_source\":{},\"status\":{},\"result_published\":false,\"result\":null}}",
            status_source_id_json(selected_source),
            normalized_status_json(status)
        ),
    }
}

fn trace_result_json(result: &TraceResult) -> String {
    match result {
        TraceResult::I64(value) => format!(
            "{{\"kind\":\"i64\",\"value\":{}}}",
            quote_json(&value.to_string())
        ),
        TraceResult::Int32(value) => format!("{{\"kind\":\"int32\",\"value\":{value}}}"),
        TraceResult::Char(value) => {
            format!("{{\"kind\":\"char\",\"value\":{value}}}")
        }
        TraceResult::F32(bits) => format!("{{\"kind\":\"f32\",\"bits\":\"{bits:08x}\"}}",),
        TraceResult::F64(bits) => format!("{{\"kind\":\"f64\",\"bits\":\"{bits:016x}\"}}",),
        TraceResult::Bool(value) => format!("{{\"kind\":\"bool\",\"value\":{value}}}"),
        TraceResult::Unit => "{\"kind\":\"unit\"}".to_owned(),
        TraceResult::Owned { type_id } => format!(
            "{{\"kind\":\"owned\",\"type_id\":{}}}",
            quote_json(type_id.as_str())
        ),
    }
}

fn result_source_json(source: &CleanupResultSource) -> String {
    match source {
        CleanupResultSource::Scalar { expression } => format!(
            "{{\"kind\":\"scalar\",\"expression\":{}}}",
            quote_json(expression.as_str())
        ),
        CleanupResultSource::Owned { storage } => {
            format!("{{\"kind\":\"owned\",\"storage\":{}}}", place_json(storage))
        }
    }
}

fn call_argument_json(argument: &CallArgumentTransfer) -> String {
    format!(
        "{{\"kind\":\"call_argument_transfer\",\"parameter_index\":{},\"source\":{}}}",
        argument.parameter_index,
        place_json(&argument.source),
    )
}

fn status_source_id_json(source: &StatusSourceId) -> String {
    format!(
        "{{\"kind\":\"status_source_id\",\"expression\":{},\"lane\":{}}}",
        quote_json(source.expression.as_str()),
        quote_json(status_lane_text(source.lane)),
    )
}

fn place_json(place: &CleanupPlace) -> String {
    format!(
        "{{\"kind\":\"cleanup_place\",\"storage\":{},\"projections\":{}}}",
        storage_json(&place.storage),
        array_json(&place.projections, |projection| quote_json(
            projection.as_str()
        )),
    )
}

fn storage_json(storage: &StorageId) -> String {
    match storage {
        StorageId::Value(value) => format!(
            "{{\"kind\":\"value\",\"value\":{}}}",
            quote_json(value.as_str())
        ),
        StorageId::Temporary(expression) => format!(
            "{{\"kind\":\"temporary\",\"expression\":{}}}",
            quote_json(expression.as_str())
        ),
        StorageId::CallArgument {
            call,
            parameter_index,
            value_expression,
        } => format!(
            "{{\"kind\":\"call_argument\",\"call\":{},\"parameter_index\":{},\"value_expression\":{}}}",
            quote_json(call.as_str()),
            parameter_index,
            quote_json(value_expression.as_str())
        ),
        StorageId::ProvisionalResult => "{\"kind\":\"provisional_result\"}".to_owned(),
    }
}

fn retryability_json(retryable: Retryability) -> String {
    match retryable {
        Retryability::Known(value) => value.to_string(),
        Retryability::Unknown => quote_json("unknown"),
    }
}

fn status_class_text(class: StatusClass) -> &'static str {
    match class {
        StatusClass::Contract => "contract",
        StatusClass::Arithmetic => "arithmetic",
        StatusClass::Import => "import",
        StatusClass::ExplicitClose => "explicit_close",
        StatusClass::Adapter => "adapter",
    }
}

fn status_lane_text(lane: StatusLane) -> &'static str {
    match lane {
        StatusLane::OperationFailure => "operation_failure",
        StatusLane::ContractFalse => "contract_false",
    }
}

fn array_json<T>(values: &[T], mut render: impl FnMut(&T) -> String) -> String {
    let values = values.iter().map(&mut render).collect::<Vec<_>>();
    format!("[{}]", values.join(","))
}

#[cfg(test)]
mod tests {
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
}
