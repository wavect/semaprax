use std::path::Path;

use semaprax::cleanup::{FieldLivenessShape, LivenessFlagId};
use semaprax::cleanup_plan::{
    CheckedOperation, CleanupPlan, CleanupResultSource, CleanupTerminator, CleanupTransition,
    ContractPhase, EdgeCondition, ExitContinuation, FinalizeAction, StatusCase, StatusLane,
    StatusProducer, StorageId,
};
use semaprax::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedProgram};
use semaprax::{codegen, parse, wasm};

const SOURCE: &str = r#"module test.cleanup_plan;

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

@id("inner.type")
record Inner {
    @id("inner.left")
    left: Token,

    @id("inner.right")
    right: Token,
}

@id("outer.type")
record Outer {
    @id("outer.inner")
    inner: Inner,

    @id("outer.tail")
    tail: Token,
}

@id("token.identity")
fn identity(value: own Token) -> Token {
    value
}

@id("token.take-two")
fn take_two(first: own Token, second: own Token) -> i64 {
    0
}

@id("token.consume")
fn consume(value: own Token) -> bool {
    true
}

@id("token.forward")
fn forward(first: own Token, second: own Token) -> i64 {
    take_two(first, second)
}

@id("token.later-failure")
fn later_failure(first: own Token, second: own Token) -> i64 {
    take_two(first, identity(second))
}

@id("pair.partial")
fn partial(first: own Token, second: own Token) -> Pair {
    Pair {
        second: second,
        first: identity(first),
    }
}

@id("pair.update-one")
fn update_one(pair: own Pair, second: own Token) -> Pair {
    pair with { second: second }
}

@id("pair.update-partial-failure")
fn update_partial_failure(
    pair: own Pair,
    first: own Token,
    second: own Token
) -> Pair {
    pair with {
        second: second,
        first: identity(first),
    }
}

@id("pair.discard")
fn discard(first: own Token, second: own Token) -> i64 {
    let pair = Pair {
        second: second,
        first: first,
    };
    0
}

@id("pair.take-first")
fn take_first(first: own Token, second: own Token) -> Token {
    let pair = Pair {
        second: second,
        first: first,
    };
    pair.first
}

@id("pair.ensure-owned")
fn ensure_owned(value: own Pair) -> Pair
    ensures result == result
{
    value
}

@id("outer.partial")
fn nested_partial(left: own Token, right: own Token, tail: own Token) -> Outer {
    Outer {
        inner: Inner {
            right: right,
            left: left,
        },
        tail: identity(tail),
    }
}

@id("outer.update-inner")
fn update_inner(outer: own Outer, inner: own Inner) -> Outer {
    outer with { inner: inner }
}

@id("outer.discard")
fn nested_discard(left: own Token, right: own Token, tail: own Token) -> i64 {
    let outer = Outer {
        tail: tail,
        inner: Inner {
            right: right,
            left: left,
        },
    };
    0
}

@id("outer.take-left")
fn nested_take_left(left: own Token, right: own Token, tail: own Token) -> Token {
    let outer = Outer {
        tail: tail,
        inner: Inner {
            right: right,
            left: left,
        },
    };
    outer.inner.left
}

@id("op.neg")
fn checked_neg(value: i64) -> i64 {
    -value
}

@id("op.add")
fn checked_add(left: i64, right: i64) -> i64 {
    left + right
}

@id("op.sub")
fn checked_sub(left: i64, right: i64) -> i64 {
    left - right
}

@id("op.mul")
fn checked_mul(left: i64, right: i64) -> i64 {
    left * right
}

@id("op.div")
fn checked_div(left: i64, right: i64) -> i64 {
    left / right
}

@id("op.rem")
fn checked_rem(left: i64, right: i64) -> i64 {
    left % right
}

@id("contract.scalar")
fn contracted(value: i64) -> i64
    requires value > 0
    requires value != 7
    ensures result > value
    ensures result != 10
{
    value + 1
}

@id("flow.choose")
fn choose(condition: bool) -> i64 {
    if condition { 1 } else { 2 }
}

@id("flow.and")
fn both(left: bool, right: bool) -> bool {
    left && right
}

@id("flow.or")
fn either(left: bool, right: bool) -> bool {
    left || right
}

@id("flow.lazy-owned")
fn lazy_owned(condition: bool, first: own Token, second: own Token) -> bool {
    condition && {
        let held = identity(first);
        consume(second)
    }
}

@id("flow.lazy-temp")
fn lazy_temp(condition: bool, first: own Token, second: own Token) -> bool {
    condition && identity(first) == identity(second)
}

@id("flow.if-owned")
fn if_owned(condition: bool, first: own Token, second: own Token) -> bool {
    if condition {
        let held = identity(first);
        consume(second)
    } else {
        false
    }
}

@id("app.main")
fn main() -> i64 {
    0
}
"#;

fn resolved() -> ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("cleanup-plan.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap_or_else(|| panic!("missing function `{id}`"))
}

fn block_tail(function: &ResolvedFunction) -> &ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("function body must be a resolved block")
    };
    tail
}

fn success_exit(plan: &CleanupPlan) -> &semaprax::cleanup_plan::ExitTarget {
    plan.exits
        .iter()
        .find(|exit| matches!(exit.continuation, ExitContinuation::CommitResult { .. }))
        .expect("cleanup plan must have one success exit")
}

fn failure_exit<'a>(
    plan: &'a CleanupPlan,
    expression: &semaprax::hir::ExpressionId,
) -> &'a semaprax::cleanup_plan::ExitTarget {
    plan.exits
        .iter()
        .find(|exit| {
            matches!(
                &exit.continuation,
                ExitContinuation::ReturnFailure { source }
                    if source.expression == *expression
            )
        })
        .expect("cleanup plan must expose the requested failure exit")
}

#[test]
fn scalar_result_has_an_explicit_commit_without_cleanup_storage() {
    let program = resolved();
    let main = function(&program, "app.main");
    let plan = &main.cleanup_plan;

    assert!(plan.slots.is_empty());
    assert!(plan.entry_state.live_owned_parameters.is_empty());
    assert!(plan.status_sources.is_empty());
    assert_eq!(plan.blocks.len(), 1);
    assert!(matches!(
        plan.blocks[0].terminator,
        CleanupTerminator::Exit(_)
    ));
    assert!(matches!(
        &success_exit(plan).continuation,
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Scalar { expression }
        } if expression == &main.body.id
    ));
}

#[test]
fn checked_operations_publish_exact_status_cases_and_branches() {
    let program = resolved();
    let expected = [
        (
            "op.neg",
            CheckedOperation::Neg,
            vec![StatusCase::NegationOverflow],
        ),
        (
            "op.add",
            CheckedOperation::Add,
            vec![StatusCase::AddOverflow],
        ),
        (
            "op.sub",
            CheckedOperation::Sub,
            vec![StatusCase::SubOverflow],
        ),
        (
            "op.mul",
            CheckedOperation::Mul,
            vec![StatusCase::MulOverflow],
        ),
        (
            "op.div",
            CheckedOperation::Div,
            vec![StatusCase::DivisionByZero, StatusCase::DivisionOverflow],
        ),
        (
            "op.rem",
            CheckedOperation::Rem,
            vec![StatusCase::RemainderByZero, StatusCase::RemainderOverflow],
        ),
    ];

    for (id, operation, cases) in expected {
        let function = function(&program, id);
        let plan = &function.cleanup_plan;
        assert_eq!(plan.status_sources.len(), 1, "unexpected sources in {id}");
        let source = &plan.status_sources[0];
        assert_eq!(source.id.expression, block_tail(function).id);
        assert_eq!(source.id.lane, StatusLane::OperationFailure);
        assert!(matches!(
            &source.producer,
            StatusProducer::CheckedArithmetic {
                operation: actual_operation,
                normalized_cases,
            } if *actual_operation == operation && *normalized_cases == cases
        ));
        assert_eq!(
            cases.iter().map(|case| case.code()).collect::<Vec<_>>(),
            match operation {
                CheckedOperation::Neg => vec![8],
                CheckedOperation::Add => vec![1],
                CheckedOperation::Sub => vec![2],
                CheckedOperation::Mul => vec![3],
                CheckedOperation::Div => vec![4, 5],
                CheckedOperation::Rem => vec![6, 7],
            }
        );

        let conditions = plan
            .edges
            .iter()
            .filter_map(|edge| match &edge.condition {
                EdgeCondition::StatusZero(candidate) if candidate == &source.id => Some(true),
                EdgeCondition::StatusNonzero(candidate) if candidate == &source.id => Some(false),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(conditions, [true, false], "status edge order in {id}");
        assert!(matches!(
            &failure_exit(plan, &source.id.expression).continuation,
            ExitContinuation::ReturnFailure { source: returned } if returned == &source.id
        ));
    }
}

#[test]
fn contract_false_sources_are_distinct_and_ordered_around_body_failures() {
    let program = resolved();
    let function = function(&program, "contract.scalar");
    let plan = &function.cleanup_plan;
    let expected = [
        (
            function.requires[0].id.clone(),
            StatusLane::ContractFalse,
            Some((ContractPhase::Requires, 0)),
        ),
        (
            function.requires[1].id.clone(),
            StatusLane::ContractFalse,
            Some((ContractPhase::Requires, 1)),
        ),
        (
            block_tail(function).id.clone(),
            StatusLane::OperationFailure,
            None,
        ),
        (
            function.ensures[0].id.clone(),
            StatusLane::ContractFalse,
            Some((ContractPhase::Ensures, 0)),
        ),
        (
            function.ensures[1].id.clone(),
            StatusLane::ContractFalse,
            Some((ContractPhase::Ensures, 1)),
        ),
    ];
    assert_eq!(plan.status_sources.len(), expected.len());

    for (source, (expression, lane, contract)) in plan.status_sources.iter().zip(expected) {
        assert_eq!(source.id.expression, expression);
        assert_eq!(source.id.lane, lane);
        match contract {
            Some((phase, ordinal)) => assert!(matches!(
                source.producer,
                StatusProducer::ContractFalse {
                    phase: actual_phase,
                    ordinal: actual_ordinal,
                } if actual_phase == phase && actual_ordinal == ordinal
            )),
            None => assert!(matches!(
                source.producer,
                StatusProducer::CheckedArithmetic {
                    operation: CheckedOperation::Add,
                    ..
                }
            )),
        }
    }

    for contract in function.requires.iter().chain(&function.ensures) {
        let outcomes = plan
            .edges
            .iter()
            .filter_map(|edge| match &edge.condition {
                EdgeCondition::BooleanResult(expression, value) if expression == &contract.id => {
                    Some(*value)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(outcomes, [true, false]);
    }
}

#[test]
fn if_and_lazy_boolean_cfg_preserve_semantic_branch_order() {
    let program = resolved();

    let choose = function(&program, "flow.choose");
    let ResolvedExprKind::If { condition, .. } = &block_tail(choose).kind else {
        panic!("choose tail must be an if")
    };
    assert_eq!(
        boolean_outcomes(&choose.cleanup_plan, &condition.id),
        [true, false]
    );

    let both = function(&program, "flow.and");
    let ResolvedExprKind::Binary { left, .. } = &block_tail(both).kind else {
        panic!("both tail must be a binary expression")
    };
    assert_eq!(
        boolean_outcomes(&both.cleanup_plan, &left.id),
        [true, false],
        "and evaluates the right operand on true before its false skip edge"
    );

    let either = function(&program, "flow.or");
    let ResolvedExprKind::Binary { left, .. } = &block_tail(either).kind else {
        panic!("either tail must be a binary expression")
    };
    assert_eq!(
        boolean_outcomes(&either.cleanup_plan, &left.id),
        [false, true],
        "or evaluates the right operand on false before its true skip edge"
    );
}

fn boolean_outcomes(plan: &CleanupPlan, expression: &semaprax::hir::ExpressionId) -> Vec<bool> {
    plan.edges
        .iter()
        .filter_map(|edge| match &edge.condition {
            EdgeCondition::BooleanResult(candidate, value) if candidate == expression => {
                Some(*value)
            }
            _ => None,
        })
        .collect()
}

fn assert_finalizer(
    plan: &CleanupPlan,
    action: &FinalizeAction,
    storage: &StorageId,
    projections: &[&str],
) {
    assert_eq!(&action.source.storage, storage);
    assert_eq!(
        action
            .source
            .projections
            .iter()
            .map(|projection| projection.as_str())
            .collect::<Vec<_>>(),
        projections
    );
    assert_eq!(action.lifecycle_id.as_str(), "token.drop");
    assert_eq!(
        action.guard_flag,
        flag_for_place(plan, storage, projections),
        "finalizer guard must be the exact leaf flag for its semantic place"
    );
}

fn flag_for_place(plan: &CleanupPlan, storage: &StorageId, projections: &[&str]) -> LivenessFlagId {
    let slot = plan
        .slots
        .iter()
        .find(|slot| &slot.storage == storage)
        .expect("finalized storage must have a cleanup slot");
    let mut shape = &slot.field_liveness_shape;
    for projection in projections {
        let FieldLivenessShape::Record { fields, .. } = shape else {
            panic!("projection must traverse a record liveness shape")
        };
        shape = &fields
            .iter()
            .find(|field| field.field.as_str() == *projection)
            .expect("projection must name a field in the liveness shape")
            .shape;
    }
    let FieldLivenessShape::Leaf { flag, .. } = shape else {
        panic!("finalizer must identify one resource leaf")
    };
    *flag
}

#[test]
fn conditional_owned_call_temporaries_have_exact_guarded_cleanup_paths() {
    let program = resolved();

    for id in ["flow.lazy-owned", "flow.if-owned"] {
        let function = function(&program, id);
        let conditional = block_tail(function);
        let branch = match &conditional.kind {
            ResolvedExprKind::Binary { right, .. } if id == "flow.lazy-owned" => right.as_ref(),
            ResolvedExprKind::If { then_branch, .. } if id == "flow.if-owned" => {
                then_branch.as_ref()
            }
            _ => panic!("{id} must retain its conditional owned branch"),
        };
        let ResolvedExprKind::Block { statements, tail } = &branch.kind else {
            panic!("{id} owned branch must be a block")
        };
        let semaprax::hir::ResolvedStatement::Let {
            binding,
            value: identity_call,
            ..
        } = &statements[0];
        assert!(matches!(identity_call.kind, ResolvedExprKind::Call { .. }));
        let consume_call = tail.as_ref();
        assert!(matches!(consume_call.kind, ResolvedExprKind::Call { .. }));
        let plan = &function.cleanup_plan;

        assert!(plan.blocks.iter().flat_map(|block| &block.transitions).any(
            |transition| matches!(
                transition,
                CleanupTransition::Initialize { at, destination }
                    if at == &identity_call.id
                        && destination.storage == StorageId::Temporary(identity_call.id.clone())
            )
        ));
        assert!(plan.blocks.iter().flat_map(|block| &block.transitions).any(
            |transition| matches!(
                transition,
                CleanupTransition::Transfer { source, destination, .. }
                    if source.storage == StorageId::Temporary(identity_call.id.clone())
                        && destination.storage == StorageId::Value(binding.id.clone())
            )
        ));

        let identity_failure = failure_exit(plan, &identity_call.id);
        assert_eq!(identity_failure.finalize_in_order.len(), 1);
        assert_finalizer(
            plan,
            &identity_failure.finalize_in_order[0],
            &StorageId::Value(function.params[2].id.clone()),
            &[],
        );

        let consume_failure = failure_exit(plan, &consume_call.id);
        assert_eq!(consume_failure.finalize_in_order.len(), 1);
        assert_finalizer(
            plan,
            &consume_failure.finalize_in_order[0],
            &StorageId::Value(binding.id.clone()),
            &[],
        );

        let normal_branch_exits = plan
            .exits
            .iter()
            .filter(|exit| matches!(exit.continuation, ExitContinuation::Continue(_)))
            .filter(|exit| {
                exit.finalize_in_order
                    .iter()
                    .any(|action| action.source.storage == StorageId::Value(binding.id.clone()))
            })
            .collect::<Vec<_>>();
        assert_eq!(normal_branch_exits.len(), 1);
        assert_eq!(normal_branch_exits[0].finalize_in_order.len(), 1);
        assert_finalizer(
            plan,
            &normal_branch_exits[0].finalize_in_order[0],
            &StorageId::Value(binding.id.clone()),
            &[],
        );

        let guarded_root = &success_exit(plan).finalize_in_order;
        assert_eq!(guarded_root.len(), 2);
        assert_finalizer(
            plan,
            &guarded_root[0],
            &StorageId::Value(function.params[2].id.clone()),
            &[],
        );
        assert_finalizer(
            plan,
            &guarded_root[1],
            &StorageId::Value(function.params[1].id.clone()),
            &[],
        );
    }
}

#[test]
fn unnamed_owned_temporaries_remain_guarded_across_a_lazy_join() {
    let program = resolved();
    let function = function(&program, "flow.lazy-temp");
    let ResolvedExprKind::Binary { right, .. } = &block_tail(function).kind else {
        panic!("lazy_temp must be a lazy binary expression")
    };
    let ResolvedExprKind::Binary {
        left: first_call,
        right: second_call,
        ..
    } = &right.kind
    else {
        panic!("lazy_temp right operand must compare two owned calls")
    };
    assert!(matches!(first_call.kind, ResolvedExprKind::Call { .. }));
    assert!(matches!(second_call.kind, ResolvedExprKind::Call { .. }));

    let plan = &function.cleanup_plan;
    let first_temp = StorageId::Temporary(first_call.id.clone());
    let second_temp = StorageId::Temporary(second_call.id.clone());
    for (call, storage) in [
        (first_call.as_ref(), &first_temp),
        (second_call.as_ref(), &second_temp),
    ] {
        assert!(plan.blocks.iter().flat_map(|block| &block.transitions).any(
            |transition| matches!(
                transition,
                CleanupTransition::Initialize { at, destination }
                    if at == &call.id && &destination.storage == storage
            )
        ));
        assert!(!plan
            .exits
            .iter()
            .filter(|exit| matches!(exit.continuation, ExitContinuation::Continue(_)))
            .flat_map(|exit| &exit.finalize_in_order)
            .any(|action| &action.source.storage == storage));
    }

    let finalizers = &success_exit(plan).finalize_in_order;
    assert_eq!(finalizers.len(), 4);
    assert_finalizer(plan, &finalizers[0], &second_temp, &[]);
    assert_finalizer(plan, &finalizers[1], &first_temp, &[]);
    assert_finalizer(
        plan,
        &finalizers[2],
        &StorageId::Value(function.params[2].id.clone()),
        &[],
    );
    assert_finalizer(
        plan,
        &finalizers[3],
        &StorageId::Value(function.params[1].id.clone()),
        &[],
    );
}

#[test]
fn owned_arguments_use_caller_epochs_and_one_atomic_commit() {
    let program = resolved();
    let function = function(&program, "token.forward");
    let call = block_tail(function);
    let ResolvedExprKind::Call { args, .. } = &call.kind else {
        panic!("forward tail must be a call")
    };
    let plan = &function.cleanup_plan;

    let epochs = plan
        .slots
        .iter()
        .filter_map(|slot| match &slot.storage {
            StorageId::CallArgument {
                call: candidate,
                parameter_index,
                value_expression,
            } if candidate == &call.id => Some((
                *parameter_index,
                value_expression.clone(),
                slot.storage.clone(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(epochs.len(), 2);
    assert_eq!(epochs[0].0, 0);
    assert_eq!(epochs[0].1, args[0].id);
    assert_eq!(epochs[1].0, 1);
    assert_eq!(epochs[1].1, args[1].id);

    let mut sequence = Vec::new();
    for block in &plan.blocks {
        for transition in &block.transitions {
            match transition {
                CleanupTransition::Transfer { destination, .. } if matches!(destination.storage, StorageId::CallArgument { call: ref candidate, .. } if candidate == &call.id) => {
                    sequence.push("stage")
                }
                CleanupTransition::CallCommit {
                    call: candidate,
                    arguments,
                } if candidate == &call.id => {
                    assert_eq!(
                        arguments
                            .iter()
                            .map(|argument| argument.parameter_index)
                            .collect::<Vec<_>>(),
                        [0, 1]
                    );
                    assert_eq!(
                        arguments
                            .iter()
                            .map(|argument| argument.source.storage.clone())
                            .collect::<Vec<_>>(),
                        [epochs[0].2.clone(), epochs[1].2.clone()]
                    );
                    sequence.push("commit");
                }
                _ => {}
            }
        }
    }
    assert_eq!(sequence, ["stage", "stage", "commit"]);

    let failure = failure_exit(plan, &call.id);
    assert!(failure.finalize_in_order.is_empty());
}

#[test]
fn later_argument_failure_cleans_earlier_caller_epoch() {
    let program = resolved();
    let function = function(&program, "token.later-failure");
    let outer = block_tail(function);
    let ResolvedExprKind::Call { args, .. } = &outer.kind else {
        panic!("later_failure tail must be an outer call")
    };
    let inner = &args[1];
    assert!(matches!(inner.kind, ResolvedExprKind::Call { .. }));

    let failure = failure_exit(&function.cleanup_plan, &inner.id);
    assert_eq!(failure.finalize_in_order.len(), 1);
    assert!(matches!(
        &failure.finalize_in_order[0].source.storage,
        StorageId::CallArgument {
            call,
            parameter_index: 0,
            value_expression,
        } if call == &outer.id && value_expression == &args[0].id
    ));
}

#[test]
fn record_plans_cover_partial_construction_normalization_and_projection() {
    let program = resolved();

    let partial = function(&program, "pair.partial");
    let constructor = block_tail(partial);
    let ResolvedExprKind::ConstructRecord { fields, .. } = &constructor.kind else {
        panic!("partial tail must construct Pair")
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.field.as_str())
            .collect::<Vec<_>>(),
        ["pair.second", "pair.first"]
    );
    let field_destinations = partial
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter_map(|transition| match transition {
            CleanupTransition::Transfer { destination, .. }
                if destination.storage == StorageId::Temporary(constructor.id.clone())
                    && !destination.projections.is_empty() =>
            {
                Some(destination.projections[0].as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(field_destinations, ["pair.second", "pair.first"]);
    let ResolvedExprKind::Call { .. } = &fields[1].value.kind else {
        panic!("second initializer must be the failing identity call")
    };
    let partial_failure = failure_exit(&partial.cleanup_plan, &fields[1].value.id);
    assert_eq!(partial_failure.finalize_in_order.len(), 1);
    assert_eq!(
        partial_failure.finalize_in_order[0]
            .source
            .projections
            .iter()
            .map(|projection| projection.as_str())
            .collect::<Vec<_>>(),
        ["pair.second"]
    );

    let discard = function(&program, "pair.discard");
    let ResolvedExprKind::Block { statements, .. } = &discard.body.kind else {
        panic!("discard body must be a block")
    };
    let semaprax::hir::ResolvedStatement::Let { binding, .. } = &statements[0];
    let discard_finalizers = &success_exit(&discard.cleanup_plan).finalize_in_order;
    assert_eq!(discard_finalizers.len(), 2);
    assert!(discard_finalizers
        .iter()
        .all(|action| action.source.storage == StorageId::Value(binding.id.clone())));
    assert_eq!(
        discard_finalizers
            .iter()
            .map(|action| action.source.projections[0].as_str())
            .collect::<Vec<_>>(),
        ["pair.second", "pair.first"],
        "complete binding normalizes to declaration order, then finalizes in reverse"
    );

    let take_first = function(&program, "pair.take-first");
    let ResolvedExprKind::Block { statements, .. } = &take_first.body.kind else {
        panic!("take_first body must be a block")
    };
    let semaprax::hir::ResolvedStatement::Let { binding, .. } = &statements[0];
    let projected = take_first
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .any(|transition| {
            matches!(
                transition,
                CleanupTransition::Transfer { source, .. }
                    if source.storage == StorageId::Value(binding.id.clone())
                        && source.projections.iter().map(|id| id.as_str()).collect::<Vec<_>>()
                            == ["pair.first"]
            )
        });
    assert!(projected);
    let finalizers = &success_exit(&take_first.cleanup_plan).finalize_in_order;
    assert_eq!(finalizers.len(), 1);
    assert_eq!(
        finalizers[0].source.storage,
        StorageId::Value(binding.id.clone())
    );
    assert_eq!(
        finalizers[0]
            .source
            .projections
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        ["pair.second"]
    );
}

#[test]
fn record_update_consumes_base_then_replaces_left_to_right_and_cleans_exactly_once() {
    let program = resolved();

    let update = function(&program, "pair.update-one");
    let expression = block_tail(update);
    let ResolvedExprKind::UpdateRecord { base, fields, .. } = &expression.kind else {
        panic!("update_one tail must update Pair")
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].field.as_str(), "pair.second");
    let base_stage = StorageId::Temporary(base.id.clone());
    let destination = StorageId::Temporary(expression.id.clone());
    let transfers = update
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter_map(|transition| match transition {
            CleanupTransition::Transfer {
                source,
                destination,
                ..
            } => Some((source, destination)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 5);
    assert_eq!(
        transfers[0].1.storage, base_stage,
        "the whole base is consumed before any replacement"
    );
    assert_eq!(
        transfers[1].1.storage, destination,
        "the authored replacement is staged before untouched fields"
    );
    assert_eq!(transfers[1].1.projections[0].as_str(), "pair.second");
    assert_eq!(transfers[2].0.storage, base_stage);
    assert_eq!(transfers[2].0.projections[0].as_str(), "pair.first");
    assert_eq!(transfers[2].1.storage, destination);
    assert_eq!(transfers[2].1.projections[0].as_str(), "pair.first");
    assert_eq!(
        transfers[4].1.storage,
        StorageId::ProvisionalResult,
        "the completed update is transferred only after displaced cleanup"
    );

    let displaced_exit = update
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| {
            matches!(exit.continuation, ExitContinuation::Continue(_))
                && exit
                    .finalize_in_order
                    .iter()
                    .any(|action| action.source.storage == base_stage)
        })
        .expect("successful update must close its base epoch");
    assert_eq!(displaced_exit.finalize_in_order.len(), 1);
    assert_finalizer(
        &update.cleanup_plan,
        &displaced_exit.finalize_in_order[0],
        &base_stage,
        &["pair.second"],
    );

    let partial = function(&program, "pair.update-partial-failure");
    let expression = block_tail(partial);
    let ResolvedExprKind::UpdateRecord { base, fields, .. } = &expression.kind else {
        panic!("update_partial_failure tail must update Pair")
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.field.as_str())
            .collect::<Vec<_>>(),
        ["pair.second", "pair.first"]
    );
    let failure = failure_exit(&partial.cleanup_plan, &fields[1].value.id);
    let sources = failure
        .finalize_in_order
        .iter()
        .map(|action| {
            (
                &action.source.storage,
                action
                    .source
                    .projections
                    .iter()
                    .map(|projection| projection.as_str())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 3);
    assert_eq!(
        sources[0],
        (
            &StorageId::Temporary(expression.id.clone()),
            vec!["pair.second"]
        ),
        "the already-staged replacement unwinds before the base epoch"
    );
    assert_eq!(
        sources[1],
        (&StorageId::Temporary(base.id.clone()), vec!["pair.second"])
    );
    assert_eq!(
        sources[2],
        (&StorageId::Temporary(base.id.clone()), vec!["pair.first"])
    );
}

#[test]
fn nested_records_preserve_recursive_partial_and_reverse_cleanup_order() {
    let program = resolved();

    let partial = function(&program, "outer.partial");
    let constructor = block_tail(partial);
    let ResolvedExprKind::ConstructRecord { fields, .. } = &constructor.kind else {
        panic!("nested_partial tail must construct Outer")
    };
    let tail_initializer = fields
        .iter()
        .find(|field| field.field.as_str() == "outer.tail")
        .expect("Outer construction must initialize tail");
    assert!(matches!(
        tail_initializer.value.kind,
        ResolvedExprKind::Call { .. }
    ));
    let partial_failure = failure_exit(&partial.cleanup_plan, &tail_initializer.value.id);
    assert_eq!(partial_failure.finalize_in_order.len(), 2);
    let partial_storage = StorageId::Temporary(constructor.id.clone());
    assert_finalizer(
        &partial.cleanup_plan,
        &partial_failure.finalize_in_order[0],
        &partial_storage,
        &["outer.inner", "inner.right"],
    );
    assert_finalizer(
        &partial.cleanup_plan,
        &partial_failure.finalize_in_order[1],
        &partial_storage,
        &["outer.inner", "inner.left"],
    );

    let update = function(&program, "outer.update-inner");
    let expression = block_tail(update);
    let ResolvedExprKind::UpdateRecord { base, .. } = &expression.kind else {
        panic!("update_inner tail must update Outer")
    };
    let base_stage = StorageId::Temporary(base.id.clone());
    let displaced_exit = update
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| {
            matches!(exit.continuation, ExitContinuation::Continue(_))
                && exit
                    .finalize_in_order
                    .iter()
                    .any(|action| action.source.storage == base_stage)
        })
        .expect("nested update must close its base epoch");
    assert_eq!(displaced_exit.finalize_in_order.len(), 2);
    assert_finalizer(
        &update.cleanup_plan,
        &displaced_exit.finalize_in_order[0],
        &base_stage,
        &["outer.inner", "inner.right"],
    );
    assert_finalizer(
        &update.cleanup_plan,
        &displaced_exit.finalize_in_order[1],
        &base_stage,
        &["outer.inner", "inner.left"],
    );
    assert!(update
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .any(|transition| matches!(
            transition,
            CleanupTransition::Transfer { source, destination, .. }
                if source.storage == base_stage
                    && source.projections.iter().map(|id| id.as_str()).collect::<Vec<_>>()
                        == ["outer.tail"]
                    && destination.storage == StorageId::Temporary(expression.id.clone())
        )));

    let discard = function(&program, "outer.discard");
    let ResolvedExprKind::Block { statements, .. } = &discard.body.kind else {
        panic!("nested_discard body must be a block")
    };
    let semaprax::hir::ResolvedStatement::Let { binding, .. } = &statements[0];
    let binding_storage = StorageId::Value(binding.id.clone());
    let finalizers = &success_exit(&discard.cleanup_plan).finalize_in_order;
    assert_eq!(finalizers.len(), 3);
    assert_finalizer(
        &discard.cleanup_plan,
        &finalizers[0],
        &binding_storage,
        &["outer.tail"],
    );
    assert_finalizer(
        &discard.cleanup_plan,
        &finalizers[1],
        &binding_storage,
        &["outer.inner", "inner.right"],
    );
    assert_finalizer(
        &discard.cleanup_plan,
        &finalizers[2],
        &binding_storage,
        &["outer.inner", "inner.left"],
    );

    let take_left = function(&program, "outer.take-left");
    let ResolvedExprKind::Block { statements, .. } = &take_left.body.kind else {
        panic!("nested_take_left body must be a block")
    };
    let semaprax::hir::ResolvedStatement::Let { binding, .. } = &statements[0];
    let binding_storage = StorageId::Value(binding.id.clone());
    let moved_nested_projection = take_left
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .any(|transition| {
            matches!(
                transition,
                CleanupTransition::Transfer { source, .. }
                    if source.storage == binding_storage
                        && source
                            .projections
                            .iter()
                            .map(|projection| projection.as_str())
                            .collect::<Vec<_>>()
                            == ["outer.inner", "inner.left"]
            )
        });
    assert!(moved_nested_projection);
    let finalizers = &success_exit(&take_left.cleanup_plan).finalize_in_order;
    assert_eq!(finalizers.len(), 2);
    assert_finalizer(
        &take_left.cleanup_plan,
        &finalizers[0],
        &binding_storage,
        &["outer.tail"],
    );
    assert_finalizer(
        &take_left.cleanup_plan,
        &finalizers[1],
        &binding_storage,
        &["outer.inner", "inner.right"],
    );
}

#[test]
fn owned_result_postconditions_use_provisional_storage_and_reject_result_value_forgery() {
    let program = resolved();
    let function = function(&program, "pair.ensure-owned");
    let contract = &function.ensures[0];
    let plan = &function.cleanup_plan;

    assert!(matches!(
        &success_exit(plan).continuation,
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Owned { storage }
        } if storage.storage == StorageId::ProvisionalResult
            && storage.projections.is_empty()
    ));
    let failed_ensures = failure_exit(plan, &contract.id);
    assert_eq!(failed_ensures.finalize_in_order.len(), 2);
    assert!(failed_ensures
        .finalize_in_order
        .iter()
        .all(|action| action.source.storage == StorageId::ProvisionalResult));
    assert_eq!(
        failed_ensures
            .finalize_in_order
            .iter()
            .map(|action| action.source.projections[0].as_str())
            .collect::<Vec<_>>(),
        ["pair.second", "pair.first"]
    );

    let mut forged = program.clone();
    let function = function_mut(&mut forged, "pair.ensure-owned");
    let result_id = function.result_id.clone();
    let failed_ensures = function
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| {
            matches!(
                exit.continuation,
                ExitContinuation::ReturnFailure { ref source }
                    if source.expression == contract.id
            )
        })
        .expect("owned ensures must have a failure exit");
    failed_ensures.finalize_in_order[0].source.storage = StorageId::Value(result_id);
    assert_all_consumers_reject(&forged);
}

#[test]
fn cleanup_plans_are_deterministic_and_display_rename_invariant() {
    let original = resolved();
    assert_eq!(original, resolved());

    let renamed_source = SOURCE
        .replace("Token", "Capability")
        .replace("Pair", "Bundle");
    let renamed = hir::resolve(
        &parse(
            &renamed_source,
            Path::new("cleanup-plan-display-renamed.spx"),
        )
        .unwrap(),
    )
    .unwrap();
    let plans = |program: &ResolvedProgram| {
        program
            .functions
            .iter()
            .map(|function| (function.id.clone(), function.cleanup_plan.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(plans(&original), plans(&renamed));
}

#[test]
fn hostile_cleanup_plan_mutations_fail_closed_for_all_consumers() {
    let original = resolved();

    let mut reordered_sources = original.clone();
    function_mut(&mut reordered_sources, "contract.scalar")
        .cleanup_plan
        .status_sources
        .swap(0, 1);
    assert_all_consumers_reject(&reordered_sources);

    let mut missing_commit = original.clone();
    let plan = &mut function_mut(&mut missing_commit, "token.forward").cleanup_plan;
    let transitions = plan
        .blocks
        .iter_mut()
        .find_map(|block| {
            block
                .transitions
                .iter()
                .position(|transition| matches!(transition, CleanupTransition::CallCommit { .. }))
                .map(|position| (&mut block.transitions, position))
        })
        .expect("forward plan must have a call commit");
    transitions.0.remove(transitions.1);
    assert_all_consumers_reject(&missing_commit);

    let mut reordered_slots = original.clone();
    let slots = &mut function_mut(&mut reordered_slots, "pair.discard")
        .cleanup_plan
        .slots;
    assert!(slots.len() >= 2);
    slots.swap(0, 1);
    assert_all_consumers_reject(&reordered_slots);

    let mut wrong_result = original;
    let foreign_expression = function(&wrong_result, "contract.scalar").requires[0]
        .id
        .clone();
    let exit = function_mut(&mut wrong_result, "app.main")
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| matches!(exit.continuation, ExitContinuation::CommitResult { .. }))
        .expect("main must have a result commit");
    exit.continuation = ExitContinuation::CommitResult {
        source: CleanupResultSource::Scalar {
            expression: foreign_expression,
        },
    };
    assert_all_consumers_reject(&wrong_result);
}

fn function_mut<'a>(program: &'a mut ResolvedProgram, id: &str) -> &'a mut ResolvedFunction {
    program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == id)
        .unwrap_or_else(|| panic!("missing function `{id}`"))
}

fn assert_all_consumers_reject(program: &ResolvedProgram) {
    assert_eq!(hir::validate(program).unwrap_err().code, "SPX-H006");
    assert_eq!(codegen::emit_hir_c(program).unwrap_err().code, "SPX-H006");
    assert_eq!(
        wasm::emit_resolved_module(program).unwrap_err().code,
        "SPX-H006"
    );
}
