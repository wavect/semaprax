//! Independent structural validation for attached cleanup plans.
//!
//! This module deliberately does not invoke the canonical builder.  It checks
//! that an attached plan is a closed, well-formed CFG whose identifiers,
//! places, status sources, guarded finalizers, and every current acyclic path
//! can be replayed safely.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::ast::{BinaryOp, UnaryOp};
use crate::cleanup::{CleanupStorageOrigin, FieldLiveness, FieldLivenessShape, LivenessFlagId};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ExpressionId, OwnershipMode, PlaceProjection, ResolvedExpr, ResolvedExprKind,
    ResolvedFunction, ResolvedMatchArm, ResolvedMatchPattern, ResolvedProgram, ResolvedStatement,
    ResolvedType, ResolvedTypeDeclarationKind,
};

use super::{
    BlockId, CleanupPlace, CleanupRegionId, CleanupResultSource, CleanupTerminator,
    CleanupTransition, EdgeCondition, EdgeId, ExitContinuation, ExitTarget, StatusCase, StatusLane,
    StatusProducer, StatusSource, StatusSourceId, StorageId, CLEANUP_PLAN_SCHEMA_V1,
};

const MAX_REPLAY_PATHS: usize = 65_536;
const MAX_REPLAY_WORK_UNITS: usize = 1_000_000;

struct ReplayBudget {
    remaining: usize,
}

impl ReplayBudget {
    fn new() -> Self {
        Self {
            remaining: MAX_REPLAY_WORK_UNITS,
        }
    }

    fn charge(
        &mut self,
        function: &ResolvedFunction,
        units: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.remaining = self.remaining.checked_sub(units).ok_or_else(|| {
            replay_error(
                function,
                format!("cleanup replay work budget exhausted during {phase}"),
            )
        })?;
        Ok(())
    }
}

#[derive(Clone)]
struct Leaf {
    place: CleanupPlace,
    lifecycle: DeclarationId,
}

#[derive(Clone)]
struct CallFact {
    callee: DeclarationId,
    arguments: Vec<ExpressionId>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PathState {
    live_order: Vec<LivenessFlagId>,
    pending_failure: Option<StatusSourceId>,
    selected_failure: Option<StatusSourceId>,
    published: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SkeletonObservation {
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
        arguments: Vec<(u32, CleanupPlace)>,
    },
    Boolean {
        expression: ExpressionId,
        value: bool,
    },
    VariantCase {
        scrutinee: ExpressionId,
        case: DeclarationId,
        matches: bool,
    },
    Status {
        source: StatusSourceId,
        success: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SkeletonTerminal {
    Success,
    Failure,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SkeletonPath {
    observations: Vec<SkeletonObservation>,
    terminal: SkeletonTerminal,
}

#[derive(Clone)]
struct ExprSkeletonPath {
    observations: Vec<SkeletonObservation>,
    owned_source: Option<CleanupPlace>,
    failed: bool,
}

/// Validate the structure of the cleanup plan attached to `function` without
/// rebuilding it from HIR.
#[cfg(test)]
fn validate_structure(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::new();
    validate_structure_with_budget(program, function, &mut budget)
}

pub(super) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::new();
    for function in &program.functions {
        validate_structure_with_budget(program, function, &mut budget)?;
    }
    Ok(())
}

fn validate_structure_with_budget(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    if plan.schema != CLEANUP_PLAN_SCHEMA_V1 {
        return Err(replay_error(
            function,
            "uses an unknown cleanup-plan schema",
        ));
    }

    validate_replay_size_budget(function)?;
    budget.charge(
        function,
        plan_structure_units(plan),
        "structural validation",
    )?;

    let expression_facts = expression_facts(function)?;
    validate_inventory_coverage(program, function)?;
    validate_required_status_sources(program, function)?;
    let storage = validate_slots(function)?;
    let leaves = collect_leaves(function)?;
    validate_status_sources(function, &expression_facts)?;
    validate_regions(function, &storage)?;
    validate_entry(function, &storage, &leaves)?;
    validate_blocks_and_edges(program, function, &expression_facts, &storage, &leaves)?;
    validate_exits(program, function, &storage, &leaves)?;
    validate_reference_coverage(function)?;
    validate_reachable_acyclic_cfg(function)?;
    validate_path_states(function, &storage, &leaves, budget)?;
    validate_typed_control_skeleton(program, function, budget)?;
    Ok(())
}

fn plan_structure_units(plan: &super::CleanupPlan) -> usize {
    let mut units = plan
        .slots
        .len()
        .saturating_add(plan.status_sources.len())
        .saturating_add(plan.blocks.len())
        .saturating_add(plan.edges.len())
        .saturating_add(plan.regions.len())
        .saturating_add(plan.exits.len());
    for block in &plan.blocks {
        units = units.saturating_add(block.transitions.len());
    }
    for exit in &plan.exits {
        units = units
            .saturating_add(exit.leaves_regions.len())
            .saturating_add(exit.finalize_in_order.len());
    }
    units
}

fn validate_replay_size_budget(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let structure_units = plan_structure_units(&function.cleanup_plan);
    if structure_units > MAX_REPLAY_WORK_UNITS {
        return Err(replay_error(
            function,
            "cleanup replay structure exceeds the global work budget",
        ));
    }
    let plan_boolean_splits = function
        .cleanup_plan
        .edges
        .iter()
        .filter(|edge| {
            matches!(
                edge.condition,
                EdgeCondition::BooleanResult(_, true)
                    | EdgeCondition::VariantCase { matches: true, .. }
            )
        })
        .count();
    let hir_boolean_splits = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .fold(
            function
                .requires
                .len()
                .saturating_add(function.ensures.len()),
            |total, expression| total.saturating_add(expression_boolean_splits(expression)),
        );
    let boolean_splits = plan_boolean_splits.max(hir_boolean_splits);
    let status_splits = function.cleanup_plan.status_sources.len();
    let boolean_paths = 1_usize
        .checked_shl(u32::try_from(boolean_splits).unwrap_or(u32::MAX))
        .unwrap_or(usize::MAX);
    let path_bound = boolean_paths.saturating_mul(status_splits.saturating_add(1));
    if path_bound > MAX_REPLAY_PATHS {
        return Err(replay_error(
            function,
            "cleanup replay path bound exceeds the global path budget",
        ));
    }
    let expression_units = expression_facts(function)?.len();
    let per_path_units = structure_units.max(expression_units).max(1);
    if per_path_units.saturating_mul(path_bound) > MAX_REPLAY_WORK_UNITS {
        return Err(replay_error(
            function,
            "cleanup replay combined path/work bound exceeds the global budget",
        ));
    }
    Ok(())
}

fn expression_boolean_splits(expression: &ResolvedExpr) -> usize {
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => args.iter().fold(0_usize, |total, argument| {
            total.saturating_add(expression_boolean_splits(argument))
        }),
        ResolvedExprKind::Unary { value, .. } | ResolvedExprKind::Project { base: value, .. } => {
            expression_boolean_splits(value)
        }
        ResolvedExprKind::Binary { op, left, right } => {
            let own = usize::from(matches!(op, BinaryOp::And | BinaryOp::Or));
            own.saturating_add(expression_boolean_splits(left))
                .saturating_add(expression_boolean_splits(right))
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements
                .iter()
                .fold(expression_boolean_splits(tail), |total, statement| {
                    let ResolvedStatement::Let { value, .. } = statement;
                    total.saturating_add(expression_boolean_splits(value))
                })
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => 1_usize
            .saturating_add(expression_boolean_splits(condition))
            .saturating_add(expression_boolean_splits(then_branch))
            .saturating_add(expression_boolean_splits(else_branch)),
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            fields.iter().fold(0_usize, |total, field| {
                total.saturating_add(expression_boolean_splits(&field.value))
            })
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.iter().fold(0_usize, |total, field| {
                total.saturating_add(expression_boolean_splits(&field.value))
            })
        }
        ResolvedExprKind::Match { scrutinee, arms } => arms.iter().fold(
            expression_boolean_splits(scrutinee).saturating_add(arms.len().saturating_sub(1)),
            |total, arm| total.saturating_add(expression_boolean_splits(&arm.value)),
        ),
        ResolvedExprKind::UpdateRecord { base, fields, .. } => fields
            .iter()
            .fold(expression_boolean_splits(base), |total, field| {
                total.saturating_add(expression_boolean_splits(&field.value))
            }),
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => 0,
    }
}

fn validate_inventory_coverage(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let base_len = function.cleanup.slots.len();
    if plan.slots.len() < base_len {
        return Err(replay_error(
            function,
            "cleanup plan omits storage required by the cleanup inventory",
        ));
    }

    let mut inventory_storage = BTreeMap::new();
    for inventory_slot in &function.cleanup.slots {
        let storage = inventory_storage_id(&inventory_slot.origin);
        inventory_storage.insert(inventory_slot.id, storage);
    }
    for (index, inventory_slot) in function.cleanup.slots.iter().enumerate() {
        let actual = &plan.slots[index];
        let expected_storage = inventory_storage_id(&inventory_slot.origin);
        if actual.id.0 != inventory_slot.id.0
            || actual.storage_index != inventory_slot.discovery_index
            || actual.storage != expected_storage
            || actual.ty != inventory_slot.ty
            || actual.field_liveness_shape != inventory_slot.shape
        {
            return Err(replay_error(
                function,
                format!("cleanup plan base slot {index} disagrees with the independent inventory"),
            ));
        }
    }

    let expected_entry = function
        .cleanup
        .entry_state
        .live_owned_parameters
        .iter()
        .map(|storage| {
            inventory_storage
                .get(storage)
                .cloned()
                .map(|storage| CleanupPlace {
                    storage,
                    projections: Vec::new(),
                })
                .ok_or_else(|| {
                    replay_error(
                        function,
                        "cleanup inventory entry references unknown storage",
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if plan.entry_state.live_owned_parameters != expected_entry {
        return Err(replay_error(
            function,
            "cleanup plan entry ownership disagrees with the independent inventory",
        ));
    }

    let mut next_flag = u32::try_from(function.cleanup.flags.len())
        .map_err(|_| replay_error(function, "too many inventory liveness flags"))?;
    let mut expected_supplemental = Vec::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        collect_supplemental_slots(
            program,
            function,
            expression,
            &mut next_flag,
            &mut expected_supplemental,
        )?;
    }
    let actual_supplemental = &plan.slots[base_len..];
    if actual_supplemental.len() != expected_supplemental.len() {
        return Err(replay_error(
            function,
            "cleanup plan has missing or extra call-argument storage",
        ));
    }
    for (offset, (actual, expected)) in actual_supplemental
        .iter()
        .zip(&expected_supplemental)
        .enumerate()
    {
        let expected_index = base_len
            .checked_add(offset)
            .ok_or_else(|| replay_error(function, "too many cleanup slots"))?;
        let expected_index = u32::try_from(expected_index)
            .map_err(|_| replay_error(function, "too many cleanup slots"))?;
        if actual.id.0 != expected_index
            || actual.storage_index != expected_index
            || actual.storage != expected.storage
            || actual.ty != expected.ty
            || actual.field_liveness_shape != expected.shape
        {
            return Err(replay_error(
                function,
                format!("supplemental call-argument slot {offset} disagrees with typed HIR"),
            ));
        }
    }
    Ok(())
}

struct ExpectedSupplementalSlot {
    storage: StorageId,
    ty: ResolvedType,
    shape: FieldLivenessShape,
}

fn inventory_storage_id(origin: &CleanupStorageOrigin) -> StorageId {
    match origin {
        CleanupStorageOrigin::Parameter { value, .. } | CleanupStorageOrigin::Binding { value } => {
            StorageId::Value(value.clone())
        }
        CleanupStorageOrigin::Temporary { expression } => StorageId::Temporary(expression.clone()),
        CleanupStorageOrigin::ProvisionalResult { .. } => StorageId::ProvisionalResult,
    }
}

fn collect_supplemental_slots(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    next_flag: &mut u32,
    slots: &mut Vec<ExpectedSupplementalSlot>,
) -> Result<(), Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Call { callee, args } => {
            let target = program
                .functions
                .iter()
                .find(|target| target.id == *callee)
                .ok_or_else(|| {
                    replay_error(
                        function,
                        format!(
                            "cleanup call `{}` has unknown callee `{callee}`",
                            expression.id
                        ),
                    )
                })?;
            if target.params.len() != args.len() {
                return Err(replay_error(
                    function,
                    format!("cleanup call `{}` has inconsistent arity", expression.id),
                ));
            }
            for (index, (argument, parameter)) in args.iter().zip(&target.params).enumerate() {
                collect_supplemental_slots(program, function, argument, next_flag, slots)?;
                if parameter.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &parameter.ty)?
                {
                    let parameter_index = u32::try_from(index)
                        .map_err(|_| replay_error(function, "too many call parameters"))?;
                    let storage = StorageId::CallArgument {
                        call: expression.id.clone(),
                        parameter_index,
                        value_expression: argument.id.clone(),
                    };
                    let shape =
                        expected_shape_for_type(program, function, &argument.ty, next_flag)?;
                    slots.push(ExpectedSupplementalSlot {
                        storage,
                        ty: argument.ty.clone(),
                        shape,
                    });
                }
            }
        }
        ResolvedExprKind::Unary { value, .. } | ResolvedExprKind::Project { base: value, .. } => {
            collect_supplemental_slots(program, function, value, next_flag, slots)?;
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_supplemental_slots(program, function, left, next_flag, slots)?;
            collect_supplemental_slots(program, function, right, next_flag, slots)?;
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let crate::hir::ResolvedStatement::Let { value, .. } = statement;
                collect_supplemental_slots(program, function, value, next_flag, slots)?;
            }
            collect_supplemental_slots(program, function, tail, next_flag, slots)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_supplemental_slots(program, function, condition, next_flag, slots)?;
            collect_supplemental_slots(program, function, then_branch, next_flag, slots)?;
            collect_supplemental_slots(program, function, else_branch, next_flag, slots)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for field in fields {
                collect_supplemental_slots(program, function, &field.value, next_flag, slots)?;
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_supplemental_slots(program, function, &field.value, next_flag, slots)?;
            }
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            collect_supplemental_slots(program, function, scrutinee, next_flag, slots)?;
            for arm in arms {
                collect_supplemental_slots(program, function, &arm.value, next_flag, slots)?;
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_supplemental_slots(program, function, base, next_flag, slots)?;
            for field in fields {
                collect_supplemental_slots(program, function, &field.value, next_flag, slots)?;
            }
        }
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
    }
    Ok(())
}

fn expected_shape_for_type(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    next_flag: &mut u32,
) -> Result<FieldLivenessShape, Diagnostic> {
    if !type_needs_drop(program, function, ty)? {
        return Ok(FieldLivenessShape::NoDrop);
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err(replay_error(
            function,
            "droppable supplemental slot is not nominal",
        ));
    };
    if !arguments.is_empty() {
        return Err(replay_error(
            function,
            "droppable supplemental slot has generic arguments",
        ));
    }
    let declaration_item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| replay_error(function, format!("unknown cleanup type `{declaration}`")))?;
    match &declaration_item.kind {
        ResolvedTypeDeclarationKind::Resource { drop } => {
            let flag = LivenessFlagId(*next_flag);
            *next_flag = next_flag
                .checked_add(1)
                .ok_or_else(|| replay_error(function, "too many cleanup flags"))?;
            Ok(FieldLivenessShape::Leaf {
                flag,
                lifecycle: drop.id.clone(),
            })
        }
        ResolvedTypeDeclarationKind::Record { fields } => {
            let mut expected_fields = Vec::with_capacity(fields.len());
            for field in fields {
                expected_fields.push(FieldLiveness {
                    field: field.id.clone(),
                    field_index: field.index,
                    shape: expected_shape_for_type(program, function, &field.ty, next_flag)?,
                });
            }
            Ok(FieldLivenessShape::Record {
                declaration: declaration.clone(),
                fields: expected_fields,
            })
        }
        ResolvedTypeDeclarationKind::Variant { .. } => Err(replay_error(
            function,
            "droppable variant cleanup is outside the copy-only v1 slice",
        )),
    }
}

fn type_needs_drop(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    ty: &ResolvedType,
) -> Result<bool, Diagnostic> {
    program
        .declarations
        .type_facts(ty)
        .map(|facts| facts.needs_drop)
        .ok_or_else(|| {
            replay_error(
                function,
                format!("type `{}` has no cleanup facts", ty.identity_key()),
            )
        })
}

fn validate_required_status_sources(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let mut expected = Vec::new();
    for (ordinal, contract) in function.requires.iter().enumerate() {
        collect_expression_statuses(program, function, contract, &mut expected)?;
        expected.push(StatusSource {
            id: StatusSourceId {
                expression: contract.id.clone(),
                lane: StatusLane::ContractFalse,
            },
            producer: StatusProducer::ContractFalse {
                phase: super::ContractPhase::Requires,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| replay_error(function, "too many preconditions"))?,
            },
        });
    }
    collect_expression_statuses(program, function, &function.body, &mut expected)?;
    for (ordinal, contract) in function.ensures.iter().enumerate() {
        collect_expression_statuses(program, function, contract, &mut expected)?;
        expected.push(StatusSource {
            id: StatusSourceId {
                expression: contract.id.clone(),
                lane: StatusLane::ContractFalse,
            },
            producer: StatusProducer::ContractFalse {
                phase: super::ContractPhase::Ensures,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| replay_error(function, "too many postconditions"))?,
            },
        });
    }
    if function.cleanup_plan.status_sources != expected {
        return Err(replay_error(
            function,
            "cleanup status sources do not exactly cover typed HIR failure producers",
        ));
    }
    Ok(())
}

fn collect_expression_statuses(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    statuses: &mut Vec<StatusSource>,
) -> Result<(), Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Call { callee, args } => {
            for argument in args {
                collect_expression_statuses(program, function, argument, statuses)?;
            }
            if !program.functions.iter().any(|target| target.id == *callee) {
                return Err(replay_error(
                    function,
                    format!("status source call has unknown callee `{callee}`"),
                ));
            }
            statuses.push(StatusSource {
                id: StatusSourceId {
                    expression: expression.id.clone(),
                    lane: StatusLane::OperationFailure,
                },
                producer: StatusProducer::PropagatedCall {
                    callee: callee.clone(),
                },
            });
        }
        ResolvedExprKind::Unary { op, value } => {
            collect_expression_statuses(program, function, value, statuses)?;
            if *op == UnaryOp::Neg {
                statuses.push(checked_status(
                    expression,
                    super::CheckedOperation::Neg,
                    vec![StatusCase::NegationOverflow],
                ));
            }
        }
        ResolvedExprKind::Binary { op, left, right } => {
            collect_expression_statuses(program, function, left, statuses)?;
            collect_expression_statuses(program, function, right, statuses)?;
            let checked = match op {
                BinaryOp::Add => {
                    Some((super::CheckedOperation::Add, vec![StatusCase::AddOverflow]))
                }
                BinaryOp::Sub => {
                    Some((super::CheckedOperation::Sub, vec![StatusCase::SubOverflow]))
                }
                BinaryOp::Mul => {
                    Some((super::CheckedOperation::Mul, vec![StatusCase::MulOverflow]))
                }
                BinaryOp::Div => Some((
                    super::CheckedOperation::Div,
                    vec![StatusCase::DivisionByZero, StatusCase::DivisionOverflow],
                )),
                BinaryOp::Rem => Some((
                    super::CheckedOperation::Rem,
                    vec![StatusCase::RemainderByZero, StatusCase::RemainderOverflow],
                )),
                BinaryOp::Eq
                | BinaryOp::Ne
                | BinaryOp::Lt
                | BinaryOp::Le
                | BinaryOp::Gt
                | BinaryOp::Ge
                | BinaryOp::And
                | BinaryOp::Or => None,
            };
            if let Some((operation, cases)) = checked {
                statuses.push(checked_status(expression, operation, cases));
            }
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let crate::hir::ResolvedStatement::Let { value, .. } = statement;
                collect_expression_statuses(program, function, value, statuses)?;
            }
            collect_expression_statuses(program, function, tail, statuses)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expression_statuses(program, function, condition, statuses)?;
            collect_expression_statuses(program, function, then_branch, statuses)?;
            collect_expression_statuses(program, function, else_branch, statuses)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for field in fields {
                collect_expression_statuses(program, function, &field.value, statuses)?;
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_expression_statuses(program, function, &field.value, statuses)?;
            }
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            collect_expression_statuses(program, function, scrutinee, statuses)?;
            for arm in arms {
                collect_expression_statuses(program, function, &arm.value, statuses)?;
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_expression_statuses(program, function, base, statuses)?;
            for field in fields {
                collect_expression_statuses(program, function, &field.value, statuses)?;
            }
        }
        ResolvedExprKind::Project { base, .. } => {
            collect_expression_statuses(program, function, base, statuses)?;
        }
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
    }
    Ok(())
}

fn checked_status(
    expression: &ResolvedExpr,
    operation: super::CheckedOperation,
    normalized_cases: Vec<StatusCase>,
) -> StatusSource {
    StatusSource {
        id: StatusSourceId {
            expression: expression.id.clone(),
            lane: StatusLane::OperationFailure,
        },
        producer: StatusProducer::CheckedArithmetic {
            operation,
            normalized_cases,
        },
    }
}

fn validate_slots(function: &ResolvedFunction) -> Result<BTreeSet<StorageId>, Diagnostic> {
    let mut storage = BTreeSet::new();
    for (index, slot) in function.cleanup_plan.slots.iter().enumerate() {
        let expected = u32_index(function, index, "cleanup slot")?;
        if slot.id.0 != expected || slot.storage_index != expected {
            return Err(replay_error(
                function,
                format!("cleanup slot at index {index} has non-contiguous identity or index"),
            ));
        }
        if !storage.insert(slot.storage.clone()) {
            return Err(replay_error(
                function,
                format!("cleanup storage `{:?}` is repeated", slot.storage),
            ));
        }
    }
    Ok(storage)
}

fn collect_leaves(
    function: &ResolvedFunction,
) -> Result<BTreeMap<LivenessFlagId, Leaf>, Diagnostic> {
    let mut leaves = BTreeMap::new();
    let mut places = BTreeSet::new();
    for slot in &function.cleanup_plan.slots {
        collect_shape(
            function,
            &slot.storage,
            &mut Vec::new(),
            &slot.field_liveness_shape,
            &mut leaves,
            &mut places,
        )?;
    }
    for (expected, flag) in leaves.keys().enumerate() {
        if flag.0 != u32_index(function, expected, "liveness flag")? {
            return Err(replay_error(
                function,
                "cleanup liveness flags are not contiguous from zero",
            ));
        }
    }
    Ok(leaves)
}

fn collect_shape(
    function: &ResolvedFunction,
    storage: &StorageId,
    projections: &mut Vec<DeclarationId>,
    shape: &FieldLivenessShape,
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
    places: &mut BTreeSet<CleanupPlace>,
) -> Result<(), Diagnostic> {
    match shape {
        FieldLivenessShape::NoDrop => Ok(()),
        FieldLivenessShape::Leaf { flag, lifecycle } => {
            let leaf = Leaf {
                place: CleanupPlace {
                    storage: storage.clone(),
                    projections: projections.clone(),
                },
                lifecycle: lifecycle.clone(),
            };
            if leaves.insert(*flag, leaf.clone()).is_some() {
                return Err(replay_error(
                    function,
                    format!("liveness flag {} is repeated", flag.0),
                ));
            }
            if !places.insert(leaf.place) {
                return Err(replay_error(function, "cleanup leaf place is repeated"));
            }
            Ok(())
        }
        FieldLivenessShape::Record { fields, .. } => {
            let mut field_ids = BTreeSet::new();
            for (index, field) in fields.iter().enumerate() {
                let expected = u32_index(function, index, "cleanup field")?;
                if field.field_index != expected || !field_ids.insert(field.field.clone()) {
                    return Err(replay_error(
                        function,
                        "record cleanup shape has non-contiguous or repeated fields",
                    ));
                }
                projections.push(field.field.clone());
                collect_shape(function, storage, projections, &field.shape, leaves, places)?;
                projections.pop();
            }
            Ok(())
        }
    }
}

fn validate_status_sources(
    function: &ResolvedFunction,
    expressions: &BTreeMap<ExpressionId, Option<CallFact>>,
) -> Result<(), Diagnostic> {
    let mut ids = BTreeSet::new();
    for source in &function.cleanup_plan.status_sources {
        if !ids.insert(source.id.clone()) {
            return Err(replay_error(
                function,
                format!("status source for `{}` is repeated", source.id.expression),
            ));
        }
        if !expressions.contains_key(&source.id.expression) {
            return Err(replay_error(
                function,
                format!(
                    "status source references unknown expression `{}`",
                    source.id.expression
                ),
            ));
        }
        match (&source.id.lane, &source.producer) {
            (StatusLane::OperationFailure, StatusProducer::PropagatedCall { callee }) => {
                let Some(Some(call)) = expressions.get(&source.id.expression) else {
                    return Err(replay_error(
                        function,
                        "propagated-call status source does not name a call expression",
                    ));
                };
                if &call.callee != callee {
                    return Err(replay_error(
                        function,
                        "propagated-call status source names the wrong callee",
                    ));
                }
            }
            (
                StatusLane::OperationFailure,
                StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                },
            ) => {
                let cases = normalized_cases.iter().copied().collect::<BTreeSet<_>>();
                if cases.is_empty() || cases.len() != normalized_cases.len() {
                    return Err(replay_error(
                        function,
                        "checked-arithmetic status cases are empty or repeated",
                    ));
                }
            }
            (StatusLane::ContractFalse, StatusProducer::ContractFalse { .. }) => {}
            _ => {
                return Err(replay_error(
                    function,
                    "status source lane does not match its producer",
                ));
            }
        }
    }
    Ok(())
}

fn validate_regions(
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    if plan.regions.is_empty() {
        return Err(replay_error(function, "cleanup plan has no root region"));
    }
    let mut assigned = BTreeSet::new();
    for (index, region) in plan.regions.iter().enumerate() {
        let expected = u32_index(function, index, "cleanup region")?;
        if region.id.0 != expected {
            return Err(replay_error(
                function,
                "cleanup region IDs are not contiguous",
            ));
        }
        match (index, region.parent) {
            (0, None) => {}
            (0, Some(_)) | (_, None) => {
                return Err(replay_error(
                    function,
                    "cleanup region tree has an invalid root",
                ));
            }
            (_, Some(parent)) if usize::try_from(parent.0).ok().is_some_and(|p| p < index) => {}
            _ => {
                return Err(replay_error(
                    function,
                    "cleanup region parent must precede its child",
                ));
            }
        }
        for item in &region.slots {
            if !storage.contains(item) {
                return Err(replay_error(
                    function,
                    "cleanup region references unknown storage",
                ));
            }
            if !assigned.insert(item.clone()) {
                return Err(replay_error(
                    function,
                    "cleanup storage is assigned to more than one region",
                ));
            }
        }
        if usize::try_from(region.normal_scope_end.0)
            .ok()
            .is_none_or(|exit| exit >= plan.exits.len())
        {
            return Err(replay_error(
                function,
                "cleanup region references an unknown normal-scope exit",
            ));
        }
    }
    if &assigned != storage {
        return Err(replay_error(
            function,
            "cleanup plan has storage not assigned to exactly one region",
        ));
    }
    Ok(())
}

fn validate_entry(
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    if usize::try_from(plan.entry.0)
        .ok()
        .is_none_or(|entry| entry >= plan.blocks.len())
    {
        return Err(replay_error(function, "cleanup entry block is unknown"));
    }
    let mut flags = BTreeSet::new();
    for place in &plan.entry_state.live_owned_parameters {
        let under = validate_place(function, place, storage, leaves)?;
        if under.iter().any(|flag| !flags.insert(*flag)) {
            return Err(replay_error(
                function,
                "cleanup entry places overlap or repeat liveness flags",
            ));
        }
    }
    Ok(())
}

fn validate_blocks_and_edges(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expressions: &BTreeMap<ExpressionId, Option<CallFact>>,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let statuses = plan
        .status_sources
        .iter()
        .map(|source| source.id.clone())
        .collect::<BTreeSet<_>>();
    for (index, edge) in plan.edges.iter().enumerate() {
        if edge.id.0 != u32_index(function, index, "cleanup edge")?
            || !block_exists(function, edge.from)
            || !block_exists(function, edge.to)
        {
            return Err(replay_error(
                function,
                "cleanup edge has a non-contiguous ID or unknown endpoint",
            ));
        }
        validate_edge_condition(function, &edge.condition, expressions, &statuses)?;
    }

    let mut referenced_edges = BTreeSet::new();
    let mut referenced_exits = BTreeSet::new();
    let mut committed_calls = BTreeSet::new();
    for (index, block) in plan.blocks.iter().enumerate() {
        if block.id.0 != u32_index(function, index, "cleanup block")?
            || usize::try_from(block.region.0)
                .ok()
                .is_none_or(|region| region >= plan.regions.len())
        {
            return Err(replay_error(
                function,
                "cleanup block has a non-contiguous ID or unknown region",
            ));
        }
        let mut selected = None;
        for transition in &block.transitions {
            match transition {
                CleanupTransition::Initialize { at, destination } => {
                    require_expression(function, expressions, at)?;
                    validate_place(function, destination, storage, leaves)?;
                }
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => {
                    require_expression(function, expressions, at)?;
                    let source_flags = validate_place(function, source, storage, leaves)?;
                    let destination_flags = validate_place(function, destination, storage, leaves)?;
                    let source_flags = source_flags.into_iter().collect::<BTreeSet<_>>();
                    let destination_flags = destination_flags.into_iter().collect::<BTreeSet<_>>();
                    if source_flags.len() != destination_flags.len()
                        || !source_flags.is_disjoint(&destination_flags)
                    {
                        return Err(replay_error(
                            function,
                            "cleanup transfer has incompatible structural places",
                        ));
                    }
                }
                CleanupTransition::CallCommit { call, arguments } => {
                    let Some(Some(fact)) = expressions.get(call) else {
                        return Err(replay_error(
                            function,
                            "call commit does not name a call expression",
                        ));
                    };
                    if !committed_calls.insert(call.clone()) {
                        return Err(replay_error(
                            function,
                            "call has more than one atomic commit",
                        ));
                    }
                    let target = program
                        .functions
                        .iter()
                        .find(|target| target.id == fact.callee)
                        .ok_or_else(|| {
                            replay_error(
                                function,
                                format!("call commit has unknown callee `{}`", fact.callee),
                            )
                        })?;
                    if target.params.len() != fact.arguments.len() {
                        return Err(replay_error(
                            function,
                            "call commit callee signature has inconsistent arity",
                        ));
                    }
                    let mut expected_parameters = Vec::new();
                    for (index, parameter) in target.params.iter().enumerate() {
                        if parameter.ownership == OwnershipMode::Own
                            && type_needs_drop(program, function, &parameter.ty)?
                        {
                            expected_parameters.push(
                                u32::try_from(index).map_err(|_| {
                                    replay_error(function, "too many call parameters")
                                })?,
                            );
                        }
                    }
                    let actual_parameters = arguments
                        .iter()
                        .map(|argument| argument.parameter_index)
                        .collect::<Vec<_>>();
                    if actual_parameters != expected_parameters {
                        return Err(replay_error(
                            function,
                            "call commit does not contain every and only owned droppable parameter",
                        ));
                    }
                    let mut previous = None;
                    let mut consumed = BTreeSet::new();
                    for argument in arguments {
                        if previous.is_some_and(|value| value >= argument.parameter_index) {
                            return Err(replay_error(
                                function,
                                "call-commit parameters are not in strict signature order",
                            ));
                        }
                        previous = Some(argument.parameter_index);
                        let index = usize::try_from(argument.parameter_index).map_err(|_| {
                            replay_error(function, "call-commit parameter index does not fit usize")
                        })?;
                        let Some(expected_expression) = fact.arguments.get(index) else {
                            return Err(replay_error(
                                function,
                                "call-commit parameter index exceeds call arity",
                            ));
                        };
                        match &argument.source.storage {
                            StorageId::CallArgument {
                                call: storage_call,
                                parameter_index,
                                value_expression,
                            } if storage_call == call
                                && parameter_index == &argument.parameter_index
                                && value_expression == expected_expression
                                && argument.source.projections.is_empty() => {}
                            _ => {
                                return Err(replay_error(
                                    function,
                                    "call commit does not consume its matching whole argument epoch",
                                ));
                            }
                        }
                        for flag in validate_place(function, &argument.source, storage, leaves)? {
                            if !consumed.insert(flag) {
                                return Err(replay_error(
                                    function,
                                    "atomic call commit consumes overlapping argument epochs",
                                ));
                            }
                        }
                    }
                }
                CleanupTransition::SelectFailure { source } => {
                    require_status(function, &statuses, source)?;
                    if selected.replace(source).is_some() {
                        return Err(replay_error(
                            function,
                            "cleanup block selects more than one failure status",
                        ));
                    }
                }
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                validate_owned_edge(function, block.id, *edge, &mut referenced_edges)?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(replay_error(function, "goto edge is conditional"));
                }
            }
            CleanupTerminator::Branch(edges) => {
                if edges.len() != 2 || edges[0] == edges[1] {
                    return Err(replay_error(
                        function,
                        "cleanup branch must own exactly two distinct edges",
                    ));
                }
                for edge in edges {
                    validate_owned_edge(function, block.id, *edge, &mut referenced_edges)?;
                }
                validate_branch_pair(
                    function,
                    &plan.edges[edges[0].0 as usize].condition,
                    &plan.edges[edges[1].0 as usize].condition,
                )?;
            }
            CleanupTerminator::Exit(exit) => {
                if usize::try_from(exit.0)
                    .ok()
                    .is_none_or(|value| value >= plan.exits.len())
                    || !referenced_exits.insert(*exit)
                {
                    return Err(replay_error(
                        function,
                        "cleanup block references an unknown or repeated exit",
                    ));
                }
                if plan.exits[exit.0 as usize].from != block.id {
                    return Err(replay_error(
                        function,
                        "cleanup exit is owned by the wrong block",
                    ));
                }
            }
        }
    }
    let expected_calls = expressions
        .iter()
        .filter_map(|(expression, fact)| fact.as_ref().map(|_| expression.clone()))
        .collect::<BTreeSet<_>>();
    if committed_calls != expected_calls {
        return Err(replay_error(
            function,
            "cleanup plan does not contain exactly one atomic commit for every call",
        ));
    }
    Ok(())
}

fn validate_exits(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let statuses = plan
        .status_sources
        .iter()
        .map(|source| source.id.clone())
        .collect::<BTreeSet<_>>();
    let mut continuation_edges = BTreeSet::new();
    for (index, exit) in plan.exits.iter().enumerate() {
        if exit.id.0 != u32_index(function, index, "cleanup exit")?
            || !block_exists(function, exit.from)
        {
            return Err(replay_error(
                function,
                "cleanup exit has a non-contiguous ID or unknown source block",
            ));
        }
        let expected_regions = expected_exit_regions(function, exit)?;
        if exit.leaves_regions != expected_regions {
            return Err(replay_error(
                function,
                "cleanup exit does not leave the exact source-to-target region chain",
            ));
        }
        let mut prior: Option<super::CleanupRegionId> = None;
        for region in &exit.leaves_regions {
            let Some(item) = plan.regions.get(region.0 as usize) else {
                return Err(replay_error(
                    function,
                    "cleanup exit references unknown region",
                ));
            };
            if let Some(child) = prior {
                if plan.regions[child.0 as usize].parent != Some(*region) {
                    return Err(replay_error(
                        function,
                        "cleanup exit regions are not a parent chain",
                    ));
                }
            }
            prior = Some(item.id);
        }
        let mut finalized = BTreeSet::new();
        for action in &exit.finalize_in_order {
            let under = validate_place(function, &action.source, storage, leaves)?;
            if under.len() != 1
                || under[0] != action.guard_flag
                || leaves[&action.guard_flag].place != action.source
            {
                return Err(replay_error(
                    function,
                    "finalizer guard does not name its exact cleanup leaf",
                ));
            }
            let leaf = leaves
                .get(&action.guard_flag)
                .expect("validated cleanup flag");
            if leaf.lifecycle != action.lifecycle_id || !finalized.insert(action.guard_flag) {
                return Err(replay_error(
                    function,
                    "finalizer lifecycle or guard is invalid or repeated",
                ));
            }
        }
        match &exit.continuation {
            ExitContinuation::Continue(edge) => {
                validate_owned_edge(function, exit.from, *edge, &mut continuation_edges)?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(replay_error(
                        function,
                        "cleanup continuation edge is conditional",
                    ));
                }
            }
            ExitContinuation::CommitResult { source } => match source {
                CleanupResultSource::Scalar { expression } => {
                    if type_needs_drop(program, function, &function.return_type)? {
                        return Err(replay_error(
                            function,
                            "droppable function result uses a scalar result commit",
                        ));
                    }
                    if expression != &function.body.id {
                        return Err(replay_error(
                            function,
                            "scalar result commit does not publish the function body",
                        ));
                    }
                }
                CleanupResultSource::Owned { storage: result } => {
                    if !matches!(function.return_type, ResolvedType::Nominal { .. })
                        || !type_needs_drop(program, function, &function.return_type)?
                        || result.storage != StorageId::ProvisionalResult
                        || !result.projections.is_empty()
                    {
                        return Err(replay_error(
                            function,
                            "owned result commit must publish the whole droppable nominal provisional result",
                        ));
                    }
                    validate_place(function, result, storage, leaves)?;
                    let provisional_leaves = leaves
                        .values()
                        .filter(|leaf| leaf.place.storage == StorageId::ProvisionalResult)
                        .count();
                    if provisional_leaves == 0 {
                        return Err(replay_error(
                            function,
                            "owned result commit has no provisional-result leaves",
                        ));
                    }
                }
            },
            ExitContinuation::ReturnFailure { source } => {
                require_status(function, &statuses, source)?;
                let selected = plan.blocks[exit.from.0 as usize]
                    .transitions
                    .iter()
                    .filter_map(|transition| match transition {
                        CleanupTransition::SelectFailure { source } => Some(source),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if selected.as_slice() != [source] {
                    return Err(replay_error(
                        function,
                        "failure exit does not return its uniquely selected status",
                    ));
                }
            }
            ExitContinuation::ReturnUnit => {
                return Err(replay_error(
                    function,
                    "ReturnUnit is invalid for current source-function return types",
                ));
            }
        }
    }
    for region in &plan.regions {
        let exit = &plan.exits[region.normal_scope_end.0 as usize];
        if !exit.leaves_regions.contains(&region.id) {
            return Err(replay_error(
                function,
                "region normal-scope exit does not leave that region",
            ));
        }
    }
    Ok(())
}

fn expected_exit_regions(
    function: &ResolvedFunction,
    exit: &ExitTarget,
) -> Result<Vec<CleanupRegionId>, Diagnostic> {
    let plan = &function.cleanup_plan;
    let source_region = plan
        .blocks
        .get(exit.from.0 as usize)
        .ok_or_else(|| replay_error(function, "cleanup exit has an unknown source block"))?
        .region;
    let source_chain = region_chain_to_root(function, source_region)?;
    match &exit.continuation {
        ExitContinuation::Continue(edge) => {
            let target = plan
                .edges
                .get(edge.0 as usize)
                .and_then(|edge| plan.blocks.get(edge.to.0 as usize))
                .ok_or_else(|| {
                    replay_error(function, "cleanup continuation has an unknown target")
                })?;
            let target_ancestors = region_chain_to_root(function, target.region)?
                .into_iter()
                .collect::<BTreeSet<_>>();
            Ok(source_chain
                .into_iter()
                .take_while(|region| !target_ancestors.contains(region))
                .collect())
        }
        ExitContinuation::CommitResult { .. }
        | ExitContinuation::ReturnFailure { .. }
        | ExitContinuation::ReturnUnit => Ok(source_chain),
    }
}

fn region_chain_to_root(
    function: &ResolvedFunction,
    start: CleanupRegionId,
) -> Result<Vec<CleanupRegionId>, Diagnostic> {
    let mut chain = Vec::new();
    let mut current = Some(start);
    while let Some(region) = current {
        let item = function
            .cleanup_plan
            .regions
            .get(region.0 as usize)
            .ok_or_else(|| {
                replay_error(
                    function,
                    "cleanup region chain references an unknown region",
                )
            })?;
        chain.push(region);
        current = item.parent;
    }
    Ok(chain)
}

fn validate_typed_control_skeleton(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let mut expected = hir_skeleton_paths(program, function)?;
    let expected_units = expected.iter().fold(0_usize, |total, path| {
        total.saturating_add(path.observations.len().saturating_add(1))
    });
    budget.charge(function, expected_units, "typed-HIR skeleton expansion")?;
    let mut actual = plan_skeleton_paths(function, budget)?;
    expected.sort();
    actual.sort();
    if actual != expected {
        return Err(replay_error(
            function,
            "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
        ));
    }
    Ok(())
}

fn hir_skeleton_paths(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    let mut paths = vec![empty_expr_path()];
    for contract in &function.requires {
        paths = sequence_expression(program, function, paths, contract)?;
        paths = split_contract(paths, contract);
    }
    paths = sequence_expression(program, function, paths, &function.body)?;
    if type_needs_drop(program, function, &function.return_type)? {
        paths = transfer_completed_paths(
            function,
            paths,
            function.body.id.clone(),
            CleanupPlace {
                storage: StorageId::ProvisionalResult,
                projections: Vec::new(),
            },
            "owned function result",
        )?;
    }
    for contract in &function.ensures {
        paths = sequence_expression(program, function, paths, contract)?;
        paths = split_contract(paths, contract);
    }
    Ok(paths
        .into_iter()
        .map(|path| SkeletonPath {
            observations: path.observations,
            terminal: if path.failed {
                SkeletonTerminal::Failure
            } else {
                SkeletonTerminal::Success
            },
        })
        .collect())
}

fn empty_expr_path() -> ExprSkeletonPath {
    ExprSkeletonPath {
        observations: Vec::new(),
        owned_source: None,
        failed: false,
    }
}

fn sequence_expression(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    prefixes: Vec<ExprSkeletonPath>,
    expression: &ResolvedExpr,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let suffixes = expression_skeleton(program, function, expression)?;
    let mut combined = Vec::new();
    for prefix in prefixes {
        if prefix.failed {
            combined.push(prefix);
            continue;
        }
        for suffix in &suffixes {
            let mut observations = prefix.observations.clone();
            observations.extend(suffix.observations.clone());
            combined.push(ExprSkeletonPath {
                observations,
                owned_source: suffix.owned_source.clone(),
                failed: suffix.failed,
            });
        }
    }
    Ok(combined)
}

fn expression_skeleton(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) => Ok(vec![empty_expr_path()]),
        ResolvedExprKind::Place(place) => {
            let owned_source = if expression.ownership == OwnershipMode::Own
                && type_needs_drop(program, function, &expression.ty)?
            {
                Some(cleanup_place_from_hir(function, place)?)
            } else {
                None
            };
            Ok(vec![ExprSkeletonPath {
                observations: Vec::new(),
                owned_source,
                failed: false,
            }])
        }
        ResolvedExprKind::Call { callee, args } => {
            call_skeleton(program, function, expression, callee, args)
        }
        ResolvedExprKind::Unary { op, value } => {
            let paths = sequence_expression(program, function, vec![empty_expr_path()], value)?;
            if *op == UnaryOp::Neg {
                Ok(split_status_paths(
                    paths,
                    StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    },
                ))
            } else {
                Ok(paths)
            }
        }
        ResolvedExprKind::Binary {
            op: BinaryOp::And | BinaryOp::Or,
            left,
            right,
        } => lazy_skeleton(
            program,
            function,
            expression.clone(),
            *left.clone(),
            *right.clone(),
        ),
        ResolvedExprKind::Binary { op, left, right } => {
            let paths = sequence_expression(program, function, vec![empty_expr_path()], left)?;
            let paths = sequence_expression(program, function, paths, right)?;
            if matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            ) {
                Ok(split_status_paths(
                    paths,
                    StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    },
                ))
            } else {
                Ok(paths)
            }
        }
        ResolvedExprKind::Block { statements, tail } => {
            let mut paths = vec![empty_expr_path()];
            for statement in statements {
                let ResolvedStatement::Let { binding, value, .. } = statement;
                paths = sequence_expression(program, function, paths, value)?;
                if binding.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &binding.ty)?
                {
                    paths = transfer_completed_paths(
                        function,
                        paths,
                        value.id.clone(),
                        CleanupPlace {
                            storage: StorageId::Value(binding.id.clone()),
                            projections: Vec::new(),
                        },
                        "owned binding",
                    )?;
                }
            }
            paths = sequence_expression(program, function, paths, tail)?;
            if expression.ownership == OwnershipMode::Own
                && type_needs_drop(program, function, &expression.ty)?
            {
                paths = transfer_completed_paths(
                    function,
                    paths,
                    expression.id.clone(),
                    temporary_place(expression),
                    "owned block result",
                )?;
            }
            Ok(paths)
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            let mut paths = vec![empty_expr_path()];
            for field in fields {
                paths = sequence_expression(program, function, paths, &field.value)?;
                if field.value.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &field.value.ty)?
                {
                    return Err(replay_error(
                        function,
                        "droppable variant payload reached the copy-only cleanup skeleton",
                    ));
                }
            }
            Ok(paths)
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            match_skeleton(program, function, expression, scrutinee, arms)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => if_skeleton(
            program,
            function,
            expression,
            condition,
            then_branch,
            else_branch,
        ),
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            let mut paths = vec![empty_expr_path()];
            let destination = temporary_place(expression);
            for field in fields {
                paths = sequence_expression(program, function, paths, &field.value)?;
                if field.value.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &field.value.ty)?
                {
                    let mut field_destination = destination.clone();
                    field_destination.projections.push(field.field.clone());
                    paths = transfer_completed_paths(
                        function,
                        paths,
                        field.value.id.clone(),
                        field_destination,
                        "owned record field",
                    )?;
                }
            }
            for path in &mut paths {
                if !path.failed {
                    path.owned_source = Some(destination.clone());
                }
            }
            Ok(paths)
        }
        ResolvedExprKind::UpdateRecord {
            base,
            record,
            fields,
        } => {
            let mut paths = sequence_expression(program, function, vec![empty_expr_path()], base)?;
            let needs_cleanup = expression.ownership == OwnershipMode::Own
                && type_needs_drop(program, function, &expression.ty)?;
            if !needs_cleanup {
                for field in fields {
                    paths = sequence_expression(program, function, paths, &field.value)?;
                }
                return Ok(paths);
            }

            let staged_base = temporary_place(base);
            for path in &mut paths {
                if path.failed {
                    continue;
                }
                let source = path.owned_source.take().ok_or_else(|| {
                    replay_error(
                        function,
                        "owned record update base has no HIR cleanup source",
                    )
                })?;
                if source != staged_base {
                    path.observations.push(SkeletonObservation::Transfer {
                        at: base.id.clone(),
                        source,
                        destination: staged_base.clone(),
                    });
                }
                path.owned_source = Some(staged_base.clone());
            }

            let destination = temporary_place(expression);
            let mut replaced = BTreeSet::new();
            for field in fields {
                if !replaced.insert(field.field.clone()) {
                    return Err(replay_error(
                        function,
                        format!("record update repeats field `{}`", field.field),
                    ));
                }
                paths = sequence_expression(program, function, paths, &field.value)?;
                if field.value.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &field.value.ty)?
                {
                    let mut field_destination = destination.clone();
                    field_destination.projections.push(field.field.clone());
                    paths = transfer_completed_paths(
                        function,
                        paths,
                        field.value.id.clone(),
                        field_destination,
                        "owned record replacement",
                    )?;
                }
            }

            let declarations = program.declarations.record_fields(record).ok_or_else(|| {
                replay_error(
                    function,
                    format!("record update has unknown record `{record}`"),
                )
            })?;
            for field in declarations {
                if replaced.contains(&field.id) || !type_needs_drop(program, function, &field.ty)? {
                    continue;
                }
                for path in &mut paths {
                    if path.failed {
                        continue;
                    }
                    let mut source = staged_base.clone();
                    source.projections.push(field.id.clone());
                    let mut field_destination = destination.clone();
                    field_destination.projections.push(field.id.clone());
                    path.observations.push(SkeletonObservation::Transfer {
                        at: expression.id.clone(),
                        source,
                        destination: field_destination,
                    });
                }
            }
            for path in &mut paths {
                if !path.failed {
                    path.owned_source = Some(destination.clone());
                }
            }
            Ok(paths)
        }
        ResolvedExprKind::Project { base, field } => {
            let mut paths = sequence_expression(program, function, vec![empty_expr_path()], base)?;
            if expression.ownership == OwnershipMode::Own
                && type_needs_drop(program, function, &expression.ty)?
            {
                for path in &mut paths {
                    if path.failed {
                        continue;
                    }
                    let mut source = path.owned_source.take().ok_or_else(|| {
                        replay_error(function, "owned projection has no HIR cleanup source")
                    })?;
                    source.projections.push(field.clone());
                    let destination = temporary_place(expression);
                    path.observations.push(SkeletonObservation::Transfer {
                        at: expression.id.clone(),
                        source,
                        destination: destination.clone(),
                    });
                    path.owned_source = Some(destination);
                }
            }
            Ok(paths)
        }
    }
}

fn match_skeleton(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arms: &[ResolvedMatchArm],
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    if type_needs_drop(program, function, &expression.ty)? {
        return Err(replay_error(
            function,
            "droppable match result reached the copy-only cleanup skeleton",
        ));
    }
    if type_needs_drop(program, function, &scrutinee.ty)? {
        return Err(replay_error(
            function,
            "droppable match scrutinee reached the copy-only cleanup skeleton",
        ));
    }
    if arms.is_empty() {
        return Err(replay_error(function, "copy-variant match has no arms"));
    }

    let scrutinee_paths = expression_skeleton(program, function, scrutinee)?;
    let mut results = Vec::new();
    for mut path in scrutinee_paths {
        if path.failed {
            results.push(path);
            continue;
        }
        path.owned_source = None;
        for (index, arm) in arms.iter().enumerate() {
            let final_arm = index + 1 == arms.len();
            let mut selected = path.clone();
            if !final_arm {
                let ResolvedMatchPattern::Variant { case, .. } = &arm.pattern else {
                    return Err(replay_error(
                        function,
                        "wildcard match arm must be the final exhaustive arm",
                    ));
                };
                selected
                    .observations
                    .push(SkeletonObservation::VariantCase {
                        scrutinee: scrutinee.id.clone(),
                        case: case.clone(),
                        matches: true,
                    });
                path.observations.push(SkeletonObservation::VariantCase {
                    scrutinee: scrutinee.id.clone(),
                    case: case.clone(),
                    matches: false,
                });
            }
            results.extend(sequence_expression(
                program,
                function,
                vec![selected],
                &arm.value,
            )?);
        }
    }
    Ok(results)
}

fn call_skeleton(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    callee: &DeclarationId,
    args: &[ResolvedExpr],
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let target = program
        .functions
        .iter()
        .find(|target| target.id == *callee)
        .ok_or_else(|| replay_error(function, format!("unknown skeleton callee `{callee}`")))?;
    let mut states = vec![(empty_expr_path(), Vec::<(u32, CleanupPlace)>::new())];
    for (index, (argument, parameter)) in args.iter().zip(&target.params).enumerate() {
        let suffixes = expression_skeleton(program, function, argument)?;
        let mut next = Vec::new();
        for (prefix, commits) in states {
            if prefix.failed {
                next.push((prefix, commits));
                continue;
            }
            for suffix in &suffixes {
                let mut observations = prefix.observations.clone();
                observations.extend(suffix.observations.clone());
                let mut path = ExprSkeletonPath {
                    observations,
                    owned_source: suffix.owned_source.clone(),
                    failed: suffix.failed,
                };
                let mut path_commits = commits.clone();
                if !path.failed
                    && parameter.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &parameter.ty)?
                {
                    let parameter_index = u32::try_from(index)
                        .map_err(|_| replay_error(function, "too many skeleton call arguments"))?;
                    let epoch = CleanupPlace {
                        storage: StorageId::CallArgument {
                            call: expression.id.clone(),
                            parameter_index,
                            value_expression: argument.id.clone(),
                        },
                        projections: Vec::new(),
                    };
                    let source = path.owned_source.take().ok_or_else(|| {
                        replay_error(function, "owned call argument has no HIR cleanup source")
                    })?;
                    path.observations.push(SkeletonObservation::Transfer {
                        at: argument.id.clone(),
                        source,
                        destination: epoch.clone(),
                    });
                    path_commits.push((parameter_index, epoch));
                }
                next.push((path, path_commits));
            }
        }
        states = next;
    }

    let source = StatusSourceId {
        expression: expression.id.clone(),
        lane: StatusLane::OperationFailure,
    };
    let mut results = Vec::new();
    for (mut path, commits) in states {
        if path.failed {
            results.push(path);
            continue;
        }
        path.observations.push(SkeletonObservation::CallCommit {
            call: expression.id.clone(),
            arguments: commits,
        });
        let mut failure = path.clone();
        failure.observations.push(SkeletonObservation::Status {
            source: source.clone(),
            success: false,
        });
        failure.failed = true;
        failure.owned_source = None;
        results.push(failure);

        path.observations.push(SkeletonObservation::Status {
            source: source.clone(),
            success: true,
        });
        if expression.ownership == OwnershipMode::Own
            && type_needs_drop(program, function, &expression.ty)?
        {
            let destination = temporary_place(expression);
            path.observations.push(SkeletonObservation::Initialize {
                at: expression.id.clone(),
                destination: destination.clone(),
            });
            path.owned_source = Some(destination);
        } else {
            path.owned_source = None;
        }
        results.push(path);
    }
    Ok(results)
}

fn lazy_skeleton(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: ResolvedExpr,
    left: ResolvedExpr,
    right: ResolvedExpr,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let ResolvedExprKind::Binary { op, .. } = expression.kind else {
        return Err(replay_error(
            function,
            "lazy skeleton received non-binary HIR",
        ));
    };
    let left_paths = expression_skeleton(program, function, &left)?;
    let mut results = Vec::new();
    for path in left_paths {
        if path.failed {
            results.push(path);
            continue;
        }
        for value in [true, false] {
            let mut branch = path.clone();
            branch.observations.push(SkeletonObservation::Boolean {
                expression: left.id.clone(),
                value,
            });
            let evaluates_right = match op {
                BinaryOp::And => value,
                BinaryOp::Or => !value,
                _ => return Err(replay_error(function, "invalid lazy skeleton operation")),
            };
            if evaluates_right {
                results.extend(sequence_expression(
                    program,
                    function,
                    vec![branch],
                    &right,
                )?);
            } else {
                branch.owned_source = None;
                results.push(branch);
            }
        }
    }
    Ok(results)
}

fn if_skeleton(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    condition: &ResolvedExpr,
    then_branch: &ResolvedExpr,
    else_branch: &ResolvedExpr,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let condition_paths = expression_skeleton(program, function, condition)?;
    let mut results = Vec::new();
    for path in condition_paths {
        if path.failed {
            results.push(path);
            continue;
        }
        for (value, branch_expression) in [(true, then_branch), (false, else_branch)] {
            let mut branch = path.clone();
            branch.observations.push(SkeletonObservation::Boolean {
                expression: condition.id.clone(),
                value,
            });
            let branch_paths =
                sequence_expression(program, function, vec![branch], branch_expression)?;
            if expression.ownership == OwnershipMode::Own
                && type_needs_drop(program, function, &expression.ty)?
            {
                results.extend(transfer_completed_paths(
                    function,
                    branch_paths,
                    expression.id.clone(),
                    temporary_place(expression),
                    "owned conditional result",
                )?);
            } else {
                results.extend(branch_paths);
            }
        }
    }
    Ok(results)
}

fn split_status_paths(
    paths: Vec<ExprSkeletonPath>,
    source: StatusSourceId,
) -> Vec<ExprSkeletonPath> {
    let mut results = Vec::new();
    for mut path in paths {
        if path.failed {
            results.push(path);
            continue;
        }
        let mut failure = path.clone();
        failure.observations.push(SkeletonObservation::Status {
            source: source.clone(),
            success: false,
        });
        failure.failed = true;
        failure.owned_source = None;
        results.push(failure);
        path.observations.push(SkeletonObservation::Status {
            source: source.clone(),
            success: true,
        });
        path.owned_source = None;
        results.push(path);
    }
    results
}

fn split_contract(paths: Vec<ExprSkeletonPath>, contract: &ResolvedExpr) -> Vec<ExprSkeletonPath> {
    let mut results = Vec::new();
    for mut path in paths {
        if path.failed {
            results.push(path);
            continue;
        }
        let mut failure = path.clone();
        failure.observations.push(SkeletonObservation::Boolean {
            expression: contract.id.clone(),
            value: false,
        });
        failure.failed = true;
        failure.owned_source = None;
        results.push(failure);
        path.observations.push(SkeletonObservation::Boolean {
            expression: contract.id.clone(),
            value: true,
        });
        path.owned_source = None;
        results.push(path);
    }
    results
}

fn transfer_completed_paths(
    function: &ResolvedFunction,
    mut paths: Vec<ExprSkeletonPath>,
    at: ExpressionId,
    destination: CleanupPlace,
    description: &str,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    for path in &mut paths {
        if path.failed {
            continue;
        }
        let source = path.owned_source.take().ok_or_else(|| {
            replay_error(function, format!("{description} has no HIR cleanup source"))
        })?;
        path.observations.push(SkeletonObservation::Transfer {
            at: at.clone(),
            source,
            destination: destination.clone(),
        });
        path.owned_source = Some(destination.clone());
    }
    Ok(paths)
}

fn temporary_place(expression: &ResolvedExpr) -> CleanupPlace {
    CleanupPlace {
        storage: StorageId::Temporary(expression.id.clone()),
        projections: Vec::new(),
    }
}

fn cleanup_place_from_hir(
    function: &ResolvedFunction,
    place: &crate::hir::Place,
) -> Result<CleanupPlace, Diagnostic> {
    let storage = if place.root == function.result_id {
        StorageId::ProvisionalResult
    } else {
        StorageId::Value(place.root.clone())
    };
    let mut projections = Vec::new();
    for projection in &place.projections {
        match projection {
            PlaceProjection::Field(field) => projections.push(field.clone()),
            PlaceProjection::VariantField { .. } => {
                return Err(replay_error(
                    function,
                    "variant projection reached current cleanup skeleton",
                ));
            }
        }
    }
    Ok(CleanupPlace {
        storage,
        projections,
    })
}

fn plan_skeleton_paths(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    let plan = &function.cleanup_plan;
    let mut queue = VecDeque::from([(plan.entry, Vec::<SkeletonObservation>::new())]);
    let mut paths = Vec::new();
    while let Some((block, mut observations)) = queue.pop_front() {
        budget.charge(
            function,
            observations.len().saturating_add(1),
            "cleanup-plan skeleton expansion",
        )?;
        let block = &plan.blocks[block.0 as usize];
        for transition in &block.transitions {
            match transition {
                CleanupTransition::Initialize { at, destination } => {
                    observations.push(SkeletonObservation::Initialize {
                        at: at.clone(),
                        destination: destination.clone(),
                    });
                }
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => observations.push(SkeletonObservation::Transfer {
                    at: at.clone(),
                    source: source.clone(),
                    destination: destination.clone(),
                }),
                CleanupTransition::CallCommit { call, arguments } => {
                    observations.push(SkeletonObservation::CallCommit {
                        call: call.clone(),
                        arguments: arguments
                            .iter()
                            .map(|argument| (argument.parameter_index, argument.source.clone()))
                            .collect(),
                    });
                }
                CleanupTransition::SelectFailure { .. } => {}
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                queue.push_back((plan.edges[edge.0 as usize].to, observations));
            }
            CleanupTerminator::Branch(edges) => {
                for edge in edges {
                    let edge = &plan.edges[edge.0 as usize];
                    let observation = match &edge.condition {
                        EdgeCondition::BooleanResult(expression, value) => {
                            SkeletonObservation::Boolean {
                                expression: expression.clone(),
                                value: *value,
                            }
                        }
                        EdgeCondition::VariantCase {
                            scrutinee,
                            case,
                            matches,
                        } => SkeletonObservation::VariantCase {
                            scrutinee: scrutinee.clone(),
                            case: case.clone(),
                            matches: *matches,
                        },
                        EdgeCondition::StatusZero(source) => SkeletonObservation::Status {
                            source: source.clone(),
                            success: true,
                        },
                        EdgeCondition::StatusNonzero(source) => SkeletonObservation::Status {
                            source: source.clone(),
                            success: false,
                        },
                        EdgeCondition::Always => {
                            return Err(replay_error(
                                function,
                                "branch skeleton contains an unconditional edge",
                            ));
                        }
                    };
                    let mut branch = observations.clone();
                    branch.push(observation);
                    queue.push_back((edge.to, branch));
                }
            }
            CleanupTerminator::Exit(exit) => {
                let exit = &plan.exits[exit.0 as usize];
                match exit.continuation {
                    ExitContinuation::Continue(edge) => {
                        queue.push_back((plan.edges[edge.0 as usize].to, observations));
                    }
                    ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit => {
                        paths.push(SkeletonPath {
                            observations,
                            terminal: SkeletonTerminal::Success,
                        });
                    }
                    ExitContinuation::ReturnFailure { .. } => paths.push(SkeletonPath {
                        observations,
                        terminal: SkeletonTerminal::Failure,
                    }),
                }
            }
        }
    }
    Ok(paths)
}

fn validate_path_states(
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let storage_regions = storage_regions(function)?;
    let contract_sources = plan
        .status_sources
        .iter()
        .filter(|source| source.id.lane == StatusLane::ContractFalse)
        .map(|source| (source.id.expression.clone(), source.id.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut initial = PathState {
        live_order: Vec::new(),
        pending_failure: None,
        selected_failure: None,
        published: false,
    };
    for place in &plan.entry_state.live_owned_parameters {
        let flags = validate_place(function, place, storage, leaves)?;
        append_dead_flags(function, &mut initial, flags, "entry state")?;
    }

    let mut incoming = vec![BTreeSet::<PathState>::new(); plan.blocks.len()];
    let mut queue = VecDeque::from([(plan.entry, initial)]);
    let mut terminal_paths = 0_usize;
    let mut successful_paths = 0_usize;

    while let Some((block_id, state)) = queue.pop_front() {
        let states = &mut incoming[block_id.0 as usize];
        let join_units = states
            .len()
            .saturating_add(1)
            .saturating_mul(state.live_order.len().saturating_add(1));
        budget.charge(function, join_units, "all-path ownership replay")?;
        validate_join_compatibility(function, states, &state, block_id)?;
        if !states.insert(state.clone()) {
            continue;
        }

        let block = &plan.blocks[block_id.0 as usize];
        let mut state = state;
        for transition in &block.transitions {
            execute_replay_transition(function, transition, &mut state, storage, leaves)?;
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                require_normal_flow_state(function, &state, block_id)?;
                queue.push_back((plan.edges[edge.0 as usize].to, state));
            }
            CleanupTerminator::Branch(edges) => {
                require_normal_flow_state(function, &state, block_id)?;
                for edge in edges {
                    let edge = &plan.edges[edge.0 as usize];
                    let next = state_for_edge(
                        function,
                        state.clone(),
                        &edge.condition,
                        &contract_sources,
                    )?;
                    queue.push_back((edge.to, next));
                }
            }
            CleanupTerminator::Exit(exit_id) => {
                let exit = &plan.exits[exit_id.0 as usize];
                match replay_exit(function, exit, state, &storage_regions, storage, leaves)? {
                    Some((edge, continued)) => {
                        queue.push_back((plan.edges[edge.0 as usize].to, continued));
                    }
                    None => {
                        terminal_paths = terminal_paths.checked_add(1).ok_or_else(|| {
                            replay_error(function, "too many terminal cleanup paths")
                        })?;
                        if matches!(
                            exit.continuation,
                            ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit
                        ) {
                            successful_paths =
                                successful_paths.checked_add(1).ok_or_else(|| {
                                    replay_error(function, "too many successful cleanup paths")
                                })?;
                        }
                    }
                }
            }
        }
    }

    if terminal_paths == 0 || successful_paths == 0 {
        return Err(replay_error(
            function,
            "cleanup CFG has no replayable terminal success path",
        ));
    }
    Ok(())
}

fn storage_regions(
    function: &ResolvedFunction,
) -> Result<BTreeMap<StorageId, CleanupRegionId>, Diagnostic> {
    let mut regions = BTreeMap::new();
    for region in &function.cleanup_plan.regions {
        for storage in &region.slots {
            if regions.insert(storage.clone(), region.id).is_some() {
                return Err(replay_error(
                    function,
                    "cleanup storage has multiple owning regions during replay",
                ));
            }
        }
    }
    Ok(regions)
}

fn execute_replay_transition(
    function: &ResolvedFunction,
    transition: &CleanupTransition,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), Diagnostic> {
    if state.published {
        return Err(replay_error(
            function,
            "cleanup transition occurs after result publication",
        ));
    }
    if state.selected_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(replay_error(
            function,
            "ordinary cleanup transition occurs after failure selection",
        ));
    }
    if state.pending_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(replay_error(
            function,
            "operation failure is not selected before another transition",
        ));
    }

    match transition {
        CleanupTransition::Initialize { destination, .. } => {
            let flags = validate_place(function, destination, storage, leaves)?;
            append_dead_flags(function, state, flags, "initialize transition")?;
        }
        CleanupTransition::Transfer {
            source,
            destination,
            ..
        } => replay_transfer(function, state, source, destination, storage, leaves)?,
        CleanupTransition::CallCommit { call, arguments } => {
            let mut consumed = BTreeSet::new();
            for argument in arguments {
                for flag in validate_place(function, &argument.source, storage, leaves)? {
                    if !state.live_order.contains(&flag) {
                        return Err(replay_error(
                            function,
                            format!(
                                "call `{call}` atomically consumes dead argument flag {}",
                                flag.0
                            ),
                        ));
                    }
                    if !consumed.insert(flag) {
                        return Err(replay_error(
                            function,
                            format!(
                                "call `{call}` atomically consumes argument flag {} twice",
                                flag.0
                            ),
                        ));
                    }
                }
            }
            state.live_order.retain(|flag| !consumed.contains(flag));
        }
        CleanupTransition::SelectFailure { source } => {
            if state.selected_failure.is_some() {
                return Err(replay_error(
                    function,
                    "failure selection is not write-once during path replay",
                ));
            }
            if state.pending_failure.as_ref() != Some(source) {
                return Err(replay_error(
                    function,
                    format!(
                        "selected failure `{}` does not match the pending failing edge",
                        source.expression
                    ),
                ));
            }
            state.pending_failure = None;
            state.selected_failure = Some(source.clone());
        }
    }
    Ok(())
}

fn replay_transfer(
    function: &ResolvedFunction,
    state: &mut PathState,
    source: &CleanupPlace,
    destination: &CleanupPlace,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), Diagnostic> {
    let source_flags = validate_place(function, source, storage, leaves)?;
    let destination_flags = validate_place(function, destination, storage, leaves)?;
    let source_set = source_flags.iter().copied().collect::<BTreeSet<_>>();
    let destination_set = destination_flags.iter().copied().collect::<BTreeSet<_>>();
    if source_set.len() != destination_set.len() || !source_set.is_disjoint(&destination_set) {
        return Err(replay_error(
            function,
            "cleanup transfer does not have disjoint equal-size ownership epochs",
        ));
    }
    if source_set
        .iter()
        .any(|flag| !state.live_order.contains(flag))
    {
        return Err(replay_error(
            function,
            "cleanup transfer reads a dead source leaf",
        ));
    }
    if destination_set
        .iter()
        .any(|flag| state.live_order.contains(flag))
    {
        return Err(replay_error(
            function,
            "cleanup transfer initializes a live destination leaf",
        ));
    }

    let mapping = transfer_mapping(function, source, destination, leaves)?;
    let source_history = state
        .live_order
        .iter()
        .filter(|flag| source_set.contains(flag))
        .copied()
        .collect::<Vec<_>>();
    state.live_order.retain(|flag| !source_set.contains(flag));
    if destination.projections.is_empty() {
        state.live_order.extend(destination_flags);
    } else {
        for source_flag in source_history {
            state.live_order.push(mapping[&source_flag]);
        }
        normalize_completed_storage(function, state, &destination.storage, leaves)?;
    }
    Ok(())
}

fn transfer_mapping(
    function: &ResolvedFunction,
    source: &CleanupPlace,
    destination: &CleanupPlace,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<BTreeMap<LivenessFlagId, LivenessFlagId>, Diagnostic> {
    let mut mapping = BTreeMap::new();
    let mut destinations = BTreeSet::new();
    for (source_flag, source_leaf) in leaves.iter().filter(|(_, leaf)| {
        leaf.place.storage == source.storage
            && leaf.place.projections.starts_with(&source.projections)
    }) {
        let relative = source_leaf
            .place
            .projections
            .strip_prefix(source.projections.as_slice())
            .ok_or_else(|| replay_error(function, "transfer source is not a projection prefix"))?;
        let projections = destination
            .projections
            .iter()
            .chain(relative)
            .cloned()
            .collect::<Vec<_>>();
        let Some((destination_flag, destination_leaf)) = leaves.iter().find(|(_, leaf)| {
            leaf.place.storage == destination.storage && leaf.place.projections == projections
        }) else {
            return Err(replay_error(
                function,
                "cleanup transfer leaf shapes do not correspond",
            ));
        };
        if source_leaf.lifecycle != destination_leaf.lifecycle
            || !destinations.insert(*destination_flag)
        {
            return Err(replay_error(
                function,
                "cleanup transfer changes a leaf lifecycle or aliases its destination",
            ));
        }
        mapping.insert(*source_flag, *destination_flag);
    }
    if mapping.is_empty() {
        return Err(replay_error(
            function,
            "cleanup transfer has no mapped ownership leaves",
        ));
    }
    Ok(mapping)
}

fn normalize_completed_storage(
    function: &ResolvedFunction,
    state: &mut PathState,
    storage: &StorageId,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), Diagnostic> {
    let flags = leaves
        .iter()
        .filter_map(|(flag, leaf)| (leaf.place.storage == *storage).then_some(*flag))
        .collect::<Vec<_>>();
    if flags.is_empty() {
        return Err(replay_error(
            function,
            "completed cleanup storage has no liveness flags",
        ));
    }
    if flags.iter().all(|flag| state.live_order.contains(flag)) {
        let set = flags.iter().copied().collect::<BTreeSet<_>>();
        state.live_order.retain(|flag| !set.contains(flag));
        state.live_order.extend(flags);
    }
    Ok(())
}

fn append_dead_flags(
    function: &ResolvedFunction,
    state: &mut PathState,
    flags: Vec<LivenessFlagId>,
    operation: &str,
) -> Result<(), Diagnostic> {
    if flags.is_empty() || flags.iter().any(|flag| state.live_order.contains(flag)) {
        return Err(replay_error(
            function,
            format!("{operation} initializes an empty or live cleanup place"),
        ));
    }
    state.live_order.extend(flags);
    Ok(())
}

fn state_for_edge(
    function: &ResolvedFunction,
    mut state: PathState,
    condition: &EdgeCondition,
    contract_sources: &BTreeMap<ExpressionId, StatusSourceId>,
) -> Result<PathState, Diagnostic> {
    if state.pending_failure.is_some() || state.selected_failure.is_some() || state.published {
        return Err(replay_error(
            function,
            "conditional edge follows pending, selected, or published state",
        ));
    }
    match condition {
        EdgeCondition::Always => {}
        EdgeCondition::BooleanResult(expression, false) => {
            if let Some(source) = contract_sources.get(expression) {
                state.pending_failure = Some(source.clone());
            }
        }
        EdgeCondition::BooleanResult(_, true)
        | EdgeCondition::VariantCase { .. }
        | EdgeCondition::StatusZero(_) => {}
        EdgeCondition::StatusNonzero(source) => {
            state.pending_failure = Some(source.clone());
        }
    }
    Ok(state)
}

fn replay_exit(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    mut state: PathState,
    storage_regions: &BTreeMap<StorageId, CleanupRegionId>,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<Option<(EdgeId, PathState)>, Diagnostic> {
    if state.published {
        return Err(replay_error(function, "cleanup exit follows publication"));
    }
    let leaving = exit.leaves_regions.iter().copied().collect::<BTreeSet<_>>();
    let protected_result = match &exit.continuation {
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Owned { storage: result },
        } => validate_place(function, result, storage, leaves)?
            .into_iter()
            .collect::<BTreeSet<_>>(),
        _ => BTreeSet::new(),
    };
    let expected = state
        .live_order
        .iter()
        .rev()
        .filter_map(|flag| {
            let leaf = &leaves[flag];
            let region = storage_regions[&leaf.place.storage];
            (leaving.contains(&region) && !protected_result.contains(flag)).then_some(*flag)
        })
        .collect::<Vec<_>>();
    let actual = exit
        .finalize_in_order
        .iter()
        .filter_map(|action| {
            state
                .live_order
                .contains(&action.guard_flag)
                .then_some(action.guard_flag)
        })
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(replay_error(
            function,
            format!(
                "exit {} does not finalize its live region leaves in reverse initialization order",
                exit.id.0
            ),
        ));
    }
    let finalized = expected.into_iter().collect::<BTreeSet<_>>();
    state.live_order.retain(|flag| !finalized.contains(flag));
    if state.live_order.iter().any(|flag| {
        let leaf = &leaves[flag];
        leaving.contains(&storage_regions[&leaf.place.storage]) && !protected_result.contains(flag)
    }) {
        return Err(replay_error(
            function,
            "cleanup exit retains a live non-result leaf in a region it leaves",
        ));
    }

    match &exit.continuation {
        ExitContinuation::Continue(edge) => {
            if state.pending_failure.is_some() || state.selected_failure.is_some() {
                return Err(replay_error(
                    function,
                    "normal cleanup continuation carries a failure state",
                ));
            }
            Ok(Some((*edge, state)))
        }
        ExitContinuation::ReturnFailure { source } => {
            if state.pending_failure.is_some()
                || state.selected_failure.as_ref() != Some(source)
                || !state.live_order.is_empty()
            {
                return Err(replay_error(
                    function,
                    "failure return changes status or retains live ownership",
                ));
            }
            Ok(None)
        }
        ExitContinuation::CommitResult { source } => {
            if state.pending_failure.is_some() || state.selected_failure.is_some() {
                return Err(replay_error(
                    function,
                    "result commit follows a pending or selected failure",
                ));
            }
            match source {
                CleanupResultSource::Scalar { .. } => {
                    if !state.live_order.is_empty() {
                        return Err(replay_error(
                            function,
                            "scalar result commits before non-result cleanup completes",
                        ));
                    }
                }
                CleanupResultSource::Owned { storage: result } => {
                    let result_flags = validate_place(function, result, storage, leaves)?;
                    if result_flags
                        .iter()
                        .any(|flag| !state.live_order.contains(flag))
                        || state.live_order.len() != result_flags.len()
                    {
                        return Err(replay_error(
                            function,
                            "owned result commit has incomplete result or remaining non-result ownership",
                        ));
                    }
                    let result_flags = result_flags.into_iter().collect::<BTreeSet<_>>();
                    state.live_order.retain(|flag| !result_flags.contains(flag));
                }
            }
            state.published = true;
            if !state.live_order.is_empty() {
                return Err(replay_error(
                    function,
                    "result publication leaves cleanup ownership live",
                ));
            }
            Ok(None)
        }
        ExitContinuation::ReturnUnit => {
            if state.pending_failure.is_some()
                || state.selected_failure.is_some()
                || !state.live_order.is_empty()
            {
                return Err(replay_error(
                    function,
                    "unit return follows failure or incomplete cleanup",
                ));
            }
            state.published = true;
            Ok(None)
        }
    }
}

fn require_normal_flow_state(
    function: &ResolvedFunction,
    state: &PathState,
    block: BlockId,
) -> Result<(), Diagnostic> {
    if state.pending_failure.is_none() && state.selected_failure.is_none() && !state.published {
        Ok(())
    } else {
        Err(replay_error(
            function,
            format!(
                "block {} continues ordinary control flow with failure or publication state",
                block.0
            ),
        ))
    }
}

fn validate_join_compatibility(
    function: &ResolvedFunction,
    existing: &BTreeSet<PathState>,
    incoming: &PathState,
    block: BlockId,
) -> Result<(), Diagnostic> {
    if existing.iter().any(|state| {
        state.pending_failure != incoming.pending_failure
            || state.selected_failure != incoming.selected_failure
            || state.published != incoming.published
    }) {
        return Err(replay_error(
            function,
            format!(
                "cleanup join at block {} has incompatible control states",
                block.0
            ),
        ));
    }

    let histories = existing
        .iter()
        .map(|state| &state.live_order)
        .chain(std::iter::once(&incoming.live_order))
        .collect::<Vec<_>>();
    let flags = histories
        .iter()
        .flat_map(|history| history.iter().copied())
        .collect::<BTreeSet<_>>();
    let mut successors = flags
        .iter()
        .map(|flag| (*flag, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut indegree = flags
        .iter()
        .map(|flag| (*flag, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for history in histories {
        for pair in history.windows(2) {
            if successors
                .get_mut(&pair[0])
                .expect("replayed live flag is indexed")
                .insert(pair[1])
            {
                indegree.entry(pair[1]).and_modify(|degree| *degree += 1);
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(flag, degree)| (*degree == 0).then_some(*flag))
        .collect::<BTreeSet<_>>();
    let mut visited = 0_usize;
    while let Some(flag) = ready.pop_first() {
        visited += 1;
        for successor in &successors[&flag] {
            let degree = indegree
                .get_mut(successor)
                .expect("replayed successor flag is indexed");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(*successor);
            }
        }
    }
    if visited != flags.len() {
        return Err(replay_error(
            function,
            format!(
                "cleanup join at block {} has conflicting initialization histories",
                block.0
            ),
        ));
    }
    Ok(())
}

fn validate_reachable_acyclic_cfg(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let mut colors = vec![0_u8; plan.blocks.len()];
    let mut stack = vec![(plan.entry, false)];
    while let Some((block, expanded)) = stack.pop() {
        let index = block.0 as usize;
        if expanded {
            colors[index] = 2;
            continue;
        }
        match colors[index] {
            1 => return Err(replay_error(function, "cleanup CFG contains a cycle")),
            2 => continue,
            _ => colors[index] = 1,
        }
        stack.push((block, true));
        let successors = block_successors(function, block);
        for successor in successors.into_iter().rev() {
            if colors[successor.0 as usize] == 1 {
                return Err(replay_error(function, "cleanup CFG contains a cycle"));
            }
            if colors[successor.0 as usize] == 0 {
                stack.push((successor, false));
            }
        }
    }
    if colors.contains(&0) {
        return Err(replay_error(
            function,
            "cleanup CFG contains an unreachable block",
        ));
    }
    Ok(())
}

fn validate_reference_coverage(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let mut edge_references = vec![0_usize; plan.edges.len()];
    let mut exit_references = vec![0_usize; plan.exits.len()];
    let mut status_references = BTreeSet::new();

    for block in &plan.blocks {
        for transition in &block.transitions {
            if let CleanupTransition::SelectFailure { source } = transition {
                status_references.insert(source.clone());
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => edge_references[edge.0 as usize] += 1,
            CleanupTerminator::Branch(edges) => {
                for edge in edges {
                    edge_references[edge.0 as usize] += 1;
                }
            }
            CleanupTerminator::Exit(exit) => exit_references[exit.0 as usize] += 1,
        }
    }
    for edge in &plan.edges {
        match &edge.condition {
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                status_references.insert(source.clone());
            }
            EdgeCondition::Always
            | EdgeCondition::BooleanResult(_, _)
            | EdgeCondition::VariantCase { .. } => {}
        }
    }
    for exit in &plan.exits {
        match &exit.continuation {
            ExitContinuation::Continue(edge) => edge_references[edge.0 as usize] += 1,
            ExitContinuation::ReturnFailure { source } => {
                status_references.insert(source.clone());
            }
            ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit => {}
        }
    }
    if edge_references.iter().any(|count| *count != 1) {
        return Err(replay_error(
            function,
            "each cleanup edge must be referenced by exactly one control-flow owner",
        ));
    }
    if exit_references.iter().any(|count| *count != 1) {
        return Err(replay_error(
            function,
            "each cleanup exit must be referenced by exactly one block",
        ));
    }
    let declared_statuses = plan
        .status_sources
        .iter()
        .map(|source| source.id.clone())
        .collect::<BTreeSet<_>>();
    if status_references != declared_statuses {
        return Err(replay_error(
            function,
            "declared and referenced cleanup status sources differ",
        ));
    }
    Ok(())
}

fn block_successors(function: &ResolvedFunction, block: BlockId) -> Vec<BlockId> {
    let plan = &function.cleanup_plan;
    let edges: Vec<EdgeId> = match &plan.blocks[block.0 as usize].terminator {
        CleanupTerminator::Goto(edge) => vec![*edge],
        CleanupTerminator::Branch(edges) => edges.clone(),
        CleanupTerminator::Exit(exit) => match &plan.exits[exit.0 as usize].continuation {
            ExitContinuation::Continue(edge) => vec![*edge],
            ExitContinuation::CommitResult { .. }
            | ExitContinuation::ReturnFailure { .. }
            | ExitContinuation::ReturnUnit => Vec::new(),
        },
    };
    edges
        .into_iter()
        .map(|edge| plan.edges[edge.0 as usize].to)
        .collect()
}

fn validate_edge_condition(
    function: &ResolvedFunction,
    condition: &EdgeCondition,
    expressions: &BTreeMap<ExpressionId, Option<CallFact>>,
    statuses: &BTreeSet<StatusSourceId>,
) -> Result<(), Diagnostic> {
    match condition {
        EdgeCondition::Always => Ok(()),
        EdgeCondition::BooleanResult(expression, _) => {
            require_expression(function, expressions, expression)
        }
        EdgeCondition::VariantCase { scrutinee, .. } => {
            require_expression(function, expressions, scrutinee)
        }
        EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
            require_status(function, statuses, source)
        }
    }
}

fn validate_branch_pair(
    function: &ResolvedFunction,
    left: &EdgeCondition,
    right: &EdgeCondition,
) -> Result<(), Diagnostic> {
    let valid = match (left, right) {
        (EdgeCondition::BooleanResult(a, av), EdgeCondition::BooleanResult(b, bv)) => {
            a == b && av != bv
        }
        (
            EdgeCondition::VariantCase {
                scrutinee: a_scrutinee,
                case: a_case,
                matches: a_matches,
            },
            EdgeCondition::VariantCase {
                scrutinee: b_scrutinee,
                case: b_case,
                matches: b_matches,
            },
        ) => a_scrutinee == b_scrutinee && a_case == b_case && a_matches != b_matches,
        (EdgeCondition::StatusZero(a), EdgeCondition::StatusNonzero(b))
        | (EdgeCondition::StatusNonzero(a), EdgeCondition::StatusZero(b)) => a == b,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(replay_error(
            function,
            "cleanup branch conditions are not complementary",
        ))
    }
}

fn validate_owned_edge(
    function: &ResolvedFunction,
    owner: BlockId,
    edge: EdgeId,
    referenced: &mut BTreeSet<EdgeId>,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let Some(item) = plan.edges.get(edge.0 as usize) else {
        return Err(replay_error(
            function,
            "cleanup terminator references unknown edge",
        ));
    };
    if item.id != edge || item.from != owner || !referenced.insert(edge) {
        return Err(replay_error(
            function,
            "cleanup edge has the wrong owner or is referenced repeatedly",
        ));
    }
    Ok(())
}

fn validate_place(
    function: &ResolvedFunction,
    place: &CleanupPlace,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<Vec<LivenessFlagId>, Diagnostic> {
    if !storage.contains(&place.storage) {
        return Err(replay_error(
            function,
            "cleanup place references unknown storage",
        ));
    }
    let under = leaves
        .iter()
        .filter_map(|(flag, leaf)| {
            (leaf.place.storage == place.storage
                && leaf.place.projections.starts_with(&place.projections))
            .then_some(*flag)
        })
        .collect::<Vec<_>>();
    if under.is_empty() {
        return Err(replay_error(
            function,
            "cleanup place has no droppable leaf",
        ));
    }
    Ok(under)
}

fn expression_facts(
    function: &ResolvedFunction,
) -> Result<BTreeMap<ExpressionId, Option<CallFact>>, Diagnostic> {
    let mut facts = BTreeMap::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        collect_expression_facts(function, expression, &mut facts)?;
    }
    Ok(facts)
}

fn collect_expression_facts(
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    facts: &mut BTreeMap<ExpressionId, Option<CallFact>>,
) -> Result<(), Diagnostic> {
    let fact = match &expression.kind {
        ResolvedExprKind::Call { callee, args } => Some(CallFact {
            callee: callee.clone(),
            arguments: args.iter().map(|argument| argument.id.clone()).collect(),
        }),
        _ => None,
    };
    if facts.insert(expression.id.clone(), fact).is_some() {
        return Err(replay_error(
            function,
            format!("HIR expression identity `{}` is repeated", expression.id),
        ));
    }
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_expression_facts(function, argument, facts)?;
            }
        }
        ResolvedExprKind::Unary { value, .. } | ResolvedExprKind::Project { base: value, .. } => {
            collect_expression_facts(function, value, facts)?;
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_expression_facts(function, left, facts)?;
            collect_expression_facts(function, right, facts)?;
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let crate::hir::ResolvedStatement::Let { value, .. } = statement;
                collect_expression_facts(function, value, facts)?;
            }
            collect_expression_facts(function, tail, facts)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expression_facts(function, condition, facts)?;
            collect_expression_facts(function, then_branch, facts)?;
            collect_expression_facts(function, else_branch, facts)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for field in fields {
                collect_expression_facts(function, &field.value, facts)?;
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_expression_facts(function, &field.value, facts)?;
            }
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            collect_expression_facts(function, scrutinee, facts)?;
            for arm in arms {
                collect_expression_facts(function, &arm.value, facts)?;
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_expression_facts(function, base, facts)?;
            for field in fields {
                collect_expression_facts(function, &field.value, facts)?;
            }
        }
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
    }
    Ok(())
}

fn require_expression(
    function: &ResolvedFunction,
    expressions: &BTreeMap<ExpressionId, Option<CallFact>>,
    expression: &ExpressionId,
) -> Result<(), Diagnostic> {
    if expressions.contains_key(expression) {
        Ok(())
    } else {
        Err(replay_error(
            function,
            format!("cleanup plan references unknown expression `{expression}`"),
        ))
    }
}

fn require_status(
    function: &ResolvedFunction,
    statuses: &BTreeSet<StatusSourceId>,
    source: &StatusSourceId,
) -> Result<(), Diagnostic> {
    if statuses.contains(source) {
        Ok(())
    } else {
        Err(replay_error(
            function,
            format!(
                "cleanup plan references unknown status source `{}`",
                source.expression
            ),
        ))
    }
}

fn block_exists(function: &ResolvedFunction, block: BlockId) -> bool {
    usize::try_from(block.0)
        .ok()
        .is_some_and(|index| index < function.cleanup_plan.blocks.len())
}

fn u32_index(function: &ResolvedFunction, index: usize, what: &str) -> Result<u32, Diagnostic> {
    u32::try_from(index).map_err(|_| replay_error(function, format!("too many {what}s")))
}

fn replay_error(function: &ResolvedFunction, message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-H006",
        format!(
            "cleanup plan for function `{}` failed independent replay: {}",
            function.id,
            message.into()
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::cleanup_plan::{CleanupBlock, CleanupEdge};
    use crate::{hir, parse};

    use super::*;

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

    fn program() -> ResolvedProgram {
        let parsed = parse(SOURCE, Path::new("cleanup-replay-paths.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
    }

    fn function(program: &ResolvedProgram, id: &str) -> ResolvedFunction {
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .cloned()
            .unwrap()
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
        let ResolvedExprKind::Match { scrutinee, arms } = &expression.kind else {
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
        let block =
            reordered
                .cleanup_plan
                .blocks
                .iter_mut()
                .find(|block| {
                    block
                    .transitions
                    .iter()
                    .filter(|transition| matches!(
                        transition,
                        CleanupTransition::Transfer { destination: target, .. }
                            if target.storage == destination && !target.projections.is_empty()
                    ))
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
            FieldLivenessShape::NoDrop | FieldLivenessShape::Record { .. } => {
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
            .contains("whole droppable nominal provisional result"));
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
    fn replay_budget_exhaustion_is_a_deterministic_diagnostic() {
        let program = program();
        let function = function(&program, "app.main");
        let mut budget = ReplayBudget { remaining: 1 };
        let diagnostic = budget.charge(&function, 2, "hostile test").unwrap_err();
        assert_eq!(diagnostic.code, "SPX-H006");
        assert!(diagnostic.message.contains("work budget exhausted"));
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
                | EdgeCondition::VariantCase { .. } => {}
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
            CleanupTransition::Initialize { at, .. } | CleanupTransition::Transfer { at, .. } => {
                *at = substitute
            }
            CleanupTransition::CallCommit { .. } | CleanupTransition::SelectFailure { .. } => {
                unreachable!()
            }
        }

        let diagnostic = validate_structure(&program, &function).unwrap_err();
        assert!(diagnostic
            .message
            .contains("decision or ownership-event sequence disagrees with typed HIR"));
    }
}
