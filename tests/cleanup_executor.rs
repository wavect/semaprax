use std::path::Path;

use semaprax::cleanup_plan::{
    execute_for_conformance, CleanupExecutionError, CleanupResultSource, CleanupScenario,
    CleanupTransition, ContractPhase, EdgeCondition, ExitContinuation, StatusCase, StatusLane,
    StatusProducer, StatusSourceId,
};
use semaprax::conformance::{
    ImportSite, InvocationPath, NormalizedStatus, OperationOutcome, Retryability, StatusClass,
    TraceEvent, TraceEventKind, TraceOutcome, TraceResult,
};
use semaprax::hir::{self, DeclarationId, ResolvedFunction, ResolvedProgram};
use semaprax::parse;

const SOURCE: &str = r#"module test.cleanup_executor;
permit { io.release }

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("file.type")
resource File {
    @id("file.drop")
    drop import "file.finalize";
}

@id("file.host")
interface FileHost permits { io.release } {
    @id("file.finalize")
    import fn finalize(file: own File) -> unit
        effects { io.release }
        failure infallible
        consumes file always;
}

@id("scalar.success")
fn scalar_success() -> i64 { 42 }

@id("contract.failure")
fn contract_failure(value: i64) -> i64
    requires value > 0
{
    value
}

@id("token.discard")
fn discard_token(value: own Token) -> i64 { 0 }

@id("file.discard")
fn discard_file(value: own File) -> i64 uses { io.release } { 0 }

@id("flow.choose")
fn choose(condition: bool) -> i64 {
    if condition { 1 } else { 2 }
}

@id("op.add")
fn checked_add(value: i64) -> i64 { value + 1 }

@id("token.identity")
fn identity_token(value: own Token) -> Token { value }

@id("token.take-two")
fn take_two(first: own Token, second: own Token) -> i64 { 0 }

@id("token.forward")
fn forward(first: own Token, second: own Token) -> i64 {
    take_two(first, second)
}

@id("token.later-failure")
fn later_failure(first: own Token, second: own Token) -> i64 {
    take_two(first, identity_token(second))
}

@id("token.ensure-owned")
fn ensure_owned(value: own Token) -> Token
    ensures result == result
{
    value
}

@id("token.branch-cleanup")
fn branch_cleanup(condition: bool, value: own Token) -> i64 {
    if condition { discard_token(value) } else { 1 }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("cleanup-executor.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap_or_else(|| panic!("missing function `{id}`"))
}

fn trace_event(function: &ResolvedFunction, event: TraceEventKind) -> TraceEvent {
    TraceEvent {
        function: function.id.clone(),
        invocation: InvocationPath::default(),
        event,
    }
}

fn result_source(function: &ResolvedFunction) -> CleanupResultSource {
    function
        .cleanup_plan
        .exits
        .iter()
        .find_map(|exit| match &exit.continuation {
            ExitContinuation::CommitResult { source } => Some(source.clone()),
            ExitContinuation::Continue(_)
            | ExitContinuation::ReturnFailure { .. }
            | ExitContinuation::ReturnUnit => None,
        })
        .unwrap_or_else(|| panic!("missing result source for `{}`", function.id))
}

fn operation_source(function: &ResolvedFunction, callee: &str) -> StatusSourceId {
    function
        .cleanup_plan
        .status_sources
        .iter()
        .find_map(|source| match &source.producer {
            StatusProducer::PropagatedCall { callee: actual } if actual.as_str() == callee => {
                Some(source.id.clone())
            }
            StatusProducer::CheckedArithmetic { .. }
            | StatusProducer::ContractFalse { .. }
            | StatusProducer::PropagatedCall { .. } => None,
        })
        .unwrap_or_else(|| panic!("missing call source for `{callee}` in `{}`", function.id))
}

fn transitions(function: &ResolvedFunction) -> Vec<CleanupTransition> {
    function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| block.transitions.iter().cloned())
        .collect()
}

fn callee_for_call(
    function: &ResolvedFunction,
    call: &semaprax::hir::ExpressionId,
) -> DeclarationId {
    function
        .cleanup_plan
        .status_sources
        .iter()
        .find_map(|source| match &source.producer {
            StatusProducer::PropagatedCall { callee } if source.id.expression == *call => {
                Some(callee.clone())
            }
            StatusProducer::CheckedArithmetic { .. }
            | StatusProducer::ContractFalse { .. }
            | StatusProducer::PropagatedCall { .. } => None,
        })
        .unwrap_or_else(|| panic!("missing callee for `{call}` in `{}`", function.id))
}

fn transition_event(
    function: &ResolvedFunction,
    transition: &CleanupTransition,
) -> Option<TraceEvent> {
    let event = match transition {
        CleanupTransition::Initialize { at, destination } => TraceEventKind::Initialize {
            at: at.clone(),
            destination: destination.clone(),
        },
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        } => TraceEventKind::Transfer {
            at: at.clone(),
            source: source.clone(),
            destination: destination.clone(),
        },
        CleanupTransition::CallCommit { call, arguments } => TraceEventKind::CallCommit {
            call: call.clone(),
            callee: callee_for_call(function, call),
            arguments: arguments.clone(),
        },
        CleanupTransition::SelectFailure { .. } => return None,
    };
    Some(trace_event(function, event))
}

#[test]
fn scalar_success_has_exact_public_json_without_physical_data() {
    let program = program();
    let function = function(&program, "scalar.success");
    let trace = execute_for_conformance(
        &program,
        &function.id,
        CleanupScenario::new("scalar-success", Some(TraceResult::I64(42))),
    )
    .unwrap();

    let expected = "{\"schema\":\"semaprax.conformance-trace.v1\",\"scenario_id\":\"scalar-success\",\"root_function\":\"scalar.success\",\"events\":[{\"kind\":\"result_commit\",\"function\":\"scalar.success\",\"invocation\":[],\"source\":{\"kind\":\"scalar\",\"expression\":\"declaration:14:scalar.success:expression:4:body\"}}],\"outcome\":{\"kind\":\"success\",\"selected_source\":null,\"status\":null,\"result_published\":true,\"result\":{\"kind\":\"i64\",\"value\":\"42\"}}}";
    let json = trace.to_json();
    assert_eq!(json, expected);
    assert!(!json.contains("pointer"));
    assert!(!json.contains("handle"));
    assert!(!json.contains("status_token"));
    assert_eq!(json, trace.to_json());
}

#[test]
fn contract_failure_selects_exact_status_and_never_commits_a_result() {
    let program = program();
    let function = function(&program, "contract.failure");
    let expression = function.requires[0].id.clone();
    let source = StatusSourceId {
        expression: expression.clone(),
        lane: StatusLane::ContractFalse,
    };
    let status = NormalizedStatus::contract(ContractPhase::Requires);
    let mut scenario = CleanupScenario::new("contract-failure", None);
    scenario.booleans.insert(expression, false);
    scenario.context_nonce = 17;

    let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();
    assert_eq!(
        trace.events,
        vec![TraceEvent {
            function: function.id.clone(),
            invocation: InvocationPath::default(),
            event: TraceEventKind::SelectFailure {
                source: source.clone(),
                status: status.clone(),
            },
        }]
    );
    assert_eq!(
        trace.outcome,
        TraceOutcome::Failure {
            selected_source: source,
            status,
        }
    );
    assert!(!trace
        .events
        .iter()
        .any(|event| matches!(event.event, TraceEventKind::ResultCommit { .. })));
    assert!(!trace.to_json().contains("\"result_published\":true"));
}

#[test]
fn owned_entry_trivial_finalization_precedes_scalar_commit() {
    let program = program();
    let function = function(&program, "token.discard");
    let action = function.cleanup_plan.exits[0].finalize_in_order[0].clone();
    let result_source = match &function.cleanup_plan.exits[0].continuation {
        ExitContinuation::CommitResult { source } => source.clone(),
        continuation => panic!("unexpected continuation: {continuation:?}"),
    };

    let trace = execute_for_conformance(
        &program,
        &function.id,
        CleanupScenario::new("trivial-finalizer", Some(TraceResult::I64(0))),
    )
    .unwrap();
    let prefix = |event| TraceEvent {
        function: function.id.clone(),
        invocation: InvocationPath::default(),
        event,
    };
    assert_eq!(
        trace.events,
        vec![
            prefix(TraceEventKind::FinalizeBegin {
                source: action.source.clone(),
                lifecycle_id: action.lifecycle_id.clone(),
                guard_flag: action.guard_flag,
                binding_import: None,
            }),
            prefix(TraceEventKind::FinalizeEnd {
                source: action.source,
                lifecycle_id: action.lifecycle_id,
                guard_flag: action.guard_flag,
                binding_import: None,
            }),
            prefix(TraceEventKind::ResultCommit {
                source: result_source,
            }),
        ]
    );
}

#[test]
fn imported_finalizer_has_exact_success_event_order() {
    let program = program();
    let function = function(&program, "file.discard");
    let exit = function
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| matches!(exit.continuation, ExitContinuation::CommitResult { .. }))
        .unwrap();
    let action = exit.finalize_in_order[0].clone();
    let ExitContinuation::CommitResult { source } = &exit.continuation else {
        unreachable!()
    };
    let import = DeclarationId::new("file.finalize");
    let mut scenario = CleanupScenario::new("imported-finalizer", Some(TraceResult::I64(0)));
    scenario.available_finalizer_imports.insert(import.clone());

    let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();
    let prefix = |event| TraceEvent {
        function: function.id.clone(),
        invocation: InvocationPath::default(),
        event,
    };
    assert_eq!(
        trace.events,
        vec![
            prefix(TraceEventKind::FinalizeBegin {
                source: action.source.clone(),
                lifecycle_id: action.lifecycle_id.clone(),
                guard_flag: action.guard_flag,
                binding_import: Some(import.clone()),
            }),
            prefix(TraceEventKind::ImportBegin {
                site: ImportSite::Finalizer {
                    source: action.source.clone(),
                    lifecycle_id: action.lifecycle_id.clone(),
                },
                import_id: import.clone(),
            }),
            prefix(TraceEventKind::FinalizerImportEnd {
                source: action.source.clone(),
                lifecycle_id: action.lifecycle_id.clone(),
                import_id: import.clone(),
            }),
            prefix(TraceEventKind::FinalizeEnd {
                source: action.source,
                lifecycle_id: action.lifecycle_id,
                guard_flag: action.guard_flag,
                binding_import: Some(import),
            }),
            prefix(TraceEventKind::ResultCommit {
                source: source.clone(),
            }),
        ]
    );
}

#[test]
fn missing_and_unused_scenario_decisions_are_rejected() {
    let program = program();
    let choose = function(&program, "flow.choose");
    let condition = choose
        .cleanup_plan
        .edges
        .iter()
        .find_map(|edge| match &edge.condition {
            EdgeCondition::BooleanResult(expression, _) => Some(expression.clone()),
            EdgeCondition::Always
            | EdgeCondition::VariantCase { .. }
            | EdgeCondition::StatusZero(_)
            | EdgeCondition::StatusNonzero(_) => None,
        })
        .unwrap();
    assert_eq!(
        execute_for_conformance(
            &program,
            &choose.id,
            CleanupScenario::new("missing-boolean", Some(TraceResult::I64(1))),
        ),
        Err(CleanupExecutionError::MissingBooleanDecision(
            condition.clone()
        ))
    );

    let scalar = function(&program, "scalar.success");
    let mut unused_boolean = CleanupScenario::new("unused-boolean", Some(TraceResult::I64(42)));
    unused_boolean.booleans.insert(condition, true);
    assert!(matches!(
        execute_for_conformance(&program, &scalar.id, unused_boolean),
        Err(CleanupExecutionError::UnusedBooleanDecisions(expressions)) if expressions.len() == 1
    ));

    let checked = function(&program, "op.add");
    let checked_source = checked.cleanup_plan.status_sources[0].id.clone();
    assert_eq!(
        execute_for_conformance(
            &program,
            &checked.id,
            CleanupScenario::new("missing-operation", Some(TraceResult::I64(1))),
        ),
        Err(CleanupExecutionError::MissingOperationOutcome(
            checked_source.clone()
        ))
    );

    let mut unused_operation = CleanupScenario::new("unused-operation", Some(TraceResult::I64(42)));
    unused_operation
        .operations
        .insert(checked_source, OperationOutcome::Success);
    assert!(matches!(
        execute_for_conformance(&program, &scalar.id, unused_operation),
        Err(CleanupExecutionError::UnusedOperationOutcomes(sources)) if sources.len() == 1
    ));
}

#[test]
fn checked_status_edges_emit_exact_success_and_failure_traces() {
    let program = program();
    let function = function(&program, "op.add");
    let source = function
        .cleanup_plan
        .status_sources
        .iter()
        .find(|source| matches!(source.producer, StatusProducer::CheckedArithmetic { .. }))
        .unwrap()
        .id
        .clone();

    let mut success = CleanupScenario::new("checked-success", Some(TraceResult::I64(42)));
    success
        .operations
        .insert(source.clone(), OperationOutcome::Success);
    let success_trace = execute_for_conformance(&program, &function.id, success).unwrap();
    assert_eq!(
        success_trace.events,
        vec![trace_event(
            function,
            TraceEventKind::ResultCommit {
                source: result_source(function),
            },
        )]
    );
    assert_eq!(
        success_trace.outcome,
        TraceOutcome::Success {
            result: TraceResult::I64(42),
        }
    );

    let status = NormalizedStatus::arithmetic(StatusCase::AddOverflow);
    let mut failure = CleanupScenario::new("checked-failure", None);
    failure
        .operations
        .insert(source.clone(), OperationOutcome::Failure(status.clone()));
    let failure_trace = execute_for_conformance(&program, &function.id, failure).unwrap();
    assert_eq!(
        failure_trace.events,
        vec![trace_event(
            function,
            TraceEventKind::SelectFailure {
                source: source.clone(),
                status: status.clone(),
            },
        )]
    );
    assert_eq!(
        failure_trace.outcome,
        TraceOutcome::Failure {
            selected_source: source,
            status,
        }
    );
}

#[test]
fn owned_transfer_and_atomic_call_commit_cover_supplied_success_and_failure() {
    let program = program();
    let function = function(&program, "token.forward");
    let source = operation_source(function, "token.take-two");
    let ownership_events = transitions(function)
        .iter()
        .filter_map(|transition| transition_event(function, transition))
        .collect::<Vec<_>>();
    assert!(matches!(
        ownership_events.as_slice(),
        [
            TraceEvent {
                event: TraceEventKind::Transfer { .. },
                ..
            },
            TraceEvent {
                event: TraceEventKind::Transfer { .. },
                ..
            },
            TraceEvent {
                event: TraceEventKind::CallCommit { .. },
                ..
            }
        ]
    ));

    let mut success = CleanupScenario::new("owned-call-success", Some(TraceResult::I64(0)));
    success
        .operations
        .insert(source.clone(), OperationOutcome::Success);
    let success_trace = execute_for_conformance(&program, &function.id, success).unwrap();
    let mut expected_success = ownership_events.clone();
    expected_success.push(trace_event(
        function,
        TraceEventKind::ResultCommit {
            source: result_source(function),
        },
    ));
    assert_eq!(success_trace.events, expected_success);
    assert_eq!(
        success_trace.outcome,
        TraceOutcome::Success {
            result: TraceResult::I64(0),
        }
    );

    // Recursive callee execution is not implemented yet. The reference lane
    // therefore exercises the documented supplied-outcome boundary here.
    let status = NormalizedStatus::try_new(
        "callee.failure.v1",
        7,
        StatusClass::Adapter,
        Retryability::Unknown,
    )
    .unwrap();
    let mut failure = CleanupScenario::new("owned-call-failure", None);
    failure
        .operations
        .insert(source.clone(), OperationOutcome::Failure(status.clone()));
    let failure_trace = execute_for_conformance(&program, &function.id, failure).unwrap();
    let mut expected_failure = ownership_events;
    expected_failure.push(trace_event(
        function,
        TraceEventKind::SelectFailure {
            source: source.clone(),
            status: status.clone(),
        },
    ));
    assert_eq!(failure_trace.events, expected_failure);
    assert_eq!(
        failure_trace.outcome,
        TraceOutcome::Failure {
            selected_source: source,
            status,
        }
    );
}

#[test]
fn later_argument_failure_cleans_the_earlier_caller_owned_epoch() {
    let program = program();
    let function = function(&program, "token.later-failure");
    let inner_source = operation_source(function, "token.identity");
    let outer_source = operation_source(function, "token.take-two");
    let all_transitions = transitions(function);
    let outer_call = outer_source.expression.clone();
    let inner_call = inner_source.expression.clone();
    let first_transfer = all_transitions
        .iter()
        .find(|transition| {
            matches!(
                transition,
                CleanupTransition::Transfer {
                    destination: semaprax::cleanup_plan::CleanupPlace {
                        storage: semaprax::cleanup_plan::StorageId::CallArgument {
                            call,
                            parameter_index: 0,
                            ..
                        },
                        ..
                    },
                    ..
                } if *call == outer_call
            )
        })
        .unwrap();
    let second_transfer = all_transitions
        .iter()
        .find(|transition| {
            matches!(
                transition,
                CleanupTransition::Transfer {
                    destination: semaprax::cleanup_plan::CleanupPlace {
                        storage: semaprax::cleanup_plan::StorageId::CallArgument {
                            call,
                            parameter_index: 0,
                            ..
                        },
                        ..
                    },
                    ..
                } if *call == inner_call
            )
        })
        .unwrap();
    let inner_commit = all_transitions
        .iter()
        .find(|transition| {
            matches!(transition, CleanupTransition::CallCommit { call, .. } if *call == inner_call)
        })
        .unwrap();
    let failure_exit = function
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| {
            matches!(
                &exit.continuation,
                ExitContinuation::ReturnFailure { source } if *source == inner_source
            )
        })
        .unwrap();
    assert_eq!(failure_exit.finalize_in_order.len(), 1);
    let cleanup = &failure_exit.finalize_in_order[0];
    let status = NormalizedStatus::try_new(
        "nested.failure.v1",
        9,
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .unwrap();
    let mut scenario = CleanupScenario::new("later-argument-failure", None);
    scenario.operations.insert(
        inner_source.clone(),
        OperationOutcome::Failure(status.clone()),
    );

    let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();
    assert_eq!(
        trace.events,
        vec![
            transition_event(function, first_transfer).unwrap(),
            transition_event(function, second_transfer).unwrap(),
            transition_event(function, inner_commit).unwrap(),
            trace_event(
                function,
                TraceEventKind::SelectFailure {
                    source: inner_source.clone(),
                    status: status.clone(),
                },
            ),
            trace_event(
                function,
                TraceEventKind::FinalizeBegin {
                    source: cleanup.source.clone(),
                    lifecycle_id: cleanup.lifecycle_id.clone(),
                    guard_flag: cleanup.guard_flag,
                    binding_import: None,
                },
            ),
            trace_event(
                function,
                TraceEventKind::FinalizeEnd {
                    source: cleanup.source.clone(),
                    lifecycle_id: cleanup.lifecycle_id.clone(),
                    guard_flag: cleanup.guard_flag,
                    binding_import: None,
                },
            ),
        ]
    );
    assert!(!trace.events.iter().any(|event| {
        matches!(
            &event.event,
            TraceEventKind::CallCommit { call, .. } if *call == outer_call
        )
    }));
    assert_eq!(
        trace.outcome,
        TraceOutcome::Failure {
            selected_source: inner_source,
            status,
        }
    );
}

#[test]
fn ensures_failure_finalizes_the_provisional_owned_result_without_publication() {
    let program = program();
    let function = function(&program, "token.ensure-owned");
    let source = function
        .cleanup_plan
        .status_sources
        .iter()
        .find_map(|source| match source.producer {
            StatusProducer::ContractFalse {
                phase: ContractPhase::Ensures,
                ..
            } => Some(source.id.clone()),
            StatusProducer::CheckedArithmetic { .. }
            | StatusProducer::ContractFalse { .. }
            | StatusProducer::PropagatedCall { .. } => None,
        })
        .unwrap();
    let transfers = transitions(function)
        .into_iter()
        .filter(|transition| matches!(transition, CleanupTransition::Transfer { .. }))
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 2);
    let failure_exit = function
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| {
            matches!(
                &exit.continuation,
                ExitContinuation::ReturnFailure { source: actual } if *actual == source
            )
        })
        .unwrap();
    assert_eq!(failure_exit.finalize_in_order.len(), 1);
    let cleanup = &failure_exit.finalize_in_order[0];
    let status = NormalizedStatus::contract(ContractPhase::Ensures);
    let mut scenario = CleanupScenario::new("owned-ensures-failure", None);
    scenario.booleans.insert(source.expression.clone(), false);

    let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();
    let mut expected = transfers
        .iter()
        .map(|transfer| transition_event(function, transfer).unwrap())
        .collect::<Vec<_>>();
    expected.extend([
        trace_event(
            function,
            TraceEventKind::SelectFailure {
                source: source.clone(),
                status: status.clone(),
            },
        ),
        trace_event(
            function,
            TraceEventKind::FinalizeBegin {
                source: cleanup.source.clone(),
                lifecycle_id: cleanup.lifecycle_id.clone(),
                guard_flag: cleanup.guard_flag,
                binding_import: None,
            },
        ),
        trace_event(
            function,
            TraceEventKind::FinalizeEnd {
                source: cleanup.source.clone(),
                lifecycle_id: cleanup.lifecycle_id.clone(),
                guard_flag: cleanup.guard_flag,
                binding_import: None,
            },
        ),
    ]);
    assert_eq!(trace.events, expected);
    assert_eq!(
        trace.outcome,
        TraceOutcome::Failure {
            selected_source: source,
            status,
        }
    );
    assert!(!trace
        .events
        .iter()
        .any(|event| matches!(event.event, TraceEventKind::ResultCommit { .. })));
}

#[test]
fn untaken_owned_branch_finalizes_before_scalar_publication() {
    let program = program();
    let function = function(&program, "token.branch-cleanup");
    let condition = function
        .cleanup_plan
        .edges
        .iter()
        .find_map(|edge| match &edge.condition {
            EdgeCondition::BooleanResult(expression, _) => Some(expression.clone()),
            EdgeCondition::Always
            | EdgeCondition::VariantCase { .. }
            | EdgeCondition::StatusZero(_)
            | EdgeCondition::StatusNonzero(_) => None,
        })
        .unwrap();
    let result = result_source(function);
    let success_exit = function
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| {
            matches!(
                &exit.continuation,
                ExitContinuation::CommitResult { source } if *source == result
            ) && !exit.finalize_in_order.is_empty()
        })
        .unwrap();
    assert_eq!(success_exit.finalize_in_order.len(), 1);
    let cleanup = &success_exit.finalize_in_order[0];
    let mut scenario = CleanupScenario::new("branch-owned-cleanup", Some(TraceResult::I64(1)));
    scenario.booleans.insert(condition, false);

    let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();
    assert_eq!(
        trace.events,
        vec![
            trace_event(
                function,
                TraceEventKind::FinalizeBegin {
                    source: cleanup.source.clone(),
                    lifecycle_id: cleanup.lifecycle_id.clone(),
                    guard_flag: cleanup.guard_flag,
                    binding_import: None,
                },
            ),
            trace_event(
                function,
                TraceEventKind::FinalizeEnd {
                    source: cleanup.source.clone(),
                    lifecycle_id: cleanup.lifecycle_id.clone(),
                    guard_flag: cleanup.guard_flag,
                    binding_import: None,
                },
            ),
            trace_event(function, TraceEventKind::ResultCommit { source: result }),
        ]
    );
    assert_eq!(
        trace.outcome,
        TraceOutcome::Success {
            result: TraceResult::I64(1),
        }
    );
}

#[test]
fn complete_owned_resource_result_is_transferred_and_published_once() {
    let program = program();
    let function = function(&program, "token.identity");
    let transfers = transitions(function)
        .into_iter()
        .filter(|transition| matches!(transition, CleanupTransition::Transfer { .. }))
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 2);
    let result = TraceResult::Owned {
        type_id: DeclarationId::new("token.type"),
    };
    let trace = execute_for_conformance(
        &program,
        &function.id,
        CleanupScenario::new("owned-result", Some(result.clone())),
    )
    .unwrap();

    let mut expected = transfers
        .iter()
        .map(|transfer| transition_event(function, transfer).unwrap())
        .collect::<Vec<_>>();
    expected.push(trace_event(
        function,
        TraceEventKind::ResultCommit {
            source: result_source(function),
        },
    ));
    assert_eq!(trace.events, expected);
    assert_eq!(trace.outcome, TraceOutcome::Success { result });
    assert_eq!(
        trace
            .events
            .iter()
            .filter(|event| matches!(event.event, TraceEventKind::ResultCommit { .. }))
            .count(),
        1
    );
}

#[test]
fn public_result_commit_source_remains_scalar_for_the_fixture() {
    let program = program();
    let function = function(&program, "scalar.success");
    assert!(matches!(
        function.cleanup_plan.exits[0].continuation,
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Scalar { .. }
        }
    ));
}

#[test]
fn executor_replays_every_function_in_the_bound_program() {
    let mut program = program();
    let unrelated = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "op.add")
        .unwrap();
    unrelated.cleanup_plan.status_sources.clear();
    let selected = DeclarationId::new("scalar.success");

    assert!(matches!(
        execute_for_conformance(
            &program,
            &selected,
            CleanupScenario::new("hostile-unrelated", Some(TraceResult::I64(42))),
        ),
        Err(CleanupExecutionError::InvalidProgram(detail))
            if detail.contains("do not exactly cover typed HIR failure producers")
    ));
}

#[test]
fn executor_normalizes_nul_inventory_identity_before_inventory_rebuild() {
    let mut program = program();
    let selected = DeclarationId::new("token.discard");
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id == selected)
        .unwrap();
    function.cleanup.flags[0].lifecycle = DeclarationId::new("token.drop\0forged");

    let error = execute_for_conformance(
        &program,
        &selected,
        CleanupScenario::new("hostile-inventory-nul", Some(TraceResult::I64(0))),
    )
    .unwrap_err();
    let CleanupExecutionError::InvalidProgram(detail) = error else {
        panic!("expected invalid-program failure")
    };
    assert!(detail.contains("error[SPX-H006]: cleanup inventory lifecycle identity contains NUL"));
    assert!(!detail.contains('\0'));
}

#[test]
fn executor_normalizes_nul_plan_identity_before_plan_replay() {
    let mut program = program();
    let selected = DeclarationId::new("token.discard");
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id == selected)
        .unwrap();
    let finalizer = function
        .cleanup_plan
        .exits
        .iter_mut()
        .find_map(|exit| exit.finalize_in_order.first_mut())
        .expect("discard must finalize its parameter");
    finalizer.lifecycle_id = DeclarationId::new("token.drop\0forged");

    let error = execute_for_conformance(
        &program,
        &selected,
        CleanupScenario::new("hostile-plan-nul", Some(TraceResult::I64(0))),
    )
    .unwrap_err();
    let CleanupExecutionError::InvalidProgram(detail) = error else {
        panic!("expected invalid-program failure")
    };
    assert!(
        detail.contains("error[SPX-H006]: cleanup-plan finalizer lifecycle identity contains NUL")
    );
    assert!(!detail.contains('\0'));
}
