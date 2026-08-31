use std::path::Path;

use crate::cleanup_plan::{CleanupBlock, CleanupEdge};
use crate::{hir, parse};

use super::*;

#[path = "replay_tests/block_work.rs"]
mod block_work;

const SOURCE: &str = r#"module test.replay_paths;

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

@id("tokens.discard")
fn discard(left: own Token, right: own Token) -> i64 { 0 }

@id("math.checked")
fn checked(left: i64, right: i64) -> i64 { (left + right) * right }

@id("flow.bool")
fn bool_flow(first: bool, second: bool) -> i64 {
    if first { if second { 1 } else { 2 } } else { 3 }
}

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.forward")
fn forward(value: own Token) -> Token { identity(value) }

@id("pair.identity")
fn identity_pair(value: own Pair) -> Pair { value }

@id("pair.update-one")
fn update_one(pair: own Pair, second: own Token) -> Pair {
    pair with { second: second }
}

@id("pair.update-both")
fn update_both(pair: own Pair, first: own Token, second: own Token) -> Pair {
    pair with { second: second, first: first }
}

@id("pair.update-partial-failure")
fn update_partial_failure(
    pair: own Pair,
    first: own Token,
    second: own Token
) -> Pair {
    pair with { second: second, first: identity(first) }
}

@id("flow.regions")
fn region_flow(flag: bool, left: i64, right: i64) -> i64 {
    if flag { { left + right } } else { 0 }
}

@id("choice.select")
fn select(choice: Choice, zero: i64) -> i64 {
    match choice {
        Choice::Left { value } => value + 1,
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

@id("choice.make-left")
fn make_left(input: i64) -> Choice { Choice::Left { value: input } }

@id("choice.from-call")
fn from_call(input: i64, zero: i64) -> i64 {
    match make_left(input) {
        Choice::Left { value } => value,
        Choice::Right { flag } => if flag { 1 } else { 0 },
        Choice::None {} => 1 / zero,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const TRY_SOURCE: &str = r#"module test.replay_try;

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
    let parsed = parse(SOURCE, Path::new("cleanup-replay-paths.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

#[test]
fn byte_range_v4_replay_rejects_legacy_schema_substitution() {
    let source = r#"
module test.replay_byte_range;
@id("window.len")
fn window_len(value: borrow Slice<u8>, start: usize, end: usize) -> usize {
  byte_len(byte_range(value, start, end))
}
@id("main") fn main() -> i64 { 0 }
"#;
    let program = hir::resolve(
        &parse(source, Path::new("cleanup-replay-byte-range.spx")).expect("source parses"),
    )
    .expect("source resolves");
    let mut function = function(&program, "window.len");
    assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V4);
    validate_structure(&program, &function).expect("canonical range plan replays");
    function.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V3;
    let diagnostic = validate_structure(&program, &function)
        .expect_err("legacy schema substitution must fail closed");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(diagnostic.message.contains("schema"));
}

fn function(program: &ResolvedProgram, id: &str) -> ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .cloned()
        .unwrap()
}

fn try_program() -> ResolvedProgram {
    let parsed = parse(TRY_SOURCE, Path::new("cleanup-replay-try.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn update_expression(function: &ResolvedFunction) -> &ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("update fixture body must be a block")
    };
    assert!(matches!(tail.kind, ResolvedExprKind::UpdateRecord { .. }));
    tail
}

fn match_expression(function: &ResolvedFunction) -> &ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("match fixture body must be a block")
    };
    assert!(matches!(tail.kind, ResolvedExprKind::Match { .. }));
    tail
}

fn assert_independent_replay_rejects(program: &ResolvedProgram, function: &ResolvedFunction) {
    let diagnostic = validate_structure(program, function).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(diagnostic.message.contains("failed independent replay"));
}

#[test]
fn copy_variant_match_is_scrutinee_once_authored_order_and_cleanup_free() {
    let program = program();
    let function = function(&program, "choice.select");
    let expression = match_expression(&function);
    let ResolvedExprKind::Match {
        scrutinee, arms, ..
    } = &expression.kind
    else {
        unreachable!()
    };
    assert_eq!(arms.len(), 3);
    assert!(function.cleanup.slots.is_empty());
    assert!(function.cleanup_plan.slots.is_empty());

    let decisions = function
        .cleanup_plan
        .edges
        .iter()
        .filter_map(|edge| match &edge.condition {
            EdgeCondition::VariantCase {
                scrutinee,
                case,
                matches,
            } => Some((scrutinee.clone(), case.clone(), *matches)),
            EdgeCondition::Always
            | EdgeCondition::BooleanResult(_, _)
            | EdgeCondition::ArmSelected { .. }
            | EdgeCondition::StatusZero(_)
            | EdgeCondition::StatusNonzero(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        decisions,
        vec![
            (
                scrutinee.id.clone(),
                DeclarationId::new("choice.left"),
                true,
            ),
            (
                scrutinee.id.clone(),
                DeclarationId::new("choice.left"),
                false,
            ),
            (
                scrutinee.id.clone(),
                DeclarationId::new("choice.right"),
                true,
            ),
            (
                scrutinee.id.clone(),
                DeclarationId::new("choice.right"),
                false,
            ),
        ]
    );
    validate_structure(&program, &function).unwrap();
}

#[test]
fn generic_instance_matches_are_cleanup_free_and_replay_rejects_scrutinee_confusion() {
    let program = program();
    let original = function(&program, "generic.dual");
    let ResolvedExprKind::Block { statements, tail } = &original.body.kind else {
        panic!("generic fixture must have a block body")
    };
    let ResolvedStatement::Let { value: first, .. } = &statements[0] else {
        panic!("generic fixture first statement must be a let")
    };
    let ResolvedExprKind::Match {
        scrutinee: first_scrutinee,
        ..
    } = &first.kind
    else {
        panic!("generic fixture first binding must be a match")
    };
    let ResolvedExprKind::Match {
        scrutinee: second_scrutinee,
        ..
    } = &tail.kind
    else {
        panic!("generic fixture tail must be a match")
    };
    let first_id = first_scrutinee.id.clone();
    let second_id = second_scrutinee.id.clone();

    assert_ne!(first_id, second_id);
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
    assert!(original.cleanup.slots.is_empty());
    assert!(original.cleanup_plan.slots.is_empty());
    let first_cases = original
        .cleanup_plan
        .edges
        .iter()
        .filter_map(|edge| match &edge.condition {
            EdgeCondition::VariantCase {
                scrutinee, case, ..
            } if scrutinee == &first_id => Some(case.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let second_cases = original
        .cleanup_plan
        .edges
        .iter()
        .filter_map(|edge| match &edge.condition {
            EdgeCondition::VariantCase {
                scrutinee, case, ..
            } if scrutinee == &second_id => Some(case.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(first_cases, second_cases);
    assert_eq!(
        first_cases,
        BTreeSet::from([DeclarationId::new("generic.choice.value")])
    );
    validate_structure(&program, &original).unwrap();

    let mut confused = original;
    for edge in &mut confused.cleanup_plan.edges {
        if let EdgeCondition::VariantCase { scrutinee, .. } = &mut edge.condition {
            if scrutinee == &first_id {
                *scrutinee = second_id.clone();
            }
        }
    }
    assert_independent_replay_rejects(&program, &confused);
}

#[test]
fn match_scrutinee_call_is_lowered_and_replayed_exactly_once() {
    let program = program();
    let function = function(&program, "choice.from-call");
    let expression = match_expression(&function);
    let ResolvedExprKind::Match { scrutinee, .. } = &expression.kind else {
        unreachable!()
    };
    assert!(matches!(scrutinee.kind, ResolvedExprKind::Call { .. }));
    assert_eq!(
        function
            .cleanup_plan
            .status_sources
            .iter()
            .filter(|source| source.id.expression == scrutinee.id)
            .count(),
        1,
        "the match scrutinee call must produce one status epoch"
    );
    assert!(function.cleanup_plan.edges.iter().all(|edge| {
        !matches!(
            &edge.condition,
            EdgeCondition::VariantCase {
                scrutinee: candidate,
                ..
            } if candidate != &scrutinee.id
        )
    }));
    validate_structure(&program, &function).unwrap();
}

#[test]
fn match_replay_rejects_authored_case_scrutinee_and_polarity_confusion() {
    let program = program();
    let original = function(&program, "choice.select");
    let match_id = match_expression(&original).id.clone();

    let mut wrong_case = original.clone();
    for edge in &mut wrong_case.cleanup_plan.edges {
        if let EdgeCondition::VariantCase { case, .. } = &mut edge.condition {
            if case.as_str() == "choice.left" {
                *case = DeclarationId::new("choice.right");
            }
        }
    }
    assert_independent_replay_rejects(&program, &wrong_case);

    let mut wrong_scrutinee = original.clone();
    for edge in &mut wrong_scrutinee.cleanup_plan.edges {
        if let EdgeCondition::VariantCase { scrutinee, .. } = &mut edge.condition {
            *scrutinee = match_id.clone();
        }
    }
    assert_independent_replay_rejects(&program, &wrong_scrutinee);

    let mut same_polarity = original;
    let first_pair = same_polarity
        .cleanup_plan
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            CleanupTerminator::Branch(edges)
                if edges.iter().all(|id| {
                    matches!(
                        same_polarity.cleanup_plan.edges[id.0 as usize].condition,
                        EdgeCondition::VariantCase { .. }
                    )
                }) =>
            {
                Some(edges.clone())
            }
            CleanupTerminator::Goto(_)
            | CleanupTerminator::Branch(_)
            | CleanupTerminator::Exit(_) => None,
        })
        .expect("first match decision must be a variant branch");
    let EdgeCondition::VariantCase { matches, .. } =
        &mut same_polarity.cleanup_plan.edges[first_pair[1].0 as usize].condition
    else {
        unreachable!()
    };
    *matches = true;
    assert_independent_replay_rejects(&program, &same_polarity);
}

#[test]
fn match_checked_arm_failure_cannot_publish_the_poisoned_result() {
    let program = program();
    let mut function = function(&program, "choice.select");
    let match_result = match_expression(&function).id.clone();
    let division = function
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
        .expect("final match arm must contain checked division")
        .id
        .clone();
    let exit = function
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| {
            matches!(
                &exit.continuation,
                ExitContinuation::ReturnFailure { source } if source == &division
            )
        })
        .expect("checked arm must retain a failure exit");
    exit.continuation = ExitContinuation::CommitResult {
        source: CleanupResultSource::Scalar {
            expression: match_result,
        },
    };
    assert_independent_replay_rejects(&program, &function);
}

#[test]
fn update_replay_rejects_missing_base_and_untouched_transfers() {
    let program = program();
    let original = function(&program, "pair.update-one");
    let update = update_expression(&original);
    let ResolvedExprKind::UpdateRecord { base, .. } = &update.kind else {
        unreachable!()
    };
    let base_stage = StorageId::Temporary(base.id.clone());
    let destination = StorageId::Temporary(update.id.clone());

    let mut missing_base = original.clone();
    let (transitions, position) = missing_base
        .cleanup_plan
        .blocks
        .iter_mut()
        .find_map(|block| {
            block
                .transitions
                .iter()
                .position(|transition| {
                    matches!(
                        transition,
                        CleanupTransition::Transfer { destination, .. }
                            if destination.storage == base_stage
                                && destination.projections.is_empty()
                    )
                })
                .map(|position| (&mut block.transitions, position))
        })
        .expect("update must stage its complete base");
    transitions.remove(position);
    assert_independent_replay_rejects(&program, &missing_base);

    let mut missing_untouched = original;
    let (transitions, position) = missing_untouched
            .cleanup_plan
            .blocks
            .iter_mut()
            .find_map(|block| {
                block
                    .transitions
                    .iter()
                    .position(|transition| matches!(
                        transition,
                        CleanupTransition::Transfer { source, destination: target, .. }
                            if source.storage == base_stage
                                && source.projections.iter().map(|id| id.as_str()).collect::<Vec<_>>()
                                    == ["pair.first"]
                                && target.storage == destination
                    ))
                    .map(|position| (&mut block.transitions, position))
            })
            .expect("update must transfer its untouched first field");
    transitions.remove(position);
    assert_independent_replay_rejects(&program, &missing_untouched);
}

#[test]
fn update_replay_rejects_reordered_authored_replacements_and_displaced_finalizers() {
    let program = program();
    let original = function(&program, "pair.update-both");
    let update = update_expression(&original);
    let ResolvedExprKind::UpdateRecord { base, .. } = &update.kind else {
        unreachable!()
    };
    let base_stage = StorageId::Temporary(base.id.clone());
    let destination = StorageId::Temporary(update.id.clone());

    let mut reordered = original.clone();
    let block = reordered
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .transitions
                .iter()
                .filter(|transition| {
                    matches!(
                        transition,
                        CleanupTransition::Transfer { destination: target, .. }
                            if target.storage == destination && !target.projections.is_empty()
                    )
                })
                .count()
                == 2
        })
        .expect("both authored replacements must be in their evaluation block");
    let replacement_positions = block
        .transitions
        .iter()
        .enumerate()
        .filter_map(|(index, transition)| {
            matches!(
                transition,
                CleanupTransition::Transfer { destination: target, .. }
                    if target.storage == destination && !target.projections.is_empty()
            )
            .then_some(index)
        })
        .collect::<Vec<_>>();
    block
        .transitions
        .swap(replacement_positions[0], replacement_positions[1]);
    assert_independent_replay_rejects(&program, &reordered);

    let mut reordered_displaced = original;
    let exit = reordered_displaced
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| {
            matches!(exit.continuation, ExitContinuation::Continue(_))
                && exit.finalize_in_order.len() == 2
                && exit
                    .finalize_in_order
                    .iter()
                    .all(|action| action.source.storage == base_stage)
        })
        .expect("successful update must finalize both displaced fields");
    exit.finalize_in_order.swap(0, 1);
    assert_independent_replay_rejects(&program, &reordered_displaced);
}

#[test]
fn update_replay_rejects_partial_failure_and_child_region_mutations() {
    let program = program();

    let mut partial = function(&program, "pair.update-partial-failure");
    let update = update_expression(&partial).clone();
    let ResolvedExprKind::UpdateRecord { fields, .. } = &update.kind else {
        unreachable!()
    };
    let failing = fields[1].value.id.clone();
    let exit = partial
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| {
            matches!(
                &exit.continuation,
                ExitContinuation::ReturnFailure { source } if source.expression == failing
            )
        })
        .expect("second replacement call must have a failure exit");
    assert!(exit.finalize_in_order.len() >= 3);
    exit.finalize_in_order.remove(0);
    assert_independent_replay_rejects(&program, &partial);

    let mut wrong_region = function(&program, "pair.update-one");
    let update = update_expression(&wrong_region).clone();
    let ResolvedExprKind::UpdateRecord { base, .. } = &update.kind else {
        unreachable!()
    };
    let base_stage = StorageId::Temporary(base.id.clone());
    let exit = wrong_region
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| {
            matches!(exit.continuation, ExitContinuation::Continue(_))
                && exit
                    .finalize_in_order
                    .iter()
                    .any(|action| action.source.storage == base_stage)
        })
        .expect("update must leave its child base epoch");
    exit.leaves_regions.clear();
    assert_independent_replay_rejects(&program, &wrong_region);
}

#[test]
fn path_replay_rejects_non_reverse_live_finalizer_order() {
    let program = program();
    let mut function = function(&program, "tokens.discard");
    let exit = function
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| matches!(exit.continuation, ExitContinuation::CommitResult { .. }))
        .unwrap();
    assert_eq!(exit.finalize_in_order.len(), 2);
    exit.finalize_in_order.swap(0, 1);

    let diagnostic = validate_structure(&program, &function).unwrap_err();
    assert!(diagnostic.message.contains("reverse initialization order"));
}

#[test]
fn path_replay_requires_selection_from_the_failing_edge() {
    let program = program();
    let mut function = function(&program, "math.checked");
    let first = function.cleanup_plan.status_sources[0].id.clone();
    let second = function.cleanup_plan.status_sources[1].id.clone();
    let exit = function
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| {
            matches!(
                &exit.continuation,
                ExitContinuation::ReturnFailure { source } if source == &first
            )
        })
        .unwrap();
    let block = &mut function.cleanup_plan.blocks[exit.from.0 as usize];
    let CleanupTransition::SelectFailure { source } = &mut block.transitions[0] else {
        panic!("checked failure block must select its source");
    };
    *source = second.clone();
    exit.continuation = ExitContinuation::ReturnFailure { source: second };

    let diagnostic = validate_structure(&program, &function).unwrap_err();
    assert!(diagnostic.message.contains("pending failing edge"));
}

#[test]
fn inventory_replay_rejects_a_coherently_deleted_owned_slot() {
    let program = program();
    let mut function = function(&program, "tokens.discard");
    let removed = function.cleanup_plan.slots.pop().unwrap();
    let removed_flag = match removed.field_liveness_shape {
        FieldLivenessShape::Leaf { flag, .. } => flag,
        FieldLivenessShape::NoDrop
        | FieldLivenessShape::Record { .. }
        | FieldLivenessShape::Variant { .. } => {
            panic!("fixture token slot must be one leaf")
        }
    };
    function.cleanup_plan.regions[0]
        .slots
        .retain(|storage| storage != &removed.storage);
    function
        .cleanup_plan
        .entry_state
        .live_owned_parameters
        .retain(|place| place.storage != removed.storage);
    for exit in &mut function.cleanup_plan.exits {
        exit.finalize_in_order
            .retain(|action| action.guard_flag != removed_flag);
    }

    let diagnostic = validate_structure(&program, &function).unwrap_err();
    assert!(diagnostic
        .message
        .contains("omits storage required by the cleanup inventory"));
}

#[test]
fn status_replay_rejects_a_deleted_checked_failure_source() {
    let program = program();
    let mut function = function(&program, "math.checked");
    function.cleanup_plan.status_sources.pop();

    let diagnostic = validate_structure(&program, &function).unwrap_err();
    assert!(diagnostic
        .message
        .contains("do not exactly cover typed HIR failure producers"));
}

#[test]
fn terminal_replay_rejects_return_unit_for_scalar_functions() {
    let program = program();
    let mut function = function(&program, "app.main");
    let exit = function
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| matches!(exit.continuation, ExitContinuation::CommitResult { .. }))
        .unwrap();
    exit.continuation = ExitContinuation::ReturnUnit;

    let diagnostic = validate_structure(&program, &function).unwrap_err();
    assert!(diagnostic
        .message
        .contains("ReturnUnit is invalid for current source-function return types"));
}

#[test]
fn terminal_replay_rejects_projected_owned_results() {
    let program = program();
    let mut function = function(&program, "pair.identity");
    let exit = function
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| matches!(exit.continuation, ExitContinuation::CommitResult { .. }))
        .unwrap();
    let ExitContinuation::CommitResult {
        source: CleanupResultSource::Owned { storage },
    } = &mut exit.continuation
    else {
        panic!("pair identity must publish an owned result")
    };
    storage.projections.push(DeclarationId::new("pair.first"));

    let diagnostic = validate_structure(&program, &function).unwrap_err();
    assert!(diagnostic
        .message
        .contains("whole droppable provisional result"));
}

#[test]
fn region_replay_rejects_over_and_under_leave_chains() {
    let program = program();
    let original = function(&program, "flow.regions");

    let mut over = original.clone();
    let over_exit = over
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| {
            matches!(exit.continuation, ExitContinuation::Continue(_))
                && exit.leaves_regions.len() == 1
        })
        .expect("nested region fixture must have a continuing scope exit");
    let parent = over.cleanup_plan.regions[over_exit.leaves_regions[0].0 as usize]
        .parent
        .expect("continuing nested region must have a parent");
    over_exit.leaves_regions.push(parent);
    let diagnostic = validate_structure(&program, &over).unwrap_err();
    assert!(diagnostic
        .message
        .contains("exact source-to-target region chain"));

    let mut under = original;
    let under_exit = under
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| {
            matches!(exit.continuation, ExitContinuation::ReturnFailure { .. })
                && exit.leaves_regions.len() >= 2
        })
        .expect("nested checked failure must leave multiple regions");
    under_exit.leaves_regions.pop();
    let diagnostic = validate_structure(&program, &under).unwrap_err();
    assert!(diagnostic
        .message
        .contains("exact source-to-target region chain"));
}

#[test]
fn deep_cfg_reachability_is_iterative() {
    let program = program();
    let mut function = function(&program, "app.main");
    const DEPTH: u32 = 20_000;
    function.cleanup_plan.blocks = (0..DEPTH)
        .map(|index| CleanupBlock {
            id: BlockId(index),
            region: CleanupRegionId(0),
            transitions: Vec::new(),
            terminator: if index + 1 == DEPTH {
                CleanupTerminator::Exit(crate::cleanup_plan::ExitTargetId(0))
            } else {
                CleanupTerminator::Goto(EdgeId(index))
            },
        })
        .collect();
    function.cleanup_plan.edges = (0..DEPTH - 1)
        .map(|index| CleanupEdge {
            id: EdgeId(index),
            from: BlockId(index),
            to: BlockId(index + 1),
            condition: EdgeCondition::Always,
        })
        .collect();
    function.cleanup_plan.exits[0].from = BlockId(DEPTH - 1);

    assert!(validate_reachable_acyclic_cfg(&function).is_ok());
}

#[test]
fn replay_preflight_rejects_every_invalid_cfg_target_and_cycles_without_panicking() {
    fn assert_unknown(program: &ResolvedProgram, function: &ResolvedFunction) {
        let diagnostic = validate_structure(program, function).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-H006");
        assert_eq!(
                diagnostic.message,
                format!(
                    "cleanup plan for function `{}` failed independent replay: cleanup replay preflight references an unknown id",
                    function.id
                )
            );
    }

    let program = program();
    let original = function(&program, "flow.regions");

    let mut invalid_entry = original.clone();
    invalid_entry.cleanup_plan.entry = BlockId(u32::MAX);
    assert_unknown(&program, &invalid_entry);

    let mut invalid_terminator_edge = original.clone();
    let edge = invalid_terminator_edge
        .cleanup_plan
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            CleanupTerminator::Goto(edge) => Some(edge),
            CleanupTerminator::Branch(edges) => edges.first_mut(),
            CleanupTerminator::Exit(_) => None,
        })
        .expect("fixture must contain a branch or goto edge");
    *edge = EdgeId(u32::MAX);
    assert_unknown(&program, &invalid_terminator_edge);

    let mut invalid_edge_target = original.clone();
    invalid_edge_target
        .cleanup_plan
        .edges
        .first_mut()
        .expect("fixture must contain an edge")
        .to = BlockId(u32::MAX);
    assert_unknown(&program, &invalid_edge_target);

    let mut invalid_exit = original.clone();
    let exit = invalid_exit
        .cleanup_plan
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            CleanupTerminator::Exit(exit) => Some(exit),
            CleanupTerminator::Goto(_) | CleanupTerminator::Branch(_) => None,
        })
        .expect("fixture must contain an exit terminator");
    *exit = crate::cleanup_plan::ExitTargetId(u32::MAX);
    assert_unknown(&program, &invalid_exit);

    let mut invalid_continue = original.clone();
    let continuation = invalid_continue
        .cleanup_plan
        .exits
        .iter_mut()
        .find_map(|exit| match &mut exit.continuation {
            ExitContinuation::Continue(edge) => Some(edge),
            ExitContinuation::CommitResult { .. }
            | ExitContinuation::ReturnFailure { .. }
            | ExitContinuation::ReturnUnit => None,
        })
        .expect("fixture must contain a continuing exit");
    *continuation = EdgeId(u32::MAX);
    assert_unknown(&program, &invalid_continue);

    let mut cycle = function(&program, "flow.bool");
    let entry = cycle.cleanup_plan.entry;
    let edge_id = match &cycle.cleanup_plan.blocks[entry.0 as usize].terminator {
        CleanupTerminator::Goto(edge) => *edge,
        CleanupTerminator::Branch(edges) => {
            *edges.first().expect("entry branch must contain an edge")
        }
        CleanupTerminator::Exit(_) => panic!("fixture entry must have a successor"),
    };
    cycle.cleanup_plan.edges[edge_id.0 as usize].to = entry;
    let diagnostic = validate_structure(&program, &cycle).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(
            diagnostic.message,
            "cleanup plan for function `flow.bool` failed independent replay: cleanup replay path bound exceeds the global path budget"
        );
}

#[test]
fn replay_budget_exhaustion_is_a_deterministic_diagnostic() {
    let program = program();
    let function = function(&program, "app.main");
    let mut budget = ReplayBudget {
        remaining: 1,
        skeleton_remaining: 0,
    };
    let diagnostic = budget.charge(&function, 2, "hostile test").unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(diagnostic.message.contains("work budget exhausted"));
}

fn assert_program_skeleton_authority(program: &ResolvedProgram) -> usize {
    let functions = || {
        program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
    };
    let independently_summed = functions()
        .try_fold(0usize, |total, function| {
            total
                .checked_add(skeleton_work_upper(program, function)?)
                .ok_or_else(|| skeleton_preflight_overflow(function))
        })
        .unwrap();
    assert!(independently_summed > 0);

    reset_skeleton_materializations();
    let mut exact = ReplayBudget {
        remaining: independently_summed,
        skeleton_remaining: 0,
    };
    assert_eq!(
        reserve_program_skeleton_work(program, functions(), &mut exact).unwrap(),
        independently_summed
    );
    assert_eq!(exact.remaining, 0);
    assert_eq!(exact.skeleton_remaining, independently_summed);
    assert_eq!(skeleton_materializations(), 0);

    reset_skeleton_materializations();
    let mut one_less = ReplayBudget {
        remaining: independently_summed - 1,
        skeleton_remaining: 0,
    };
    let diagnostic =
        reserve_program_skeleton_work(program, functions(), &mut one_less).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(diagnostic
        .message
        .contains("skeleton-work preflight exceeds"));
    assert_eq!(skeleton_materializations(), 0);

    reset_skeleton_materializations();
    let mut actual = ReplayBudget::new();
    let derived = reserve_program_skeleton_work(program, functions(), &mut actual).unwrap();
    for function in functions() {
        validate_structure_with_budget(program, function, &mut actual).unwrap();
    }
    let charged = derived - actual.skeleton_remaining;
    assert!(charged <= derived);
    assert!(skeleton_materializations() > 0);
    assert!(skeleton_materializations() <= charged);
    independently_summed
}

#[test]
fn program_wide_skeleton_preflight_sums_every_function_before_materialization() {
    let program = program();
    let derived = assert_program_skeleton_authority(&program);
    let largest_function = program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .map(|function| skeleton_work_upper(&program, function))
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .into_iter()
        .max()
        .unwrap();
    assert!(derived > largest_function);
}

#[test]
fn many_functions_and_deep_lazy_paths_share_one_exact_skeleton_authority() {
    let mut source = String::from("module replay.aggregate;\n");
    for index in 0..48 {
        source.push_str(&format!(
            "@id(\"aggregate.f{index}\") fn f{index}(flag: bool) -> bool {{ flag && flag }}\n"
        ));
    }
    let mut expression = String::from("flag");
    // Keep parser construction deliberately shallow; the private replay
    // depth-512 gate uses a prebuilt Program in the builder crate.
    for _ in 0..32 {
        expression = format!("flag && ({expression})");
    }
    source.push_str(&format!(
        "@id(\"aggregate.deep\") fn deep(flag: bool) -> bool {{ {expression} }}\n"
    ));
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
    let parsed = parse(&source, Path::new("cleanup-replay-aggregate.spx")).unwrap();
    let program = hir::resolve(&parsed).unwrap();
    assert_eq!(program.functions.len(), 50);
    assert_program_skeleton_authority(&program);
}

#[test]
fn wide_resource_update_untouched_fields_are_inside_charge_first_authority() {
    let mut source = String::from(
        "module replay.wide_update;\n\
             @id(\"wide.token\") resource Token { @id(\"wide.token.drop\") drop trivial; }\n\
             @id(\"wide.record\") record Wide {\n",
    );
    for index in 0..32 {
        source.push_str(&format!(
            "@id(\"wide.field.{index}\") field_{index}: Token,\n"
        ));
    }
    source.push_str(
        "}\n\
             @id(\"wide.update\") fn update(value: own Wide, replacement: own Token) -> Wide {\n\
                 value with { field_0: replacement }\n\
             }\n\
             @id(\"app.main\") fn main() -> i64 { 0 }\n",
    );
    let parsed = parse(&source, Path::new("cleanup-replay-wide-update.spx")).unwrap();
    let program = hir::resolve(&parsed).unwrap();
    assert_program_skeleton_authority(&program);

    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "wide.update")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("wide update body remains a block")
    };
    let ResolvedExprKind::UpdateRecord { record, fields, .. } = &tail.kind else {
        panic!("wide update tail remains an update")
    };
    assert_eq!(fields.len(), 1);
    let untouched_droppable = program
        .declarations
        .record_fields(record)
        .unwrap()
        .iter()
        .filter(|field| {
            fields
                .iter()
                .all(|replacement| replacement.field != field.id)
        })
        .filter(|field| type_needs_drop(&program, function, &field.ty).unwrap())
        .count();
    assert_eq!(untouched_droppable, 31);
    let active_paths = expression_path_counts(function, tail).unwrap().normal;
    let untouched_work = untouched_droppable
        .checked_mul(active_paths)
        .and_then(|units| units.checked_mul(8))
        .unwrap();
    let derived = skeleton_work_upper(&program, function).unwrap();
    assert!(derived >= untouched_work);

    reset_skeleton_materializations();
    let mut budget = ReplayBudget::with_skeleton_limit(derived);
    validate_structure_with_budget(&program, function, &mut budget).unwrap();
    let charged = derived - budget.skeleton_remaining;
    assert!(charged <= derived);
    assert!(skeleton_materializations() <= charged);
}

#[test]
fn terminated_prefix_skips_unreachable_invalid_lazy_if_and_match_children() {
    fn unreachable_prefix(
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
    ) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut work = SkeletonWork {
            function,
            budget: &mut budget,
        };
        let mut path = empty_expr_path();
        path.failed = true;
        let prefixes = work.singleton_path(path, "unreachable hostile prefix")?;
        sequence_expression(program, function, prefixes, expression, &mut work)
    }

    fn poison(expression: &mut ResolvedExpr) {
        expression.kind = ResolvedExprKind::Call {
            callee: DeclarationId::new("hostile.unreachable.callee"),
            type_arguments: Vec::new(),
            instance: None,
            args: Vec::new(),
        };
    }

    let program = program();
    let mut if_function = function(&program, "flow.bool");
    let ResolvedExprKind::Block { tail, .. } = &mut if_function.body.kind else {
        panic!("if fixture retains its body block")
    };
    let ResolvedExprKind::If { then_branch, .. } = &mut tail.kind else {
        panic!("if fixture retains its conditional tail")
    };
    poison(then_branch);
    let expression = (**tail).clone();
    let paths = unreachable_prefix(&program, &if_function, &expression).unwrap();
    assert_eq!(paths.len(), 1);
    assert!(paths[0].failed);

    let mut match_function = function(&program, "choice.select");
    let ResolvedExprKind::Block { tail, .. } = &mut match_function.body.kind else {
        panic!("match fixture retains its body block")
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        panic!("match fixture retains its match tail")
    };
    poison(&mut arms[0].value);
    let expression = (**tail).clone();
    let paths = unreachable_prefix(&program, &match_function, &expression).unwrap();
    assert_eq!(paths.len(), 1);
    assert!(paths[0].failed);

    let parsed = parse(
            "module replay.lazy_unreachable; @id(\"lazy\") fn lazy(left: bool, right: bool) -> bool { left && right } @id(\"app.main\") fn main() -> i64 { 0 }",
            Path::new("cleanup-replay-lazy-unreachable.spx"),
        )
        .unwrap();
    let lazy_program = hir::resolve(&parsed).unwrap();
    let mut lazy_function = function(&lazy_program, "lazy");
    let ResolvedExprKind::Block { tail, .. } = &mut lazy_function.body.kind else {
        panic!("lazy fixture retains its body block")
    };
    let ResolvedExprKind::Binary { right, .. } = &mut tail.kind else {
        panic!("lazy fixture retains its binary tail")
    };
    poison(right);
    let expression = (**tail).clone();
    let paths = unreachable_prefix(&lazy_program, &lazy_function, &expression).unwrap();
    assert_eq!(paths.len(), 1);
    assert!(paths[0].failed);
}

#[test]
fn wide_match_path_clones_and_pushes_are_charged_before_materialization() {
    fn replay_with_limit(
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        limit: usize,
    ) -> Result<(Vec<ExprSkeletonPath>, usize), Diagnostic> {
        let mut budget = ReplayBudget::with_skeleton_limit(limit);
        let paths = {
            let mut work = SkeletonWork {
                function,
                budget: &mut budget,
            };
            expression_skeleton(program, function, expression, &mut work)?
        };
        Ok((paths, limit - budget.skeleton_remaining))
    }

    let program = program();
    let function = function(&program, "choice.select");
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("wide match fixture retains its body block")
    };
    let (paths, charged) =
        replay_with_limit(&program, &function, tail, MAX_REPLAY_WORK_UNITS).unwrap();
    let retained_units = paths.iter().fold(0usize, |total, path| {
        total.saturating_add(path.observations.len().saturating_add(1))
    });
    assert!(
        paths.len() >= 4,
        "wide match produced {} paths",
        paths.len()
    );
    assert!(
        charged > retained_units,
        "clone/push work must exceed retained paths"
    );
    replay_with_limit(&program, &function, tail, charged).unwrap();
    let diagnostic = match replay_with_limit(&program, &function, tail, charged - 1) {
        Err(diagnostic) => diagnostic,
        Ok(_) => panic!("one-less path budget unexpectedly succeeded"),
    };
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(diagnostic.message.contains("work budget exhausted during"));
}

#[test]
fn skeleton_replay_rejects_a_checked_status_lane_swap() {
    let program = program();
    let mut function = function(&program, "math.checked");
    let first = function.cleanup_plan.status_sources[0].id.clone();
    let second = function.cleanup_plan.status_sources[1].id.clone();

    for edge in &mut function.cleanup_plan.edges {
        match &mut edge.condition {
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                if source == &first {
                    *source = second.clone();
                } else if source == &second {
                    *source = first.clone();
                }
            }
            EdgeCondition::Always
            | EdgeCondition::BooleanResult(_, _)
            | EdgeCondition::VariantCase { .. }
            | EdgeCondition::ArmSelected { .. } => {}
        }
    }
    for block in &mut function.cleanup_plan.blocks {
        for transition in &mut block.transitions {
            if let CleanupTransition::SelectFailure { source } = transition {
                if source == &first {
                    *source = second.clone();
                } else if source == &second {
                    *source = first.clone();
                }
            }
        }
    }
    for exit in &mut function.cleanup_plan.exits {
        if let ExitContinuation::ReturnFailure { source } = &mut exit.continuation {
            if source == &first {
                *source = second.clone();
            } else if source == &second {
                *source = first.clone();
            }
        }
    }

    let diagnostic = validate_structure(&program, &function).unwrap_err();
    assert!(diagnostic
        .message
        .contains("decision or ownership-event sequence disagrees with typed HIR"));
}

#[test]
fn skeleton_replay_rejects_a_boolean_expression_id_swap() {
    let program = program();
    let mut function = function(&program, "flow.bool");
    let boolean_ids = function
        .cleanup_plan
        .edges
        .iter()
        .filter_map(|edge| match &edge.condition {
            EdgeCondition::BooleanResult(expression, _) => Some(expression.clone()),
            EdgeCondition::Always
            | EdgeCondition::VariantCase { .. }
            | EdgeCondition::ArmSelected { .. }
            | EdgeCondition::StatusZero(_)
            | EdgeCondition::StatusNonzero(_) => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    assert_eq!(boolean_ids.len(), 2);
    let first = &boolean_ids[0];
    let second = &boolean_ids[1];
    for edge in &mut function.cleanup_plan.edges {
        if let EdgeCondition::BooleanResult(expression, _) = &mut edge.condition {
            if expression == first {
                *expression = second.clone();
            } else if expression == second {
                *expression = first.clone();
            }
        }
    }

    let diagnostic = validate_structure(&program, &function).unwrap_err();
    assert!(diagnostic
        .message
        .contains("decision or ownership-event sequence disagrees with typed HIR"));
}

#[test]
fn skeleton_replay_rejects_a_transition_location_substitution() {
    let program = program();
    let mut function = function(&program, "token.forward");
    let substitute = function.body.id.clone();
    let transition = function
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
        .find(|transition| {
            matches!(
                transition,
                CleanupTransition::Initialize { at, .. }
                    | CleanupTransition::Transfer { at, .. }
                    if at != &substitute
            )
        })
        .expect("fixture must contain a transition at a non-body expression");
    match transition {
        CleanupTransition::Initialize { at, .. }
        | CleanupTransition::InitializeVariant { at, .. }
        | CleanupTransition::Transfer { at, .. }
        | CleanupTransition::TransferVariant { at, .. }
        | CleanupTransition::AuthenticateVariantCase { at, .. } => *at = substitute,
        CleanupTransition::CallCommit { .. }
        | CleanupTransition::SelectFailure { .. }
        | CleanupTransition::StageCopyResult { .. } => {
            unreachable!()
        }
    }

    let diagnostic = validate_structure(&program, &function).unwrap_err();
    assert!(diagnostic
        .message
        .contains("decision or ownership-event sequence disagrees with typed HIR"));
}

#[test]
fn try_replay_authenticates_complementary_result_cases_and_exact_staging() {
    let program = try_program();
    let function = function(&program, "result.forward");
    validate_structure(&program, &function).unwrap();
    assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V2);
    assert!(function
        .cleanup_plan
        .status_sources
        .iter()
        .all(|source| source.id.lane == StatusLane::ContractFalse));

    let stages = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter_map(|transition| match transition {
            CleanupTransition::StageCopyResult { source } => Some(source),
            CleanupTransition::Initialize { .. }
            | CleanupTransition::InitializeVariant { .. }
            | CleanupTransition::Transfer { .. }
            | CleanupTransition::TransferVariant { .. }
            | CleanupTransition::AuthenticateVariantCase { .. }
            | CleanupTransition::CallCommit { .. }
            | CleanupTransition::SelectFailure { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(stages.len(), 2);
    let residual = stages
        .iter()
        .find_map(|source| match source {
            StagedCopyResultSource::TryResidual { .. } => Some((*source).clone()),
            StagedCopyResultSource::Body { .. } | StagedCopyResultSource::TryOptionNone { .. } => {
                None
            }
        })
        .unwrap();
    let StagedCopyResultSource::TryResidual {
        operand,
        source_instance,
        target_instance,
        ok_case,
        ..
    } = &residual
    else {
        unreachable!()
    };
    assert_ne!(source_instance, target_instance);
    let decisions = function
        .cleanup_plan
        .edges
        .iter()
        .filter_map(|edge| match &edge.condition {
            EdgeCondition::VariantCase {
                scrutinee,
                case,
                matches,
            } if scrutinee == operand && case == ok_case => Some(*matches),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(decisions, BTreeSet::from([false, true]));

    let mut wrong_instance = function.clone();
    for transition in wrong_instance
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
    {
        if let CleanupTransition::StageCopyResult {
            source:
                StagedCopyResultSource::TryResidual {
                    source_instance,
                    target_instance,
                    ..
                },
        } = transition
        {
            *target_instance = source_instance.clone();
        }
    }
    assert!(validate_structure(&program, &wrong_instance).is_err());

    let mut wrong_operand = function.clone();
    for transition in wrong_operand
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
    {
        if let CleanupTransition::StageCopyResult {
            source: StagedCopyResultSource::TryResidual { operand, .. },
        } = transition
        {
            *operand = function.body.id.clone();
        }
    }
    assert!(validate_structure(&program, &wrong_operand).is_err());

    let mut deleted = function.clone();
    for block in &mut deleted.cleanup_plan.blocks {
        block.transitions.retain(|transition| {
            !matches!(
                transition,
                CleanupTransition::StageCopyResult {
                    source: StagedCopyResultSource::TryResidual { .. }
                }
            )
        });
    }
    assert!(validate_structure(&program, &deleted).is_err());

    for mutation in 0..7 {
        let mut hostile = function.clone();
        let source = hostile
            .cleanup_plan
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.transitions)
            .find_map(|transition| match transition {
                CleanupTransition::StageCopyResult {
                    source: source @ StagedCopyResultSource::TryResidual { .. },
                } => Some(source),
                _ => None,
            })
            .unwrap();
        let StagedCopyResultSource::TryResidual {
            expression,
            source_instance,
            target_instance,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            ..
        } = source
        else {
            unreachable!()
        };
        match mutation {
            0 => *source_instance = target_instance.clone(),
            1 => *expression = function.body.id.clone(),
            2 => *result = DeclarationId::new("hostile.result"),
            3 => *ok_case = err_case.clone(),
            4 => *ok_field = err_field.clone(),
            5 => *err_case = ok_case.clone(),
            6 => *err_field = ok_field.clone(),
            _ => unreachable!(),
        }
        assert!(
            validate_structure(&program, &hostile).is_err(),
            "stage mutation {mutation} must fail closed"
        );
    }

    let mut status_confusion = function.clone();
    let residual_transition = status_confusion
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
        .find(|transition| {
            matches!(
                transition,
                CleanupTransition::StageCopyResult {
                    source: StagedCopyResultSource::TryResidual { .. }
                }
            )
        })
        .unwrap();
    *residual_transition = CleanupTransition::SelectFailure {
        source: StatusSourceId {
            expression: operand.clone(),
            lane: StatusLane::OperationFailure,
        },
    };
    assert!(validate_structure(&program, &status_confusion).is_err());

    let mut duplicate = function.clone();
    let residual_block = duplicate
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| {
            block.transitions.iter().any(|transition| {
                matches!(
                    transition,
                    CleanupTransition::StageCopyResult {
                        source: StagedCopyResultSource::TryResidual { .. }
                    }
                )
            })
        })
        .unwrap();
    residual_block
        .transitions
        .push(CleanupTransition::StageCopyResult { source: residual });
    assert!(validate_structure(&program, &duplicate).is_err());
}

#[test]
fn owned_record_match_v5_replay_authenticates_transfer_region_and_borrow_absence() {
    let source = r#"
module test.owned_record_match_replay;
@id("packet.type")
record Packet {
  @id("packet.payload") payload: Bytes,
  @id("packet.tag") tag: i64,
}
@id("packet.take")
fn take(value: own Packet) -> i64 {
  match own value { Packet { payload, tag: _ } => 0, }
}
@id("packet.inspect-owned")
fn inspect_owned(value: own Packet) -> i64 {
  match borrow value { Packet { payload, tag: _ } => 0, }
}
@id("packet.inspect-borrowed")
fn inspect_borrowed(value: borrow Packet) -> i64 {
  match borrow value { Packet { payload, tag: _ } => 0, }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = hir::resolve(
        &parse(source, Path::new("cleanup-replay-owned-record-match.spx")).expect("source parses"),
    )
    .expect("source resolves");
    let own = function(&program, "packet.take");
    let borrowed_owned = function(&program, "packet.inspect-owned");
    let borrowed_input = function(&program, "packet.inspect-borrowed");
    for candidate in [&own, &borrowed_owned, &borrowed_input] {
        assert_eq!(candidate.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V5);
        validate_structure(&program, candidate).expect("canonical v5 plan replays");
        crate::cleanup_plan::build::assert_expression_lowering_oracle(
            &program,
            candidate,
            &candidate.body,
        );
    }
    assert_eq!(
        function(&program, "app.main").cleanup_plan.schema,
        CLEANUP_PLAN_SCHEMA_V2,
        "v5 selection must not widen a legacy function in the same program"
    );

    let expression = match_expression(&own);
    let match_id = expression.id.clone();
    let ResolvedExprKind::Match { arms, .. } = &expression.kind else {
        unreachable!()
    };
    let ResolvedMatchPattern::Record { fields, .. } = &arms[0].pattern else {
        panic!("owned fixture must resolve one record pattern")
    };
    let payload_binding = fields
        .iter()
        .find_map(|field| match &field.pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding)
                if field.field.as_str() == "packet.payload" =>
            {
                Some(binding)
            }
            _ => None,
        })
        .expect("owned payload binding");
    assert_eq!(payload_binding.ownership, OwnershipMode::Own);

    let (transfer_block, source_place, destination_place) = own
        .cleanup_plan
        .blocks
        .iter()
        .find_map(|block| {
            block
                .transitions
                .iter()
                .find_map(|transition| match transition {
                    CleanupTransition::Transfer {
                        at,
                        source,
                        destination,
                    } if at == &match_id => Some((block, source, destination)),
                    _ => None,
                })
        })
        .expect("owned match transfer");
    assert_eq!(
        source_place.projections,
        [DeclarationId::new("packet.payload")]
    );
    assert_eq!(
        destination_place,
        &CleanupPlace {
            storage: StorageId::Value(payload_binding.id.clone()),
            projections: Vec::new(),
        }
    );
    let arm_region = own
        .cleanup_plan
        .regions
        .iter()
        .find(|region| region.slots.contains(&destination_place.storage))
        .expect("binding-owning arm region");
    assert_eq!(transfer_block.region, arm_region.id);
    assert!(arm_region.parent.is_some());
    assert!(own.cleanup_plan.exits.iter().any(|exit| {
        exit.leaves_regions.contains(&arm_region.id)
            && exit.finalize_in_order.iter().any(|action| {
                action.source == *destination_place
                    && action.lifecycle_id.as_str() == crate::cleanup::BYTES_DROP_LIFECYCLE_ID
            })
    }));

    for borrowed in [&borrowed_owned, &borrowed_input] {
        let borrowed_id = match_expression(borrowed).id.clone();
        assert!(borrowed.cleanup_plan.blocks.iter().all(|block| {
                block.transitions.iter().all(|transition| {
                    !matches!(transition, CleanupTransition::Transfer { at, .. } if at == &borrowed_id)
                })
            }));
    }

    let mut wrong_schema = own.clone();
    wrong_schema.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V4;
    assert!(validate_structure(&program, &wrong_schema).is_err());

    let mut wrong_mode = own.clone();
    let ResolvedExprKind::Block { tail, .. } = &mut wrong_mode.body.kind else {
        unreachable!()
    };
    let ResolvedExprKind::Match { mode, .. } = &mut tail.kind else {
        unreachable!()
    };
    *mode = crate::hir::ResolvedMatchMode::Borrow;
    assert!(validate_structure(&program, &wrong_mode).is_err());

    let mut wrong_binding = own.clone();
    let ResolvedExprKind::Block { tail, .. } = &mut wrong_binding.body.kind else {
        unreachable!()
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        unreachable!()
    };
    let ResolvedMatchPattern::Record { fields, .. } = &mut arms[0].pattern else {
        unreachable!()
    };
    let ResolvedRecordMatchFieldPattern::Binding(binding) = &mut fields[0].pattern else {
        unreachable!()
    };
    binding.ownership = OwnershipMode::Borrow;
    assert!(validate_structure(&program, &wrong_binding).is_err());

    let mut wildcard_pattern = own.clone();
    let ResolvedExprKind::Block { tail, .. } = &mut wildcard_pattern.body.kind else {
        unreachable!()
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        unreachable!()
    };
    arms[0].pattern = ResolvedMatchPattern::Wildcard;
    assert!(validate_structure(&program, &wildcard_pattern).is_err());

    fn transfer_position(candidate: &ResolvedFunction, match_id: &ExpressionId) -> (usize, usize) {
        candidate
                .cleanup_plan
                .blocks
                .iter()
                .enumerate()
                .find_map(|(block_index, block)| {
                    block.transitions.iter().enumerate().find_map(
                        |(transition_index, transition)| {
                            matches!(transition, CleanupTransition::Transfer { at, .. } if at == match_id)
                                .then_some((block_index, transition_index))
                        },
                    )
                })
                .expect("owned transfer position")
    }

    let mut wrong_source = own.clone();
    let (block, transition) = transfer_position(&wrong_source, &match_id);
    let CleanupTransition::Transfer { source, .. } =
        &mut wrong_source.cleanup_plan.blocks[block].transitions[transition]
    else {
        unreachable!()
    };
    source.projections.clear();
    assert!(validate_structure(&program, &wrong_source).is_err());

    let mut wrong_destination = own.clone();
    let (block, transition) = transfer_position(&wrong_destination, &match_id);
    let CleanupTransition::Transfer { destination, .. } =
        &mut wrong_destination.cleanup_plan.blocks[block].transitions[transition]
    else {
        unreachable!()
    };
    *destination = source_place.clone();
    assert!(validate_structure(&program, &wrong_destination).is_err());

    let mut wrong_region = own.clone();
    let arm_region_index = wrong_region
        .cleanup_plan
        .regions
        .iter()
        .position(|region| region.id == arm_region.id)
        .expect("arm region index");
    let parent = wrong_region.cleanup_plan.regions[arm_region_index]
        .parent
        .expect("arm region parent");
    wrong_region.cleanup_plan.regions[arm_region_index]
        .slots
        .retain(|storage| storage != &destination_place.storage);
    wrong_region.cleanup_plan.regions[parent.0 as usize]
        .slots
        .push(destination_place.storage.clone());
    assert!(validate_structure(&program, &wrong_region).is_err());

    let mut omitted_finalizer = own.clone();
    let exit = omitted_finalizer
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| {
            exit.leaves_regions.contains(&arm_region.id)
                && exit
                    .finalize_in_order
                    .iter()
                    .any(|action| action.source == *destination_place)
        })
        .expect("arm-region finalizer exit");
    exit.finalize_in_order
        .retain(|action| action.source != *destination_place);
    assert!(validate_structure(&program, &omitted_finalizer).is_err());

    let mut omitted = own.clone();
    let (block, transition) = transfer_position(&omitted, &match_id);
    omitted.cleanup_plan.blocks[block]
        .transitions
        .remove(transition);
    assert!(validate_structure(&program, &omitted).is_err());
}

#[test]
fn supplemental_call_argument_replay_rejects_depth_two_bytes_and_oracles_agree() {
    let source = r#"
module test.nested_supplemental_replay;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("inner.type") record Inner {
  @id("inner.payload") payload: Token,
}
@id("outer.type") record Outer {
  @id("outer.inner") inner: Inner,
}
@id("outer.sink") fn sink(value: own Outer) -> i64 { 0 }
@id("outer.forward") fn forward(value: own Outer) -> i64 { sink(value) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let mut program = hir::resolve(
        &parse(source, Path::new("cleanup-replay-nested-supplemental.spx")).expect("source parses"),
    )
    .expect("resource-backed nested fixture resolves");
    let forward = function(&program, "outer.forward");
    validate_structure(&program, &forward).expect("canonical resource plan replays");
    crate::cleanup_plan::build::assert_expression_lowering_oracle(
        &program,
        &forward,
        &forward.body,
    );

    let inner = program
        .types
        .iter_mut()
        .find(|declaration| declaration.id.as_str() == "inner.type")
        .expect("inner declaration");
    let ResolvedTypeDeclarationKind::Record { fields } = &mut inner.kind else {
        panic!("inner fixture remains a record")
    };
    fields[0].ty = ResolvedType::Bytes;

    // Both lowering implementations must fail in the same way for the
    // hostile typed shape; replay must independently reject it as well.
    crate::cleanup_plan::build::assert_expression_lowering_oracle(
        &program,
        &forward,
        &forward.body,
    );
    let diagnostic = validate_structure(&program, &forward)
        .expect_err("depth-two supplemental Bytes must fail closed");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(
        diagnostic
            .message
            .contains("nested compiler-owned Bytes cleanup leaf is outside flat record v1"),
        "{diagnostic:?}"
    );
}
