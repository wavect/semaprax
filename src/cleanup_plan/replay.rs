//! Independent structural validation for attached cleanup plans.
//!
//! This module deliberately does not invoke the canonical builder.  It checks
//! that an attached plan is a closed, well-formed CFG whose identifiers,
//! places, status sources, guarded finalizers, and every current acyclic path
//! can be replayed safely.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[cfg(test)]
use std::cell::Cell;

use crate::ast::{BinaryOp, UnaryOp};
use crate::cleanup::{CleanupStorageOrigin, FieldLiveness, FieldLivenessShape, LivenessFlagId};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, DeclarationKind, ExpressionId, FunctionInstanceId, IdentityOrigin,
    OwnershipMode, PlaceProjection, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedMatchArm, ResolvedMatchPattern, ResolvedProgram, ResolvedStatement, ResolvedType,
    ResolvedTypeDeclarationKind,
};
use crate::prelude;

use super::{
    BlockId, CleanupPlace, CleanupRegionId, CleanupResultSource, CleanupTerminator,
    CleanupTransition, EdgeCondition, EdgeId, ExitContinuation, ExitTarget, StagedCopyResultSource,
    StatusCase, StatusLane, StatusProducer, StatusSource, StatusSourceId, StorageId,
    CLEANUP_PLAN_SCHEMA_V2, CLEANUP_PLAN_SCHEMA_V3,
};

const MAX_REPLAY_PATHS: usize = 65_536;
// Independent fail-closed work cap. Valid admitted shapes are preflighted
// before path materialization; depth alone is not a work bound because wide
// calls and blocks can emit several observations per node.
const MAX_REPLAY_WORK_UNITS: usize = 8_000_000;

struct ReplayBudget {
    remaining: usize,
    skeleton_remaining: usize,
}

impl ReplayBudget {
    fn new() -> Self {
        Self {
            remaining: MAX_REPLAY_WORK_UNITS,
            skeleton_remaining: 0,
        }
    }

    #[cfg(test)]
    fn with_skeleton_limit(limit: usize) -> Self {
        Self {
            remaining: MAX_REPLAY_WORK_UNITS,
            skeleton_remaining: limit,
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

    fn reserve_skeleton(
        &mut self,
        function: &ResolvedFunction,
        units: usize,
    ) -> Result<(), Diagnostic> {
        self.remaining = self.remaining.checked_sub(units).ok_or_else(|| {
            replay_error(
                function,
                "cleanup replay skeleton-work preflight exceeds the global budget",
            )
        })?;
        self.skeleton_remaining = units;
        Ok(())
    }

    fn charge_skeleton(
        &mut self,
        function: &ResolvedFunction,
        units: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.skeleton_remaining = self.skeleton_remaining.checked_sub(units).ok_or_else(|| {
            replay_error(
                function,
                format!("cleanup replay work budget exhausted during {phase}"),
            )
        })?;
        Ok(())
    }
}

#[cfg(test)]
thread_local! {
    static SKELETON_MATERIALIZATIONS: Cell<usize> = const { Cell::new(0) };
}

fn note_skeleton_materialization() {
    #[cfg(test)]
    SKELETON_MATERIALIZATIONS.with(|count| count.set(count.get().saturating_add(1)));
}

#[cfg(test)]
fn reset_skeleton_materializations() {
    SKELETON_MATERIALIZATIONS.with(|count| count.set(0));
}

#[cfg(test)]
fn skeleton_materializations() -> usize {
    SKELETON_MATERIALIZATIONS.with(Cell::get)
}

fn skeleton_clone<T: Clone>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    value: &T,
    phase: &str,
) -> Result<T, Diagnostic> {
    budget.charge_skeleton(function, 1, phase)?;
    note_skeleton_materialization();
    Ok(value.clone())
}

fn skeleton_push<T>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    target: &mut Vec<T>,
    value: T,
    phase: &str,
) -> Result<(), Diagnostic> {
    budget.charge_skeleton(function, 1, phase)?;
    note_skeleton_materialization();
    target.push(value);
    Ok(())
}

fn skeleton_queue_push<T>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    target: &mut VecDeque<T>,
    value: T,
    phase: &str,
) -> Result<(), Diagnostic> {
    budget.charge_skeleton(function, 1, phase)?;
    note_skeleton_materialization();
    target.push_back(value);
    Ok(())
}

#[derive(Clone)]
struct Leaf {
    place: CleanupPlace,
    lifecycle: DeclarationId,
}

#[derive(Clone)]
struct CallFact {
    callee: DeclarationId,
    instance: Option<FunctionInstanceId>,
    arguments: Vec<ExpressionId>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PathState {
    live_order: Vec<LivenessFlagId>,
    pending_failure: Option<StatusSourceId>,
    selected_failure: Option<StatusSourceId>,
    staged_copy_result: Option<StagedCopyResultSource>,
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
    StageCopyResult(StagedCopyResultSource),
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
    residual: bool,
}

type BooleanSkeletonSplit = (
    Vec<ExprSkeletonPath>,
    Vec<ExprSkeletonPath>,
    Vec<ExprSkeletonPath>,
);
type CallSkeletonState = (ExprSkeletonPath, Vec<(u32, CleanupPlace)>);

/// Validate the structure of the cleanup plan attached to `function` without
/// rebuilding it from HIR.
#[cfg(test)]
fn validate_structure(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget)?;
    validate_structure_with_budget(program, function, &mut budget)
}

pub(super) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(
        program,
        program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ),
        &mut budget,
    )?;
    for function in &program.functions {
        validate_structure_with_budget(program, function, &mut budget)?;
    }
    for instance in &program.function_instances {
        validate_structure_with_budget(program, &instance.function, &mut budget)?;
    }
    Ok(())
}

fn reserve_program_skeleton_work<'a>(
    program: &ResolvedProgram,
    functions: impl IntoIterator<Item = &'a ResolvedFunction>,
    budget: &mut ReplayBudget,
) -> Result<usize, Diagnostic> {
    let mut total = 0usize;
    let mut first = None;
    for function in functions {
        first.get_or_insert(function);
        let function_upper = skeleton_work_upper(program, function)?;
        total = total.checked_add(function_upper).ok_or_else(|| {
            replay_error(
                function,
                "cleanup replay program-wide skeleton-work preflight overflowed",
            )
        })?;
        if total > MAX_REPLAY_WORK_UNITS {
            return Err(replay_error(
                function,
                "cleanup replay program-wide skeleton-work preflight exceeds the global budget",
            ));
        }
    }
    if let Some(function) = first {
        budget.reserve_skeleton(function, total)?;
    }
    Ok(total)
}

fn validate_structure_with_budget(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let expected_schema = if function.requires.iter().any(expression_has_option_try)
        || function.ensures.iter().any(expression_has_option_try)
        || expression_has_option_try(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V3
    } else {
        CLEANUP_PLAN_SCHEMA_V2
    };
    if plan.schema != expected_schema {
        return Err(replay_error(
            function,
            format!(
                "uses cleanup-plan schema `{}` instead of HIR-derived `{expected_schema}`",
                plan.schema
            ),
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
    let cfg = branch_sensitive_cfg_bounds(function)?;
    if cfg.terminal_paths > MAX_REPLAY_PATHS {
        return Err(replay_error(
            function,
            "cleanup replay path bound exceeds the global path budget",
        ));
    }
    let semantic_paths = hir_terminal_path_bound(function)?;
    if semantic_paths > MAX_REPLAY_PATHS {
        return Err(replay_error(
            function,
            "cleanup replay semantic path bound exceeds the global path budget",
        ));
    }
    let expression_units = expression_facts(function)?.len();
    if cfg.work.saturating_add(expression_units) > MAX_REPLAY_WORK_UNITS {
        return Err(replay_error(
            function,
            "cleanup replay combined path/work bound exceeds the global budget",
        ));
    }
    Ok(())
}

struct CfgReplayBounds {
    terminal_paths: usize,
    work: usize,
}

/// Bound actual CFG traversal work without multiplying every structure item by
/// every terminal path. The cleanup graph is authenticated as acyclic later;
/// this independent saturating propagation is deliberately cycle-safe and
/// fails closed if a forged graph keeps increasing multiplicity.
fn branch_sensitive_cfg_bounds(function: &ResolvedFunction) -> Result<CfgReplayBounds, Diagnostic> {
    let plan = &function.cleanup_plan;
    let invalid = || {
        replay_error(
            function,
            "cleanup replay preflight references an unknown id",
        )
    };
    let entry = plan
        .blocks
        .get(plan.entry.0 as usize)
        .map(|_| plan.entry.0 as usize)
        .ok_or_else(invalid)?;
    let mut successors = vec![Vec::<usize>::new(); plan.blocks.len()];
    let mut terminal = vec![false; plan.blocks.len()];
    for (index, block) in plan.blocks.iter().enumerate() {
        let targets = match &block.terminator {
            CleanupTerminator::Goto(edge) => plan
                .edges
                .get(edge.0 as usize)
                .map(|edge| vec![edge.to.0 as usize])
                .ok_or_else(invalid)?,
            CleanupTerminator::Branch(edges) => edges
                .iter()
                .map(|edge| {
                    plan.edges
                        .get(edge.0 as usize)
                        .map(|edge| edge.to.0 as usize)
                        .ok_or_else(invalid)
                })
                .collect::<Result<Vec<_>, _>>()?,
            CleanupTerminator::Exit(exit) => {
                match &plan
                    .exits
                    .get(exit.0 as usize)
                    .ok_or_else(invalid)?
                    .continuation
                {
                    ExitContinuation::Continue(edge) => plan
                        .edges
                        .get(edge.0 as usize)
                        .map(|edge| vec![edge.to.0 as usize])
                        .ok_or_else(invalid)?,
                    ExitContinuation::CommitResult { .. }
                    | ExitContinuation::ReturnUnit
                    | ExitContinuation::ReturnFailure { .. } => {
                        terminal[index] = true;
                        Vec::new()
                    }
                }
            }
        };
        if targets.iter().any(|target| *target >= plan.blocks.len()) {
            return Err(invalid());
        }
        successors[index] = targets;
    }

    let mut reachable = vec![false; plan.blocks.len()];
    let mut discover = vec![entry];
    while let Some(index) = discover.pop() {
        if std::mem::replace(&mut reachable[index], true) {
            continue;
        }
        discover.extend(successors[index].iter().copied());
    }
    let mut indegree = vec![0_usize; plan.blocks.len()];
    for (index, targets) in successors.iter().enumerate() {
        if reachable[index] {
            for target in targets {
                indegree[*target] = indegree[*target].saturating_add(1);
            }
        }
    }
    let mut pending = VecDeque::from([entry]);
    let mut incoming = vec![0_usize; plan.blocks.len()];
    incoming[entry] = 1;
    let mut total = 0_usize;
    let mut terminal_paths = 0_usize;
    let mut visited = 0_usize;
    let ceiling = MAX_REPLAY_WORK_UNITS.saturating_add(1);

    while let Some(index) = pending.pop_front() {
        visited = visited.saturating_add(1);
        let paths = incoming[index];
        let block = &plan.blocks[index];
        let local = block.transitions.len().saturating_add(1);
        total = total
            .saturating_add(local.saturating_mul(paths))
            .min(ceiling);
        if terminal[index] {
            terminal_paths = terminal_paths.saturating_add(paths);
        }
        for successor in &successors[index] {
            incoming[*successor] = incoming[*successor].saturating_add(paths).min(ceiling);
            indegree[*successor] = indegree[*successor].saturating_sub(1);
            if indegree[*successor] == 0 {
                pending.push_back(*successor);
            }
        }
        if total >= ceiling {
            return Ok(CfgReplayBounds {
                terminal_paths: MAX_REPLAY_PATHS.saturating_add(1),
                work: ceiling,
            });
        }
    }
    if visited != reachable.iter().filter(|reachable| **reachable).count() {
        return Err(replay_error(function, "cleanup CFG contains a cycle"));
    }
    Ok(CfgReplayBounds {
        terminal_paths,
        work: total,
    })
}

#[derive(Clone, Copy, Default)]
struct HirPathCounts {
    normal: usize,
    failed: usize,
    residual: usize,
}

impl HirPathCounts {
    const ONE: Self = Self {
        normal: 1,
        failed: 0,
        residual: 0,
    };

    fn total(self) -> usize {
        self.normal
            .saturating_add(self.failed)
            .saturating_add(self.residual)
    }
}

fn sequence_path_counts(left: HirPathCounts, right: HirPathCounts) -> HirPathCounts {
    HirPathCounts {
        normal: left.normal.saturating_mul(right.normal),
        failed: left
            .failed
            .saturating_add(left.normal.saturating_mul(right.failed)),
        residual: left
            .residual
            .saturating_add(left.normal.saturating_mul(right.residual)),
    }
}

fn hir_terminal_path_bound(function: &ResolvedFunction) -> Result<usize, Diagnostic> {
    let mut paths = HirPathCounts::ONE;
    for contract in &function.requires {
        paths = sequence_path_counts(paths, expression_path_counts(function, contract)?);
        paths.failed = paths.failed.saturating_add(paths.normal);
    }
    paths = sequence_path_counts(paths, expression_path_counts(function, &function.body)?);
    // Residual paths terminate at the function boundary; successful body paths
    // continue through postconditions.
    for contract in &function.ensures {
        paths = sequence_path_counts(paths, expression_path_counts(function, contract)?);
        paths.failed = paths.failed.saturating_add(paths.normal);
    }
    Ok(paths.total())
}

fn skeleton_work_upper(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<usize, Diagnostic> {
    let semantic_paths = hir_terminal_path_bound(function)?.max(1);
    if semantic_paths > MAX_REPLAY_PATHS {
        return Ok(0);
    }
    let mut hir_upper = checked_skeleton_mul(function, 4, semantic_paths)?;
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        hir_upper = checked_skeleton_add(
            function,
            hir_upper,
            expression_skeleton_work_upper(program, function, expression)?,
        )?;
    }
    hir_upper = checked_skeleton_add(
        function,
        hir_upper,
        checked_skeleton_mul(
            function,
            function
                .requires
                .len()
                .checked_add(function.ensures.len())
                .and_then(|contracts| contracts.checked_mul(6))
                .ok_or_else(|| skeleton_preflight_overflow(function))?,
            semantic_paths,
        )?,
    )?;

    let cfg = match branch_sensitive_cfg_bounds(function) {
        Ok(cfg) if cfg.terminal_paths <= MAX_REPLAY_PATHS => cfg,
        Ok(_) | Err(_) => return Ok(0),
    };
    let mut max_unit_weight = 10usize;
    for block in &function.cleanup_plan.blocks {
        for transition in &block.transitions {
            let weight = match transition {
                CleanupTransition::Initialize { .. } => 4,
                CleanupTransition::Transfer { .. } => 5,
                CleanupTransition::CallCommit { arguments, .. } => arguments
                    .len()
                    .checked_mul(2)
                    .and_then(|arguments| arguments.checked_add(4))
                    .ok_or_else(|| skeleton_preflight_overflow(function))?,
                CleanupTransition::SelectFailure { .. } => 1,
                CleanupTransition::StageCopyResult { .. } => 3,
            };
            max_unit_weight = max_unit_weight.max(weight);
        }
    }
    let plan_expansion = checked_skeleton_mul(function, cfg.work.max(1), max_unit_weight)?;
    let plan_terminals = checked_skeleton_mul(function, cfg.terminal_paths.max(1), 4)?;
    let comparison = checked_skeleton_mul(function, semantic_paths.max(cfg.terminal_paths), 2)?;
    checked_skeleton_add(
        function,
        checked_skeleton_add(function, hir_upper, plan_expansion)?,
        checked_skeleton_add(function, plan_terminals, comparison)?,
    )
}

fn expression_skeleton_work_upper(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    root: &ResolvedExpr,
) -> Result<usize, Diagnostic> {
    let mut stack = [None; 515];
    stack[0] = Some((root, 0usize));
    let mut len = 1usize;
    let mut weight = 0usize;
    while len != 0 {
        let (expression, next) = stack[len - 1]
            .as_mut()
            .expect("skeleton census frame retained");
        if *next == 0 {
            let local = match &expression.kind {
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::Bool(_)
                | ResolvedExprKind::String(_) => 2,
                ResolvedExprKind::Place(place) => place.projections.len().saturating_mul(2) + 8,
                ResolvedExprKind::Unary { .. } => 8,
                ResolvedExprKind::Binary { .. } => 12,
                ResolvedExprKind::Call { args, .. } => args.len().saturating_mul(6) + 14,
                ResolvedExprKind::NativeRustImportCall(call) => {
                    call.args.len().saturating_mul(4) + 8
                }
                ResolvedExprKind::Block { statements, .. } => {
                    statements.len().saturating_mul(4) + 5
                }
                ResolvedExprKind::ConstructVariant { fields, .. }
                | ResolvedExprKind::ConstructRecord { fields, .. } => {
                    fields.len().saturating_mul(4) + 6
                }
                ResolvedExprKind::UpdateRecord { record, fields, .. } => checked_skeleton_add(
                    function,
                    fields
                        .len()
                        .checked_mul(5)
                        .and_then(|fields| fields.checked_add(8))
                        .ok_or_else(|| skeleton_preflight_overflow(function))?,
                    untouched_update_field_work_upper(
                        program, function, expression, record, fields,
                    )?,
                )?,
                ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => 10,
                ResolvedExprKind::Project { .. } => 6,
                ResolvedExprKind::If { .. } => 10,
                ResolvedExprKind::Match { arms, .. } => arms.len().saturating_mul(4) + 8,
            };
            let paths = expression_path_counts(function, expression)?.total().max(1);
            if paths > MAX_REPLAY_PATHS {
                return Ok(0);
            }
            weight = checked_skeleton_add(
                function,
                weight,
                checked_skeleton_mul(function, local, paths)?,
            )?;
        }
        if let Some(child) = replay_expression_child(expression, *next) {
            *next += 1;
            if len == stack.len() {
                return Err(replay_error(
                    function,
                    "typed-HIR skeleton-work census exceeds the admitted expression depth",
                ));
            }
            stack[len] = Some((child, 0));
            len += 1;
        } else {
            len -= 1;
            stack[len] = None;
        }
    }
    Ok(weight)
}

fn untouched_update_field_work_upper(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    record: &DeclarationId,
    replacements: &[crate::hir::ResolvedFieldInitializer],
) -> Result<usize, Diagnostic> {
    if expression.ownership != OwnershipMode::Own
        || !type_needs_drop(program, function, &expression.ty)?
    {
        return Ok(0);
    }
    let declarations = program.declarations.record_fields(record).ok_or_else(|| {
        replay_error(
            function,
            format!("record update has unknown record `{record}`"),
        )
    })?;
    let mut untouched_droppable = 0usize;
    for field in declarations {
        if replacements
            .iter()
            .any(|replacement| replacement.field == field.id)
            || !type_needs_drop(program, function, &field.ty)?
        {
            continue;
        }
        untouched_droppable = untouched_droppable
            .checked_add(1)
            .ok_or_else(|| skeleton_preflight_overflow(function))?;
    }
    let active_paths = expression_path_counts(function, expression)?.normal;
    checked_skeleton_mul(
        function,
        checked_skeleton_mul(function, untouched_droppable, active_paths)?,
        8,
    )
}

fn checked_skeleton_add(
    function: &ResolvedFunction,
    left: usize,
    right: usize,
) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| skeleton_preflight_overflow(function))
}

fn checked_skeleton_mul(
    function: &ResolvedFunction,
    left: usize,
    right: usize,
) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| skeleton_preflight_overflow(function))
}

fn skeleton_preflight_overflow(function: &ResolvedFunction) -> Diagnostic {
    replay_error(
        function,
        "cleanup replay program-wide skeleton-work preflight overflowed",
    )
}

fn expression_path_counts(
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<HirPathCounts, Diagnostic> {
    #[derive(Clone, Copy)]
    struct Frame<'a> {
        expression: &'a ResolvedExpr,
        next: usize,
        accumulator: HirPathCounts,
        first: HirPathCounts,
    }
    let mut stack = [None; 514];
    stack[0] = Some(Frame {
        expression,
        next: 0,
        accumulator: HirPathCounts::ONE,
        first: HirPathCounts::default(),
    });
    let mut len = 1usize;
    let mut result = HirPathCounts::ONE;
    while len != 0 {
        len -= 1;
        let mut frame = stack[len].take().expect("path-count frame retained");
        if frame.next != 0 {
            let child_index = frame.next - 1;
            match &frame.expression.kind {
                ResolvedExprKind::If { .. } => match child_index {
                    0 => frame.first = result,
                    1 => frame.accumulator = result,
                    2 => {
                        let condition = frame.first;
                        frame.accumulator = HirPathCounts {
                            normal: condition.normal.saturating_mul(
                                frame.accumulator.normal.saturating_add(result.normal),
                            ),
                            failed: condition.failed.saturating_add(
                                condition.normal.saturating_mul(
                                    frame.accumulator.failed.saturating_add(result.failed),
                                ),
                            ),
                            residual: condition.residual.saturating_add(
                                condition.normal.saturating_mul(
                                    frame.accumulator.residual.saturating_add(result.residual),
                                ),
                            ),
                        };
                    }
                    _ => unreachable!(),
                },
                ResolvedExprKind::Binary {
                    op: BinaryOp::And | BinaryOp::Or,
                    ..
                } => {
                    if child_index == 0 {
                        frame.first = result;
                    } else {
                        let left = frame.first;
                        frame.accumulator = HirPathCounts {
                            normal: left.normal.saturating_mul(result.normal.saturating_add(1)),
                            failed: left
                                .failed
                                .saturating_add(left.normal.saturating_mul(result.failed)),
                            residual: left
                                .residual
                                .saturating_add(left.normal.saturating_mul(result.residual)),
                        };
                    }
                }
                ResolvedExprKind::Match { .. } => {
                    if child_index == 0 {
                        frame.first = result;
                        frame.accumulator = HirPathCounts::default();
                    } else {
                        frame.accumulator.normal =
                            frame.accumulator.normal.saturating_add(result.normal);
                        frame.accumulator.failed =
                            frame.accumulator.failed.saturating_add(result.failed);
                        frame.accumulator.residual =
                            frame.accumulator.residual.saturating_add(result.residual);
                    }
                }
                _ => frame.accumulator = sequence_path_counts(frame.accumulator, result),
            }
        }
        if frame.accumulator.total() > MAX_REPLAY_PATHS || frame.first.total() > MAX_REPLAY_PATHS {
            return Ok(HirPathCounts {
                normal: MAX_REPLAY_PATHS + 1,
                failed: 0,
                residual: 0,
            });
        }
        if let Some(child) = replay_expression_child(frame.expression, frame.next) {
            if len + 2 > stack.len() {
                return Err(replay_error(
                    function,
                    "typed-HIR path census exceeds the admitted expression depth",
                ));
            }
            frame.next += 1;
            stack[len] = Some(frame);
            stack[len + 1] = Some(Frame {
                expression: child,
                next: 0,
                accumulator: HirPathCounts::ONE,
                first: HirPathCounts::default(),
            });
            len += 2;
            continue;
        }
        result = match &frame.expression.kind {
            ResolvedExprKind::Call { .. } => HirPathCounts {
                normal: frame.accumulator.normal,
                failed: frame
                    .accumulator
                    .failed
                    .saturating_add(frame.accumulator.normal),
                residual: frame.accumulator.residual,
            },
            ResolvedExprKind::Unary {
                op: UnaryOp::Neg, ..
            }
            | ResolvedExprKind::Binary {
                op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem,
                ..
            } => HirPathCounts {
                normal: frame.accumulator.normal,
                failed: frame
                    .accumulator
                    .failed
                    .saturating_add(frame.accumulator.normal),
                residual: frame.accumulator.residual,
            },
            ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => HirPathCounts {
                normal: frame.accumulator.normal,
                failed: frame.accumulator.failed,
                residual: frame
                    .accumulator
                    .residual
                    .saturating_add(frame.accumulator.normal),
            },
            ResolvedExprKind::If { .. }
            | ResolvedExprKind::Binary {
                op: BinaryOp::And | BinaryOp::Or,
                ..
            } => frame.accumulator,
            ResolvedExprKind::Match { .. } => HirPathCounts {
                normal: frame.first.normal.saturating_mul(frame.accumulator.normal),
                failed: frame
                    .first
                    .failed
                    .saturating_add(frame.first.normal.saturating_mul(frame.accumulator.failed)),
                residual: frame.first.residual.saturating_add(
                    frame
                        .first
                        .normal
                        .saturating_mul(frame.accumulator.residual),
                ),
            },
            _ => frame.accumulator,
        };
    }
    Ok(result)
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
    enum Frame<'a> {
        Expr(&'a ResolvedExpr, usize),
        CallArgument(&'a ResolvedExpr, usize),
    }
    let mut frames = Vec::with_capacity(1028);
    frames.push(Frame::Expr(expression, 0));
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Expr(expression, next) => {
                if let ResolvedExprKind::Call {
                    callee,
                    instance,
                    args,
                    ..
                } = &expression.kind
                {
                    let params =
                        resolved_call_params(program, function, callee, instance.as_ref())?;
                    if params.len() != args.len() {
                        return Err(replay_error(
                            function,
                            format!("cleanup call `{}` has inconsistent arity", expression.id),
                        ));
                    }
                    if let Some(argument) = args.get(next) {
                        if frames.len() + 3 > frames.capacity() {
                            return Err(replay_error(
                                function,
                                "supplemental-slot traversal exceeds the admitted depth",
                            ));
                        }
                        frames.push(Frame::Expr(expression, next + 1));
                        frames.push(Frame::CallArgument(expression, next));
                        frames.push(Frame::Expr(argument, 0));
                    }
                } else if let Some(child) = replay_expression_child(expression, next) {
                    if frames.len() + 2 > frames.capacity() {
                        return Err(replay_error(
                            function,
                            "supplemental-slot traversal exceeds the admitted depth",
                        ));
                    }
                    frames.push(Frame::Expr(expression, next + 1));
                    frames.push(Frame::Expr(child, 0));
                }
            }
            Frame::CallArgument(expression, index) => {
                let ResolvedExprKind::Call {
                    callee,
                    instance,
                    args,
                    ..
                } = &expression.kind
                else {
                    unreachable!("call-argument continuation retains a call");
                };
                let params = resolved_call_params(program, function, callee, instance.as_ref())?;
                let argument = &args[index];
                let parameter = &params[index];
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
        ResolvedTypeDeclarationKind::Record { fields }
        | ResolvedTypeDeclarationKind::Class { fields, .. } => {
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
    // Owned strings free their heap buffer inline in each backend; they never
    // join the resource-lifecycle cleanup plan.
    Ok(program
        .declarations
        .type_facts(ty)
        .map(|facts| facts.needs_drop)
        .ok_or_else(|| {
            replay_error(
                function,
                format!("type `{}` has no cleanup facts", ty.identity_key()),
            )
        })?
        && !matches!(ty, ResolvedType::String))
}

/// Resolve one call's parameters for replay: compiler-owned string operations
/// carry their reserved identity instead of an authored declaration and use
/// their synthetic parameters.
fn resolved_call_params(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    callee: &DeclarationId,
    instance: Option<&crate::hir::FunctionInstanceId>,
) -> Result<Vec<crate::hir::ResolvedParam>, Diagnostic> {
    if instance.is_none() {
        if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
            return Ok(crate::string_ops::resolved_params(op));
        }
    }
    let target = program
        .resolve_call_target(callee, instance)
        .ok_or_else(|| {
            replay_error(
                function,
                format!("cleanup call has unknown callee `{callee}`"),
            )
        })?;
    Ok(target.params.clone())
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
    let mut stack = [None; 514];
    stack[0] = Some((expression, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = stack[len].take().expect("status frame retained");
        if let Some(child) = replay_expression_child(expression, next) {
            if len + 2 > stack.len() {
                return Err(replay_error(
                    function,
                    "cleanup status expression depth exceeds 512",
                ));
            }
            stack[len] = Some((expression, next + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
            continue;
        }
        match &expression.kind {
            ResolvedExprKind::Call {
                callee, instance, ..
            } => {
                if instance.is_none() && crate::string_ops::by_id(callee.as_str()).is_some() {
                    // String operations project like ordinary propagated calls.
                } else if program
                    .resolve_call_target(callee, instance.as_ref())
                    .is_none()
                {
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
            ResolvedExprKind::Unary {
                op: UnaryOp::Neg, ..
            } => statuses.push(checked_status(
                expression,
                super::CheckedOperation::Neg,
                vec![StatusCase::NegationOverflow],
            )),
            ResolvedExprKind::Unary {
                op: UnaryOp::Not, ..
            } => {}
            ResolvedExprKind::Binary { op, .. } => {
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
            ResolvedExprKind::NativeRustImportCall(_)
            | ResolvedExprKind::Block { .. }
            | ResolvedExprKind::If { .. }
            | ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::Match { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_) => {}
        }
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
                    let params = resolved_call_params(
                        program,
                        function,
                        &fact.callee,
                        fact.instance.as_ref(),
                    )
                    .map_err(|_| {
                        replay_error(
                            function,
                            format!("call commit has unknown callee `{}`", fact.callee),
                        )
                    })?;
                    if params.len() != fact.arguments.len() {
                        return Err(replay_error(
                            function,
                            "call commit callee signature has inconsistent arity",
                        ));
                    }
                    let mut expected_parameters = Vec::new();
                    for (index, parameter) in params.iter().enumerate() {
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
                CleanupTransition::StageCopyResult { source } => {
                    validate_staged_target(function, source)?;
                    match source {
                        StagedCopyResultSource::Body { expression, .. } => {
                            if expression != &function.body.id {
                                return Err(replay_error(
                                    function,
                                    "body Copy-result stage names another expression",
                                ));
                            }
                        }
                        StagedCopyResultSource::TryResidual {
                            expression,
                            operand,
                            ..
                        }
                        | StagedCopyResultSource::TryOptionNone {
                            expression,
                            operand,
                            ..
                        } => {
                            require_expression(function, expressions, expression)?;
                            require_expression(function, expressions, operand)?;
                        }
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
    let mut expected = hir_skeleton_paths(program, function, budget)?;
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
    budget: &mut ReplayBudget,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    let mut work = SkeletonWork { function, budget };
    let mut paths = work.singleton_path(empty_expr_path(), "HIR root path")?;
    for contract in &function.requires {
        paths = sequence_expression(program, function, paths, contract, &mut work)?;
        paths = split_contract(paths, contract, &mut work)?;
    }
    paths = sequence_expression(program, function, paths, &function.body, &mut work)?;
    if paths.iter().any(|path| path.residual) {
        if !function.cleanup_plan.slots.is_empty() {
            return Err(replay_error(
                function,
                "postfix `?` cleanup skeleton contains resource slots",
            ));
        }
        for path in &mut paths {
            if path.failed {
                continue;
            }
            if !path.residual {
                let expression =
                    work.clone_owned(&function.body.id, "body result expression clone")?;
                let instance = work.clone_owned(&function.return_type, "body result type clone")?;
                work.push_observation(
                    path,
                    SkeletonObservation::StageCopyResult(StagedCopyResultSource::Body {
                        expression,
                        instance,
                    }),
                    "body result staging",
                )?;
            }
            path.residual = false;
        }
    }
    if type_needs_drop(program, function, &function.return_type)? {
        let body_id = work.clone_owned(&function.body.id, "owned result expression clone")?;
        paths = transfer_completed_paths(
            function,
            paths,
            body_id,
            CleanupPlace {
                storage: StorageId::ProvisionalResult,
                projections: Vec::new(),
            },
            "owned function result",
            &mut work,
        )?;
    }
    for contract in &function.ensures {
        paths = sequence_expression(program, function, paths, contract, &mut work)?;
        paths = split_contract(paths, contract, &mut work)?;
    }
    let mut completed = Vec::new();
    for path in paths {
        work.push_skeleton_path(
            &mut completed,
            SkeletonPath {
                observations: path.observations,
                terminal: if path.failed {
                    SkeletonTerminal::Failure
                } else {
                    SkeletonTerminal::Success
                },
            },
            "completed HIR skeleton path",
        )?;
    }
    Ok(completed)
}

struct SkeletonWork<'a, 'b> {
    function: &'a ResolvedFunction,
    budget: &'b mut ReplayBudget,
}

impl SkeletonWork<'_, '_> {
    fn charge(&mut self, units: usize, phase: &str) -> Result<(), Diagnostic> {
        self.budget.charge_skeleton(self.function, units, phase)
    }

    fn clone_owned<T: Clone>(&mut self, value: &T, phase: &str) -> Result<T, Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        Ok(value.clone())
    }

    fn push_expr_path(
        &mut self,
        paths: &mut Vec<ExprSkeletonPath>,
        path: ExprSkeletonPath,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        paths.push(path);
        Ok(())
    }

    fn push_skeleton_path(
        &mut self,
        paths: &mut Vec<SkeletonPath>,
        path: SkeletonPath,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        paths.push(path);
        Ok(())
    }

    fn singleton_path(
        &mut self,
        path: ExprSkeletonPath,
        phase: &str,
    ) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
        let mut paths = Vec::new();
        self.push_expr_path(&mut paths, path, phase)?;
        Ok(paths)
    }

    fn clone_expr_path(
        &mut self,
        path: &ExprSkeletonPath,
        phase: &str,
    ) -> Result<ExprSkeletonPath, Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        Ok(path.clone())
    }

    fn clone_observations(
        &mut self,
        observations: &[SkeletonObservation],
        phase: &str,
    ) -> Result<Vec<SkeletonObservation>, Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        Ok(observations.to_vec())
    }

    fn extend_observations(
        &mut self,
        target: &mut Vec<SkeletonObservation>,
        observations: &[SkeletonObservation],
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        target.extend_from_slice(observations);
        Ok(())
    }

    fn push_observation(
        &mut self,
        path: &mut ExprSkeletonPath,
        observation: SkeletonObservation,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        path.observations.push(observation);
        Ok(())
    }
}

fn empty_expr_path() -> ExprSkeletonPath {
    ExprSkeletonPath {
        observations: Vec::new(),
        owned_source: None,
        failed: false,
        residual: false,
    }
}

fn sequence_expression(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    prefixes: Vec<ExprSkeletonPath>,
    expression: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    if !has_active_paths(&prefixes) {
        return Ok(prefixes);
    }
    let suffixes = expression_skeleton(program, function, expression, work)?;
    sequence_skeleton_paths(prefixes, &suffixes, work)
}

fn sequence_skeleton_paths(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut combined = Vec::new();
    for prefix in prefixes {
        if prefix.failed || prefix.residual {
            work.push_expr_path(&mut combined, prefix, "short-circuited skeleton path")?;
            continue;
        }
        for suffix in suffixes {
            let mut observations =
                work.clone_observations(&prefix.observations, "skeleton prefix clone")?;
            work.extend_observations(
                &mut observations,
                &suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source = work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            work.push_expr_path(
                &mut combined,
                ExprSkeletonPath {
                    observations,
                    owned_source,
                    failed: suffix.failed,
                    residual: suffix.residual,
                },
                "sequenced skeleton path",
            )?;
        }
    }
    Ok(combined)
}

fn has_active_paths(paths: &[ExprSkeletonPath]) -> bool {
    paths.iter().any(|path| !path.failed && !path.residual)
}

fn expression_skeleton(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    enum Frame<'a> {
        Eval(&'a ResolvedExpr),
        Unary {
            expression: &'a ResolvedExpr,
            op: UnaryOp,
        },
        BinaryLeft {
            expression: &'a ResolvedExpr,
            op: BinaryOp,
            right: &'a ResolvedExpr,
        },
        BinaryRight {
            expression: &'a ResolvedExpr,
            op: BinaryOp,
            left_paths: Vec<ExprSkeletonPath>,
        },
        LazyRight {
            expression: &'a ResolvedExpr,
            op: BinaryOp,
            left: &'a ResolvedExpr,
            left_paths: Vec<ExprSkeletonPath>,
        },
        CallArgument {
            expression: &'a ResolvedExpr,
            params: Vec<crate::hir::ResolvedParam>,
            args: &'a [ResolvedExpr],
            index: usize,
            states: Vec<CallSkeletonState>,
        },
        NativeArgument {
            args: &'a [ResolvedExpr],
            index: usize,
            paths: Vec<ExprSkeletonPath>,
        },
        BlockValue {
            expression: &'a ResolvedExpr,
            statements: &'a [ResolvedStatement],
            tail: &'a ResolvedExpr,
            index: usize,
            paths: Vec<ExprSkeletonPath>,
        },
        BlockTail {
            expression: &'a ResolvedExpr,
            paths: Vec<ExprSkeletonPath>,
        },
        VariantField {
            fields: &'a [crate::hir::ResolvedFieldInitializer],
            index: usize,
            paths: Vec<ExprSkeletonPath>,
        },
        RecordField {
            expression: &'a ResolvedExpr,
            fields: &'a [crate::hir::ResolvedFieldInitializer],
            index: usize,
            paths: Vec<ExprSkeletonPath>,
        },
        UpdateBase {
            expression: &'a ResolvedExpr,
            base: &'a ResolvedExpr,
            record: &'a DeclarationId,
            fields: &'a [crate::hir::ResolvedFieldInitializer],
        },
        UpdateField {
            expression: &'a ResolvedExpr,
            base: &'a ResolvedExpr,
            record: &'a DeclarationId,
            fields: &'a [crate::hir::ResolvedFieldInitializer],
            index: usize,
            paths: Vec<ExprSkeletonPath>,
            replaced: BTreeSet<DeclarationId>,
            needs_cleanup: bool,
        },
        Try {
            expression: &'a ResolvedExpr,
        },
        TryOption {
            expression: &'a ResolvedExpr,
        },
        Project {
            expression: &'a ResolvedExpr,
            field: &'a DeclarationId,
        },
        IfCondition {
            expression: &'a ResolvedExpr,
            condition: &'a ResolvedExpr,
            then_branch: &'a ResolvedExpr,
            else_branch: &'a ResolvedExpr,
        },
        IfThen {
            expression: &'a ResolvedExpr,
            else_branch: &'a ResolvedExpr,
            true_prefixes: Vec<ExprSkeletonPath>,
            false_prefixes: Vec<ExprSkeletonPath>,
            results: Vec<ExprSkeletonPath>,
        },
        IfElse {
            expression: &'a ResolvedExpr,
            false_prefixes: Vec<ExprSkeletonPath>,
            results: Vec<ExprSkeletonPath>,
        },
        MatchScrutinee {
            expression: &'a ResolvedExpr,
            scrutinee: &'a ResolvedExpr,
            arms: &'a [ResolvedMatchArm],
        },
        MatchArm {
            expression: &'a ResolvedExpr,
            scrutinee: &'a ResolvedExpr,
            arms: &'a [ResolvedMatchArm],
            index: usize,
            remaining: Vec<ExprSkeletonPath>,
            results: Vec<ExprSkeletonPath>,
            is_record: bool,
        },
    }

    macro_rules! push_frame {
        ($frames:expr, $frame:expr) => {{
            if $frames.len() == $frames.capacity() {
                return Err(replay_error(
                    function,
                    "typed-HIR skeleton traversal exceeds the admitted depth",
                ));
            }
            work.charge(1, "typed-HIR skeleton continuation push")?;
            note_skeleton_materialization();
            $frames.push($frame);
        }};
    }

    // The semantic depth ceiling excludes the function-body block.  The
    // continuation machine also holds the currently evaluated child beside
    // that block and the 512 authored expression ancestors.
    let mut frames = Vec::with_capacity(515);
    push_frame!(frames, Frame::Eval(expression));
    let mut produced = None;
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Eval(expression) => {
                debug_assert!(produced.is_none());
                match &expression.kind {
                    ResolvedExprKind::Int(_)
                    | ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::Bool(_)
                    | ResolvedExprKind::String(_) => {
                        produced =
                            Some(work.singleton_path(empty_expr_path(), "literal skeleton path")?);
                    }
                    ResolvedExprKind::Place(place) => {
                        let owned_source = if expression.ownership == OwnershipMode::Own
                            && type_needs_drop(program, function, &expression.ty)?
                        {
                            Some(cleanup_place_from_hir(function, place, work)?)
                        } else {
                            None
                        };
                        produced = Some(work.singleton_path(
                            ExprSkeletonPath {
                                observations: Vec::new(),
                                owned_source,
                                failed: false,
                                residual: false,
                            },
                            "place skeleton path",
                        )?);
                    }
                    ResolvedExprKind::Unary { op, value } => {
                        push_frame!(
                            frames,
                            Frame::Unary {
                                expression,
                                op: *op
                            }
                        );
                        push_frame!(frames, Frame::Eval(value));
                    }
                    ResolvedExprKind::Try { operand, .. } => {
                        push_frame!(frames, Frame::Try { expression });
                        push_frame!(frames, Frame::Eval(operand));
                    }
                    ResolvedExprKind::TryOption { operand, .. } => {
                        push_frame!(frames, Frame::TryOption { expression });
                        push_frame!(frames, Frame::Eval(operand));
                    }
                    ResolvedExprKind::Project { base, field } => {
                        push_frame!(frames, Frame::Project { expression, field });
                        push_frame!(frames, Frame::Eval(base));
                    }
                    ResolvedExprKind::Binary { op, left, right } => {
                        push_frame!(
                            frames,
                            Frame::BinaryLeft {
                                expression,
                                op: *op,
                                right,
                            }
                        );
                        push_frame!(frames, Frame::Eval(left));
                    }
                    ResolvedExprKind::Call {
                        callee,
                        instance,
                        args,
                        ..
                    } => {
                        let intrinsic = if instance.is_none() {
                            crate::string_ops::by_id(callee.as_str())
                        } else {
                            None
                        };
                        let params = if let Some(op) = intrinsic {
                            crate::string_ops::resolved_params(op)
                        } else {
                            let target = program
                                .resolve_call_target(callee, instance.as_ref())
                                .ok_or_else(|| {
                                    replay_error(
                                        function,
                                        format!("unknown skeleton callee `{callee}`"),
                                    )
                                })?;
                            target.params.clone()
                        };
                        work.charge(1, "call skeleton root state")?;
                        let states = vec![(empty_expr_path(), Vec::new())];
                        if let Some(argument) = args.first() {
                            push_frame!(
                                frames,
                                Frame::CallArgument {
                                    expression,
                                    params,
                                    args,
                                    index: 0,
                                    states,
                                }
                            );
                            push_frame!(frames, Frame::Eval(argument));
                        } else {
                            produced = Some(finish_call_states(
                                program, function, expression, states, work,
                            )?);
                        }
                    }
                    ResolvedExprKind::NativeRustImportCall(call) => {
                        let paths =
                            work.singleton_path(empty_expr_path(), "native-call root path")?;
                        if let Some(argument) = call.args.first() {
                            push_frame!(
                                frames,
                                Frame::NativeArgument {
                                    args: &call.args,
                                    index: 0,
                                    paths,
                                }
                            );
                            push_frame!(frames, Frame::Eval(argument));
                        } else {
                            produced = Some(paths);
                        }
                    }
                    ResolvedExprKind::Block { statements, tail } => {
                        let paths = work.singleton_path(empty_expr_path(), "block root path")?;
                        if let Some(first_statement) = statements.first() {
                            push_frame!(
                                frames,
                                Frame::BlockValue {
                                    expression,
                                    statements,
                                    tail,
                                    index: 0,
                                    paths,
                                }
                            );
                            push_frame!(frames, Frame::Eval(first_statement.value()));
                        } else {
                            push_frame!(frames, Frame::BlockTail { expression, paths });
                            push_frame!(frames, Frame::Eval(tail));
                        }
                    }
                    ResolvedExprKind::ConstructVariant { fields, .. } => {
                        let paths = work
                            .singleton_path(empty_expr_path(), "variant-construction root path")?;
                        if let Some(field) = fields.first() {
                            push_frame!(
                                frames,
                                Frame::VariantField {
                                    fields,
                                    index: 0,
                                    paths,
                                }
                            );
                            push_frame!(frames, Frame::Eval(&field.value));
                        } else {
                            produced = Some(paths);
                        }
                    }
                    ResolvedExprKind::ConstructRecord { fields, .. } => {
                        let paths = work
                            .singleton_path(empty_expr_path(), "record-construction root path")?;
                        if let Some(field) = fields.first() {
                            push_frame!(
                                frames,
                                Frame::RecordField {
                                    expression,
                                    fields,
                                    index: 0,
                                    paths,
                                }
                            );
                            push_frame!(frames, Frame::Eval(&field.value));
                        } else {
                            produced = Some(finish_record_paths(expression, paths, work)?);
                        }
                    }
                    ResolvedExprKind::UpdateRecord {
                        base,
                        record,
                        fields,
                    } => {
                        push_frame!(
                            frames,
                            Frame::UpdateBase {
                                expression,
                                base,
                                record,
                                fields,
                            }
                        );
                        push_frame!(frames, Frame::Eval(base));
                    }
                    ResolvedExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        push_frame!(
                            frames,
                            Frame::IfCondition {
                                expression,
                                condition,
                                then_branch,
                                else_branch,
                            }
                        );
                        push_frame!(frames, Frame::Eval(condition));
                    }
                    ResolvedExprKind::Match { scrutinee, arms } => {
                        push_frame!(
                            frames,
                            Frame::MatchScrutinee {
                                expression,
                                scrutinee,
                                arms,
                            }
                        );
                        push_frame!(frames, Frame::Eval(scrutinee));
                    }
                }
            }
            Frame::Unary { expression, op } => {
                let paths = produced.take().expect("unary operand path retained");
                produced = Some(if op == UnaryOp::Neg {
                    let expression_id =
                        work.clone_owned(&expression.id, "unary status expression clone")?;
                    split_status_paths(
                        paths,
                        StatusSourceId {
                            expression: expression_id,
                            lane: StatusLane::OperationFailure,
                        },
                        work,
                    )?
                } else {
                    paths
                });
            }
            Frame::BinaryLeft {
                expression,
                op,
                right,
            } => {
                let left_paths = produced.take().expect("binary left path retained");
                if !has_active_paths(&left_paths) {
                    produced = Some(left_paths);
                } else if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    push_frame!(
                        frames,
                        Frame::LazyRight {
                            expression,
                            op,
                            left: match &expression.kind {
                                ResolvedExprKind::Binary { left, .. } => left,
                                _ => unreachable!(),
                            },
                            left_paths,
                        }
                    );
                    push_frame!(frames, Frame::Eval(right));
                } else {
                    push_frame!(
                        frames,
                        Frame::BinaryRight {
                            expression,
                            op,
                            left_paths,
                        }
                    );
                    push_frame!(frames, Frame::Eval(right));
                }
            }
            Frame::BinaryRight {
                expression,
                op,
                left_paths,
            } => {
                let right_paths = produced.take().expect("binary right path retained");
                let paths = sequence_skeleton_paths(left_paths, &right_paths, work)?;
                produced = Some(
                    if matches!(
                        op,
                        BinaryOp::Add
                            | BinaryOp::Sub
                            | BinaryOp::Mul
                            | BinaryOp::Div
                            | BinaryOp::Rem
                    ) {
                        let expression_id =
                            work.clone_owned(&expression.id, "binary status expression clone")?;
                        split_status_paths(
                            paths,
                            StatusSourceId {
                                expression: expression_id,
                                lane: StatusLane::OperationFailure,
                            },
                            work,
                        )?
                    } else {
                        paths
                    },
                );
            }
            Frame::LazyRight {
                expression,
                op,
                left,
                left_paths,
            } => {
                let right_paths = produced.take().expect("lazy right path retained");
                produced = Some(finish_lazy_paths(
                    function,
                    expression,
                    op,
                    left,
                    left_paths,
                    &right_paths,
                    work,
                )?);
            }
            Frame::NativeArgument { args, index, paths } => {
                let suffixes = produced.take().expect("native argument path retained");
                let paths = sequence_skeleton_paths(paths, &suffixes, work)?;
                let next = index + 1;
                if has_active_paths(&paths) && next < args.len() {
                    push_frame!(
                        frames,
                        Frame::NativeArgument {
                            args,
                            index: next,
                            paths,
                        }
                    );
                    push_frame!(frames, Frame::Eval(&args[next]));
                } else {
                    produced = Some(paths);
                }
            }
            Frame::CallArgument {
                expression,
                params,
                args,
                index,
                states,
            } => {
                let suffixes = produced.take().expect("call argument path retained");
                let states = sequence_call_argument(
                    program, function, expression, &params, args, index, states, &suffixes, work,
                )?;
                let next = index + 1;
                if call_states_have_active(&states) && next < args.len() {
                    push_frame!(
                        frames,
                        Frame::CallArgument {
                            expression,
                            params,
                            args,
                            index: next,
                            states,
                        }
                    );
                    push_frame!(frames, Frame::Eval(&args[next]));
                } else {
                    produced = Some(finish_call_states(
                        program, function, expression, states, work,
                    )?);
                }
            }
            Frame::BlockValue {
                expression,
                statements,
                tail,
                index,
                paths,
            } => {
                let suffixes = produced.take().expect("binding path retained");
                let mut paths = sequence_skeleton_paths(paths, &suffixes, work)?;
                match &statements[index] {
                    ResolvedStatement::Unsafe { .. } => {
                        // Unsafe boundaries bind and own nothing here: their
                        // ordinary block body was evaluated as this
                        // statement's value expression.
                        let next = index + 1;
                        if has_active_paths(&paths) && next < statements.len() {
                            push_frame!(
                                frames,
                                Frame::BlockValue {
                                    expression,
                                    statements,
                                    tail,
                                    index: next,
                                    paths,
                                }
                            );
                            push_frame!(frames, Frame::Eval(statements[next].value()));
                        } else if has_active_paths(&paths) {
                            push_frame!(frames, Frame::BlockTail { expression, paths });
                            push_frame!(frames, Frame::Eval(tail));
                        } else {
                            produced = Some(paths);
                        }
                    }
                    statement => {
                        let (binding, value) = (statement.binding(), statement.value());
                        if binding.ownership == OwnershipMode::Own
                            && type_needs_drop(program, function, &binding.ty)?
                        {
                            let value_id =
                                work.clone_owned(&value.id, "binding value expression clone")?;
                            let binding_id =
                                work.clone_owned(&binding.id, "binding storage clone")?;
                            paths = transfer_completed_paths(
                                function,
                                paths,
                                value_id,
                                CleanupPlace {
                                    storage: StorageId::Value(binding_id),
                                    projections: Vec::new(),
                                },
                                "owned binding",
                                work,
                            )?;
                        }
                        let next = index + 1;
                        if has_active_paths(&paths) && next < statements.len() {
                            push_frame!(
                                frames,
                                Frame::BlockValue {
                                    expression,
                                    statements,
                                    tail,
                                    index: next,
                                    paths,
                                }
                            );
                            push_frame!(frames, Frame::Eval(statements[next].value()));
                        } else if has_active_paths(&paths) {
                            push_frame!(frames, Frame::BlockTail { expression, paths });
                            push_frame!(frames, Frame::Eval(tail));
                        } else {
                            produced = Some(paths);
                        }
                    }
                }
            }
            Frame::BlockTail { expression, paths } => {
                let suffixes = produced.take().expect("block tail path retained");
                let mut paths = sequence_skeleton_paths(paths, &suffixes, work)?;
                if expression.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &expression.ty)?
                {
                    let expression_id =
                        work.clone_owned(&expression.id, "block result expression clone")?;
                    paths = transfer_completed_paths(
                        function,
                        paths,
                        expression_id,
                        temporary_place(expression, work)?,
                        "owned block result",
                        work,
                    )?;
                }
                produced = Some(paths);
            }
            Frame::VariantField {
                fields,
                index,
                paths,
            } => {
                let suffixes = produced.take().expect("variant field path retained");
                let paths = sequence_skeleton_paths(paths, &suffixes, work)?;
                let field = &fields[index];
                if has_active_paths(&paths)
                    && field.value.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &field.value.ty)?
                {
                    return Err(replay_error(
                        function,
                        "droppable variant payload reached the copy-only cleanup skeleton",
                    ));
                }
                let next = index + 1;
                if has_active_paths(&paths) && next < fields.len() {
                    push_frame!(
                        frames,
                        Frame::VariantField {
                            fields,
                            index: next,
                            paths,
                        }
                    );
                    push_frame!(frames, Frame::Eval(&fields[next].value));
                } else {
                    produced = Some(paths);
                }
            }
            Frame::RecordField {
                expression,
                fields,
                index,
                paths,
            } => {
                let suffixes = produced.take().expect("record field path retained");
                let mut paths = sequence_skeleton_paths(paths, &suffixes, work)?;
                let field = &fields[index];
                if field.value.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &field.value.ty)?
                {
                    let mut destination = temporary_place(expression, work)?;
                    let field_id =
                        work.clone_owned(&field.field, "record field projection clone")?;
                    work.charge(1, "record field projection push")?;
                    note_skeleton_materialization();
                    destination.projections.push(field_id);
                    let value_id = work.clone_owned(&field.value.id, "record field value clone")?;
                    paths = transfer_completed_paths(
                        function,
                        paths,
                        value_id,
                        destination,
                        "owned record field",
                        work,
                    )?;
                }
                let next = index + 1;
                if has_active_paths(&paths) && next < fields.len() {
                    push_frame!(
                        frames,
                        Frame::RecordField {
                            expression,
                            fields,
                            index: next,
                            paths,
                        }
                    );
                    push_frame!(frames, Frame::Eval(&fields[next].value));
                } else {
                    produced = Some(finish_record_paths(expression, paths, work)?);
                }
            }
            Frame::UpdateBase {
                expression,
                base,
                record,
                fields,
            } => {
                let mut paths = produced.take().expect("update base path retained");
                let needs_cleanup = expression.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &expression.ty)?;
                if needs_cleanup {
                    let staged_base = temporary_place(base, work)?;
                    for path in &mut paths {
                        if path.failed || path.residual {
                            continue;
                        }
                        let source = path.owned_source.take().ok_or_else(|| {
                            replay_error(
                                function,
                                "owned record update base has no HIR cleanup source",
                            )
                        })?;
                        if source != staged_base {
                            let at = work.clone_owned(&base.id, "update base expression clone")?;
                            let staged =
                                work.clone_owned(&staged_base, "update base destination clone")?;
                            work.push_observation(
                                path,
                                SkeletonObservation::Transfer {
                                    at,
                                    source,
                                    destination: staged,
                                },
                                "update base transfer",
                            )?;
                        }
                        path.owned_source =
                            Some(work.clone_owned(&staged_base, "update base source clone")?);
                    }
                }
                if has_active_paths(&paths) && !fields.is_empty() {
                    push_frame!(
                        frames,
                        Frame::UpdateField {
                            expression,
                            base,
                            record,
                            fields,
                            index: 0,
                            paths,
                            replaced: BTreeSet::new(),
                            needs_cleanup,
                        }
                    );
                    push_frame!(frames, Frame::Eval(&fields[0].value));
                } else {
                    produced = Some(finish_update_paths(
                        program,
                        function,
                        expression,
                        base,
                        record,
                        paths,
                        &BTreeSet::new(),
                        needs_cleanup,
                        work,
                    )?);
                }
            }
            Frame::UpdateField {
                expression,
                base,
                record,
                fields,
                index,
                paths,
                mut replaced,
                needs_cleanup,
            } => {
                let field = &fields[index];
                let field_id = work.clone_owned(&field.field, "updated-field set clone")?;
                work.charge(1, "updated-field set insertion")?;
                note_skeleton_materialization();
                if !replaced.insert(field_id) {
                    return Err(replay_error(
                        function,
                        format!("record update repeats field `{}`", field.field),
                    ));
                }
                let suffixes = produced.take().expect("update field path retained");
                let mut paths = sequence_skeleton_paths(paths, &suffixes, work)?;
                if needs_cleanup
                    && field.value.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &field.value.ty)?
                {
                    let mut destination = temporary_place(expression, work)?;
                    let field_id =
                        work.clone_owned(&field.field, "update field projection clone")?;
                    work.charge(1, "update field projection push")?;
                    note_skeleton_materialization();
                    destination.projections.push(field_id);
                    let value_id = work.clone_owned(&field.value.id, "update field value clone")?;
                    paths = transfer_completed_paths(
                        function,
                        paths,
                        value_id,
                        destination,
                        "owned record replacement",
                        work,
                    )?;
                }
                let next = index + 1;
                if has_active_paths(&paths) && next < fields.len() {
                    push_frame!(
                        frames,
                        Frame::UpdateField {
                            expression,
                            base,
                            record,
                            fields,
                            index: next,
                            paths,
                            replaced,
                            needs_cleanup,
                        }
                    );
                    push_frame!(frames, Frame::Eval(&fields[next].value));
                } else {
                    produced = Some(finish_update_paths(
                        program,
                        function,
                        expression,
                        base,
                        record,
                        paths,
                        &replaced,
                        needs_cleanup,
                        work,
                    )?);
                }
            }
            Frame::Try { expression } => {
                let operand_paths = produced.take().expect("try operand path retained");
                produced = Some(finish_try_paths(
                    program,
                    function,
                    expression,
                    operand_paths,
                    false,
                    work,
                )?);
            }
            Frame::TryOption { expression } => {
                let operand_paths = produced.take().expect("Option try operand path retained");
                produced = Some(finish_try_paths(
                    program,
                    function,
                    expression,
                    operand_paths,
                    true,
                    work,
                )?);
            }
            Frame::Project { expression, field } => {
                let mut paths = produced.take().expect("projection base path retained");
                if expression.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &expression.ty)?
                {
                    for path in &mut paths {
                        if path.failed || path.residual {
                            continue;
                        }
                        let mut source = path.owned_source.take().ok_or_else(|| {
                            replay_error(function, "owned projection has no HIR cleanup source")
                        })?;
                        let field = work.clone_owned(field, "projection field clone")?;
                        work.charge(1, "projection field push")?;
                        note_skeleton_materialization();
                        source.projections.push(field);
                        let destination = temporary_place(expression, work)?;
                        let at = work.clone_owned(&expression.id, "projection expression clone")?;
                        let transferred_destination = work
                            .clone_owned(&destination, "projection transfer destination clone")?;
                        work.push_observation(
                            path,
                            SkeletonObservation::Transfer {
                                at,
                                source,
                                destination: transferred_destination,
                            },
                            "owned projection transfer",
                        )?;
                        path.owned_source = Some(destination);
                    }
                }
                produced = Some(paths);
            }
            Frame::IfCondition {
                expression,
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_paths = produced.take().expect("if condition path retained");
                let (results, true_prefixes, false_prefixes) =
                    split_boolean_prefixes(condition_paths, &condition.id, work)?;
                if true_prefixes.is_empty() && false_prefixes.is_empty() {
                    produced = Some(results);
                } else if true_prefixes.is_empty() {
                    push_frame!(
                        frames,
                        Frame::IfElse {
                            expression,
                            false_prefixes,
                            results,
                        }
                    );
                    push_frame!(frames, Frame::Eval(else_branch));
                } else {
                    push_frame!(
                        frames,
                        Frame::IfThen {
                            expression,
                            else_branch,
                            true_prefixes,
                            false_prefixes,
                            results,
                        }
                    );
                    push_frame!(frames, Frame::Eval(then_branch));
                }
            }
            Frame::IfThen {
                expression,
                else_branch,
                true_prefixes,
                false_prefixes,
                mut results,
            } => {
                let then_paths = produced.take().expect("if then path retained");
                let mut selected = sequence_skeleton_paths(true_prefixes, &then_paths, work)?;
                selected =
                    finish_conditional_result(program, function, expression, selected, work)?;
                append_expr_paths(&mut results, selected, work, "if then result")?;
                if false_prefixes.is_empty() {
                    produced = Some(results);
                } else {
                    push_frame!(
                        frames,
                        Frame::IfElse {
                            expression,
                            false_prefixes,
                            results,
                        }
                    );
                    push_frame!(frames, Frame::Eval(else_branch));
                }
            }
            Frame::IfElse {
                expression,
                false_prefixes,
                mut results,
            } => {
                let else_paths = produced.take().expect("if else path retained");
                let mut selected = sequence_skeleton_paths(false_prefixes, &else_paths, work)?;
                selected =
                    finish_conditional_result(program, function, expression, selected, work)?;
                append_expr_paths(&mut results, selected, work, "if else result")?;
                produced = Some(results);
            }
            Frame::MatchScrutinee {
                expression,
                scrutinee,
                arms,
            } => {
                let scrutinee_paths = produced.take().expect("match scrutinee path retained");
                if !has_active_paths(&scrutinee_paths) {
                    produced = Some(scrutinee_paths);
                    continue;
                }
                let is_record =
                    validate_match_skeleton_shape(program, function, expression, scrutinee, arms)?;
                push_frame!(
                    frames,
                    Frame::MatchArm {
                        expression,
                        scrutinee,
                        arms,
                        index: 0,
                        remaining: scrutinee_paths,
                        results: Vec::new(),
                        is_record,
                    }
                );
                push_frame!(frames, Frame::Eval(&arms[0].value));
            }
            Frame::MatchArm {
                expression,
                scrutinee,
                arms,
                index,
                remaining,
                mut results,
                is_record,
            } => {
                let arm_paths = produced.take().expect("match arm path retained");
                let next_remaining = finish_match_arm(
                    program,
                    function,
                    expression,
                    scrutinee,
                    arms,
                    index,
                    remaining,
                    &arm_paths,
                    &mut results,
                    is_record,
                    work,
                )?;
                let next = index + 1;
                if !next_remaining.is_empty() && next < arms.len() {
                    push_frame!(
                        frames,
                        Frame::MatchArm {
                            expression,
                            scrutinee,
                            arms,
                            index: next,
                            remaining: next_remaining,
                            results,
                            is_record,
                        }
                    );
                    push_frame!(frames, Frame::Eval(&arms[next].value));
                } else {
                    append_expr_paths(&mut results, next_remaining, work, "match remaining path")?;
                    produced = Some(results);
                }
            }
        }
    }
    produced.ok_or_else(|| replay_error(function, "typed-HIR skeleton produced no root value"))
}

#[allow(clippy::too_many_arguments)]
fn authenticated_try_stage_source(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    operand: &ResolvedExpr,
    result: &DeclarationId,
    ok_case: &DeclarationId,
    ok_field: &DeclarationId,
    err_case: &DeclarationId,
    err_field: &DeclarationId,
    residual_type: &ResolvedType,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<StagedCopyResultSource, Diagnostic> {
    if result.as_str() != prelude::RESULT_ID
        || ok_case.as_str() != prelude::RESULT_OK_ID
        || ok_field.as_str() != prelude::RESULT_OK_VALUE_ID
        || err_case.as_str() != prelude::RESULT_ERR_ID
        || err_field.as_str() != prelude::RESULT_ERR_ERROR_ID
    {
        return Err(replay_error(
            function,
            "postfix `?` does not authenticate the ordinary Result prelude",
        ));
    }
    for id in [result, ok_case, ok_field, err_case, err_field] {
        let declaration = program.declarations.declaration(id).ok_or_else(|| {
            replay_error(function, format!("postfix `?` references unknown `{id}`"))
        })?;
        if declaration.identity_origin != IdentityOrigin::CompilerOwned {
            return Err(replay_error(
                function,
                format!("postfix `?` reference `{id}` is not compiler-owned"),
            ));
        }
    }
    let source_arguments = replay_result_arguments(function, &operand.ty, result)?;
    let target_arguments = replay_result_arguments(function, residual_type, result)?;
    if source_arguments.len() != 2
        || target_arguments.len() != 2
        || source_arguments
            .iter()
            .chain(target_arguments.iter())
            .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
        || expression.ty != source_arguments[0]
        || source_arguments[1] != target_arguments[1]
        || residual_type != &function.return_type
    {
        return Err(replay_error(
            function,
            "postfix `?` source, value, residual, or function type is inconsistent",
        ));
    }
    for ty in [&operand.ty, residual_type] {
        let facts = program.declarations.type_facts(ty).ok_or_else(|| {
            replay_error(function, "postfix `?` Result instance has no type facts")
        })?;
        if !facts.copy || !facts.sized || facts.contains_resource || facts.needs_drop {
            return Err(replay_error(
                function,
                "postfix `?` is outside the Copy Result cleanup slice",
            ));
        }
    }
    Ok(StagedCopyResultSource::TryResidual {
        expression: work.clone_owned(&expression.id, "try source expression clone")?,
        operand: work.clone_owned(&operand.id, "try source operand clone")?,
        source_instance: work.clone_owned(&operand.ty, "try source instance clone")?,
        target_instance: work.clone_owned(residual_type, "try target instance clone")?,
        result: work.clone_owned(result, "try Result identity clone")?,
        ok_case: work.clone_owned(ok_case, "try Ok identity clone")?,
        ok_field: work.clone_owned(ok_field, "try Ok field clone")?,
        err_case: work.clone_owned(err_case, "try Err identity clone")?,
        err_field: work.clone_owned(err_field, "try Err field clone")?,
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticated_try_option_stage_source(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    operand: &ResolvedExpr,
    option: &DeclarationId,
    some_case: &DeclarationId,
    some_field: &DeclarationId,
    none_case: &DeclarationId,
    residual_type: &ResolvedType,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<StagedCopyResultSource, Diagnostic> {
    if option.as_str() != prelude::OPTION_ID
        || some_case.as_str() != prelude::OPTION_SOME_ID
        || some_field.as_str() != prelude::OPTION_SOME_VALUE_ID
        || none_case.as_str() != prelude::OPTION_NONE_ID
    {
        return Err(replay_error(
            function,
            "Option postfix `?` does not authenticate the ordinary Option prelude",
        ));
    }
    for id in [option, some_case, some_field, none_case] {
        let declaration = program.declarations.declaration(id).ok_or_else(|| {
            replay_error(
                function,
                format!("Option postfix `?` references unknown `{id}`"),
            )
        })?;
        if declaration.identity_origin != IdentityOrigin::CompilerOwned {
            return Err(replay_error(
                function,
                format!("Option postfix `?` reference `{id}` is not compiler-owned"),
            ));
        }
    }
    let source_arguments = replay_option_arguments(function, &operand.ty, option)?;
    let target_arguments = replay_option_arguments(function, residual_type, option)?;
    if source_arguments.len() != 1
        || target_arguments.len() != 1
        || source_arguments
            .iter()
            .chain(target_arguments.iter())
            .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
        || expression.ty != source_arguments[0]
        || residual_type != &function.return_type
    {
        return Err(replay_error(
            function,
            "Option postfix `?` source, value, residual, or function type is inconsistent",
        ));
    }
    for ty in [&operand.ty, residual_type] {
        let facts = program.declarations.type_facts(ty).ok_or_else(|| {
            replay_error(function, "Option postfix `?` instance has no type facts")
        })?;
        if !facts.copy || !facts.sized || facts.contains_resource || facts.needs_drop {
            return Err(replay_error(
                function,
                "Option postfix `?` is outside the Copy Option cleanup slice",
            ));
        }
    }
    Ok(StagedCopyResultSource::TryOptionNone {
        expression: work.clone_owned(&expression.id, "Option try expression clone")?,
        operand: work.clone_owned(&operand.id, "Option try operand clone")?,
        source_instance: work.clone_owned(&operand.ty, "Option try source instance clone")?,
        target_instance: work.clone_owned(residual_type, "Option try target instance clone")?,
        option: work.clone_owned(option, "Option try identity clone")?,
        some_case: work.clone_owned(some_case, "Option Some identity clone")?,
        some_field: work.clone_owned(some_field, "Option Some field clone")?,
        none_case: work.clone_owned(none_case, "Option None identity clone")?,
    })
}

fn replay_result_arguments<'a>(
    function: &ResolvedFunction,
    ty: &'a ResolvedType,
    result: &DeclarationId,
) -> Result<&'a [ResolvedType], Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err(replay_error(function, "postfix `?` type is not nominal"));
    };
    if declaration != result {
        return Err(replay_error(
            function,
            "postfix `?` type is not the authenticated Result",
        ));
    }
    Ok(arguments)
}

fn replay_option_arguments<'a>(
    function: &ResolvedFunction,
    ty: &'a ResolvedType,
    option: &DeclarationId,
) -> Result<&'a [ResolvedType], Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err(replay_error(
            function,
            "Option postfix `?` type is not nominal Option",
        ));
    };
    if declaration != option {
        return Err(replay_error(
            function,
            "Option postfix `?` type is not the authenticated Option",
        ));
    }
    Ok(arguments)
}

fn validate_match_skeleton_shape(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arms: &[ResolvedMatchArm],
) -> Result<bool, Diagnostic> {
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

    let is_record = match &scrutinee.ty {
        ResolvedType::Nominal { declaration, .. } => program
            .declarations
            .declaration(declaration)
            .is_some_and(|item| item.kind == DeclarationKind::Record),
        ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool
        | ResolvedType::String
        | ResolvedType::TypeParameter { .. } => false,
    };
    if is_record {
        let [arm] = arms else {
            return Err(replay_error(
                function,
                "irrefutable record match must have exactly one arm",
            ));
        };
        if matches!(&arm.pattern, ResolvedMatchPattern::Variant { .. }) {
            return Err(replay_error(
                function,
                "variant pattern has a record match scrutinee",
            ));
        }
    }
    Ok(is_record)
}

#[allow(clippy::too_many_arguments)]
fn finish_match_arm(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arms: &[ResolvedMatchArm],
    index: usize,
    remaining: Vec<ExprSkeletonPath>,
    arm_paths: &[ExprSkeletonPath],
    results: &mut Vec<ExprSkeletonPath>,
    is_record: bool,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut next_remaining = Vec::new();
    for mut path in remaining {
        if path.failed || path.residual {
            work.push_expr_path(results, path, "match terminal scrutinee path")?;
            continue;
        }
        path.owned_source = None;
        if is_record || index + 1 == arms.len() {
            let selected = sequence_skeleton_paths(
                work.singleton_path(path, "match selected prefix")?,
                arm_paths,
                work,
            )?;
            let selected =
                finish_conditional_result(program, function, expression, selected, work)?;
            append_expr_paths(results, selected, work, "match selected result")?;
            continue;
        }
        let ResolvedMatchPattern::Variant { case, .. } = &arms[index].pattern else {
            return Err(replay_error(
                function,
                "wildcard match arm must be the final exhaustive arm",
            ));
        };
        let mut selected = work.clone_expr_path(&path, "match selected path clone")?;
        let selected_scrutinee =
            work.clone_owned(&scrutinee.id, "match selected scrutinee clone")?;
        let selected_case = work.clone_owned(case, "match selected case clone")?;
        work.push_observation(
            &mut selected,
            SkeletonObservation::VariantCase {
                scrutinee: selected_scrutinee,
                case: selected_case,
                matches: true,
            },
            "match selected observation",
        )?;
        let selected = sequence_skeleton_paths(
            work.singleton_path(selected, "match selected prefix")?,
            arm_paths,
            work,
        )?;
        let selected = finish_conditional_result(program, function, expression, selected, work)?;
        append_expr_paths(results, selected, work, "match selected result")?;
        let rejected_scrutinee =
            work.clone_owned(&scrutinee.id, "match rejected scrutinee clone")?;
        let rejected_case = work.clone_owned(case, "match rejected case clone")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::VariantCase {
                scrutinee: rejected_scrutinee,
                case: rejected_case,
                matches: false,
            },
            "match rejected observation",
        )?;
        work.push_expr_path(&mut next_remaining, path, "match remaining scrutinee path")?;
    }
    Ok(next_remaining)
}

fn append_expr_paths(
    target: &mut Vec<ExprSkeletonPath>,
    paths: Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
    phase: &str,
) -> Result<(), Diagnostic> {
    for path in paths {
        work.push_expr_path(target, path, phase)?;
    }
    Ok(())
}

fn finish_record_paths(
    expression: &ResolvedExpr,
    mut paths: Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let destination = temporary_place(expression, work)?;
    for path in &mut paths {
        if !path.failed && !path.residual {
            path.owned_source =
                Some(work.clone_owned(&destination, "record result destination clone")?);
        }
    }
    Ok(paths)
}

#[allow(clippy::too_many_arguments)]
fn finish_update_paths(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    base: &ResolvedExpr,
    record: &DeclarationId,
    mut paths: Vec<ExprSkeletonPath>,
    replaced: &BTreeSet<DeclarationId>,
    needs_cleanup: bool,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    if !needs_cleanup {
        return Ok(paths);
    }
    let staged_base = temporary_place(base, work)?;
    let destination = temporary_place(expression, work)?;
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
            if path.failed || path.residual {
                continue;
            }
            let mut source = work.clone_owned(&staged_base, "update source place clone")?;
            let source_field = work.clone_owned(&field.id, "update source field clone")?;
            work.charge(1, "update source projection push")?;
            note_skeleton_materialization();
            source.projections.push(source_field);
            let mut field_destination =
                work.clone_owned(&destination, "update destination place clone")?;
            let destination_field =
                work.clone_owned(&field.id, "update destination field clone")?;
            work.charge(1, "update destination projection push")?;
            note_skeleton_materialization();
            field_destination.projections.push(destination_field);
            let at = work.clone_owned(&expression.id, "update transfer expression clone")?;
            work.push_observation(
                path,
                SkeletonObservation::Transfer {
                    at,
                    source,
                    destination: field_destination,
                },
                "untouched update-field transfer",
            )?;
        }
    }
    finish_record_paths(expression, paths, work)
}

fn finish_try_paths(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    operand_paths: Vec<ExprSkeletonPath>,
    option: bool,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let (operand, success_case, source) = if option {
        let ResolvedExprKind::TryOption {
            operand,
            option,
            some_case,
            some_field,
            none_case,
            residual_type,
        } = &expression.kind
        else {
            unreachable!("Option try continuation retains Option try HIR")
        };
        (
            operand.as_ref(),
            some_case,
            authenticated_try_option_stage_source(
                program,
                function,
                expression,
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
                work,
            )?,
        )
    } else {
        let ResolvedExprKind::Try {
            operand,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            residual_type,
        } = &expression.kind
        else {
            unreachable!("try continuation retains Result try HIR")
        };
        (
            operand.as_ref(),
            ok_case,
            authenticated_try_stage_source(
                program,
                function,
                expression,
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
                work,
            )?,
        )
    };
    let mut paths = Vec::new();
    for path in operand_paths {
        if path.failed || path.residual {
            work.push_expr_path(&mut paths, path, "short-circuited try path")?;
            continue;
        }
        let mut success = work.clone_expr_path(&path, "try success path clone")?;
        let success_scrutinee = work.clone_owned(&operand.id, "try success scrutinee clone")?;
        let selected_case = work.clone_owned(success_case, "try success case clone")?;
        let residual_case = work.clone_owned(success_case, "try residual case clone")?;
        work.push_observation(
            &mut success,
            SkeletonObservation::VariantCase {
                scrutinee: success_scrutinee,
                case: selected_case,
                matches: true,
            },
            "try success observation",
        )?;
        work.push_expr_path(&mut paths, success, "try success path")?;

        let mut residual = path;
        let residual_scrutinee = work.clone_owned(&operand.id, "try residual scrutinee clone")?;
        work.push_observation(
            &mut residual,
            SkeletonObservation::VariantCase {
                scrutinee: residual_scrutinee,
                case: residual_case,
                matches: false,
            },
            "try residual case observation",
        )?;
        let staged_source = work.clone_owned(&source, "try staged-result source clone")?;
        work.push_observation(
            &mut residual,
            SkeletonObservation::StageCopyResult(staged_source),
            "try residual staging observation",
        )?;
        residual.residual = true;
        work.push_expr_path(&mut paths, residual, "try residual path")?;
    }
    Ok(paths)
}

fn split_boolean_prefixes(
    paths: Vec<ExprSkeletonPath>,
    expression: &ExpressionId,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<BooleanSkeletonSplit, Diagnostic> {
    let mut terminal = Vec::new();
    let mut true_paths = Vec::new();
    let mut false_paths = Vec::new();
    for mut path in paths {
        if path.failed || path.residual {
            work.push_expr_path(&mut terminal, path, "conditional terminal path")?;
            continue;
        }
        let mut when_true = work.clone_expr_path(&path, "conditional true path clone")?;
        let true_expression = work.clone_owned(expression, "conditional true expression clone")?;
        work.push_observation(
            &mut when_true,
            SkeletonObservation::Boolean {
                expression: true_expression,
                value: true,
            },
            "conditional true observation",
        )?;
        work.push_expr_path(&mut true_paths, when_true, "conditional true prefix")?;
        let false_expression =
            work.clone_owned(expression, "conditional false expression clone")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::Boolean {
                expression: false_expression,
                value: false,
            },
            "conditional false observation",
        )?;
        work.push_expr_path(&mut false_paths, path, "conditional false prefix")?;
    }
    Ok((terminal, true_paths, false_paths))
}

fn finish_conditional_result(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    paths: Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    if expression.ownership == OwnershipMode::Own
        && type_needs_drop(program, function, &expression.ty)?
    {
        let expression_id =
            work.clone_owned(&expression.id, "conditional result expression clone")?;
        transfer_completed_paths(
            function,
            paths,
            expression_id,
            temporary_place(expression, work)?,
            "owned conditional result",
            work,
        )
    } else {
        Ok(paths)
    }
}

fn finish_lazy_paths(
    function: &ResolvedFunction,
    _expression: &ResolvedExpr,
    op: BinaryOp,
    left: &ResolvedExpr,
    left_paths: Vec<ExprSkeletonPath>,
    right_paths: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    if !matches!(op, BinaryOp::And | BinaryOp::Or) {
        return Err(replay_error(function, "invalid lazy skeleton operation"));
    }
    let (mut terminal, true_paths, false_paths) =
        split_boolean_prefixes(left_paths, &left.id, work)?;
    let (evaluated, short) = if op == BinaryOp::And {
        (true_paths, false_paths)
    } else {
        (false_paths, true_paths)
    };
    for mut path in short {
        path.owned_source = None;
        work.push_expr_path(&mut terminal, path, "lazy short-circuit path")?;
    }
    let evaluated = sequence_skeleton_paths(evaluated, right_paths, work)?;
    append_expr_paths(&mut terminal, evaluated, work, "lazy evaluated-right path")?;
    Ok(terminal)
}

fn call_states_have_active(states: &[CallSkeletonState]) -> bool {
    states
        .iter()
        .any(|(path, _)| !path.failed && !path.residual)
}

#[allow(clippy::too_many_arguments)]
fn sequence_call_argument(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    params: &[crate::hir::ResolvedParam],
    args: &[ResolvedExpr],
    index: usize,
    states: Vec<CallSkeletonState>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<CallSkeletonState>, Diagnostic> {
    let argument = &args[index];
    let parameter = params
        .get(index)
        .ok_or_else(|| replay_error(function, "skeleton call arity is inconsistent"))?;
    let mut next = Vec::new();
    for (prefix, commits) in states {
        if prefix.failed || prefix.residual {
            work.charge(1, "short-circuited call state push")?;
            note_skeleton_materialization();
            next.push((prefix, commits));
            continue;
        }
        for suffix in suffixes {
            let mut observations =
                work.clone_observations(&prefix.observations, "call prefix clone")?;
            work.extend_observations(&mut observations, &suffix.observations, "call suffix clone")?;
            let owned_source =
                work.clone_owned(&suffix.owned_source, "call suffix owned-source clone")?;
            let mut path = ExprSkeletonPath {
                observations,
                owned_source,
                failed: suffix.failed,
                residual: suffix.residual,
            };
            let mut path_commits = work.clone_owned(&commits, "call commit-state clone")?;
            if !path.failed
                && !path.residual
                && parameter.ownership == OwnershipMode::Own
                && type_needs_drop(program, function, &parameter.ty)?
            {
                let parameter_index = u32::try_from(index)
                    .map_err(|_| replay_error(function, "too many skeleton call arguments"))?;
                let call = work.clone_owned(&expression.id, "call-epoch call identity clone")?;
                let value_expression =
                    work.clone_owned(&argument.id, "call-epoch value identity clone")?;
                let epoch = CleanupPlace {
                    storage: StorageId::CallArgument {
                        call,
                        parameter_index,
                        value_expression,
                    },
                    projections: Vec::new(),
                };
                let source = path.owned_source.take().ok_or_else(|| {
                    replay_error(function, "owned call argument has no HIR cleanup source")
                })?;
                let at = work.clone_owned(&argument.id, "call-argument transfer identity clone")?;
                let destination = work.clone_owned(&epoch, "call-argument epoch clone")?;
                work.push_observation(
                    &mut path,
                    SkeletonObservation::Transfer {
                        at,
                        source,
                        destination,
                    },
                    "owned call-argument transfer",
                )?;
                work.charge(1, "call-commit argument push")?;
                note_skeleton_materialization();
                path_commits.push((parameter_index, epoch));
            }
            work.charge(1, "call state push")?;
            note_skeleton_materialization();
            next.push((path, path_commits));
        }
    }
    Ok(next)
}

fn finish_call_states(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    states: Vec<CallSkeletonState>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let source_expression =
        work.clone_owned(&expression.id, "call status source expression clone")?;
    let source = StatusSourceId {
        expression: source_expression,
        lane: StatusLane::OperationFailure,
    };
    let mut results = Vec::new();
    for (mut path, commits) in states {
        if path.failed || path.residual {
            work.push_expr_path(&mut results, path, "short-circuited call path")?;
            continue;
        }
        let call = work.clone_owned(&expression.id, "call-commit identity clone")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::CallCommit {
                call,
                arguments: commits,
            },
            "call-commit observation",
        )?;
        let mut failure = work.clone_expr_path(&path, "call failure path clone")?;
        let failure_source = work.clone_owned(&source, "call failure status-source clone")?;
        work.push_observation(
            &mut failure,
            SkeletonObservation::Status {
                source: failure_source,
                success: false,
            },
            "call failure observation",
        )?;
        failure.failed = true;
        failure.owned_source = None;
        work.push_expr_path(&mut results, failure, "call failure path")?;

        let success_source = work.clone_owned(&source, "call success status-source clone")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::Status {
                source: success_source,
                success: true,
            },
            "call success observation",
        )?;
        if expression.ownership == OwnershipMode::Own
            && type_needs_drop(program, function, &expression.ty)?
        {
            let destination = temporary_place(expression, work)?;
            let at = work.clone_owned(&expression.id, "call result expression clone")?;
            let initialized = work.clone_owned(&destination, "call result destination clone")?;
            work.push_observation(
                &mut path,
                SkeletonObservation::Initialize {
                    at,
                    destination: initialized,
                },
                "owned call result initialization",
            )?;
            path.owned_source = Some(destination);
        } else {
            path.owned_source = None;
        }
        work.push_expr_path(&mut results, path, "call success path")?;
    }
    Ok(results)
}

fn split_status_paths(
    paths: Vec<ExprSkeletonPath>,
    source: StatusSourceId,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut results = Vec::new();
    for mut path in paths {
        if path.failed || path.residual {
            work.push_expr_path(&mut results, path, "short-circuited status path")?;
            continue;
        }
        let mut failure = work.clone_expr_path(&path, "status failure path clone")?;
        let failure_source = work.clone_owned(&source, "failure status-source clone")?;
        work.push_observation(
            &mut failure,
            SkeletonObservation::Status {
                source: failure_source,
                success: false,
            },
            "status failure observation",
        )?;
        failure.failed = true;
        failure.owned_source = None;
        work.push_expr_path(&mut results, failure, "status failure path")?;
        let success_source = work.clone_owned(&source, "success status-source clone")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::Status {
                source: success_source,
                success: true,
            },
            "status success observation",
        )?;
        path.owned_source = None;
        work.push_expr_path(&mut results, path, "status success path")?;
    }
    Ok(results)
}

fn split_contract(
    paths: Vec<ExprSkeletonPath>,
    contract: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut results = Vec::new();
    for mut path in paths {
        if path.failed || path.residual {
            work.push_expr_path(&mut results, path, "short-circuited contract path")?;
            continue;
        }
        let mut failure = work.clone_expr_path(&path, "contract failure path clone")?;
        let failure_expression =
            work.clone_owned(&contract.id, "contract failure expression clone")?;
        work.push_observation(
            &mut failure,
            SkeletonObservation::Boolean {
                expression: failure_expression,
                value: false,
            },
            "contract failure observation",
        )?;
        failure.failed = true;
        failure.owned_source = None;
        work.push_expr_path(&mut results, failure, "contract failure path")?;
        let success_expression =
            work.clone_owned(&contract.id, "contract success expression clone")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::Boolean {
                expression: success_expression,
                value: true,
            },
            "contract success observation",
        )?;
        path.owned_source = None;
        work.push_expr_path(&mut results, path, "contract success path")?;
    }
    Ok(results)
}

fn transfer_completed_paths(
    function: &ResolvedFunction,
    mut paths: Vec<ExprSkeletonPath>,
    at: ExpressionId,
    destination: CleanupPlace,
    description: &str,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    for path in &mut paths {
        if path.failed || path.residual {
            continue;
        }
        let source = path.owned_source.take().ok_or_else(|| {
            replay_error(function, format!("{description} has no HIR cleanup source"))
        })?;
        let transfer_at = work.clone_owned(&at, "completed transfer expression clone")?;
        let transfer_destination =
            work.clone_owned(&destination, "completed transfer destination clone")?;
        work.push_observation(
            path,
            SkeletonObservation::Transfer {
                at: transfer_at,
                source,
                destination: transfer_destination,
            },
            "completed-path transfer observation",
        )?;
        path.owned_source =
            Some(work.clone_owned(&destination, "completed transfer result-place clone")?);
    }
    Ok(paths)
}

fn temporary_place(
    expression: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<CleanupPlace, Diagnostic> {
    Ok(CleanupPlace {
        storage: StorageId::Temporary(
            work.clone_owned(&expression.id, "temporary-place expression clone")?,
        ),
        projections: Vec::new(),
    })
}

fn cleanup_place_from_hir(
    function: &ResolvedFunction,
    place: &crate::hir::Place,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<CleanupPlace, Diagnostic> {
    let storage = if place.root == function.result_id {
        StorageId::ProvisionalResult
    } else {
        StorageId::Value(work.clone_owned(&place.root, "place-root clone")?)
    };
    let mut projections = Vec::new();
    for projection in &place.projections {
        match projection {
            PlaceProjection::Field(field) => {
                let field = work.clone_owned(field, "place-projection clone")?;
                work.charge(1, "place-projection push")?;
                note_skeleton_materialization();
                projections.push(field);
            }
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
    let mut queue = VecDeque::new();
    skeleton_queue_push(
        budget,
        function,
        &mut queue,
        (plan.entry, Vec::<SkeletonObservation>::new()),
        "cleanup-plan root state push",
    )?;
    let mut paths = Vec::new();
    while let Some((block, mut observations)) = queue.pop_front() {
        let block = &plan.blocks[block.0 as usize];
        // Charge only work performed at this block. The previous charge used
        // the entire accumulated observation length on every linear block,
        // turning a depth-D skewed conditional into artificial O(D^3) work.
        // Observation history is copied only at a real branch, charged below
        // immediately before each clone.
        budget.charge_skeleton(
            function,
            block.transitions.len().saturating_add(1),
            "cleanup-plan skeleton expansion",
        )?;
        for transition in &block.transitions {
            match transition {
                CleanupTransition::Initialize { at, destination } => {
                    let at =
                        skeleton_clone(budget, function, at, "plan initialize expression clone")?;
                    let destination = skeleton_clone(
                        budget,
                        function,
                        destination,
                        "plan initialize destination clone",
                    )?;
                    skeleton_push(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::Initialize { at, destination },
                        "plan initialize observation push",
                    )?;
                }
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => {
                    let at =
                        skeleton_clone(budget, function, at, "plan transfer expression clone")?;
                    let source =
                        skeleton_clone(budget, function, source, "plan transfer source clone")?;
                    let destination = skeleton_clone(
                        budget,
                        function,
                        destination,
                        "plan transfer destination clone",
                    )?;
                    skeleton_push(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::Transfer {
                            at,
                            source,
                            destination,
                        },
                        "plan transfer observation push",
                    )?;
                }
                CleanupTransition::CallCommit { call, arguments } => {
                    let call = skeleton_clone(budget, function, call, "plan call identity clone")?;
                    let mut cloned_arguments = Vec::new();
                    for argument in arguments {
                        let source = skeleton_clone(
                            budget,
                            function,
                            &argument.source,
                            "plan call argument source clone",
                        )?;
                        skeleton_push(
                            budget,
                            function,
                            &mut cloned_arguments,
                            (argument.parameter_index, source),
                            "plan call argument push",
                        )?;
                    }
                    skeleton_push(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::CallCommit {
                            call,
                            arguments: cloned_arguments,
                        },
                        "plan call observation push",
                    )?;
                }
                CleanupTransition::SelectFailure { .. } => {}
                CleanupTransition::StageCopyResult { source } => {
                    let source = skeleton_clone(
                        budget,
                        function,
                        source,
                        "plan staged-result source clone",
                    )?;
                    skeleton_push(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::StageCopyResult(source),
                        "plan staged-result observation push",
                    )?;
                }
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                skeleton_queue_push(
                    budget,
                    function,
                    &mut queue,
                    (plan.edges[edge.0 as usize].to, observations),
                    "plan goto state push",
                )?;
            }
            CleanupTerminator::Branch(edges) => {
                for edge in edges {
                    let edge = &plan.edges[edge.0 as usize];
                    let observation = match &edge.condition {
                        EdgeCondition::BooleanResult(expression, value) => {
                            SkeletonObservation::Boolean {
                                expression: skeleton_clone(
                                    budget,
                                    function,
                                    expression,
                                    "plan Boolean expression clone",
                                )?,
                                value: *value,
                            }
                        }
                        EdgeCondition::VariantCase {
                            scrutinee,
                            case,
                            matches,
                        } => SkeletonObservation::VariantCase {
                            scrutinee: skeleton_clone(
                                budget,
                                function,
                                scrutinee,
                                "plan variant scrutinee clone",
                            )?,
                            case: skeleton_clone(
                                budget,
                                function,
                                case,
                                "plan variant case clone",
                            )?,
                            matches: *matches,
                        },
                        EdgeCondition::StatusZero(source) => SkeletonObservation::Status {
                            source: skeleton_clone(
                                budget,
                                function,
                                source,
                                "plan zero status-source clone",
                            )?,
                            success: true,
                        },
                        EdgeCondition::StatusNonzero(source) => SkeletonObservation::Status {
                            source: skeleton_clone(
                                budget,
                                function,
                                source,
                                "plan nonzero status-source clone",
                            )?,
                            success: false,
                        },
                        EdgeCondition::Always => {
                            return Err(replay_error(
                                function,
                                "branch skeleton contains an unconditional edge",
                            ));
                        }
                    };
                    let mut branch = skeleton_clone(
                        budget,
                        function,
                        &observations,
                        "cleanup-plan skeleton branch clone",
                    )?;
                    skeleton_push(
                        budget,
                        function,
                        &mut branch,
                        observation,
                        "plan branch observation push",
                    )?;
                    skeleton_queue_push(
                        budget,
                        function,
                        &mut queue,
                        (edge.to, branch),
                        "plan branch state push",
                    )?;
                }
            }
            CleanupTerminator::Exit(exit) => {
                let exit = &plan.exits[exit.0 as usize];
                match exit.continuation {
                    ExitContinuation::Continue(edge) => {
                        skeleton_queue_push(
                            budget,
                            function,
                            &mut queue,
                            (plan.edges[edge.0 as usize].to, observations),
                            "plan continuation state push",
                        )?;
                    }
                    ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit => {
                        skeleton_push(
                            budget,
                            function,
                            &mut paths,
                            SkeletonPath {
                                observations,
                                terminal: SkeletonTerminal::Success,
                            },
                            "plan success path push",
                        )?;
                    }
                    ExitContinuation::ReturnFailure { .. } => skeleton_push(
                        budget,
                        function,
                        &mut paths,
                        SkeletonPath {
                            observations,
                            terminal: SkeletonTerminal::Failure,
                        },
                        "plan failure path push",
                    )?,
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
        staged_copy_result: None,
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
        CleanupTransition::StageCopyResult { source } => {
            if state.staged_copy_result.is_some() {
                return Err(replay_error(
                    function,
                    "Copy aggregate result is staged more than once on one path",
                ));
            }
            if !state.live_order.is_empty() {
                return Err(replay_error(
                    function,
                    "Copy aggregate result staging carries resource liveness",
                ));
            }
            state.staged_copy_result = Some(source.clone());
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
                    if expression_has_try(&function.body) {
                        let staged = state.staged_copy_result.take().ok_or_else(|| {
                            replay_error(
                                function,
                                "Copy Result commit has no authenticated staged producer",
                            )
                        })?;
                        validate_staged_target(function, &staged)?;
                    } else if state.staged_copy_result.is_some() {
                        return Err(replay_error(
                            function,
                            "non-try scalar result carries a staged Copy aggregate",
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

fn validate_staged_target(
    function: &ResolvedFunction,
    source: &StagedCopyResultSource,
) -> Result<(), Diagnostic> {
    let target = match source {
        StagedCopyResultSource::Body {
            expression,
            instance,
        } => {
            if expression != &function.body.id {
                return Err(replay_error(
                    function,
                    "body result staging references another expression",
                ));
            }
            instance
        }
        StagedCopyResultSource::TryResidual {
            target_instance, ..
        }
        | StagedCopyResultSource::TryOptionNone {
            target_instance, ..
        } => target_instance,
    };
    if target != &function.return_type {
        return Err(replay_error(
            function,
            "staged Copy result targets another concrete function result",
        ));
    }
    Ok(())
}

fn replay_expression_child(expression: &ResolvedExpr, index: usize) -> Option<&ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => args.get(index),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.get(index),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. } => (index == 0).then_some(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            [left.as_ref(), right.as_ref()].get(index).copied()
        }
        ResolvedExprKind::Block { statements, tail } => statements
            .get(index)
            .map(|statement| statement.value())
            .or_else(|| (index == statements.len()).then_some(tail)),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => [
            condition.as_ref(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        ]
        .get(index)
        .copied(),
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.get(index).map(|field| &field.value)
        }
        ResolvedExprKind::Match { scrutinee, arms } => (index == 0)
            .then_some(scrutinee.as_ref())
            .or_else(|| arms.get(index - 1).map(|arm| &arm.value)),
        ResolvedExprKind::UpdateRecord { base, fields, .. } => (index == 0)
            .then_some(base.as_ref())
            .or_else(|| fields.get(index - 1).map(|field| &field.value)),
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_) => None,
    }
}

fn expression_has_kind(
    expression: &ResolvedExpr,
    predicate: impl Fn(&ResolvedExprKind) -> bool,
) -> bool {
    let mut stack = [None; 514];
    stack[0] = Some((expression, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = stack[len].take().expect("expression frame retained");
        if next == 0 && predicate(&expression.kind) {
            return true;
        }
        if let Some(child) = replay_expression_child(expression, next) {
            if len + 2 > stack.len() {
                return false;
            }
            stack[len] = Some((expression, next + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
        }
    }
    false
}

fn expression_has_try(expression: &ResolvedExpr) -> bool {
    expression_has_kind(expression, |kind| {
        matches!(
            kind,
            ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. }
        )
    })
}

fn expression_has_option_try(expression: &ResolvedExpr) -> bool {
    expression_has_kind(expression, |kind| {
        matches!(kind, ResolvedExprKind::TryOption { .. })
    })
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
    // The private replay entry admits at most 512 semantic expression levels.
    // Keep one indexed continuation per ancestor so wide calls, records, blocks,
    // and matches never create a width-sized frontier and callback order stays
    // identical to the former recursive pre-order walk.
    let mut stack = [None; 514];
    stack[0] = Some((expression, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (current, next_child) = stack[len].take().expect("expression-fact frame retained");
        if next_child == 0 {
            let fact = match &current.kind {
                ResolvedExprKind::Call {
                    callee,
                    instance,
                    args,
                    ..
                } => Some(CallFact {
                    callee: callee.clone(),
                    instance: instance.clone(),
                    arguments: args.iter().map(|argument| argument.id.clone()).collect(),
                }),
                _ => None,
            };
            if facts.insert(current.id.clone(), fact).is_some() {
                return Err(replay_error(
                    function,
                    format!("HIR expression identity `{}` is repeated", current.id),
                ));
            }
        }
        if let Some(child) = replay_expression_child(current, next_child) {
            if len + 2 > stack.len() {
                return Err(replay_error(
                    function,
                    "HIR expression fact traversal exceeds the admitted depth",
                ));
            }
            stack[len] = Some((current, next_child + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
        }
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
                | CleanupTransition::Transfer { .. }
                | CleanupTransition::CallCommit { .. }
                | CleanupTransition::SelectFailure { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(stages.len(), 2);
        let residual = stages
            .iter()
            .find_map(|source| match source {
                StagedCopyResultSource::TryResidual { .. } => Some((*source).clone()),
                StagedCopyResultSource::Body { .. }
                | StagedCopyResultSource::TryOptionNone { .. } => None,
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
}
