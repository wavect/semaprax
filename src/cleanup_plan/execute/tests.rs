use std::path::Path;

use crate::conformance::TraceEventKind;
use crate::{hir, parse};

use super::*;

fn adapter_status(domain: &str, code: u32) -> NormalizedStatus {
    NormalizedStatus::try_new(
        domain,
        code,
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .expect("test adapter status is normalized")
}

#[test]
fn compiler_operation_status_domains_are_exact_and_non_interchangeable() {
    let range = DeclarationId::new(crate::byte_ops::RANGE_ID);
    validate_propagated_status(
        &range,
        &adapter_status(
            crate::byte_ops::RANGE_STATUS_DOMAIN,
            crate::byte_ops::RANGE_START_AFTER_END_CODE,
        ),
    )
    .expect("first byte-range status is admitted");
    validate_propagated_status(
        &range,
        &adapter_status(
            crate::byte_ops::RANGE_STATUS_DOMAIN,
            crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
        ),
    )
    .expect("second byte-range status is admitted");
    assert!(validate_propagated_status(
        &range,
        &adapter_status(
            crate::command_io_ops::OUTPUT_STATUS_DOMAIN,
            crate::command_io_ops::OUTPUT_CAPACITY_EXCEEDED,
        ),
    )
    .is_err());

    let append = DeclarationId::new(crate::command_io_ops::STDOUT_APPEND_ID);
    validate_propagated_status(
        &append,
        &adapter_status(
            crate::command_io_ops::OUTPUT_STATUS_DOMAIN,
            crate::command_io_ops::OUTPUT_CAPACITY_EXCEEDED,
        ),
    )
    .expect("exact append capacity status is admitted");
    assert!(validate_propagated_status(
        &append,
        &adapter_status(
            crate::command_io_ops::INPUT_STATUS_DOMAIN,
            crate::command_io_ops::ARG_INDEX_OUT_OF_BOUNDS,
        ),
    )
    .is_err());
    assert!(validate_propagated_status(
        &append,
        &adapter_status(crate::command_io_ops::OUTPUT_STATUS_DOMAIN, 2),
    )
    .is_err());

    let infallible = DeclarationId::new(crate::command_io_ops::ARGS_LEN_ID);
    assert!(validate_propagated_status(
        &infallible,
        &adapter_status(crate::command_io_ops::INPUT_STATUS_DOMAIN, 1),
    )
    .is_err());
}

const SOURCE: &str = r#"module test.cleanup_execute;
permit { io.release }

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("pair.type")
record Pair {
    @id("pair.first")
    first: Token,

    @id("pair.second")
    second: Token,
}

@id("choice.type")
variant Choice {
    @id("choice.left")
    Left {
        @id("choice.left.value")
        value: i64,
    },

    @id("choice.right")
    Right {
        @id("choice.right.flag")
        flag: bool,
    },

    @id("choice.none")
    None,
}

@id("generic.choice")
variant GenericChoice<T> {
    @id("generic.choice.none")
    None,

    @id("generic.choice.value")
    Value {
        @id("generic.choice.value.value")
        value: T,
    },
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

@id("pair.identity")
fn identity_pair(value: own Pair) -> Pair { value }

@id("choice.checked")
fn checked_choice(choice: Choice, zero: i64) -> i64 {
    match choice {
        Choice::Left { value } => value,
        Choice::Right { flag } => if flag { 1 } else { 0 },
        Choice::None {} => 1 / zero,
    }
}

@id("generic.dual")
fn generic_dual(left: GenericChoice<i64>, right: GenericChoice<bool>) -> i64 {
    let first = match left {
        GenericChoice::Value { value } => value,
        GenericChoice::None {} => 0,
    };
    match right {
        GenericChoice::Value { value } => first,
        GenericChoice::None {} => 0,
    }
}

@id("file.discard")
fn discard_file(value: own File) -> i64 uses { io.release } { 0 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

const TRY_SOURCE: &str = r#"module test.execute_try;

@id("result.forward")
fn forward(value: Result<i64, bool>) -> Result<bool, bool>
    ensures true
{
    let number = value?;
    Result<bool, bool>::Ok { value: true }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("cleanup-execute.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

fn try_program() -> ResolvedProgram {
    let parsed = parse(TRY_SOURCE, Path::new("cleanup-execute-try.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

#[test]
fn scalar_success_commits_the_supplied_result() {
    let program = program();
    let function = function(&program, "scalar.success");
    let trace = execute_for_conformance(
        &program,
        &function.id,
        CleanupScenario::new("scalar-success", Some(TraceResult::I64(42))),
    )
    .unwrap();

    assert_eq!(
        trace.outcome,
        TraceOutcome::Success {
            result: TraceResult::I64(42)
        }
    );
    assert!(matches!(
        trace.events.as_slice(),
        [TraceEvent {
            event: TraceEventKind::ResultCommit {
                source: CleanupResultSource::Scalar { .. }
            },
            ..
        }]
    ));
}

#[test]
fn copy_variant_decision_executes_selected_arm_once() {
    let program = program();
    let function = function(&program, "choice.checked");
    let hir::ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("choice fixture must have a block body")
    };
    let hir::ResolvedExprKind::Match { scrutinee, .. } = &tail.kind else {
        panic!("choice fixture must end in a match")
    };
    let mut scenario = CleanupScenario::new("variant-left", Some(TraceResult::I64(42)));
    scenario
        .variant_cases
        .insert(scrutinee.id.clone(), DeclarationId::new("choice.left"));
    let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();
    assert_eq!(
        trace.outcome,
        TraceOutcome::Success {
            result: TraceResult::I64(42),
        }
    );
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
fn concrete_generic_instances_share_cases_without_sharing_cleanup_decisions() {
    let program = program();
    let function = function(&program, "generic.dual");
    let hir::ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        panic!("generic fixture must have a block body")
    };
    let hir::ResolvedStatement::Let { value: first, .. } = &statements[0] else {
        panic!("generic fixture first statement must be a let")
    };
    let hir::ResolvedExprKind::Match {
        scrutinee: first_scrutinee,
        ..
    } = &first.kind
    else {
        panic!("generic fixture first binding must be a match")
    };
    let hir::ResolvedExprKind::Match {
        scrutinee: second_scrutinee,
        ..
    } = &tail.kind
    else {
        panic!("generic fixture tail must be a match")
    };

    assert_ne!(first_scrutinee.id, second_scrutinee.id);
    let (
        ResolvedType::Nominal {
            declaration: first_declaration,
            arguments: first_arguments,
        },
        ResolvedType::Nominal {
            declaration: second_declaration,
            arguments: second_arguments,
        },
    ) = (&first_scrutinee.ty, &second_scrutinee.ty)
    else {
        panic!("generic match scrutinees must have concrete nominal types")
    };
    assert_eq!(first_declaration, second_declaration);
    assert_eq!(first_arguments, &[ResolvedType::I64]);
    assert_eq!(second_arguments, &[ResolvedType::Bool]);
    assert!(function.cleanup.slots.is_empty());
    assert!(function.cleanup_plan.slots.is_empty());

    let shared_case = DeclarationId::new("generic.choice.value");
    let mut scenario = CleanupScenario::new("generic-instances", Some(TraceResult::I64(42)));
    scenario
        .variant_cases
        .insert(first_scrutinee.id.clone(), shared_case.clone());
    scenario
        .variant_cases
        .insert(second_scrutinee.id.clone(), shared_case);
    let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();
    assert_eq!(
        trace.outcome,
        TraceOutcome::Success {
            result: TraceResult::I64(42),
        }
    );
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
fn copy_variant_decision_rejects_missing_foreign_and_unused_cases() {
    let program = program();
    let choice = function(&program, "choice.checked");
    let hir::ResolvedExprKind::Block { tail, .. } = &choice.body.kind else {
        panic!("choice fixture must have a block body")
    };
    let hir::ResolvedExprKind::Match { scrutinee, .. } = &tail.kind else {
        panic!("choice fixture must end in a match")
    };

    assert!(matches!(
        execute_for_conformance(
            &program,
            &choice.id,
            CleanupScenario::new("missing-variant", Some(TraceResult::I64(0))),
        ),
        Err(CleanupExecutionError::MissingVariantDecision(expression))
            if expression == scrutinee.id
    ));

    let mut foreign = CleanupScenario::new("foreign-variant", Some(TraceResult::I64(0)));
    foreign
        .variant_cases
        .insert(scrutinee.id.clone(), DeclarationId::new("choice.foreign"));
    assert!(matches!(
        execute_for_conformance(&program, &choice.id, foreign),
        Err(CleanupExecutionError::InvalidVariantDecision { scrutinee: expression, case })
            if expression == scrutinee.id && case.as_str() == "choice.foreign"
    ));

    let scalar = function(&program, "scalar.success");
    let mut unused = CleanupScenario::new("unused-variant", Some(TraceResult::I64(42)));
    unused
        .variant_cases
        .insert(scrutinee.id.clone(), DeclarationId::new("choice.left"));
    assert!(matches!(
        execute_for_conformance(&program, &scalar.id, unused),
        Err(CleanupExecutionError::UnusedVariantDecisions(expressions))
            if expressions == vec![scrutinee.id.clone()]
    ));
}

#[test]
fn failing_selected_variant_arm_keeps_the_result_poisoned() {
    let program = program();
    let function = function(&program, "choice.checked");
    let hir::ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("choice fixture must have a block body")
    };
    let hir::ResolvedExprKind::Match { scrutinee, .. } = &tail.kind else {
        panic!("choice fixture must end in a match")
    };
    let source = function
        .cleanup_plan
        .status_sources
        .iter()
        .find(|source| {
            matches!(
                source.producer,
                StatusProducer::CheckedArithmetic {
                    operation: super::super::CheckedOperation::Div,
                    ..
                }
            )
        })
        .expect("None arm must contain checked division")
        .id
        .clone();
    let mut scenario = CleanupScenario::new("variant-failure", None);
    scenario
        .variant_cases
        .insert(scrutinee.id.clone(), DeclarationId::new("choice.none"));
    scenario.operations.insert(
        source.clone(),
        OperationOutcome::Failure(NormalizedStatus::arithmetic(
            super::super::StatusCase::DivisionByZero,
        )),
    );
    let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();
    assert!(matches!(
        trace.outcome,
        TraceOutcome::Failure {
            selected_source,
            ..
        } if selected_source == source
    ));
    assert!(!trace
        .events
        .iter()
        .any(|event| matches!(event.event, TraceEventKind::ResultCommit { .. })));
}

#[test]
fn false_contract_selects_and_returns_the_exact_status() {
    let program = program();
    let function = function(&program, "contract.failure");
    let contract = function.requires[0].id.clone();
    let source = StatusSourceId {
        expression: contract.clone(),
        lane: StatusLane::ContractFalse,
    };
    let mut scenario = CleanupScenario::new("contract-failure", None);
    scenario.booleans.insert(contract, false);
    scenario.context_nonce = 9;
    let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();

    assert_eq!(
        trace.outcome,
        TraceOutcome::Failure {
            selected_source: source.clone(),
            status: NormalizedStatus::contract(super::super::ContractPhase::Requires),
        }
    );
    assert!(matches!(
        trace.events.as_slice(),
        [TraceEvent {
            event: TraceEventKind::SelectFailure {
                source: actual,
                ..
            },
            ..
        }] if actual == &source
    ));
}

#[test]
fn owned_entry_is_finalized_before_scalar_publication() {
    let program = program();
    let function = function(&program, "token.discard");
    let trace = execute_for_conformance(
        &program,
        &function.id,
        CleanupScenario::new("owned-finalizer", Some(TraceResult::I64(0))),
    )
    .unwrap();

    assert!(matches!(
        trace.events.as_slice(),
        [
            TraceEvent {
                event: TraceEventKind::FinalizeBegin {
                    binding_import: None,
                    ..
                },
                ..
            },
            TraceEvent {
                event: TraceEventKind::FinalizeEnd {
                    binding_import: None,
                    ..
                },
                ..
            },
            TraceEvent {
                event: TraceEventKind::ResultCommit { .. },
                ..
            }
        ]
    ));
}

#[test]
fn imported_finalizer_emits_split_success_completion() {
    let program = program();
    let function = function(&program, "file.discard");
    let mut scenario = CleanupScenario::new("imported-finalizer", Some(TraceResult::I64(0)));
    scenario
        .available_finalizer_imports
        .insert(DeclarationId::new("file.finalize"));
    let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();

    assert!(matches!(
        trace.events.as_slice(),
        [
            TraceEvent {
                event: TraceEventKind::FinalizeBegin {
                    binding_import: Some(import),
                    ..
                },
                ..
            },
            TraceEvent {
                event: TraceEventKind::ImportBegin { .. },
                ..
            },
            TraceEvent {
                event: TraceEventKind::FinalizerImportEnd { .. },
                ..
            },
            TraceEvent {
                event: TraceEventKind::FinalizeEnd { .. },
                ..
            },
            TraceEvent {
                event: TraceEventKind::ResultCommit { .. },
                ..
            }
        ] if import.as_str() == "file.finalize"
    ));
}

#[test]
fn finalizer_bindings_are_preflighted_but_need_not_be_path_used() {
    let program = program();
    let file = function(&program, "file.discard");
    assert!(matches!(
        execute_for_conformance(
            &program,
            &file.id,
            CleanupScenario::new("missing-binding", Some(TraceResult::I64(0))),
        ),
        Err(CleanupExecutionError::MissingFinalizerBinding(import))
            if import.as_str() == "file.finalize"
    ));

    let scalar = function(&program, "scalar.success");
    let mut unknown = CleanupScenario::new("unknown-binding", Some(TraceResult::I64(42)));
    unknown
        .available_finalizer_imports
        .insert(DeclarationId::new("missing.finalizer"));
    assert!(matches!(
        execute_for_conformance(&program, &scalar.id, unknown),
        Err(CleanupExecutionError::UnknownFinalizerBinding(import))
            if import.as_str() == "missing.finalizer"
    ));

    // Bindings are adapter configuration, not execution outcomes. A known
    // binding configured for a path/function that does not use it is valid.
    let mut configured = CleanupScenario::new("configured-unused", Some(TraceResult::I64(42)));
    configured
        .available_finalizer_imports
        .insert(DeclarationId::new("file.finalize"));
    assert!(execute_for_conformance(&program, &scalar.id, configured).is_ok());
}

#[test]
fn scalar_source_functions_reject_return_unit() {
    let program = program();
    let function = function(&program, "scalar.success");
    let mut executor =
        Executor::new(&program, function, CleanupScenario::new("unit-none", None)).unwrap();
    assert_eq!(
        executor.return_unit(),
        Err(CleanupExecutionError::HarnessInvariant(
            "ReturnUnit is invalid for source functions without a unit return type".to_owned(),
        ))
    );
    assert_eq!(executor.result_slot, ResultSlotState::Uninitialized);
    assert!(executor.events.is_empty());
}

#[test]
fn owned_publication_rejects_projected_and_incomplete_provisional_results() {
    let program = program();
    let function = function(&program, "pair.identity");
    let result = TraceResult::Owned {
        type_id: DeclarationId::new("pair.type"),
    };
    let projected_source = CleanupResultSource::Owned {
        storage: CleanupPlace {
            storage: StorageId::ProvisionalResult,
            projections: vec![DeclarationId::new("pair.first")],
        },
    };
    let mut projected = Executor::new(
        &program,
        function,
        CleanupScenario::new("projected-result", Some(result.clone())),
    )
    .unwrap();
    assert_eq!(
        projected.commit_result(projected_source),
        Err(CleanupExecutionError::HarnessInvariant(
            "supplied trace result disagrees with function result".to_owned(),
        ))
    );

    let whole_source = CleanupResultSource::Owned {
        storage: CleanupPlace {
            storage: StorageId::ProvisionalResult,
            projections: Vec::new(),
        },
    };
    let mut incomplete = Executor::new(
        &program,
        function,
        CleanupScenario::new("incomplete-result", Some(result)),
    )
    .unwrap();
    let one_result_flag = incomplete
        .leaves
        .iter()
        .find_map(|(flag, leaf)| {
            (leaf.place.storage == StorageId::ProvisionalResult).then_some(*flag)
        })
        .expect("pair result must have provisional liveness flags");
    incomplete.live.clear();
    incomplete.live.insert(one_result_flag);
    assert_eq!(
        incomplete.commit_result(whole_source),
        Err(CleanupExecutionError::HarnessInvariant(
            "owned result is incomplete at publication".to_owned(),
        ))
    );
    assert_eq!(incomplete.result_slot, ResultSlotState::Uninitialized);
    assert!(incomplete.scenario.result.is_some());
    assert!(incomplete.events.is_empty());
}

#[test]
fn public_executor_rejects_aggregate_result_projection_until_trace_v2() {
    let program = program();
    let function = function(&program, "pair.identity");
    assert!(matches!(
        execute_for_conformance(
            &program,
            &function.id,
            CleanupScenario::new(
                "aggregate-result",
                Some(TraceResult::Owned {
                    type_id: DeclarationId::new("pair.type"),
                }),
            ),
        ),
        Err(CleanupExecutionError::UnsupportedResultType(result))
            if result.contains("pair.type")
    ));
}

#[test]
fn contract_failure_keeps_the_caller_result_slot_poisoned() {
    let program = program();
    let function = function(&program, "contract.failure");
    let source = function
        .cleanup_plan
        .status_sources
        .iter()
        .find(|source| source.id.lane == StatusLane::ContractFalse)
        .unwrap()
        .id
        .clone();
    let mut executor = Executor::new(
        &program,
        function,
        CleanupScenario::new("contract-poison", None),
    )
    .unwrap();
    assert_eq!(executor.result_slot, ResultSlotState::Uninitialized);

    executor.select_failure(source.clone()).unwrap();
    let outcome = executor.return_failure(source).unwrap();
    assert!(matches!(outcome, TraceOutcome::Failure { .. }));
    assert_eq!(executor.result_slot, ResultSlotState::Uninitialized);
    assert!(!executor
        .events
        .iter()
        .any(|event| matches!(event.event, TraceEventKind::ResultCommit { .. })));
}

#[test]
fn result_publication_rejects_early_cleanup_and_duplicate_commits() {
    let program = program();
    let owned = function(&program, "token.discard");
    let owned_source = owned
        .cleanup_plan
        .exits
        .iter()
        .find_map(|exit| match &exit.continuation {
            ExitContinuation::CommitResult { source } => Some(source.clone()),
            ExitContinuation::Continue(_)
            | ExitContinuation::ReturnFailure { .. }
            | ExitContinuation::ReturnUnit => None,
        })
        .unwrap();
    let mut early = Executor::new(
        &program,
        owned,
        CleanupScenario::new("early-publication", Some(TraceResult::I64(0))),
    )
    .unwrap();
    assert_eq!(
        early.commit_result(owned_source),
        Err(CleanupExecutionError::HarnessInvariant(
            "result publication occurs before non-result cleanup".to_owned(),
        ))
    );
    assert_eq!(early.result_slot, ResultSlotState::Uninitialized);
    assert!(early.scenario.result.is_some());
    assert!(early.events.is_empty());

    let scalar = function(&program, "scalar.success");
    let scalar_source = scalar
        .cleanup_plan
        .exits
        .iter()
        .find_map(|exit| match &exit.continuation {
            ExitContinuation::CommitResult { source } => Some(source.clone()),
            ExitContinuation::Continue(_)
            | ExitContinuation::ReturnFailure { .. }
            | ExitContinuation::ReturnUnit => None,
        })
        .unwrap();
    let mut duplicate = Executor::new(
        &program,
        scalar,
        CleanupScenario::new("duplicate-publication", Some(TraceResult::I64(42))),
    )
    .unwrap();
    duplicate.commit_result(scalar_source.clone()).unwrap();
    assert_eq!(duplicate.result_slot, ResultSlotState::Published);
    duplicate.scenario.result = Some(TraceResult::I64(42));
    assert_eq!(
        duplicate.commit_result(scalar_source),
        Err(CleanupExecutionError::HarnessInvariant(
            "caller result slot is already published".to_owned(),
        ))
    );
    assert_eq!(duplicate.result_slot, ResultSlotState::Published);
    assert_eq!(
        duplicate
            .events
            .iter()
            .filter(|event| matches!(event.event, TraceEventKind::ResultCommit { .. }))
            .count(),
        1
    );
}

#[test]
fn try_executor_stages_each_normal_result_path_then_fails_closed_at_materialization() {
    let program = try_program();
    let function = function(&program, "result.forward");
    let residual = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .find_map(|transition| match transition {
            CleanupTransition::StageCopyResult {
                source: source @ StagedCopyResultSource::TryResidual { .. },
            } => Some(source.clone()),
            _ => None,
        })
        .unwrap();
    let StagedCopyResultSource::TryResidual {
        operand, ok_case, ..
    } = &residual
    else {
        unreachable!()
    };

    for selected_case in [ok_case.clone(), DeclarationId::new(prelude::RESULT_ERR_ID)] {
        let mut scenario = CleanupScenario::new("copy-result-closed", None);
        scenario
            .variant_cases
            .insert(operand.clone(), selected_case);
        for source in &function.cleanup_plan.status_sources {
            if source.id.lane == StatusLane::ContractFalse {
                scenario.booleans.insert(source.id.expression.clone(), true);
            }
        }
        let outcome = execute_for_conformance(&program, &function.id, scenario);
        assert!(
            matches!(
                outcome,
            Err(CleanupExecutionError::UnsupportedResultType(ref result))
                if result == &function.return_type.identity_key()
            ),
            "unexpected executor outcome: {outcome:?}"
        );
    }

    let mut executor = Executor::new(
        &program,
        function,
        CleanupScenario::new("staged-state", None),
    )
    .unwrap();
    executor.stage_copy_result(residual.clone()).unwrap();
    assert_eq!(executor.staged_copy_result, Some(residual.clone()));
    assert_eq!(
        executor.stage_copy_result(residual.clone()),
        Err(CleanupExecutionError::HarnessInvariant(
            "Copy result is staged more than once".to_owned(),
        ))
    );
    assert_eq!(executor.result_slot, ResultSlotState::Uninitialized);
    assert!(executor.events.is_empty());

    let scalar_source = function
        .cleanup_plan
        .exits
        .iter()
        .find_map(|exit| match &exit.continuation {
            ExitContinuation::CommitResult { source } => Some(source.clone()),
            ExitContinuation::Continue(_)
            | ExitContinuation::ReturnFailure { .. }
            | ExitContinuation::ReturnUnit => None,
        })
        .unwrap();
    let mut missing = Executor::new(
        &program,
        function,
        CleanupScenario::new("missing-stage", None),
    )
    .unwrap();
    assert_eq!(
        missing.commit_result(scalar_source),
        Err(CleanupExecutionError::HarnessInvariant(
            "Copy Result commit has no staged producer".to_owned(),
        ))
    );

    let mut wrong_target = residual;
    let StagedCopyResultSource::TryResidual {
        source_instance,
        target_instance,
        ..
    } = &mut wrong_target
    else {
        unreachable!()
    };
    *target_instance = source_instance.clone();
    let mut hostile = Executor::new(
        &program,
        function,
        CleanupScenario::new("wrong-target", None),
    )
    .unwrap();
    assert_eq!(
        hostile.stage_copy_result(wrong_target),
        Err(CleanupExecutionError::HarnessInvariant(
            "Try residual stage changes Result identities or target instance".to_owned(),
        ))
    );
}
