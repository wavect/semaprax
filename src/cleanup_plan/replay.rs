//! Independent structural validation for attached cleanup plans.
//!
//! This module deliberately does not invoke the canonical builder.  It checks
//! that an attached plan is a closed, well-formed CFG whose identifiers,
//! places, status sources, guarded finalizers, and every current acyclic path
//! can be replayed safely.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[cfg(test)]
use crate::hir::ResolvedTypeDeclarationKind;
#[cfg(test)]
use std::cell::Cell;

use crate::ast::{BinaryOp, UnaryOp};
use crate::cleanup::{CleanupStorageOrigin, FieldLivenessShape, LivenessFlagId};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, DeclarationKind, ExpressionId, FunctionInstanceId, IdentityOrigin,
    OwnershipMode, PlaceProjection, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedMatchArm, ResolvedMatchPattern, ResolvedProgram, ResolvedRecordMatchFieldPattern,
    ResolvedStatement, ResolvedType,
};
use crate::prelude;

use super::{
    BlockId, CleanupPlace, CleanupRegionId, CleanupResultSource, CleanupTerminator,
    CleanupTransition, ConditionalVariantCase, ConditionalVariantEntry, EdgeCondition, EdgeId,
    ExitContinuation, ExitTarget, StagedCopyResultSource, StatusCase, StatusLane, StatusProducer,
    StatusSource, StatusSourceId, StorageId, CLEANUP_PLAN_SCHEMA_V2, CLEANUP_PLAN_SCHEMA_V3,
    CLEANUP_PLAN_SCHEMA_V4, CLEANUP_PLAN_SCHEMA_V5, CLEANUP_PLAN_SCHEMA_V6, CLEANUP_PLAN_SCHEMA_V7,
    CLEANUP_PLAN_SCHEMA_V8, CLEANUP_PLAN_SCHEMA_V9,
};

mod nested_shape;
mod record_destructure;
use nested_shape::expected_shape_for_type;

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
    conditional_variants: Vec<ReplayConditionalVariant>,
    pending_failure: Option<StatusSourceId>,
    selected_failure: Option<StatusSourceId>,
    staged_copy_result: Option<StagedCopyResultSource>,
    published: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ReplayConditionalVariant {
    root: CleanupPlace,
    variant: DeclarationId,
    cases: Vec<(DeclarationId, Vec<LivenessFlagId>)>,
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
    /// Refutable Match v1: one scalar decision-chain selection.
    ArmSelected {
        scrutinee: ExpressionId,
        arm: u32,
        selected: bool,
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
    let has_nested_owned_bytes =
        function
            .cleanup
            .slots
            .iter()
            .try_fold(false, |nested, slot| {
                crate::cleanup::cleanup_shape_profile(&slot.shape)
                    .map(|profile| nested || profile.has_nested_owned_bytes)
            })?;
    let has_nested_record_destructure = record_destructure::function_contains(function);
    let has_nested_record_update =
        record_destructure::update::function_contains(program, function)?;
    let expected_schema = if has_nested_record_update {
        CLEANUP_PLAN_SCHEMA_V9
    } else if has_nested_record_destructure {
        CLEANUP_PLAN_SCHEMA_V8
    } else if has_nested_owned_bytes {
        CLEANUP_PLAN_SCHEMA_V7
    } else if function.cleanup.schema == crate::cleanup::CLEANUP_INVENTORY_SCHEMA_V2
        || function
            .requires
            .iter()
            .any(expression_has_explicit_variant_match)
        || function
            .ensures
            .iter()
            .any(expression_has_explicit_variant_match)
        || expression_has_explicit_variant_match(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V6
    } else if function
        .requires
        .iter()
        .any(expression_has_explicit_record_match)
        || function
            .ensures
            .iter()
            .any(expression_has_explicit_record_match)
        || expression_has_explicit_record_match(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V5
    } else if function.requires.iter().any(expression_has_byte_range)
        || function.ensures.iter().any(expression_has_byte_range)
        || expression_has_byte_range(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V4
    } else if function.requires.iter().any(expression_has_option_try)
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
    validate_path_states(program, function, &storage, &leaves, budget)?;
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

/// `true` when the resolved expression tree contains a while statement.
fn expression_contains_while(expression: &ResolvedExpr) -> bool {
    let mut stack = [None; 514];
    stack[0] = Some(expression);
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let expression = stack[len].take().expect("census frame retained");
        if let ResolvedExprKind::Block { statements, tail } = &expression.kind {
            if statements
                .iter()
                .any(|statement| matches!(statement, ResolvedStatement::While { .. }))
            {
                return true;
            }
            for statement in statements {
                for index in 0..statement.child_count() {
                    if len + 1 >= stack.len() {
                        return false;
                    }
                    if let Some(child) = statement.child(index) {
                        stack[len] = Some(child);
                        len += 1;
                    }
                }
            }
            if len + 1 >= stack.len() {
                return false;
            }
            stack[len] = Some(tail.as_ref());
            len += 1;
            continue;
        }
        for index in 0.. {
            match replay_expression_child(expression, index) {
                Some(child) => {
                    if len + 1 >= stack.len() {
                        return false;
                    }
                    stack[len] = Some(child);
                    len += 1;
                }
                None => break,
            }
        }
    }
    false
}

/// While-aware path census. Mirrors `expression_path_counts` exactly and adds
/// the single-pass while contribution: the condition branches into one body
/// pass or the skip continuation, so normal paths multiply by
/// `body.normal + 1` while failure paths propagate through both sides.
fn expression_path_counts_with_while(
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<HirPathCounts, Diagnostic> {
    fn child(expression: &ResolvedExpr, mut index: usize) -> Option<&ResolvedExpr> {
        match &expression.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. } => None,
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Upcast { source: value }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. } => {
                (index == 0).then_some(value.as_ref())
            }
            ResolvedExprKind::Binary { left, right, .. } => {
                [left.as_ref(), right.as_ref()].get(index).copied()
            }
            ResolvedExprKind::Call { args, .. } => args.get(index),
            ResolvedExprKind::NativeRustImportCall(call) => call.args.get(index),
            ResolvedExprKind::HostCommandCall(call) => call.args.get(index),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => [source.as_ref(), start.as_ref(), end.as_ref()]
                .get(index)
                .copied(),
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
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                if index == 0 {
                    Some(scrutinee)
                } else {
                    arms.get(index - 1).map(|arm| &arm.value)
                }
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                if index == 0 {
                    Some(base)
                } else {
                    fields.get(index - 1).map(|field| &field.value)
                }
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                fields.get(index).map(|field| &field.value)
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    let child_count = statement.child_count();
                    if index < child_count {
                        return statement.child(index);
                    }
                    index -= child_count;
                }
                (index == 0).then_some(tail.as_ref())
            }
        }
    }

    enum Frame<'a> {
        Enter(&'a ResolvedExpr),
        Finish {
            expression: &'a ResolvedExpr,
            result_base: usize,
        },
    }

    let saturating_total = HirPathCounts {
        normal: MAX_REPLAY_PATHS + 1,
        failed: 0,
        residual: 0,
    };
    let mut frames = vec![Frame::Enter(expression)];
    let mut results: Vec<HirPathCounts> = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(expression) => {
                let mut child_count = 0usize;
                while child(expression, child_count).is_some() {
                    child_count = child_count.saturating_add(1);
                }
                frames
                    .try_reserve(child_count.saturating_add(1))
                    .map_err(|_| {
                        replay_error(function, "while path census capacity exceeds address space")
                    })?;
                frames.push(Frame::Finish {
                    expression,
                    result_base: results.len(),
                });
                for index in (0..child_count).rev() {
                    frames.push(Frame::Enter(
                        child(expression, index).expect("counted expression child is present"),
                    ));
                }
            }
            Frame::Finish {
                expression,
                result_base,
            } => {
                let children = results.get(result_base..).ok_or_else(|| {
                    replay_error(function, "while path census result stack is invalid")
                })?;
                let sequence = |children: &[HirPathCounts]| {
                    children
                        .iter()
                        .copied()
                        .fold(HirPathCounts::ONE, sequence_path_counts)
                };
                let counts = match &expression.kind {
                    ResolvedExprKind::Int(_)
                    | ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Usize(_)
                    | ResolvedExprKind::ArrayU8(_)
                    | ResolvedExprKind::RepeatArrayU8 { .. }
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::Bool(_)
                    | ResolvedExprKind::String(_)
                    | ResolvedExprKind::Place(_)
                    | ResolvedExprKind::BorrowPlace { .. } => HirPathCounts::ONE,
                    ResolvedExprKind::Unary { op, .. } => {
                        let inner = children[0];
                        if *op == UnaryOp::Neg {
                            HirPathCounts {
                                normal: inner.normal,
                                failed: inner.failed.saturating_add(inner.normal),
                                residual: inner.residual,
                            }
                        } else {
                            inner
                        }
                    }
                    ResolvedExprKind::Upcast { .. } | ResolvedExprKind::Project { .. } => {
                        children[0]
                    }
                    ResolvedExprKind::Binary { op, .. } => {
                        let left = children[0];
                        let right = children[1];
                        if matches!(op, BinaryOp::And | BinaryOp::Or) {
                            HirPathCounts {
                                normal: left.normal.saturating_mul(right.normal.saturating_add(1)),
                                failed: left
                                    .failed
                                    .saturating_add(left.normal.saturating_mul(right.failed)),
                                residual: left
                                    .residual
                                    .saturating_add(left.normal.saturating_mul(right.residual)),
                            }
                        } else {
                            let sequenced = sequence_path_counts(left, right);
                            if matches!(
                                op,
                                BinaryOp::Add
                                    | BinaryOp::Sub
                                    | BinaryOp::Mul
                                    | BinaryOp::Div
                                    | BinaryOp::Rem
                            ) {
                                HirPathCounts {
                                    failed: sequenced.failed.saturating_add(sequenced.normal),
                                    ..sequenced
                                }
                            } else {
                                sequenced
                            }
                        }
                    }
                    ResolvedExprKind::Call { .. } | ResolvedExprKind::NativeRustImportCall(_) => {
                        let accumulator = sequence(children);
                        HirPathCounts {
                            failed: accumulator.failed.saturating_add(accumulator.normal),
                            ..accumulator
                        }
                    }
                    ResolvedExprKind::HostCommandCall(call) => {
                        let accumulator = sequence(children);
                        if crate::command_io_ops::failure(call.operation)
                            == crate::command_io_ops::CommandIoFailure::Status
                        {
                            HirPathCounts {
                                failed: accumulator.failed.saturating_add(accumulator.normal),
                                ..accumulator
                            }
                        } else {
                            accumulator
                        }
                    }
                    ResolvedExprKind::ByteRange { .. } => {
                        let accumulator = sequence(children);
                        HirPathCounts {
                            failed: accumulator.failed.saturating_add(accumulator.normal),
                            ..accumulator
                        }
                    }
                    ResolvedExprKind::If { .. } => {
                        let condition = children[0];
                        let then_branch = children[1];
                        let else_branch = children[2];
                        HirPathCounts {
                            normal: condition.normal.saturating_mul(
                                then_branch.normal.saturating_add(else_branch.normal),
                            ),
                            failed: condition.failed.saturating_add(
                                condition.normal.saturating_mul(
                                    then_branch.failed.saturating_add(else_branch.failed),
                                ),
                            ),
                            residual: condition.residual.saturating_add(
                                condition.normal.saturating_mul(
                                    then_branch.residual.saturating_add(else_branch.residual),
                                ),
                            ),
                        }
                    }
                    ResolvedExprKind::Match { .. } => {
                        let scrutinee = children[0];
                        let arms = &children[1..];
                        let arms_normal = arms
                            .iter()
                            .fold(0usize, |sum, arm| sum.saturating_add(arm.normal));
                        let arms_failed = arms
                            .iter()
                            .fold(0usize, |sum, arm| sum.saturating_add(arm.failed));
                        let arms_residual = arms
                            .iter()
                            .fold(0usize, |sum, arm| sum.saturating_add(arm.residual));
                        HirPathCounts {
                            normal: scrutinee.normal.saturating_mul(arms_normal),
                            failed: scrutinee
                                .failed
                                .saturating_add(scrutinee.normal.saturating_mul(arms_failed)),
                            residual: scrutinee
                                .residual
                                .saturating_add(scrutinee.normal.saturating_mul(arms_residual)),
                        }
                    }
                    ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => {
                        let operand = children[0];
                        HirPathCounts {
                            residual: operand.residual.saturating_add(operand.normal),
                            ..operand
                        }
                    }
                    ResolvedExprKind::UpdateRecord { .. }
                    | ResolvedExprKind::ConstructRecord { .. }
                    | ResolvedExprKind::ConstructVariant { .. } => sequence(children),
                    ResolvedExprKind::Block { statements, .. } => {
                        let mut accumulator = HirPathCounts::ONE;
                        let mut index = 0usize;
                        for statement in statements {
                            if matches!(statement, ResolvedStatement::While { .. }) {
                                let condition = children[index];
                                let body = children[index + 1];
                                index += 2;
                                let contribution = HirPathCounts {
                                    normal: condition
                                        .normal
                                        .saturating_mul(body.normal.saturating_add(1)),
                                    failed: condition.failed.saturating_add(
                                        condition.normal.saturating_mul(body.failed),
                                    ),
                                    residual: condition.residual.saturating_add(
                                        condition.normal.saturating_mul(body.residual),
                                    ),
                                };
                                accumulator = sequence_path_counts(accumulator, contribution);
                            } else {
                                for _ in 0..statement.child_count() {
                                    accumulator =
                                        sequence_path_counts(accumulator, children[index]);
                                    index += 1;
                                }
                            }
                        }
                        sequence_path_counts(accumulator, children[index])
                    }
                };
                results.truncate(result_base);
                results.push(if counts.total() > MAX_REPLAY_PATHS {
                    saturating_total
                } else {
                    counts
                });
            }
        }
    }
    results
        .pop()
        .ok_or_else(|| replay_error(function, "while path census produced no result"))
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
                CleanupTransition::InitializeVariant { .. } => 6,
                CleanupTransition::Transfer { .. } => 5,
                CleanupTransition::TransferVariant { .. } => 6,
                CleanupTransition::AuthenticateVariantCase { .. } => 6,
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
                | ResolvedExprKind::Usize(_)
                | ResolvedExprKind::ArrayU8(_)
                | ResolvedExprKind::RepeatArrayU8 { .. }
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::Bool(_)
                | ResolvedExprKind::String(_) => 2,
                ResolvedExprKind::Place(place) => place.projections.len().saturating_mul(2) + 8,
                ResolvedExprKind::BorrowPlace { place, .. } => {
                    place.projections.len().saturating_mul(2) + 8
                }
                ResolvedExprKind::Unary { .. } => 8,
                ResolvedExprKind::Binary { .. } => 12,
                ResolvedExprKind::Call { args, .. } => args.len().saturating_mul(6) + 14,
                ResolvedExprKind::NativeRustImportCall(call) => {
                    call.args.len().saturating_mul(4) + 8
                }
                ResolvedExprKind::HostCommandCall(call) => call.args.len().saturating_mul(6) + 14,
                ResolvedExprKind::ByteRange { .. } => 32,
                ResolvedExprKind::Block { statements, .. } => {
                    // Child censuses include their Eval push. Each statement
                    // additionally needs its continuation and four sequencing
                    // operations; root setup and tail sequencing need seven.
                    let mut local = checked_skeleton_add(
                        function,
                        checked_skeleton_mul(function, statements.len(), 5)?,
                        7,
                    )?;
                    for statement in statements {
                        let extra = match statement {
                            ResolvedStatement::While { .. } => 6,
                            ResolvedStatement::Let { binding, .. }
                            | ResolvedStatement::Assign { binding, .. }
                                if binding.ownership == OwnershipMode::Own
                                    && type_needs_drop(program, function, &binding.ty)? =>
                            {
                                // Two identity clones and four completed-transfer
                                // operations, matching BlockValue below.
                                6
                            }
                            _ => 0,
                        };
                        local = checked_skeleton_add(function, local, extra)?;
                    }
                    if expression.ownership == OwnershipMode::Own
                        && type_needs_drop(program, function, &expression.ty)?
                    {
                        // BlockTail also authenticates an owned result transfer.
                        local = checked_skeleton_add(function, local, 6)?;
                    }
                    local
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
                ResolvedExprKind::Project { .. } | ResolvedExprKind::Upcast { .. } => 6,
                ResolvedExprKind::If { .. } => 10,
                ResolvedExprKind::Match { arms, .. } => {
                    // Guards recurse as separate sub-skeletons, so each arm
                    // carries extra headroom over the aggregate baseline.
                    arms.len().saturating_mul(16).saturating_add(8)
                }
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
    if expression_contains_while(expression) {
        return expression_path_counts_with_while(function, expression);
    }
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
    let mut independently_derived_flag = 0u32;
    for (index, inventory_slot) in function.cleanup.slots.iter().enumerate() {
        let actual = &plan.slots[index];
        let expected_storage = inventory_storage_id(&inventory_slot.origin);
        let independently_derived = expected_shape_for_type(
            program,
            function,
            &inventory_slot.ty,
            &mut independently_derived_flag,
        )?;
        if inventory_slot.shape != independently_derived {
            return Err(replay_error(
                function,
                format!(
                    "cleanup inventory base slot {index} disagrees with independently derived typed HIR"
                ),
            ));
        }
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
    if usize::try_from(independently_derived_flag).ok() != Some(function.cleanup.flags.len()) {
        return Err(replay_error(
            function,
            "cleanup inventory liveness census disagrees with independently derived typed HIR",
        ));
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
    let expected_conditional = function
        .cleanup
        .entry_state
        .conditional_owned_parameters
        .iter()
        .map(|entry| {
            let storage = inventory_storage
                .get(&entry.storage)
                .cloned()
                .ok_or_else(|| {
                    replay_error(
                        function,
                        "conditional cleanup inventory entry references unknown storage",
                    )
                })?;
            let cases = entry
                .cases
                .iter()
                .map(|case| {
                    let live_places = case
                        .live_flags
                        .iter()
                        .map(|flag| {
                            let inventory = function
                                .cleanup
                                .flags
                                .iter()
                                .find(|candidate| candidate.id == *flag)
                                .ok_or_else(|| {
                                    replay_error(
                                        function,
                                        "conditional cleanup entry references unknown flag",
                                    )
                                })?;
                            if inventory.place.storage != entry.storage
                                || inventory.place.projections.first() != Some(&case.case)
                            {
                                return Err(replay_error(
                                    function,
                                    "conditional cleanup entry flag has a foreign case path",
                                ));
                            }
                            Ok(CleanupPlace {
                                storage: storage.clone(),
                                projections: inventory.place.projections.clone(),
                            })
                        })
                        .collect::<Result<Vec<_>, Diagnostic>>()?;
                    Ok(ConditionalVariantCase {
                        case: case.case.clone(),
                        live_places,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            Ok(ConditionalVariantEntry {
                storage,
                variant: entry.variant.clone(),
                cases,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    if plan.entry_state.conditional_owned_parameters != expected_conditional {
        return Err(replay_error(
            function,
            "conditional cleanup-plan entry disagrees with the independent inventory",
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

fn type_needs_drop(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    ty: &ResolvedType,
) -> Result<bool, Diagnostic> {
    crate::cleanup::type_needs_resource_cleanup(program, ty)
        .map_err(|message| replay_error(function, message))
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
        if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
            return Ok(crate::str_ops::resolved_params(op));
        }
        if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
            return Ok(crate::byte_ops::resolved_params(op));
        }
        if let Some(op) = crate::host_io_ops::by_id(callee.as_str()) {
            return Ok(crate::host_io_ops::resolved_params(op));
        }
        if let Some(op) = crate::command_io_ops::by_id(callee.as_str()) {
            return Ok(crate::command_io_ops::resolved_params(op));
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
            ResolvedExprKind::ByteRange { operation, .. } => {
                if operation.as_str() != crate::byte_ops::RANGE_ID {
                    return Err(replay_error(
                        function,
                        "byte range status source carries an unknown operation identity",
                    ));
                }
                statuses.push(StatusSource {
                    id: StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    },
                    producer: StatusProducer::PropagatedCall {
                        callee: operation.clone(),
                    },
                });
            }
            ResolvedExprKind::Call {
                callee, instance, ..
            } => {
                if instance.is_none()
                    && (crate::byte_ops::by_id(callee.as_str()).is_some()
                        || crate::host_io_ops::by_id(callee.as_str()).is_some())
                {
                    // Byte-data operations are total after HIR admission.
                    // Physical allocation failure is invariant fail-stop, not
                    // a recoverable operation status.
                    continue;
                }
                if instance.is_none()
                    && (crate::string_ops::by_id(callee.as_str()).is_some()
                        || crate::str_ops::by_id(callee.as_str()).is_some())
                {
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
            ResolvedExprKind::HostCommandCall(call) => {
                if crate::command_io_ops::failure(call.operation)
                    == crate::command_io_ops::CommandIoFailure::Status
                {
                    statuses.push(StatusSource {
                        id: StatusSourceId {
                            expression: expression.id.clone(),
                            lane: StatusLane::OperationFailure,
                        },
                        producer: StatusProducer::PropagatedCall {
                            callee: DeclarationId::new(crate::command_io_ops::id(call.operation)),
                        },
                    });
                }
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
            | ResolvedExprKind::Upcast { .. }
            | ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. } => {}
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
    enum Item<'a> {
        Shape(&'a FieldLivenessShape),
        Push(&'a DeclarationId),
        Pop,
    }

    let initial_projection_depth = projections.len();
    let mut pending = vec![Item::Shape(shape)];
    let mut work = 0usize;
    while let Some(item) = pending.pop() {
        work = work
            .checked_add(1)
            .ok_or_else(|| replay_error(function, "cleanup shape replay work overflowed"))?;
        if work > MAX_REPLAY_WORK_UNITS {
            return Err(replay_error(
                function,
                "cleanup shape replay exceeds the global work budget",
            ));
        }
        match item {
            Item::Push(projection) => {
                projections.try_reserve(1).map_err(|_| {
                    replay_error(
                        function,
                        "cleanup projection capacity exceeds address space",
                    )
                })?;
                projections.push(projection.clone());
            }
            Item::Pop => {
                if projections.len() == initial_projection_depth {
                    return Err(replay_error(
                        function,
                        "cleanup projection stack is unbalanced",
                    ));
                }
                projections.pop();
            }
            Item::Shape(FieldLivenessShape::NoDrop) => {}
            Item::Shape(FieldLivenessShape::Leaf { flag, lifecycle }) => {
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
            }
            Item::Shape(FieldLivenessShape::Record { fields, .. }) => {
                let mut field_ids = BTreeSet::new();
                for (index, field) in fields.iter().enumerate() {
                    let expected = u32_index(function, index, "cleanup field")?;
                    if field.field_index != expected || !field_ids.insert(field.field.clone()) {
                        return Err(replay_error(
                            function,
                            "record cleanup shape has non-contiguous or repeated fields",
                        ));
                    }
                }
                pending
                    .try_reserve(fields.len().saturating_mul(3))
                    .map_err(|_| {
                        replay_error(
                            function,
                            "cleanup shape replay capacity exceeds address space",
                        )
                    })?;
                for field in fields.iter().rev() {
                    pending.push(Item::Pop);
                    pending.push(Item::Shape(&field.shape));
                    pending.push(Item::Push(&field.field));
                }
            }
            Item::Shape(FieldLivenessShape::Variant { declaration, cases }) => {
                if !cases.is_empty()
                    && function
                        .cleanup_plan
                        .slots
                        .iter()
                        .find(|slot| slot.storage == *storage)
                        .and_then(|slot| match &slot.ty {
                            ResolvedType::Nominal { declaration, .. } => Some(declaration),
                            _ => None,
                        })
                        != Some(declaration)
                {
                    return Err(replay_error(
                        function,
                        "variant cleanup shape declaration disagrees with storage type",
                    ));
                }
                let mut case_ids = BTreeSet::new();
                let mut item_count = 0usize;
                for (case_index, case) in cases.iter().enumerate() {
                    if case.case_index != u32_index(function, case_index, "cleanup variant case")?
                        || !case_ids.insert(case.case.clone())
                    {
                        return Err(replay_error(
                            function,
                            "variant cleanup shape has non-contiguous or repeated cases",
                        ));
                    }
                    let mut field_ids = BTreeSet::new();
                    for (field_index, field) in case.fields.iter().enumerate() {
                        if field.field_index
                            != u32_index(function, field_index, "cleanup variant field")?
                            || !field_ids.insert(field.field.clone())
                        {
                            return Err(replay_error(
                                function,
                                "variant cleanup case has non-contiguous or repeated fields",
                            ));
                        }
                    }
                    item_count = item_count
                        .checked_add(2)
                        .and_then(|count| {
                            case.fields
                                .len()
                                .checked_mul(3)
                                .and_then(|fields| count.checked_add(fields))
                        })
                        .ok_or_else(|| {
                            replay_error(function, "cleanup shape replay work overflowed")
                        })?;
                }
                pending.try_reserve(item_count).map_err(|_| {
                    replay_error(
                        function,
                        "cleanup shape replay capacity exceeds address space",
                    )
                })?;
                for case in cases.iter().rev() {
                    pending.push(Item::Pop);
                    for field in case.fields.iter().rev() {
                        pending.push(Item::Pop);
                        pending.push(Item::Shape(&field.shape));
                        pending.push(Item::Push(&field.field));
                    }
                    pending.push(Item::Push(&case.case));
                }
            }
        }
    }
    if projections.len() != initial_projection_depth {
        return Err(replay_error(
            function,
            "cleanup projection stack remains unbalanced",
        ));
    }
    Ok(())
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
                CleanupTransition::AuthenticateVariantCase {
                    at,
                    source,
                    variant,
                    case,
                } => {
                    require_expression(function, expressions, at)?;
                    validate_place(function, source, storage, leaves)?;
                    let Some(cases) = program.declarations.variant_cases(variant) else {
                        return Err(replay_error(
                            function,
                            "variant authentication references a non-variant declaration",
                        ));
                    };
                    if !cases.iter().any(|candidate| candidate.id == *case) {
                        return Err(replay_error(
                            function,
                            "variant authentication references a foreign case",
                        ));
                    }
                    let slot = function
                        .cleanup_plan
                        .slots
                        .iter()
                        .find(|slot| slot.storage == source.storage)
                        .ok_or_else(|| {
                            replay_error(function, "variant authentication source has no slot")
                        })?;
                    if !matches!(
                        &slot.ty,
                        ResolvedType::Nominal { declaration, .. } if declaration == variant
                    ) {
                        return Err(replay_error(
                            function,
                            "variant authentication source type disagrees with its variant",
                        ));
                    }
                }
                CleanupTransition::TransferVariant {
                    at,
                    source,
                    destination,
                    variant,
                } => {
                    require_expression(function, expressions, at)?;
                    let source_flags = validate_place(function, source, storage, leaves)?;
                    let destination_flags = validate_place(function, destination, storage, leaves)?;
                    if source_flags.len() != destination_flags.len()
                        || function
                            .cleanup_plan
                            .slots
                            .iter()
                            .filter(|slot| {
                                slot.storage == source.storage
                                    || slot.storage == destination.storage
                            })
                            .any(|slot| {
                                !matches!(
                                    &slot.ty,
                                    ResolvedType::Nominal { declaration, .. }
                                        if declaration == variant
                                )
                            })
                    {
                        return Err(replay_error(
                            function,
                            "variant transfer has incompatible conditional inventories",
                        ));
                    }
                }
                CleanupTransition::InitializeVariant {
                    at,
                    destination,
                    variant,
                } => {
                    require_expression(function, expressions, at)?;
                    validate_place(function, destination, storage, leaves)?;
                    let slot = function
                        .cleanup_plan
                        .slots
                        .iter()
                        .find(|slot| slot.storage == destination.storage)
                        .ok_or_else(|| {
                            replay_error(function, "variant initialization has no destination slot")
                        })?;
                    if !destination.projections.is_empty()
                        || !matches!(
                            &slot.ty,
                            ResolvedType::Nominal { declaration, .. }
                                if declaration == variant
                        )
                        || !matches!(
                            &slot.field_liveness_shape,
                            FieldLivenessShape::Variant { declaration, .. }
                                if declaration == variant
                        )
                    {
                        return Err(replay_error(
                            function,
                            "variant initialization destination disagrees with its variant",
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
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always)
                    && !(matches!(
                        function.cleanup_plan.schema,
                        CLEANUP_PLAN_SCHEMA_V6
                            | CLEANUP_PLAN_SCHEMA_V7
                            | CLEANUP_PLAN_SCHEMA_V8
                            | CLEANUP_PLAN_SCHEMA_V9
                    ) && matches!(
                        plan.edges[edge.0 as usize].condition,
                        EdgeCondition::VariantCase { matches: true, .. }
                    ))
                {
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
            if let Some(condition) = &action.active_case {
                if condition.storage != action.source.storage
                    || action.source.projections.first() != Some(&condition.case)
                    || function
                        .cleanup_plan
                        .slots
                        .iter()
                        .find(|slot| slot.storage == condition.storage)
                        .is_none_or(|slot| {
                            !matches!(
                                &slot.ty,
                                ResolvedType::Nominal { declaration, .. }
                                    if declaration == &condition.variant
                            )
                        })
                {
                    return Err(replay_error(
                        function,
                        "conditional finalizer does not bind its exact variant case path",
                    ));
                }
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
                    if !matches!(
                        function.return_type,
                        ResolvedType::Nominal { .. } | ResolvedType::Bytes
                    ) || !type_needs_drop(program, function, &function.return_type)?
                        || result.storage != StorageId::ProvisionalResult
                        || !result.projections.is_empty()
                    {
                        return Err(replay_error(
                            function,
                            "owned result commit must publish the whole droppable provisional result",
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
        if std::env::var_os("SPX_PROBE_SKELETON").is_some() {
            for (index, path) in expected.iter().enumerate() {
                eprintln!("EXP[{index}] {path:?}");
            }
            for (index, path) in actual.iter().enumerate() {
                eprintln!("ACT[{index}] {path:?}");
            }
        }
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
        UpcastPassthrough,
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
        ByteRangeArgument {
            expression: &'a ResolvedExpr,
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
        WhileAfterCondition {
            prefixes: Vec<ExprSkeletonPath>,
            expression: &'a ResolvedExpr,
            statements: &'a [ResolvedStatement],
            tail: &'a ResolvedExpr,
            index: usize,
            condition: &'a ResolvedExpr,
            body: &'a ResolvedExpr,
        },
        WhileAfterBody {
            expression: &'a ResolvedExpr,
            statements: &'a [ResolvedStatement],
            tail: &'a ResolvedExpr,
            index: usize,
            results: Vec<ExprSkeletonPath>,
            true_prefixes: Vec<ExprSkeletonPath>,
            false_prefixes: Vec<ExprSkeletonPath>,
        },
        VariantField {
            expression: &'a ResolvedExpr,
            case: &'a DeclarationId,
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

    /// Continue one block's statement walk after a statement completes:
    /// advance to the next statement, fall through to the tail, or report
    /// settled paths when no active paths remain.
    #[allow(clippy::too_many_arguments)]
    fn advance_block_value<'a, 'e>(
        function: &'a ResolvedFunction,
        frames: &mut Vec<Frame<'e>>,
        work: &mut SkeletonWork<'_, '_>,
        expression: &'e ResolvedExpr,
        statements: &'e [ResolvedStatement],
        tail: &'e ResolvedExpr,
        index: usize,
        paths: Vec<ExprSkeletonPath>,
    ) -> Result<Option<Vec<ExprSkeletonPath>>, Diagnostic> {
        let mut push = |frames: &mut Vec<Frame<'e>>, frame: Frame<'e>| -> Result<(), Diagnostic> {
            if frames.len() == frames.capacity() {
                return Err(replay_error(
                    function,
                    "typed-HIR skeleton traversal exceeds the admitted depth",
                ));
            }
            work.charge(1, "typed-HIR skeleton continuation push")?;
            note_skeleton_materialization();
            frames.push(frame);
            Ok(())
        };
        let next = index + 1;
        if has_active_paths(&paths) && next < statements.len() {
            match &statements[next] {
                ResolvedStatement::While {
                    condition, body, ..
                } => {
                    // While statements route through their own continuation
                    // pair; the accumulated prefixes thread through the loop.
                    push(
                        frames,
                        Frame::WhileAfterCondition {
                            prefixes: paths,
                            expression,
                            statements,
                            tail,
                            index: next,
                            condition,
                            body,
                        },
                    )?;
                    push(frames, Frame::Eval(condition))?;
                }
                _ => {
                    push(
                        frames,
                        Frame::BlockValue {
                            expression,
                            statements,
                            tail,
                            index: next,
                            paths,
                        },
                    )?;
                    push(frames, Frame::Eval(statements[next].value()))?;
                }
            }
        } else if has_active_paths(&paths) {
            push(frames, Frame::BlockTail { expression, paths })?;
            push(frames, Frame::Eval(tail))?;
        } else {
            return Ok(Some(paths));
        }
        Ok(None)
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
                    | ResolvedExprKind::Usize(_)
                    | ResolvedExprKind::ArrayU8(_)
                    | ResolvedExprKind::RepeatArrayU8 { .. }
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
                    ResolvedExprKind::BorrowPlace { .. } => {
                        produced =
                            Some(work.singleton_path(empty_expr_path(), "borrow skeleton path")?);
                    }
                    ResolvedExprKind::ByteRange {
                        operation, source, ..
                    } => {
                        if operation.as_str() != crate::byte_ops::RANGE_ID {
                            return Err(replay_error(
                                function,
                                "byte range skeleton carries an unknown operation identity",
                            ));
                        }
                        work.charge(1, "byte-range skeleton root state")?;
                        push_frame!(
                            frames,
                            Frame::ByteRangeArgument {
                                expression,
                                index: 0,
                                states: vec![(empty_expr_path(), Vec::new())],
                            }
                        );
                        push_frame!(frames, Frame::Eval(source));
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
                    // Class Inheritance v1: the upcast is transparent to the
                    // skeleton; the consumed source's place stays the
                    // surrounding transfer's owned source.
                    ResolvedExprKind::Upcast { source } => {
                        push_frame!(frames, Frame::UpcastPassthrough);
                        push_frame!(frames, Frame::Eval(source));
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
                        let string_intrinsic = instance
                            .is_none()
                            .then(|| crate::string_ops::by_id(callee.as_str()))
                            .flatten();
                        let str_intrinsic = instance
                            .is_none()
                            .then(|| crate::str_ops::by_id(callee.as_str()))
                            .flatten();
                        let byte_intrinsic = instance
                            .is_none()
                            .then(|| crate::byte_ops::by_id(callee.as_str()))
                            .flatten();
                        let host_io_intrinsic = instance
                            .is_none()
                            .then(|| crate::host_io_ops::by_id(callee.as_str()))
                            .flatten();
                        let params = if let Some(op) = string_intrinsic {
                            crate::string_ops::resolved_params(op)
                        } else if let Some(op) = str_intrinsic {
                            crate::str_ops::resolved_params(op)
                        } else if let Some(op) = byte_intrinsic {
                            crate::byte_ops::resolved_params(op)
                        } else if let Some(op) = host_io_intrinsic {
                            crate::host_io_ops::resolved_params(op)
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
                    ResolvedExprKind::HostCommandCall(call) => {
                        let params = crate::command_io_ops::resolved_params(call.operation);
                        work.charge(1, "host-command skeleton root state")?;
                        let states = vec![(empty_expr_path(), Vec::new())];
                        if let Some(argument) = call.args.first() {
                            push_frame!(
                                frames,
                                Frame::CallArgument {
                                    expression,
                                    params,
                                    args: &call.args,
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
                    ResolvedExprKind::Block { statements, tail } => {
                        let paths = work.singleton_path(empty_expr_path(), "block root path")?;
                        if let Some(first_statement) = statements.first() {
                            match first_statement {
                                ResolvedStatement::While {
                                    condition, body, ..
                                } => {
                                    // While statements route through their
                                    // own condition/body continuation pair;
                                    // no per-statement value is produced.
                                    push_frame!(
                                        frames,
                                        Frame::WhileAfterCondition {
                                            prefixes: paths,
                                            expression,
                                            statements,
                                            tail,
                                            index: 0,
                                            condition,
                                            body,
                                        }
                                    );
                                    push_frame!(frames, Frame::Eval(condition));
                                }
                                _ => {
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
                                }
                            }
                        } else {
                            push_frame!(frames, Frame::BlockTail { expression, paths });
                            push_frame!(frames, Frame::Eval(tail));
                        }
                    }
                    ResolvedExprKind::ConstructVariant { case, fields, .. } => {
                        let paths = work
                            .singleton_path(empty_expr_path(), "variant-construction root path")?;
                        if let Some(field) = fields.first() {
                            push_frame!(
                                frames,
                                Frame::VariantField {
                                    expression,
                                    case,
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
                    ResolvedExprKind::Match {
                        scrutinee, arms, ..
                    } => {
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
                let argument = args
                    .get(index)
                    .ok_or_else(|| replay_error(function, "skeleton call arity is inconsistent"))?;
                let parameter = params
                    .get(index)
                    .ok_or_else(|| replay_error(function, "skeleton call arity is inconsistent"))?;
                let states = sequence_call_argument(
                    program, function, expression, parameter, argument, index, states, &suffixes,
                    work,
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
            Frame::ByteRangeArgument {
                expression,
                index,
                states,
            } => {
                let suffixes = produced.take().expect("byte-range argument path retained");
                let argument = replay_expression_child(expression, index).ok_or_else(|| {
                    replay_error(function, "byte range skeleton arity is inconsistent")
                })?;
                let params = crate::byte_ops::resolved_params(crate::byte_ops::ByteOp::Range);
                let parameter = params.get(index).ok_or_else(|| {
                    replay_error(function, "byte range parameter arity is inconsistent")
                })?;
                let states = sequence_call_argument(
                    program, function, expression, parameter, argument, index, states, &suffixes,
                    work,
                )?;
                let next = index + 1;
                if call_states_have_active(&states) && next < 3 {
                    let argument = replay_expression_child(expression, next).ok_or_else(|| {
                        replay_error(function, "byte range skeleton arity is inconsistent")
                    })?;
                    push_frame!(
                        frames,
                        Frame::ByteRangeArgument {
                            expression,
                            index: next,
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
                        if let Some(settled) = advance_block_value(
                            function,
                            &mut frames,
                            work,
                            expression,
                            statements,
                            tail,
                            index,
                            paths,
                        )? {
                            produced = Some(settled);
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
                        if let Some(settled) = advance_block_value(
                            function,
                            &mut frames,
                            work,
                            expression,
                            statements,
                            tail,
                            index,
                            paths,
                        )? {
                            produced = Some(settled);
                        }
                    }
                }
            }
            Frame::WhileAfterCondition {
                prefixes,
                expression,
                statements,
                tail,
                index,
                condition,
                body,
            } => {
                let condition_paths = produced.take().expect("while condition path retained");
                let prefixed = sequence_skeleton_paths(prefixes, &condition_paths, work)?;
                let (results, true_prefixes, false_prefixes) =
                    split_boolean_prefixes(prefixed, &condition.id, work)?;
                push_frame!(
                    frames,
                    Frame::WhileAfterBody {
                        expression,
                        statements,
                        tail,
                        index,
                        results,
                        true_prefixes,
                        false_prefixes,
                    }
                );
                push_frame!(frames, Frame::Eval(body));
            }
            Frame::WhileAfterBody {
                expression,
                statements,
                tail,
                index,
                mut results,
                true_prefixes,
                false_prefixes,
            } => {
                let body_paths = produced.take().expect("while body path retained");
                let joined = sequence_skeleton_paths(true_prefixes, &body_paths, work)?;
                // The skip branch joins the body branch at the loop
                // continuation without further observations.
                for path in joined.into_iter().chain(false_prefixes) {
                    work.push_expr_path(&mut results, path, "while join path")?;
                }
                if let Some(settled) = advance_block_value(
                    function,
                    &mut frames,
                    work,
                    expression,
                    statements,
                    tail,
                    index,
                    results,
                )? {
                    produced = Some(settled);
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
                expression,
                case,
                fields,
                index,
                paths,
            } => {
                let suffixes = produced.take().expect("variant field path retained");
                let mut paths = sequence_skeleton_paths(paths, &suffixes, work)?;
                let field = &fields[index];
                if has_active_paths(&paths)
                    && field.value.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &field.value.ty)?
                {
                    let mut destination = temporary_place(expression, work)?;
                    destination
                        .projections
                        .push(work.clone_owned(case, "variant case projection clone")?);
                    destination
                        .projections
                        .push(work.clone_owned(&field.field, "variant field projection clone")?);
                    let value_id =
                        work.clone_owned(&field.value.id, "variant field value clone")?;
                    paths = transfer_completed_paths(
                        function,
                        paths,
                        value_id,
                        destination,
                        "owned variant field",
                        work,
                    )?;
                }
                let next = index + 1;
                if has_active_paths(&paths) && next < fields.len() {
                    push_frame!(
                        frames,
                        Frame::VariantField {
                            expression,
                            case,
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
            // The upcast contributes no skeleton observations of its own.
            Frame::UpcastPassthrough => {}
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
                let mut scrutinee_paths = produced.take().expect("match scrutinee path retained");
                if !has_active_paths(&scrutinee_paths) {
                    produced = Some(scrutinee_paths);
                    continue;
                }
                // Refutable Match v1: scalar decision chains authenticate as
                // one ArmSelected observation per arm plus the guard's own
                // Boolean join; guards recurse as sub-skeletons.
                if matches!(
                    scrutinee.ty,
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::U8
                        | ResolvedType::Char
                        | ResolvedType::Bool
                ) {
                    produced = Some(finish_scalar_match_skeleton(
                        program,
                        function,
                        expression,
                        scrutinee,
                        arms,
                        scrutinee_paths,
                        work,
                    )?);
                    continue;
                }
                let is_record =
                    validate_match_skeleton_shape(program, function, expression, scrutinee, arms)?;
                if is_record {
                    let ResolvedExprKind::Match { mode, .. } = &expression.kind else {
                        unreachable!("match skeleton frame retained its expression")
                    };
                    if *mode != crate::hir::ResolvedMatchMode::Value {
                        let [arm] = arms else {
                            unreachable!("record shape validation checked one arm")
                        };
                        for path in &mut scrutinee_paths {
                            if path.failed || path.residual {
                                continue;
                            }
                            if *mode == crate::hir::ResolvedMatchMode::Own {
                                let source = path.owned_source.take().ok_or_else(|| {
                                    replay_error(
                                        function,
                                        "owned record match path has no owned source",
                                    )
                                })?;
                                let ResolvedMatchPattern::Record {
                                    record,
                                    instance,
                                    fields,
                                } = &arm.pattern
                                else {
                                    return Err(replay_error(
                                        function,
                                        "owned record match lacks an exact record pattern",
                                    ));
                                };
                                if record_destructure::contains_nested(fields) {
                                    let expected = record_destructure::replay(
                                        program, function, record, instance, fields, *mode,
                                    )?;
                                    if !expected.nested {
                                        return Err(replay_error(
                                            function,
                                            "recursive record destructure lost its nested shape",
                                        ));
                                    }
                                    for binding in expected.bindings {
                                        let mut field_source = source.clone();
                                        for field in binding.path {
                                            field_source.projections.push(field);
                                        }
                                        path.observations.push(SkeletonObservation::Transfer {
                                            at: expression.id.clone(),
                                            source: field_source,
                                            destination: CleanupPlace::whole(StorageId::Value(
                                                binding.binding,
                                            )),
                                        });
                                    }
                                    continue;
                                }
                                // Transfers follow the authenticated declaration
                                // inventory, independently of pattern spelling order.
                                // Derive the expected sequence here; never reorder
                                // the emitted plan to make it pass replay.
                                let declarations = program
                                    .declarations
                                    .record_fields(record)
                                    .ok_or_else(|| {
                                        replay_error(
                                            function,
                                            "owned record match has no field inventory",
                                        )
                                    })?;
                                for declaration in declarations {
                                    let field = fields
                                        .iter()
                                        .find(|field| field.field == declaration.id)
                                        .ok_or_else(|| {
                                            replay_error(
                                                function,
                                                "owned record pattern is incomplete",
                                            )
                                        })?;
                                    let ResolvedRecordMatchFieldPattern::Binding(binding) =
                                        &field.pattern
                                    else {
                                        continue;
                                    };
                                    if binding.ownership != crate::hir::OwnershipMode::Own {
                                        continue;
                                    }
                                    path.observations.push(SkeletonObservation::Transfer {
                                        at: expression.id.clone(),
                                        source: source.projected(field.field.clone()),
                                        destination: CleanupPlace::whole(StorageId::Value(
                                            binding.id.clone(),
                                        )),
                                    });
                                }
                            } else {
                                // A borrowed match observes a named owned or borrowed
                                // place without moving any cleanup epoch. The match
                                // expression itself cannot forward that place as an
                                // owned result.
                                path.owned_source = None;
                            }
                        }
                    }
                }
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
    let ResolvedExprKind::Match { mode, .. } = &expression.kind else {
        return Err(replay_error(
            function,
            "match skeleton received a non-match",
        ));
    };
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
        | ResolvedType::Usize
        | ResolvedType::ArrayU8(_)
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool
        | ResolvedType::String
        | ResolvedType::Bytes
        | ResolvedType::Str
        | ResolvedType::SliceU8
        | ResolvedType::TypeParameter { .. } => false,
    };
    let is_variant = match &scrutinee.ty {
        ResolvedType::Nominal { declaration, .. } => program
            .declarations
            .declaration(declaration)
            .is_some_and(|item| item.kind == DeclarationKind::Variant),
        _ => false,
    };
    if is_record {
        if type_needs_drop(program, function, &scrutinee.ty)? {
            if !matches!(
                mode,
                crate::hir::ResolvedMatchMode::Own | crate::hir::ResolvedMatchMode::Borrow
            ) {
                return Err(replay_error(
                    function,
                    "droppable record match lacks an explicit ownership mode",
                ));
            }
        } else if *mode != crate::hir::ResolvedMatchMode::Value {
            return Err(replay_error(
                function,
                "Copy record match carries an explicit ownership mode",
            ));
        }
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
        let ResolvedMatchPattern::Record { record, fields, .. } = &arm.pattern else {
            if *mode != crate::hir::ResolvedMatchMode::Value {
                return Err(replay_error(
                    function,
                    "explicit record match requires one exact record pattern",
                ));
            }
            return Ok(is_record);
        };
        if *mode != crate::hir::ResolvedMatchMode::Value {
            if *mode == crate::hir::ResolvedMatchMode::Borrow
                && !matches!(
                    &scrutinee.kind,
                    ResolvedExprKind::Place(place) if place.projections.is_empty()
                )
            {
                return Err(replay_error(
                    function,
                    "borrowed record match scrutinee is not an unprojected named place",
                ));
            }
            let expected_scrutinee_ownership = match mode {
                crate::hir::ResolvedMatchMode::Own => {
                    if scrutinee.ownership != OwnershipMode::Own {
                        return Err(replay_error(
                            function,
                            "owned record match scrutinee is not owned",
                        ));
                    }
                    OwnershipMode::Own
                }
                crate::hir::ResolvedMatchMode::Borrow => {
                    if !matches!(
                        scrutinee.ownership,
                        OwnershipMode::Own | OwnershipMode::Borrow
                    ) {
                        return Err(replay_error(
                            function,
                            "borrowed record match scrutinee is neither owned nor borrowed",
                        ));
                    }
                    OwnershipMode::Borrow
                }
                crate::hir::ResolvedMatchMode::Value => unreachable!(),
            };
            let declarations = program.declarations.record_fields(record).ok_or_else(|| {
                replay_error(function, "explicit record match has no field inventory")
            })?;
            if record_destructure::contains_nested(fields) {
                let ResolvedMatchPattern::Record { instance, .. } = &arm.pattern else {
                    unreachable!("record pattern matched above")
                };
                let expected =
                    record_destructure::replay(program, function, record, instance, fields, *mode)?;
                if !expected.nested {
                    return Err(replay_error(
                        function,
                        "recursive record destructure replay lost its nested shape",
                    ));
                }
                return Ok(is_record);
            }
            for declaration in declarations {
                let field = fields
                    .iter()
                    .find(|field| field.field == declaration.id)
                    .ok_or_else(|| {
                        replay_error(function, "explicit record pattern is incomplete")
                    })?;
                let expected = if type_needs_drop(program, function, &declaration.ty)? {
                    expected_scrutinee_ownership
                } else {
                    OwnershipMode::Value
                };
                match &field.pattern {
                    ResolvedRecordMatchFieldPattern::Binding(binding)
                        if binding.ty == declaration.ty && binding.ownership == expected => {}
                    ResolvedRecordMatchFieldPattern::Wildcard
                        if *mode == crate::hir::ResolvedMatchMode::Borrow
                            || !type_needs_drop(program, function, &declaration.ty)? => {}
                    ResolvedRecordMatchFieldPattern::Binding(_) => {
                        return Err(replay_error(
                            function,
                            "explicit record binding ownership or type disagrees with its field",
                        ));
                    }
                    ResolvedRecordMatchFieldPattern::Wildcard
                    | ResolvedRecordMatchFieldPattern::Record { .. } => {
                        return Err(replay_error(
                            function,
                            "explicit record pattern does not bind a droppable direct field",
                        ));
                    }
                }
            }
        }
    } else if is_variant && type_needs_drop(program, function, &scrutinee.ty)? {
        if !matches!(
            mode,
            crate::hir::ResolvedMatchMode::Own | crate::hir::ResolvedMatchMode::Borrow
        ) {
            return Err(replay_error(
                function,
                "droppable variant match lacks an explicit ownership mode",
            ));
        }
        if *mode == crate::hir::ResolvedMatchMode::Borrow
            && !matches!(
                &scrutinee.kind,
                ResolvedExprKind::Place(place) if place.projections.is_empty()
            )
        {
            return Err(replay_error(
                function,
                "borrowed variant match scrutinee is not an unprojected named place",
            ));
        }
        if *mode == crate::hir::ResolvedMatchMode::Own && scrutinee.ownership != OwnershipMode::Own
        {
            return Err(replay_error(
                function,
                "owned variant match scrutinee is not owned",
            ));
        }
        for arm in arms {
            let ResolvedMatchPattern::Variant { fields, .. } = &arm.pattern else {
                return Err(replay_error(
                    function,
                    "explicit variant match requires exact case patterns",
                ));
            };
            for field in fields {
                let expected = if type_needs_drop(program, function, &field.binding.ty)? {
                    match mode {
                        crate::hir::ResolvedMatchMode::Own => OwnershipMode::Own,
                        crate::hir::ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
                        crate::hir::ResolvedMatchMode::Value => unreachable!(),
                    }
                } else {
                    OwnershipMode::Value
                };
                if field.binding.ownership != expected {
                    return Err(replay_error(
                        function,
                        "explicit variant binding ownership disagrees with its payload",
                    ));
                }
            }
        }
    } else if type_needs_drop(program, function, &scrutinee.ty)? {
        return Err(replay_error(
            function,
            "droppable non-record match reached the copy-only cleanup skeleton",
        ));
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
    let mode = match &expression.kind {
        ResolvedExprKind::Match { mode, .. } => *mode,
        _ => return Err(replay_error(function, "match arm has no match expression")),
    };
    for mut path in remaining {
        if path.failed || path.residual {
            work.push_expr_path(results, path, "match terminal scrutinee path")?;
            continue;
        }
        if is_record || index + 1 == arms.len() {
            if !is_record && mode != crate::hir::ResolvedMatchMode::Value {
                let ResolvedMatchPattern::Variant { case, .. } = &arms[index].pattern else {
                    return Err(replay_error(
                        function,
                        "explicit final variant arm has no exact case",
                    ));
                };
                let scrutinee =
                    work.clone_owned(&scrutinee.id, "final variant match scrutinee clone")?;
                let case = work.clone_owned(case, "final variant match case clone")?;
                work.push_observation(
                    &mut path,
                    SkeletonObservation::VariantCase {
                        scrutinee,
                        case,
                        matches: true,
                    },
                    "final variant match observation",
                )?;
            }
            prepare_selected_match_path(
                program,
                function,
                expression,
                &arms[index],
                &mut path,
                work,
            )?;
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
        prepare_selected_match_path(
            program,
            function,
            expression,
            &arms[index],
            &mut selected,
            work,
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

fn prepare_selected_match_path(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    arm: &ResolvedMatchArm,
    path: &mut ExprSkeletonPath,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(), Diagnostic> {
    let mode = match &expression.kind {
        ResolvedExprKind::Match { mode, .. } => *mode,
        _ => return Err(replay_error(function, "match arm has no match expression")),
    };
    if mode == crate::hir::ResolvedMatchMode::Own {
        if let ResolvedMatchPattern::Variant { case, fields, .. } = &arm.pattern {
            let source = path.owned_source.clone().ok_or_else(|| {
                replay_error(function, "owned variant match has no HIR cleanup source")
            })?;
            for field in fields {
                if !type_needs_drop(program, function, &field.binding.ty)? {
                    continue;
                }
                let mut field_source =
                    work.clone_owned(&source, "owned variant match source clone")?;
                field_source
                    .projections
                    .push(work.clone_owned(case, "owned variant match case clone")?);
                field_source
                    .projections
                    .push(work.clone_owned(&field.field, "owned variant match field clone")?);
                let destination = CleanupPlace::whole(StorageId::Value(
                    work.clone_owned(&field.binding.id, "owned variant binding clone")?,
                ));
                let at = work.clone_owned(&expression.id, "owned variant match identity clone")?;
                work.push_observation(
                    path,
                    SkeletonObservation::Transfer {
                        at,
                        source: field_source,
                        destination,
                    },
                    "owned variant binding transfer observation",
                )?;
            }
        }
    }
    path.owned_source = None;
    Ok(())
}

/// Refutable Match v1 skeleton expectations for a Copy-scalar match. Each
/// non-final arm contributes one `ArmSelected` pair (selected paths continue
/// through the optional guard's Boolean join; rejected paths — including
/// false guards — fall through), and the trailing catch-all arm consumes
/// everything unconditionally, mirroring the canonical builder exactly.
#[allow(clippy::too_many_arguments)]
fn finish_scalar_match_skeleton(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arms: &[ResolvedMatchArm],
    scrutinee_paths: Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    if type_needs_drop(program, function, &expression.ty)? {
        return Err(replay_error(
            function,
            "droppable refutable-match result reached the copy-only cleanup skeleton",
        ));
    }
    let last = arms
        .last()
        .ok_or_else(|| replay_error(function, "refutable match has no arms"))?;
    let catch_all = matches!(
        &last.pattern,
        ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Binding(_)
    );
    if !catch_all || last.guard.is_some() {
        return Err(replay_error(
            function,
            "resolved refutable match lacks a trailing irrefutable guard-free catch-all",
        ));
    }

    let mut results: Vec<ExprSkeletonPath> = Vec::new();
    let mut remaining = scrutinee_paths;
    for (index, arm) in arms.iter().enumerate() {
        let final_arm = index + 1 == arms.len();

        // Terminal paths bypass the rest of the decision chain.
        let mut active = Vec::new();
        for path in std::mem::take(&mut remaining) {
            if path.failed || path.residual {
                results.push(path);
            } else {
                active.push(path);
            }
        }

        let mut selected_paths: Vec<ExprSkeletonPath> = Vec::new();
        let mut rejected_paths: Vec<ExprSkeletonPath> = Vec::new();
        if final_arm {
            selected_paths = active;
        } else {
            let arm_index = u32::try_from(index)
                .map_err(|_| replay_error(function, "too many scalar match arms"))?;
            for path in active {
                let mut selected = work.clone_expr_path(&path, "scalar match selected clone")?;
                let scrutinee_id =
                    work.clone_owned(&scrutinee.id, "scalar match selected scrutinee clone")?;
                work.push_observation(
                    &mut selected,
                    SkeletonObservation::ArmSelected {
                        scrutinee: scrutinee_id,
                        arm: arm_index,
                        selected: true,
                    },
                    "scalar match selected observation",
                )?;
                selected_paths.push(selected);

                let mut rejected = path;
                let scrutinee_id =
                    work.clone_owned(&scrutinee.id, "scalar match rejected scrutinee clone")?;
                work.push_observation(
                    &mut rejected,
                    SkeletonObservation::ArmSelected {
                        scrutinee: scrutinee_id,
                        arm: arm_index,
                        selected: false,
                    },
                    "scalar match rejected observation",
                )?;
                rejected_paths.push(rejected);
            }
        }

        if let Some(guard) = &arm.guard {
            let guard_paths = expression_skeleton(program, function, guard.as_ref(), work)?;
            let (terminal, when_true, when_false) =
                split_boolean_prefixes_at(guard_paths, &guard.id, work)?;
            // The true continuation consumes the shared prefixes; false and
            // terminal continuations clone them.
            let mut with_false = Vec::new();
            for prefix in &selected_paths {
                let cloned = clone_expr_path_shallow(prefix, work)?;
                let prefixed = sequence_skeleton_paths(
                    work.singleton_path(cloned, "guard-false prefix")?,
                    &when_false,
                    work,
                )?;
                with_false.extend(prefixed);
            }
            let mut with_terminal = Vec::new();
            for prefix in &selected_paths {
                let cloned = clone_expr_path_shallow(prefix, work)?;
                let prefixed = sequence_skeleton_paths(
                    work.singleton_path(cloned, "guard-terminal prefix")?,
                    &terminal,
                    work,
                )?;
                with_terminal.extend(prefixed);
            }
            let with_true = sequence_skeleton_paths(selected_paths, &when_true, work)?;
            // False guards and failed guard evaluations fall through.
            rejected_paths.extend(with_false);
            rejected_paths.extend(with_terminal);
            remaining.extend(rejected_paths);
            for selected in with_true {
                append_scalar_arm_value(
                    program,
                    function,
                    expression,
                    arm,
                    selected,
                    &mut results,
                    work,
                )?;
            }
        } else {
            remaining.extend(rejected_paths);
            for selected in selected_paths {
                append_scalar_arm_value(
                    program,
                    function,
                    expression,
                    arm,
                    selected,
                    &mut results,
                    work,
                )?;
            }
        }
    }
    for path in remaining {
        results.push(path);
    }
    Ok(results)
}

/// Sequences one selected-arm prefix with its arm-value sub-skeleton and
/// publishes the conditional-result transfer for owned values.
#[allow(clippy::too_many_arguments)]
fn append_scalar_arm_value(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    arm: &ResolvedMatchArm,
    selected_prefix: ExprSkeletonPath,
    results: &mut Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(), Diagnostic> {
    let value_paths = expression_skeleton(program, function, &arm.value, work)?;
    let sequenced = sequence_skeleton_paths(
        work.singleton_path(selected_prefix, "scalar match selected prefix")?,
        &value_paths,
        work,
    )?;
    let finished = finish_conditional_result(program, function, expression, sequenced, work)?;
    append_expr_paths(results, finished, work, "scalar match arm result")?;
    Ok(())
}

fn clone_expr_path_shallow(
    path: &ExprSkeletonPath,
    _work: &mut SkeletonWork<'_, '_>,
) -> Result<ExprSkeletonPath, Diagnostic> {
    Ok(path.clone())
}

#[allow(clippy::type_complexity)]
fn split_boolean_prefixes_at(
    paths: Vec<ExprSkeletonPath>,
    expression: &ExpressionId,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<
    (
        Vec<ExprSkeletonPath>,
        Vec<ExprSkeletonPath>,
        Vec<ExprSkeletonPath>,
    ),
    Diagnostic,
> {
    let mut terminal = Vec::new();
    let mut true_paths = Vec::new();
    let mut false_paths = Vec::new();
    for mut path in paths {
        if path.failed || path.residual {
            work.push_expr_path(&mut terminal, path, "guard terminal path")?;
            continue;
        }
        let mut when_true = work.clone_expr_path(&path, "guard true path clone")?;
        let true_expression = work.clone_owned(expression, "guard true expression clone")?;
        work.push_observation(
            &mut when_true,
            SkeletonObservation::Boolean {
                expression: true_expression,
                value: true,
            },
            "guard true observation",
        )?;
        true_paths.push(when_true);
        let false_expression = work.clone_owned(expression, "guard false expression clone")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::Boolean {
                expression: false_expression,
                value: false,
            },
            "guard false observation",
        )?;
        false_paths.push(path);
    }
    Ok((terminal, true_paths, false_paths))
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
    parameter: &crate::hir::ResolvedParam,
    argument: &ResolvedExpr,
    index: usize,
    states: Vec<CallSkeletonState>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<CallSkeletonState>, Diagnostic> {
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
    let infallible_compiler_operation = matches!(
        &expression.kind,
        ResolvedExprKind::Call {
            callee,
            instance: None,
            ..
        } if crate::byte_ops::by_id(callee.as_str()).is_some()
            || crate::host_io_ops::by_id(callee.as_str()).is_some()
    );
    let infallible_compiler_operation = infallible_compiler_operation
        || matches!(
            &expression.kind,
            ResolvedExprKind::HostCommandCall(call)
                if crate::command_io_ops::failure(call.operation)
                    == crate::command_io_ops::CommandIoFailure::Infallible
        );
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
        if infallible_compiler_operation {
            if expression.ownership == OwnershipMode::Own
                && type_needs_drop(program, function, &expression.ty)?
            {
                let destination = temporary_place(expression, work)?;
                let at = work.clone_owned(&expression.id, "byte result expression clone")?;
                let initialized =
                    work.clone_owned(&destination, "byte result destination clone")?;
                work.push_observation(
                    &mut path,
                    SkeletonObservation::Initialize {
                        at,
                        destination: initialized,
                    },
                    "owned infallible byte result initialization",
                )?;
                path.owned_source = Some(destination);
            } else {
                path.owned_source = None;
            }
            work.push_expr_path(&mut results, path, "infallible byte call path")?;
            continue;
        }
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
                CleanupTransition::AuthenticateVariantCase { .. } => {}
                CleanupTransition::InitializeVariant {
                    at, destination, ..
                } => {
                    let at =
                        skeleton_clone(budget, function, at, "plan variant initialize identity")?;
                    let destination = skeleton_clone(
                        budget,
                        function,
                        destination,
                        "plan variant initialize destination",
                    )?;
                    skeleton_push(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::Initialize { at, destination },
                        "plan variant initialize observation push",
                    )?;
                }
                CleanupTransition::TransferVariant {
                    at,
                    source,
                    destination,
                    ..
                } => {
                    let at =
                        skeleton_clone(budget, function, at, "plan variant transfer identity")?;
                    let source =
                        skeleton_clone(budget, function, source, "plan variant transfer source")?;
                    let destination = skeleton_clone(
                        budget,
                        function,
                        destination,
                        "plan variant transfer destination",
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
                        "plan variant transfer observation push",
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
                let edge = &plan.edges[edge.0 as usize];
                if let EdgeCondition::VariantCase {
                    scrutinee,
                    case,
                    matches,
                } = &edge.condition
                {
                    let scrutinee = skeleton_clone(
                        budget,
                        function,
                        scrutinee,
                        "plan final variant scrutinee clone",
                    )?;
                    let case =
                        skeleton_clone(budget, function, case, "plan final variant case clone")?;
                    skeleton_push(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::VariantCase {
                            scrutinee,
                            case,
                            matches: *matches,
                        },
                        "plan final variant observation push",
                    )?;
                }
                skeleton_queue_push(
                    budget,
                    function,
                    &mut queue,
                    (edge.to, observations),
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
                        EdgeCondition::ArmSelected {
                            scrutinee,
                            arm,
                            selected,
                        } => SkeletonObservation::ArmSelected {
                            scrutinee: skeleton_clone(
                                budget,
                                function,
                                scrutinee,
                                "plan scalar-match scrutinee clone",
                            )?,
                            arm: *arm,
                            selected: *selected,
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
    program: &ResolvedProgram,
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
        conditional_variants: Vec::new(),
        pending_failure: None,
        selected_failure: None,
        staged_copy_result: None,
        published: false,
    };
    for place in &plan.entry_state.live_owned_parameters {
        let flags = validate_place(function, place, storage, leaves)?;
        append_dead_flags(function, &mut initial, flags, "entry state")?;
    }
    for entry in &plan.entry_state.conditional_owned_parameters {
        let mut cases = Vec::with_capacity(entry.cases.len());
        for case in &entry.cases {
            let mut flags = Vec::with_capacity(case.live_places.len());
            for place in &case.live_places {
                let under = validate_place(function, place, storage, leaves)?;
                if under.len() != 1 || place.projections.first() != Some(&case.case) {
                    return Err(replay_error(
                        function,
                        "conditional entry case does not name exact case-qualified leaves",
                    ));
                }
                flags.push(under[0]);
            }
            cases.push((case.case.clone(), flags));
        }
        initial.conditional_variants.push(ReplayConditionalVariant {
            root: CleanupPlace {
                storage: entry.storage.clone(),
                projections: Vec::new(),
            },
            variant: entry.variant.clone(),
            cases,
        });
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
            execute_replay_transition(program, function, transition, &mut state, storage, leaves)?;
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                require_normal_flow_state(function, &state, block_id)?;
                let edge = &plan.edges[edge.0 as usize];
                let state = state_for_edge(function, state, &edge.condition, &contract_sources)?;
                queue.push_back((edge.to, state));
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
    program: &ResolvedProgram,
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
        CleanupTransition::TransferVariant {
            source,
            destination,
            variant,
            ..
        } => {
            if !state
                .conditional_variants
                .iter()
                .any(|conditional| conditional.root == *source)
            {
                materialize_constructed_variant(
                    program, function, state, source, variant, storage, leaves,
                )?;
            }
            replay_transfer(function, state, source, destination, storage, leaves)?;
        }
        CleanupTransition::InitializeVariant {
            destination,
            variant,
            ..
        } => {
            let flags = validate_place(function, destination, storage, leaves)?;
            if flags.iter().any(|flag| {
                state.live_order.contains(flag)
                    || state.conditional_variants.iter().any(|conditional| {
                        conditional
                            .cases
                            .iter()
                            .any(|(_, case_flags)| case_flags.contains(flag))
                    })
            }) {
                return Err(replay_error(
                    function,
                    "variant initialization targets a live cleanup place",
                ));
            }
            let slot = function
                .cleanup_plan
                .slots
                .iter()
                .find(|slot| slot.storage == destination.storage)
                .ok_or_else(|| replay_error(function, "variant initialization has no slot"))?;
            let FieldLivenessShape::Variant { cases, .. } = &slot.field_liveness_shape else {
                return Err(replay_error(
                    function,
                    "variant initialization destination is not conditional storage",
                ));
            };
            let conditional_cases = cases
                .iter()
                .map(|case| {
                    let prefix = destination
                        .projections
                        .iter()
                        .chain(std::iter::once(&case.case))
                        .cloned()
                        .collect::<Vec<_>>();
                    let case_flags = flags
                        .iter()
                        .filter(|flag| leaves[flag].place.projections.starts_with(&prefix))
                        .copied()
                        .collect::<Vec<_>>();
                    (case.case.clone(), case_flags)
                })
                .collect();
            state.conditional_variants.push(ReplayConditionalVariant {
                root: destination.clone(),
                variant: variant.clone(),
                cases: conditional_cases,
            });
        }
        CleanupTransition::AuthenticateVariantCase {
            source,
            variant,
            case,
            ..
        } => {
            if let Some(index) = state
                .conditional_variants
                .iter()
                .position(|candidate| candidate.root == *source && candidate.variant == *variant)
            {
                let conditional = state.conditional_variants.remove(index);
                let flags = conditional
                    .cases
                    .into_iter()
                    .find_map(|(candidate, flags)| (candidate == *case).then_some(flags))
                    .ok_or_else(|| {
                        replay_error(function, "conditional state omits authenticated case")
                    })?;
                // A valid selected case may carry only Copy fields. Its
                // authenticated case state is consumed even though there are
                // no cleanup flags to materialize.
                if !flags.is_empty() {
                    append_dead_flags(function, state, flags, "variant authentication")?;
                }
            }
            let prefix = source
                .projections
                .iter()
                .chain(std::iter::once(case))
                .cloned()
                .collect::<Vec<_>>();
            if validate_place(function, source, storage, leaves)?
                .iter()
                .any(|flag| {
                    state.live_order.contains(flag)
                        && !leaves[flag].place.projections.starts_with(&prefix)
                })
            {
                return Err(replay_error(
                    function,
                    "variant authentication retains a live inactive-case payload",
                ));
            }
        }
        CleanupTransition::CallCommit { call, arguments } => {
            let mut consumed = BTreeSet::new();
            let mut consumed_conditional_roots = BTreeSet::new();
            for argument in arguments {
                if let Some(index) = state
                    .conditional_variants
                    .iter()
                    .position(|variant| variant.root == argument.source)
                {
                    if !consumed_conditional_roots.insert(argument.source.clone()) {
                        return Err(replay_error(
                            function,
                            format!("call `{call}` atomically consumes a variant epoch twice"),
                        ));
                    }
                    let conditional = state.conditional_variants.remove(index);
                    for flag in conditional.cases.into_iter().flat_map(|(_, flags)| flags) {
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
                    continue;
                }
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
            if !state.live_order.is_empty() || !state.conditional_variants.is_empty() {
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
    if let Some(index) = state
        .conditional_variants
        .iter()
        .position(|variant| variant.root == *source)
    {
        let mapping = transfer_mapping(function, source, destination, leaves)?;
        let variant = state.conditional_variants.remove(index);
        let cases = variant
            .cases
            .into_iter()
            .map(|(case, flags)| {
                flags
                    .into_iter()
                    .map(|flag| {
                        mapping.get(&flag).copied().ok_or_else(|| {
                            replay_error(function, "conditional cleanup transfer omits a case leaf")
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()
                    .map(|flags| (case, flags))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        state.conditional_variants.push(ReplayConditionalVariant {
            root: destination.clone(),
            variant: variant.variant,
            cases,
        });
        return Ok(());
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
    // A whole-source transfer is a completed aggregate boundary, authenticated
    // by the typed control skeleton. Filling every owned destination field is
    // not: later Copy initializers can still fail, so preserve their preceding
    // field-initialization history until the constructor actually completes.
    let source_history = if source.projections.is_empty() {
        source_flags
    } else {
        state
            .live_order
            .iter()
            .filter(|flag| source_set.contains(flag))
            .copied()
            .collect::<Vec<_>>()
    };
    state.live_order.retain(|flag| !source_set.contains(flag));
    if destination.projections.is_empty() {
        state.live_order.extend(destination_flags);
    } else {
        for source_flag in source_history {
            state.live_order.push(mapping[&source_flag]);
        }
    }
    Ok(())
}

fn materialize_constructed_variant(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    state: &mut PathState,
    source: &CleanupPlace,
    variant: &DeclarationId,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), Diagnostic> {
    let StorageId::Temporary(expression) = &source.storage else {
        return Err(replay_error(
            function,
            "variant transfer has no authenticated conditional source state",
        ));
    };
    let expression = find_resolved_expression(function, expression).ok_or_else(|| {
        replay_error(
            function,
            "variant transfer source temporary has no typed-HIR expression",
        )
    })?;
    let ResolvedExprKind::ConstructVariant {
        variant: constructed_variant,
        case,
        ..
    } = &expression.kind
    else {
        return Err(replay_error(
            function,
            "variant transfer lacks a conditional or constructed source",
        ));
    };
    if constructed_variant != variant
        || program
            .declarations
            .variant_cases(variant)
            .is_none_or(|cases| !cases.iter().any(|candidate| candidate.id == *case))
    {
        return Err(replay_error(
            function,
            "constructed variant transfer has unauthenticated case metadata",
        ));
    }
    let all_flags = validate_place(function, source, storage, leaves)?;
    let prefix = source
        .projections
        .iter()
        .chain(std::iter::once(case))
        .cloned()
        .collect::<Vec<_>>();
    let selected = all_flags
        .iter()
        .filter(|flag| leaves[flag].place.projections.starts_with(&prefix))
        .copied()
        .collect::<Vec<_>>();
    if selected.iter().any(|flag| !state.live_order.contains(flag))
        || all_flags.iter().any(|flag| {
            state.live_order.contains(flag) && !leaves[flag].place.projections.starts_with(&prefix)
        })
    {
        return Err(replay_error(
            function,
            "constructed variant transfer has incomplete or inactive payload liveness",
        ));
    }
    let selected_set = selected.iter().copied().collect::<BTreeSet<_>>();
    state.live_order.retain(|flag| !selected_set.contains(flag));
    state.conditional_variants.push(ReplayConditionalVariant {
        root: source.clone(),
        variant: variant.clone(),
        cases: vec![(case.clone(), selected)],
    });
    Ok(())
}

fn find_resolved_expression<'a>(
    function: &'a ResolvedFunction,
    identity: &ExpressionId,
) -> Option<&'a ResolvedExpr> {
    let mut stack = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    while let Some(expression) = stack.pop() {
        if expression.id == *identity {
            return Some(expression);
        }
        let mut index = 0;
        while let Some(child) = replay_expression_child(expression, index) {
            stack.push(child);
            index += 1;
        }
    }
    None
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

fn append_dead_flags(
    function: &ResolvedFunction,
    state: &mut PathState,
    flags: Vec<LivenessFlagId>,
    operation: &str,
) -> Result<(), Diagnostic> {
    if flags.is_empty()
        || flags.iter().any(|flag| {
            state.live_order.contains(flag)
                || state.conditional_variants.iter().any(|variant| {
                    variant
                        .cases
                        .iter()
                        .any(|(_, case_flags)| case_flags.contains(flag))
                })
        })
    {
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
        | EdgeCondition::ArmSelected { .. }
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
    let mut expected_conditional = Vec::new();
    for variant in state.conditional_variants.iter().rev() {
        let region = storage_regions[&variant.root.storage];
        if !leaving.contains(&region) || variant.root.storage == StorageId::ProvisionalResult {
            continue;
        }
        for (case, flags) in variant.cases.iter().rev() {
            for flag in flags.iter().rev() {
                expected_conditional.push((
                    *flag,
                    super::VariantCaseGuard {
                        storage: variant.root.storage.clone(),
                        variant: variant.variant.clone(),
                        case: case.clone(),
                    },
                ));
            }
        }
    }
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
    let actual_conditional = exit
        .finalize_in_order
        .iter()
        .filter_map(|action| {
            action
                .active_case
                .as_ref()
                .map(|condition| (action.guard_flag, condition.clone()))
        })
        .collect::<Vec<_>>();
    if actual_conditional != expected_conditional {
        return Err(replay_error(
            function,
            "exit does not preserve exact conditional variant finalizers",
        ));
    }
    let finalized = expected.into_iter().collect::<BTreeSet<_>>();
    state.live_order.retain(|flag| !finalized.contains(flag));
    state.conditional_variants.retain(|variant| {
        !leaving.contains(&storage_regions[&variant.root.storage])
            || variant.root.storage == StorageId::ProvisionalResult
    });
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
                || !state.conditional_variants.is_empty()
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
                    if !state.live_order.is_empty() || !state.conditional_variants.is_empty() {
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
                    if let Some(index) = state
                        .conditional_variants
                        .iter()
                        .position(|variant| variant.root == *result)
                    {
                        if !state.live_order.is_empty() || state.conditional_variants.len() != 1 {
                            return Err(replay_error(
                                function,
                                "conditional owned result retains non-result ownership",
                            ));
                        }
                        state.conditional_variants.remove(index);
                    } else {
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
            }
            state.published = true;
            if !state.live_order.is_empty() || !state.conditional_variants.is_empty() {
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
                || !state.conditional_variants.is_empty()
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
        ResolvedExprKind::HostCommandCall(call) => call.args.get(index),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => [source.as_ref(), start.as_ref(), end.as_ref()]
            .get(index)
            .copied(),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => (index == 0).then_some(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            [left.as_ref(), right.as_ref()].get(index).copied()
        }
        ResolvedExprKind::Block { statements, tail } => {
            let mut offset = 0;
            for statement in statements {
                let count = statement.child_count();
                if index < offset + count {
                    return statement.child(index - offset);
                }
                offset += count;
            }
            (index == offset).then_some(tail)
        }
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
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            if index == 0 {
                Some(scrutinee.as_ref())
            } else {
                // Refutable Match v1: each arm contributes its optional
                // guard first, then its value.
                let mut cursor = index - 1;
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        if cursor == 0 {
                            return Some(guard.as_ref());
                        }
                        cursor -= 1;
                    }
                    if cursor == 0 {
                        return Some(&arm.value);
                    }
                    cursor -= 1;
                }
                None
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => (index == 0)
            .then_some(base.as_ref())
            .or_else(|| fields.get(index - 1).map(|field| &field.value)),
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => None,
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

fn expression_has_byte_range(expression: &ResolvedExpr) -> bool {
    expression_has_kind(expression, |kind| {
        matches!(kind, ResolvedExprKind::ByteRange { .. })
    })
}

fn expression_has_explicit_record_match(expression: &ResolvedExpr) -> bool {
    expression_has_kind(expression, |kind| {
        matches!(
            kind,
            ResolvedExprKind::Match {
                mode: crate::hir::ResolvedMatchMode::Own | crate::hir::ResolvedMatchMode::Borrow,
                arms,
                ..
            }
                if arms.iter().any(|arm| matches!(arm.pattern, ResolvedMatchPattern::Record { .. }))
        )
    })
}

fn expression_has_nested_record_destructure(expression: &ResolvedExpr) -> bool {
    expression_has_kind(expression, |kind| {
        matches!(
            kind,
            ResolvedExprKind::Match {
                mode: crate::hir::ResolvedMatchMode::Own | crate::hir::ResolvedMatchMode::Borrow,
                arms,
                ..
            } if arms.iter().any(|arm| matches!(
                &arm.pattern,
                ResolvedMatchPattern::Record { fields, .. }
                    if record_destructure::contains_nested(fields)
            ))
        )
    })
}

fn expression_has_explicit_variant_match(expression: &ResolvedExpr) -> bool {
    expression_has_kind(expression, |kind| {
        matches!(
            kind,
            ResolvedExprKind::Match {
                mode: crate::hir::ResolvedMatchMode::Own | crate::hir::ResolvedMatchMode::Borrow,
                arms,
                ..
            }
                if arms.iter().any(|arm| matches!(arm.pattern, ResolvedMatchPattern::Variant { .. }))
        )
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
            | EdgeCondition::VariantCase { .. }
            | EdgeCondition::ArmSelected { .. } => {}
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
        EdgeCondition::ArmSelected { scrutinee, .. } => {
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
        (
            EdgeCondition::ArmSelected {
                scrutinee: a_scrutinee,
                arm: a_arm,
                selected: a_selected,
            },
            EdgeCondition::ArmSelected {
                scrutinee: b_scrutinee,
                arm: b_arm,
                selected: b_selected,
            },
        ) => a_scrutinee == b_scrutinee && a_arm == b_arm && a_selected != b_selected,
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
                ResolvedExprKind::HostCommandCall(call) => Some(CallFact {
                    callee: DeclarationId::new(crate::command_io_ops::id(call.operation)),
                    instance: None,
                    arguments: call
                        .args
                        .iter()
                        .map(|argument| argument.id.clone())
                        .collect(),
                }),
                ResolvedExprKind::ByteRange {
                    operation,
                    source,
                    start,
                    end,
                } => Some(CallFact {
                    callee: operation.clone(),
                    instance: None,
                    arguments: [source.as_ref(), start.as_ref(), end.as_ref()]
                        .into_iter()
                        .map(|argument| argument.id.clone())
                        .collect(),
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
#[path = "replay_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "nested_owned_records_tests.rs"]
mod nested_owned_records_tests;

#[cfg(test)]
#[path = "nested_record_destructure_tests.rs"]
mod nested_record_destructure_tests;
