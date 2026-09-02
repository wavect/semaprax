use std::path::Path;

use crate::cleanup_plan::{CleanupResultSource, CleanupTransition, ExitContinuation};
use crate::conformance::{
    CONTRACT_REQUIRES_FALSE_CODE, CONTRACT_STATUS_DOMAIN_V1, NORMALIZED_STATUS_SCHEMA_V1,
};
use crate::{hir, parse};

use super::*;

const SOURCE: &str = r#"module test.native_materialize;

@id("token.type")
resource Token {
@id("token.drop")
drop trivial;
}

@id("token.consume")
fn consume(value: own Token, allowed: bool) -> i64
requires allowed
{
7
}

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("native-materialize.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

fn wire_place(place: &CleanupPlace) -> WirePlace {
    assert!(place.projections.is_empty());
    let storage = match &place.storage {
        StorageId::Value(value) => WireStorage::Value {
            value_id: value.as_str().to_owned(),
        },
        StorageId::Temporary(expression) => WireStorage::Temporary {
            expression_id: expression.as_str().to_owned(),
        },
        StorageId::ProvisionalResult => WireStorage::ProvisionalResult,
        StorageId::CallArgument { .. } => panic!("fixture has no call arguments"),
    };
    WirePlace {
        storage,
        projections: Vec::new(),
    }
}

fn contract_wire_status() -> WireStatus {
    WireStatus {
        schema: NORMALIZED_STATUS_SCHEMA_V1.to_owned(),
        domain_id: CONTRACT_STATUS_DOMAIN_V1.to_owned(),
        code: CONTRACT_REQUIRES_FALSE_CODE,
        class: WireStatusClass::Contract,
        retryability: WireRetryability::False,
    }
}

fn successful_consume_wire(function: &ResolvedFunction) -> WireTrace {
    let finalizer = function
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
        .next()
        .unwrap();
    let result = function
        .cleanup_plan
        .exits
        .iter()
        .find_map(|exit| match &exit.continuation {
            ExitContinuation::CommitResult { source } => Some(source),
            _ => None,
        })
        .unwrap();
    let CleanupResultSource::Scalar { expression } = result else {
        panic!("consume has a scalar result")
    };
    let event = |kind| WireEvent {
        function_id: function.id.as_str().to_owned(),
        invocation: Vec::new(),
        kind,
    };
    WireTrace {
        scenario_id: "consume-success".to_owned(),
        root_function_id: function.id.as_str().to_owned(),
        events: vec![
            event(WireEventKind::FinalizeBegin {
                source: wire_place(&finalizer.source),
                lifecycle_id: finalizer.lifecycle_id.as_str().to_owned(),
                guard_flag: finalizer.guard_flag.0,
                binding_import_id: None,
            }),
            event(WireEventKind::FinalizeEnd {
                source: wire_place(&finalizer.source),
                lifecycle_id: finalizer.lifecycle_id.as_str().to_owned(),
                guard_flag: finalizer.guard_flag.0,
                binding_import_id: None,
            }),
            event(WireEventKind::ResultCommit {
                source: WireResultSource::Scalar {
                    expression_id: expression.as_str().to_owned(),
                },
            }),
        ],
        outcome: WireOutcome::Success(WireResult::I64(7)),
    }
}

#[test]
fn materializes_success_by_cloning_validated_plan_identities() {
    let program = program();
    let function = function(&program, "token.consume");
    let trace = materialize(&program, function, successful_consume_wire(function)).unwrap();

    assert_eq!(trace.root_function, function.id);
    assert_eq!(trace.events.len(), 3);
    assert!(matches!(
        &trace.events[0].event,
        TraceEventKind::FinalizeBegin {
            lifecycle_id,
            binding_import: None,
            ..
        } if lifecycle_id.as_str() == "token.drop"
    ));
    assert_eq!(
        trace.outcome,
        TraceOutcome::Success {
            result: TraceResult::I64(7)
        }
    );
}

#[test]
fn materializes_owned_transfer_and_result_source() {
    let program = program();
    let function = function(&program, "token.identity");
    let (at, source, destination) = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .find_map(|transition| match transition {
            CleanupTransition::Transfer {
                at,
                source,
                destination,
            } => Some((at, source, destination)),
            _ => None,
        })
        .unwrap();
    let result = function
        .cleanup_plan
        .exits
        .iter()
        .find_map(|exit| match &exit.continuation {
            ExitContinuation::CommitResult { source } => Some(source),
            _ => None,
        })
        .unwrap();
    let CleanupResultSource::Owned { storage } = result else {
        panic!("identity has an owned result")
    };
    let event = |kind| WireEvent {
        function_id: function.id.as_str().to_owned(),
        invocation: Vec::new(),
        kind,
    };
    let wire = WireTrace {
        scenario_id: "identity-success".to_owned(),
        root_function_id: function.id.as_str().to_owned(),
        events: vec![
            event(WireEventKind::Transfer {
                at: at.as_str().to_owned(),
                source: wire_place(source),
                destination: wire_place(destination),
            }),
            event(WireEventKind::ResultCommit {
                source: WireResultSource::Owned {
                    storage: wire_place(storage),
                },
            }),
        ],
        outcome: WireOutcome::Success(WireResult::Owned {
            type_id: "token.type".to_owned(),
        }),
    };

    let trace = materialize(&program, function, wire).unwrap();
    assert!(matches!(
        &trace.events[0].event,
        TraceEventKind::Transfer { at: actual, .. } if actual == at
    ));
    assert_eq!(
        trace.outcome,
        TraceOutcome::Success {
            result: TraceResult::Owned {
                type_id: program.types[0].id.clone()
            }
        }
    );
}

#[test]
fn materializes_only_the_trusted_contract_status() {
    let program = program();
    let function = function(&program, "token.consume");
    let source = function
        .cleanup_plan
        .status_sources
        .iter()
        .find(|source| {
            matches!(
                source.producer,
                StatusProducer::ContractFalse {
                    phase: ContractPhase::Requires,
                    ..
                }
            )
        })
        .unwrap();
    let wire_source = WireStatusSource {
        expression_id: source.id.expression.as_str().to_owned(),
        lane: WireStatusLane::ContractFalse,
    };
    let wire = WireTrace {
        scenario_id: "requires-false".to_owned(),
        root_function_id: function.id.as_str().to_owned(),
        events: vec![WireEvent {
            function_id: function.id.as_str().to_owned(),
            invocation: Vec::new(),
            kind: WireEventKind::SelectFailure {
                source: wire_source.clone(),
                status: contract_wire_status(),
            },
        }],
        outcome: WireOutcome::Failure {
            selected_source: wire_source,
            status: contract_wire_status(),
        },
    };

    let trace = materialize(&program, function, wire).unwrap();
    assert!(matches!(
        trace.outcome,
        TraceOutcome::Failure { status, .. }
            if status == NormalizedStatus::contract(ContractPhase::Requires)
    ));
}

#[test]
fn rejects_hostile_function_invocation_identity_and_status_fields() {
    let program = program();
    let function = function(&program, "token.consume");

    let mut wrong_root = successful_consume_wire(function);
    wrong_root.root_function_id = "other.function".to_owned();
    assert_eq!(
        materialize(&program, function, wrong_root),
        Err(MaterializeError::RootFunctionMismatch)
    );

    let mut wrong_event_function = successful_consume_wire(function);
    wrong_event_function.events[0].function_id = "other.function".to_owned();
    assert_eq!(
        materialize(&program, function, wrong_event_function),
        Err(MaterializeError::EventFunctionMismatch { event_index: 0 })
    );

    let mut nested = successful_consume_wire(function);
    nested.events[0]
        .invocation
        .push(function.body.id.as_str().to_owned());
    assert_eq!(
        materialize(&program, function, nested),
        Err(MaterializeError::NonRootInvocation { event_index: 0 })
    );

    let source = function.cleanup_plan.status_sources.first().unwrap();
    let mut forged = contract_wire_status();
    forged.code = CONTRACT_REQUIRES_FALSE_CODE + 1;
    let wire_source = WireStatusSource {
        expression_id: source.id.expression.as_str().to_owned(),
        lane: WireStatusLane::ContractFalse,
    };
    let forged_trace = WireTrace {
        scenario_id: "forged-status".to_owned(),
        root_function_id: function.id.as_str().to_owned(),
        events: vec![WireEvent {
            function_id: function.id.as_str().to_owned(),
            invocation: Vec::new(),
            kind: WireEventKind::SelectFailure {
                source: wire_source.clone(),
                status: forged.clone(),
            },
        }],
        outcome: WireOutcome::Failure {
            selected_source: wire_source,
            status: forged,
        },
    };
    assert_eq!(
        materialize(&program, function, forged_trace),
        Err(MaterializeError::StatusMismatch)
    );
}

#[test]
fn rejects_unknown_places_result_sources_and_unsupported_shapes() {
    let program = program();
    let identity = function(&program, "token.identity");
    let mut wire = successful_consume_wire(function(&program, "token.consume"));
    wire.root_function_id = identity.id.as_str().to_owned();
    wire.events[0].function_id = identity.id.as_str().to_owned();
    if let WireEventKind::FinalizeBegin { source, .. } = &mut wire.events[0].kind {
        source.storage = WireStorage::Value {
            value_id: "hostile.value".to_owned(),
        };
    }
    assert_eq!(
        materialize(&program, identity, wire),
        Err(MaterializeError::UnknownFinalizer)
    );

    let consume = function(&program, "token.consume");
    let mut projected = successful_consume_wire(consume);
    if let WireEventKind::FinalizeBegin { source, .. } = &mut projected.events[0].kind {
        source.projections.push("hostile.field".to_owned());
    }
    assert_eq!(
        materialize(&program, consume, projected),
        Err(MaterializeError::UnsupportedProjection)
    );

    let mut call_argument = successful_consume_wire(consume);
    if let WireEventKind::FinalizeBegin { source, .. } = &mut call_argument.events[0].kind {
        source.storage = WireStorage::CallArgument {
            call_id: "call".to_owned(),
            parameter_index: 0,
            value_expression_id: "value".to_owned(),
        };
    }
    assert_eq!(
        materialize(&program, consume, call_argument),
        Err(MaterializeError::UnsupportedCallArgumentStorage)
    );

    let mut result = successful_consume_wire(consume);
    if let WireEventKind::ResultCommit { source } = &mut result.events[2].kind {
        *source = WireResultSource::Scalar {
            expression_id: "hostile.expression".to_owned(),
        };
    }
    assert_eq!(
        materialize(&program, consume, result),
        Err(MaterializeError::UnknownResultSource)
    );
}
