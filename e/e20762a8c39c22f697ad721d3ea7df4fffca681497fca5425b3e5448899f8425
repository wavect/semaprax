//! Independent structural validation of attached cleanup plans without invoking the builder.
// Private prerequisite only: no selected replay admission hook.
mod branch_clone;
mod compact_stage;
mod constructed_variant;
#[cfg(test)]
mod context_closure;
mod continuation_storage;
mod final_output;
mod path_array;
use constructed_variant::materialize_constructed_variant;
pub(in crate::cleanup_plan) mod allocation_coverage;
mod allocation_paths;
mod call_paths;
mod call_payload;
mod expression_children;
mod expression_facts;
mod flat_records;
use call_paths::finish_call_states;
mod inventory_coverage;
pub(in crate::cleanup_plan) mod ordinary_status;
mod physical_budget;
mod physical_clone;
mod physical_storage;
mod scalar_match;
mod scalar_replay;
#[cfg(test)]
#[allow(
    dead_code,
    reason = "complete independently compiled original Tree scalar receiver"
)]
mod scalar_replay_tree;
mod selected_callees;
mod selected_context;
mod selected_entry;
mod shape_cursor;
mod status_construction;
mod structural_control;
mod structural_maps;
#[cfg(test)]
pub(crate) use selected_context::borrowed::{
    Identity as ReplayBorrowedIdentity, Lease as ReplayBorrowedLease,
};
#[cfg(test)]
pub(crate) use selected_entry::validate_retained_lexical_with_limit;
#[cfg(test)]
pub(crate) use selected_entry::validate_retained_lexical_with_reservation;
#[cfg(test)]
pub(crate) use selected_entry::Session as SelectedReplaySession;
pub(crate) use selected_entry::{
    Acquired as SelectedReplayReport, PaidRefusal as SelectedReplayRefusal,
    Preparation as SelectedReplayPreparation, Refused as SelectedReplayRefused,
    ReportForm as SelectedReplayReportForm, Unpaid as SelectedReplayUnpaid,
};
#[cfg(test)]
pub(crate) use selected_entry::{
    Cause as SelectedReplayCause, UnpaidReason as SelectedReplayUnpaidReason,
};
mod selected_owner;
mod selected_profile;
mod semantic_refusal;
mod status_validation;
pub(crate) use physical_clone::ClonePayload;
#[cfg(test)]
pub(crate) use physical_storage::PhysicalRefusal as ReplayPhysicalRefusal;
pub(crate) use physical_storage::{
    btree_node_upper as replay_btree_node_upper, replay_reserve_vec, PhysicalReservation,
};
#[cfg(test)]
mod physical_observation_tests;
#[cfg(test)]
mod primitive_fixture;
use expression_children::replay_expression_child;
use expression_facts::expression_facts;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[cfg(test)]
use crate::hir::ResolvedTypeDeclarationKind;
#[cfg(test)]
use std::cell::Cell;

use super::{
    BlockId, CleanupPlace, CleanupRegionId, CleanupResultSource, CleanupTerminator,
    CleanupTransition, EdgeCondition, EdgeId, ExitContinuation, ExitTarget, StagedCopyResultSource,
    StatusCase, StatusLane, StatusProducer, StatusSource, StatusSourceId, StorageId,
    CLEANUP_PLAN_SCHEMA_V10, CLEANUP_PLAN_SCHEMA_V11, CLEANUP_PLAN_SCHEMA_V12,
    CLEANUP_PLAN_SCHEMA_V13, CLEANUP_PLAN_SCHEMA_V14, CLEANUP_PLAN_SCHEMA_V15,
    CLEANUP_PLAN_SCHEMA_V16, CLEANUP_PLAN_SCHEMA_V2, CLEANUP_PLAN_SCHEMA_V3,
    CLEANUP_PLAN_SCHEMA_V4, CLEANUP_PLAN_SCHEMA_V5, CLEANUP_PLAN_SCHEMA_V6, CLEANUP_PLAN_SCHEMA_V7,
    CLEANUP_PLAN_SCHEMA_V8, CLEANUP_PLAN_SCHEMA_V9,
};
#[cfg(test)]
use super::{ConditionalVariantCase, ConditionalVariantEntry};
use crate::ast::{BinaryOp, UnaryOp};
use crate::cleanup::ReplayContext;
use crate::cleanup::{CleanupStorageOrigin, FieldLivenessShape, LivenessFlagId};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, DeclarationKind, ExpressionId, FunctionInstanceId, IdentityOrigin,
    OwnershipMode, PlaceProjection, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedMatchArm, ResolvedMatchPattern, ResolvedProgram, ResolvedRecordMatchFieldPattern,
    ResolvedStatement, ResolvedType,
};
use crate::prelude;
mod factored;
mod leaf_index;
use leaf_index::Leaves;
mod path_summary;
mod skeleton_bound;
mod skeleton_work;
mod typed_storage;
use compact_stage::CopySource;
use path_summary::{
    cleanup_inert_large_decisions_can_be_summarized, cleanup_inert_path_product_can_be_summarized,
    or_arm_path_counts, plan_structure_units, status_only_paths_can_be_summarized,
    validate_replay_size_budget,
};
#[cfg(test)]
use path_summary::{cleanup_plan_requires_path_replay, STATUS_ONLY_PATH_SUMMARY_THRESHOLD};
use skeleton_work::{empty_expr_path, Observations, SkeletonWork};

#[cfg(test)]
mod context_tests;
#[cfg(test)]
mod copy_success_result_tests;
mod guarded_variant;
mod lexical_boundary;
mod lexical_census;
mod lexical_cert;
mod lexical_flow;
mod lexical_hir;
mod lexical_replay;
mod lexical_skeleton;
#[cfg(test)]
mod mixed_result_tests;
mod nested_shape;
mod ordinary_inventory;
mod ordinary_renewals;
mod paid_type_facts;
mod path_join;
mod renewal;
mod string_replacement;
mod strings;
mod transition;
mod transition_storage;
use renewal::validate_join_compatibility;
use strings::temporary_place;
use transition::{
    append_dead_flags, execute_replay_transition, find_resolved_expression, replay_exit,
    replay_transfer, require_normal_flow_state, state_for_edge, validate_staged_target, ExitFlow,
};
mod record_destructure;
mod resolved_call;
mod supplemental;
#[cfg(test)]
use supplemental::collect_supplemental_slots;
mod schema;
#[cfg(test)]
mod schema_tests;
#[cfg(test)]
use nested_shape::expected_shape_for_type;
use nested_shape::type_needs_drop;
use path_join::validate_path_states;
use record_destructure::finish_owned_match_result as finish_owned;
use resolved_call::{
    exact_owned_try, owned_try_residual_places, owned_try_residual_result_place,
    resolved_call_params, seal_changed_success_try_residual as seal_try_residual,
};
pub(crate) use schema::selected_schema;

/// Provisional normal-scope end of a region whose independently proven flow has
/// no normal completion. Only the lexical certificate replay may accept it.
const UNRESOLVED_NORMAL_END: super::ExitTargetId = super::ExitTargetId(u32::MAX);
const MAX_REPLAY_PATHS: usize = 65_536;
const MAX_REPLAY_WORK_UNITS: usize = 32_000_000;

mod allocation;
use allocation::{AdmissionMode, AllocationPayer};

struct ReplayBudget {
    admission: AdmissionMode,
    payer: AllocationPayer,
    remaining: usize,
    skeleton_remaining: usize,
    /// Actual successful skeleton debits, including deferred lexical walks.
    skeleton_spent: usize,
    /// Ordinary constructors share the temporarily lent skeleton allowance.
    lent_skeleton: bool,
    /// Factored replay: merge equal-state skeleton paths (`skeleton_work`).
    merge_paths: bool,
    /// Present only for the selected retained lexical replay entry.
    physical: Option<Box<physical_storage::PhysicalReservation>>,
    /// Prepaid selected-budget refusal text; consumed without a fresh allocation.
    refusal_text: Option<String>,
    refusal_spent: bool,
    /// One reusable fixed clone census cursor on the ordinary builder path.
    builder_clone_cursor_paid: bool,
}

impl ReplayBudget {
    fn charge(
        &mut self,
        function: &ResolvedFunction,
        units: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.prepare_builder(function)?;
        if self.refusal_spent {
            return Err(self.budget_refusal(function, "selected replay budget already failed", ""));
        }
        if self.lent_skeleton {
            return self.charge_skeleton(function, units, phase);
        }
        self.charge_oracle_work(function, units, phase)?;
        self.remaining = self.remaining.checked_sub(units).ok_or_else(|| {
            self.budget_refusal(
                function,
                "cleanup replay work budget exhausted during ",
                phase,
            )
        })?;
        Ok(())
    }

    fn reserve_skeleton(
        &mut self,
        function: &ResolvedFunction,
        units: usize,
    ) -> Result<(), Diagnostic> {
        self.prepare_builder(function)?;
        if self.refusal_spent {
            return Err(self.budget_refusal(function, "selected replay budget already failed", ""));
        }
        self.remaining = self.remaining.checked_sub(units).ok_or_else(|| {
            self.budget_refusal(
                function,
                "cleanup replay skeleton-work preflight exceeds the global budget",
                "",
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
        self.prepare_builder(function)?;
        if self.refusal_spent {
            return Err(self.budget_refusal(function, "selected replay budget already failed", ""));
        }
        self.charge_oracle_work(function, units, phase)?;
        let spent = self.skeleton_spent.checked_add(units).ok_or_else(|| {
            self.budget_refusal(
                function,
                "cleanup replay skeleton debit count overflowed",
                "",
            )
        })?;
        self.skeleton_remaining = self.skeleton_remaining.checked_sub(units).ok_or_else(|| {
            self.budget_refusal(
                function,
                "cleanup replay work budget exhausted during ",
                phase,
            )
        })?;
        self.skeleton_spent = spent;
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

#[track_caller]
fn skeleton_clone<T: Clone + ClonePayload>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    value: &T,
    phase: &str,
) -> Result<T, Diagnostic> {
    budget.charge_skeleton(function, 1, phase)?;
    budget.reserve_clone(function, value, phase)?;
    note_skeleton_materialization();
    Ok(value.clone())
}

#[track_caller]
fn skeleton_push<T>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    target: &mut Vec<T>,
    value: impl physical_clone::IntoReplay<T>,
    phase: &str,
) -> Result<(), Diagnostic> {
    budget.charge_skeleton(function, 1, phase)?;
    note_skeleton_materialization();
    budget.reserve_vec(function, target, 1, phase)?;
    budget.reserve_bytes(
        function,
        physical_clone::conversion_bytes::<T, _>(&value),
        phase,
    )?;
    target.push(value.into());
    Ok(())
}

#[track_caller]
fn skeleton_queue_push<T>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    target: &mut VecDeque<T>,
    value: T,
    phase: &str,
) -> Result<(), Diagnostic> {
    budget.charge_skeleton(function, 1, phase)?;
    note_skeleton_materialization();
    budget.reserve_deque(function, target, 1, phase)?;
    target.push_back(value);
    Ok(())
}

#[derive(Clone)]
struct Leaf {
    place: CleanupPlace,
    lifecycle: DeclarationId,
}

#[derive(Clone)]
struct CallFact<'hir> {
    callee: std::borrow::Cow<'hir, DeclarationId>,
    instance: Option<&'hir FunctionInstanceId>,
    arguments: call_payload::Arguments<'hir>,
    type_arguments: call_payload::Types<'hir>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PathState {
    renewals: BTreeMap<ExpressionId, super::renewal_history::RenewalHistory>,
    live_order: Vec<LivenessFlagId>,
    conditional_variants: Vec<ReplayConditionalVariant>,
    pending_failure: Option<StatusSourceId>,
    selected_failure: Option<StatusSourceId>,
    staged_copy_result: Option<StagedCopyResultSource>,
    published: bool,
    /// Result source a lexical `return` handed to its per-return postcondition
    /// domain. A commit in that domain must publish exactly this source.
    returned: Option<ReturnedSource>,
}

/// Orderable image of a `CleanupResultSource` carried through one return domain.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ReturnedSource {
    Scalar(ExpressionId),
    Owned(CleanupPlace),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ReplayConditionalVariant {
    root: CleanupPlace,
    variant: DeclarationId,
    cases: Vec<(DeclarationId, Vec<LivenessFlagId>)>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SkeletonObservation {
    ReserveRenewal {
        at: ExpressionId,
        binding: CleanupPlace,
    },
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
    StageCopyResult(CopySource),
    /// A lexical `return`, `break` or `continue` that left its contour.
    LexicalExit {
        site: ExpressionId,
        kind: crate::ast::LexicalExitKind,
        target: crate::hir::ControlTarget,
    },
    /// A finite iteration witness reached its authenticated runtime latch.
    LoopLatch {
        target: crate::hir::ControlTarget,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SkeletonTerminal {
    Success,
    Failure,
    /// Normal iteration end or `continue` handing off to the loop latch.
    LoopLatch,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SkeletonPath {
    observations: Observations,
    terminal: SkeletonTerminal,
}

#[derive(Clone)]
struct ExprSkeletonPath {
    observations: Observations,
    owned_source: Option<CleanupPlace>,
    failed: bool,
    residual: bool,
    continuation: lexical_skeleton::Continuation,
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
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget)?;
    validate_structure_with_budget(program, function, &mut budget, None)
}

pub(super) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    validate_program_context(program)
}

pub(in crate::cleanup_plan) fn validate_program_context(
    program: &dyn crate::cleanup::ReplayContext,
) -> Result<(), Diagnostic> {
    let full = program.full_program().ok_or_else(|| {
        Diagnostic::io(
            "SPX-H006",
            "allocation replay requires the held complete program",
        )
    })?;
    let mut budget = ReplayBudget::new();
    let helpers = ordinary_renewals::index(full, &mut budget)?;
    reserve_program_skeleton_work(
        program,
        program.functions().iter().chain(
            program
                .function_instances()
                .iter()
                .map(|instance| &instance.function),
        ),
        &mut budget,
    )?;
    let mut continuations = continuation_storage::Storage::for_program(full, &mut budget)?;
    for function in program.functions() {
        validate_structure_with_budget_storage(
            program,
            function,
            &mut budget,
            Some(&helpers),
            Some(&mut continuations),
        )?;
    }
    for instance in program.function_instances() {
        validate_structure_with_budget_storage(
            program,
            &instance.function,
            &mut budget,
            Some(&helpers),
            Some(&mut continuations),
        )?;
    }
    Ok(())
}

fn reserve_program_skeleton_work<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    functions: impl IntoIterator<Item = &'a ResolvedFunction>,
    budget: &mut ReplayBudget,
) -> Result<usize, Diagnostic> {
    let mut total = 0usize;
    let mut first = None;
    for function in functions {
        first.get_or_insert(function);
        // Actual lexical HIR selects a deferred resource plan before the
        // capped enumerating census. This is no proof or zero-work estimate.
        if factored::strategy::provisional(function)
            == factored::strategy::ResourceStrategy::DeferredLexical
        {
            continue;
        }
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
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<(), Diagnostic> {
    validate_structure_with_budget_storage(program, function, budget, helpers, None)
}
fn validate_structure_with_budget_storage<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    if helpers.is_some_and(|index| {
        !program
            .full_program()
            .is_some_and(|full| index.matches_program(full))
    }) {
        return Err(replay_error(
            function,
            "cleanup replay helper index belongs to a different typed program",
        ));
    }
    // Reconstruct the ordinary schema first, preserving all its current-source
    // probes/error order. A private source-held projection adds allocation
    // branches; neither its attached v20 string nor an ordinary context selects it.
    let ordinary_schema =
        schema::paid_summary::selected_with_budget(program, function, helpers, budget)?;
    let expected_schema = if allocation_paths::current(program, function).is_some() {
        super::CLEANUP_PLAN_SCHEMA_V20
    } else {
        ordinary_schema
    };
    if plan.schema != expected_schema {
        return Err(replay_error(
            function,
            format_args!(
                "uses cleanup-plan schema `{}` instead of HIR-derived `{expected_schema}`",
                plan.schema
            ),
        ));
    }

    if lexical_cert::requires_lexical(function) {
        return lexical_replay::validate_with_index(program, function, budget, helpers);
    }
    if plan.lexical_control.is_some() {
        return Err(replay_error(
            function,
            "function without lexical exits carries a lexical-control certificate",
        ));
    }
    validate_replay_size_budget(function)?;
    budget.charge(
        function,
        plan_structure_units(plan),
        "structural validation",
    )?;

    let expression_facts = expression_facts::with_budget(function, budget)?;
    ordinary_inventory::validate_inventory_coverage(program, function, budget)?;
    let status_cases =
        expression_facts::receiving::StatusCases::validate(program, function, budget)?;
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    let statuses = expression_facts::receiving::validate_status_sources_certified(
        function,
        &expression_facts,
        budget,
        &status_cases,
    )?;
    expression_facts::receiving::validate_regions(function, &storage, budget)?;
    expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;
    expression_facts::receiving::validate_blocks_and_edges_retained(
        program,
        function,
        &expression_facts,
        &storage,
        &leaves,
        helpers,
        budget,
        &statuses,
    )?;
    expression_facts::receiving::validate_exits_retained(
        program,
        function,
        &storage,
        &leaves,
        &BTreeSet::new(),
        budget,
        &statuses,
    )?;
    structural_maps::validate_reference_coverage(function, budget)?;
    structural_control::validate_ordinary_cfg(function, budget)?;
    if !cleanup_inert_path_product_can_be_summarized(function)?
        && !status_only_paths_can_be_summarized(function)
        && !cleanup_inert_large_decisions_can_be_summarized(function)
    {
        validate_path_states::<Diagnostic>(
            program, function, &storage, &leaves, budget, None, helpers,
        )?;
        validate_typed_control_skeleton_with_index(
            program,
            function,
            budget,
            helpers,
            continuations,
        )?;
    }
    Ok(())
}

struct CfgReplayBounds {
    terminal_paths: usize,
    work: usize,
    skeleton_work: usize,
}

mod cfg_bounds;
fn branch_sensitive_cfg_bounds(function: &ResolvedFunction) -> Result<CfgReplayBounds, Diagnostic> {
    // Preflight callers have no selected reservation. Their scratch uses the
    // ordinary active payer without granting any schema or replay authority.
    cfg_bounds::bounds(function, &mut ReplayBudget::new())
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
        if let Some((_, args)) = crate::hir::closure::once::call(expression) {
            return args.get(index);
        }
        match &expression.kind {
            ResolvedExprKind::LexicalExit { value, .. } => {
                (index == 0).then(|| value.as_deref()).flatten()
            }
            ResolvedExprKind::Closure { .. }
            | ResolvedExprKind::FunctionReference { .. }
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
            | ResolvedExprKind::BorrowedUtf8Literal(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::RowLoan { .. } => None,
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Upcast { source: value }
            | ResolvedExprKind::Yield { request: value }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. } => {
                (index == 0).then_some(value.as_ref())
            }
            ResolvedExprKind::Binary { left, right, .. } => {
                [left.as_ref(), right.as_ref()].get(index).copied()
            }
            ResolvedExprKind::Invoke { args, .. } => args.get(index),
            ResolvedExprKind::Call { args, .. }
            | ResolvedExprKind::LiteralFormat { args, .. }
            | ResolvedExprKind::VecFieldRead { args, .. } => args.get(index),
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
            ResolvedExprKind::Match { .. } => replay_expression_child(expression, index),
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
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
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
                    _ if crate::hir::closure::once::call(expression).is_some() => {
                        let accumulator = sequence(children);
                        HirPathCounts {
                            failed: accumulator.failed.saturating_add(accumulator.normal),
                            ..accumulator
                        }
                    }
                    ResolvedExprKind::Closure { .. }
                    | ResolvedExprKind::FunctionReference { .. }
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
                    | ResolvedExprKind::BorrowedUtf8Literal(_)
                    | ResolvedExprKind::Place(_)
                    | ResolvedExprKind::BorrowPlace { .. }
                    | ResolvedExprKind::RowLoan { .. } => HirPathCounts::ONE,
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
                    ResolvedExprKind::Upcast { .. }
                    | ResolvedExprKind::Yield { .. }
                    | ResolvedExprKind::Project { .. } => children[0],
                    // Lexical profile functions never reach the legacy census;
                    // a stray exit conservatively terminates its operand paths.
                    ResolvedExprKind::LexicalExit { .. } => {
                        let operand = children.first().copied().unwrap_or(HirPathCounts::ONE);
                        HirPathCounts {
                            normal: 0,
                            failed: operand.failed.saturating_add(operand.normal).max(1),
                            residual: operand.residual,
                        }
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
                    _ if super::deferred_commit::infallible_borrowed_vector_observer(
                        expression,
                    ) =>
                    {
                        sequence(children)
                    }
                    ResolvedExprKind::Invoke { .. }
                    | ResolvedExprKind::Call { .. }
                    | ResolvedExprKind::LiteralFormat { .. }
                    | ResolvedExprKind::VecFieldRead { .. }
                    | ResolvedExprKind::NativeRustImportCall(_) => {
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
                    ResolvedExprKind::Match { arms, .. }
                        if guarded_variant::selected(expression) =>
                    {
                        guarded_variant::census(arms, sequence(children))
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
                    ResolvedExprKind::Block { statements, .. }
                    | ResolvedExprKind::VecConstructor { statements, .. } => {
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
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<usize, Diagnostic> {
    if lexical_cert::requires_lexical(function) {
        return lexical_census::skeleton_work_upper(program, function);
    }
    if status_only_paths_can_be_summarized(function)
        || cleanup_inert_large_decisions_can_be_summarized(function)
    {
        return Ok(0);
    }
    let semantic_paths = hir_terminal_path_bound(function)?.max(1);
    if semantic_paths > MAX_REPLAY_PATHS {
        return Ok(0);
    }
    // Root path, four body-sequencing operations, and completed path;
    // an owned result also clones its identity and performs four transfers.
    let root_work = if type_needs_drop(program, function, &function.return_type)? {
        11
    } else {
        6
    };
    let mut hir_upper = checked_skeleton_mul(function, root_work, semantic_paths)?;
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

    let mut cfg_budget = ReplayBudget::new();
    let result = cfg_bounds::bounds(function, &mut cfg_budget);
    let cfg = match cfg_bounds::enumerating_result(result, &cfg_budget)? {
        Some(cfg) => cfg,
        None => return Ok(0),
    };
    let plan_expansion = cfg.skeleton_work;
    let plan_terminals = checked_skeleton_mul(function, cfg.terminal_paths.max(1), 4)?;
    let comparison = checked_skeleton_mul(function, semantic_paths.max(cfg.terminal_paths), 2)?;
    checked_skeleton_add(
        function,
        checked_skeleton_add(function, hir_upper, plan_expansion)?,
        checked_skeleton_add(function, plan_terminals, comparison)?,
    )
}

fn expression_skeleton_work_upper(
    program: &dyn crate::cleanup::ReplayContext,
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
                // Eval plus strings::paths: temporary, two identities,
                // initialization observation, and the singleton path.
                _ if strings::owns_clone(expression) => 6,
                _ if crate::hir::closure::once::call(expression).is_some() => 20,
                ResolvedExprKind::Closure { .. }
                | ResolvedExprKind::FunctionReference { .. }
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
                | ResolvedExprKind::BorrowedUtf8Literal(_) => 2,
                ResolvedExprKind::Place(place) => place.projections.len().saturating_mul(2) + 8,
                ResolvedExprKind::BorrowPlace { place, .. } => {
                    place.projections.len().saturating_mul(2) + 8
                }
                ResolvedExprKind::RowLoan { .. } => 8,
                ResolvedExprKind::Unary { .. } => 8,
                ResolvedExprKind::Binary { .. } => 12,
                ResolvedExprKind::Invoke { args, .. }
                | ResolvedExprKind::Call { args, .. }
                | ResolvedExprKind::LiteralFormat { args, .. }
                | ResolvedExprKind::VecFieldRead { args, .. } => args.len().saturating_mul(6) + 14,
                ResolvedExprKind::NativeRustImportCall(call) => {
                    if super::native_rust::owns(expression) {
                        call.args.len().saturating_mul(6) + 14
                    } else {
                        call.args.len().saturating_mul(4) + 8
                    }
                }
                ResolvedExprKind::HostCommandCall(call) => call.args.len().saturating_mul(6) + 14,
                ResolvedExprKind::ByteRange { .. } => 32,
                ResolvedExprKind::Block { statements, .. }
                | ResolvedExprKind::VecConstructor { statements, .. } => {
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
                    skeleton_bound::constructor_work_upper(program, function, fields)?
                }
                ResolvedExprKind::UpdateRecord { record, fields, .. } => {
                    skeleton_bound::update_work_upper(
                        program, function, expression, record, fields,
                    )?
                }
                ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => 10,
                ResolvedExprKind::Project { .. } | ResolvedExprKind::Upcast { .. } => 6,
                ResolvedExprKind::Yield { .. } => 6,
                ResolvedExprKind::LexicalExit { .. } => 16,
                ResolvedExprKind::If { .. } => 10,
                ResolvedExprKind::Match { arms, .. } => {
                    // Guards recurse as separate sub-skeletons, so each arm
                    // carries extra headroom over the aggregate baseline.
                    arms.len()
                        .saturating_mul(if guarded_variant::selected(expression) {
                            32
                        } else {
                            16
                        })
                        .saturating_add(8)
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
            let result = or_arm_path_counts(frame.expression, child_index, result);
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
                ResolvedExprKind::Match { .. } if guarded_variant::selected(frame.expression) => {
                    frame.accumulator = sequence_path_counts(frame.accumulator, result)
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
            _ if super::deferred_commit::infallible_borrowed_vector_observer(frame.expression) => {
                frame.accumulator
            }
            kind if matches!(
                kind,
                ResolvedExprKind::Call { .. }
                    | ResolvedExprKind::LiteralFormat { .. }
                    | ResolvedExprKind::VecFieldRead { .. }
            ) || super::native_rust::owns(frame.expression) =>
            {
                HirPathCounts {
                    normal: frame.accumulator.normal,
                    failed: frame
                        .accumulator
                        .failed
                        .saturating_add(frame.accumulator.normal),
                    residual: frame.accumulator.residual,
                }
            }
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
            ResolvedExprKind::Match { arms, .. } if guarded_variant::selected(frame.expression) => {
                guarded_variant::census(arms, frame.accumulator)
            }
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

fn validate_required_status_sources(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    match allocation_paths::current(program, function) {
        Some(current) => {
            ordinary_status::validate_allocation_projection(current, &function.cleanup_plan)
        }
        None => ordinary_status::validate(program, function),
    }
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

#[cfg(test)]
fn validate_slots(function: &ResolvedFunction) -> Result<BTreeSet<StorageId>, Diagnostic> {
    validate_slots_paid(function, &mut ReplayBudget::unobserved())
}
fn validate_slots_paid(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeSet<StorageId>, Diagnostic> {
    validate_slots_paid_core::<Diagnostic>(function, budget)
}

fn validate_slots_paid_core<E: semantic_refusal::Error>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeSet<StorageId>, E> {
    let mut storage = BTreeSet::new();
    for (index, slot) in function.cleanup_plan.slots.iter().enumerate() {
        let expected = u32_index(function, index, "cleanup slot")?;
        if slot.id.0 != expected || slot.storage_index != expected {
            return Err(E::structural(
                function,
                semantic_refusal::Atom::NoncontiguousSlot(index),
            ));
        }
        allocation::set_key(function, budget, &storage, &slot.storage)?;
        if !storage.insert(slot.storage.clone()) {
            return Err(E::structural(
                function,
                semantic_refusal::Atom::RepeatedStorage(slot.storage.clone()),
            ));
        }
    }
    Ok(storage)
}

#[cfg(test)]
fn collect_leaves(function: &ResolvedFunction) -> Result<Leaves, Diagnostic> {
    collect_leaves_paid(function, &mut ReplayBudget::unobserved())
}
fn collect_leaves_paid(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Leaves, Diagnostic> {
    let mut leaves = BTreeMap::new();
    let mut places = BTreeSet::new();
    let mut pending = shape_cursor::Pending::new();
    for slot in &function.cleanup_plan.slots {
        collect_shape(
            function,
            &slot.storage,
            &mut Vec::new(),
            &slot.field_liveness_shape,
            &mut leaves,
            &mut places,
            (budget, &mut pending),
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
    Leaves::new_with_budget(function, leaves, budget)
}

fn collect_shape<'shape>(
    function: &ResolvedFunction,
    storage: &StorageId,
    projections: &mut Vec<DeclarationId>,
    shape: &'shape FieldLivenessShape,
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
    places: &mut BTreeSet<CleanupPlace>,
    scratch: (&mut ReplayBudget, &mut shape_cursor::Pending<'shape>),
) -> Result<(), Diagnostic> {
    let (budget, pending) = scratch;
    use shape_cursor::Item;

    let initial_projection_depth = projections.len();
    pending.reserve(function, budget, 1, "shape worklist")?;
    pending.push(Item::Shape(shape));
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
                budget.reserve_vec(function, projections, 1, "shape projections")?;
                budget.reserve_clone(function, projection, "shape identity")?;
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
                allocation::leaf_payload(function, budget, storage, projections, lifecycle)?;
                let leaf = Leaf {
                    place: CleanupPlace {
                        storage: storage.clone(),
                        projections: projections.clone(),
                    },
                    lifecycle: lifecycle.clone(),
                };
                allocation::leaf_indexes(function, budget, leaves, places, *flag, &leaf)?;
                if leaves.insert(*flag, leaf.clone()).is_some() {
                    return Err(replay_error(
                        function,
                        format_args!("liveness flag {} is repeated", flag.0),
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
                    if field.field_index != expected
                        || !allocation::insert_id(function, budget, &mut field_ids, &field.field)?
                    {
                        return Err(replay_error(
                            function,
                            "record cleanup shape has non-contiguous or repeated fields",
                        ));
                    }
                }
                pending.reserve(
                    function,
                    budget,
                    fields.len().saturating_mul(3),
                    "record shape worklist",
                )?;
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
                        || !allocation::insert_id(function, budget, &mut case_ids, &case.case)?
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
                            || !allocation::insert_id(
                                function,
                                budget,
                                &mut field_ids,
                                &field.field,
                            )?
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
                pending.reserve(function, budget, item_count, "variant shape worklist")?;
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
    expressions: &BTreeMap<ExpressionId, Option<CallFact<'_>>>,
) -> Result<(), Diagnostic> {
    validate_status_sources_core::<Diagnostic>(function, expressions)
}

fn validate_status_sources_core<E: semantic_refusal::Error>(
    function: &ResolvedFunction,
    expressions: &BTreeMap<ExpressionId, Option<CallFact<'_>>>,
) -> Result<(), E> {
    let mut ids = BTreeSet::new();
    for source in &function.cleanup_plan.status_sources {
        if !ids.insert(source.id.clone()) {
            return Err(E::structural(
                function,
                semantic_refusal::Atom::RepeatedStatus(source.id.expression.clone()),
            ));
        }
        if !expressions.contains_key(&source.id.expression) {
            return Err(E::structural(
                function,
                semantic_refusal::Atom::UnknownStatus(source.id.expression.clone()),
            ));
        }
        match (&source.id.lane, &source.producer) {
            (
                StatusLane::OperationFailure | StatusLane::OwnerAdmission,
                StatusProducer::PropagatedCall { callee },
            ) => {
                let Some(Some(call)) = expressions.get(&source.id.expression) else {
                    return Err(replay_error(
                        function,
                        "propagated-call status source does not name a call expression",
                    )
                    .into());
                };
                if call.callee.as_ref() != callee {
                    return Err(replay_error(
                        function,
                        "propagated-call status source names the wrong callee",
                    )
                    .into());
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
                    )
                    .into());
                }
            }
            (
                StatusLane::AllocationFailure,
                StatusProducer::CheckedAllocation {
                    normalized_cases, ..
                },
            ) if function.cleanup_plan.schema == super::CLEANUP_PLAN_SCHEMA_V20
                && *normalized_cases
                    == [
                        super::AllocationCase::PayloadRefused,
                        super::AllocationCase::SizeOverflow,
                    ] => {}
            (StatusLane::ContractFalse, StatusProducer::ContractFalse { .. }) => {}
            _ => {
                return Err(replay_error(
                    function,
                    "status source lane does not match its producer",
                )
                .into());
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
        let unresolved =
            plan.lexical_control.is_some() && region.normal_scope_end == UNRESOLVED_NORMAL_END;
        if !unresolved
            && usize::try_from(region.normal_scope_end.0)
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
    leaves: &Leaves,
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
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expressions: &BTreeMap<ExpressionId, Option<CallFact<'_>>>,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
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
                CleanupTransition::ReserveRenewal { at, binding } => {
                    renewal::validate_binding(program, function, at, binding, helpers, budget)?;
                    require_expression(function, expressions, at)?;
                    validate_place(function, binding, storage, leaves)?;
                }
                CleanupTransition::Renew {
                    at,
                    source,
                    destination,
                } => {
                    renewal::validate_binding(program, function, at, destination, helpers, budget)?;
                    require_expression(function, expressions, at)?;
                    validate_place(function, source, storage, leaves)?;
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
                    let Some(cases) = program.declarations().variant_cases(variant) else {
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
                    // Lexical return domains duplicate postcondition bodies, so one
                    // call legitimately commits once per proof domain; the typed
                    // skeleton comparison counts those per-domain commits exactly.
                    if !committed_calls.insert(call.clone()) && plan.lexical_control.is_none() {
                        return Err(replay_error(
                            function,
                            "call has more than one atomic commit",
                        ));
                    }
                    let params = resolved_call::resolved_call_param_view(
                        program,
                        function,
                        find_resolved_expression(function, call).ok_or_else(|| {
                            replay_error(function, "call commit expression is absent")
                        })?,
                        &fact.callee,
                        fact.instance,
                        fact.type_arguments.as_slice(),
                    )
                    .map_err(|_| {
                        replay_error(
                            function,
                            format_args!("call commit has unknown callee `{}`", fact.callee),
                        )
                    })?;
                    if params.len() != fact.arguments.len() {
                        return Err(replay_error(
                            function,
                            "call commit callee signature has inconsistent arity",
                        ));
                    }
                    if !resolved_call::owned_parameter_indices_match(
                        &params,
                        arguments,
                        function,
                        |ty| type_needs_drop(program, function, ty),
                    )? {
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
                            | CLEANUP_PLAN_SCHEMA_V10
                            | CLEANUP_PLAN_SCHEMA_V11
                            | CLEANUP_PLAN_SCHEMA_V12
                            | CLEANUP_PLAN_SCHEMA_V13
                            | CLEANUP_PLAN_SCHEMA_V15
                            | CLEANUP_PLAN_SCHEMA_V16
                            | super::CLEANUP_PLAN_SCHEMA_V17
                            | super::CLEANUP_PLAN_SCHEMA_V18
                            | super::CLEANUP_PLAN_SCHEMA_V19
                            | CLEANUP_PLAN_SCHEMA_V14
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
    // Control-flow-dead calls after an abrupt exit are never lowered, so the
    // lexical profile checks commit coverage in the typed skeleton instead.
    if plan.lexical_control.is_none() && committed_calls != expected_calls {
        return Err(replay_error(
            function,
            "cleanup plan does not contain exactly one atomic commit for every call",
        ));
    }
    Ok(())
}

fn validate_exits(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    return_operands: &BTreeSet<ExpressionId>,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let statuses = plan
        .status_sources
        .iter()
        .map(|source| source.id.clone())
        .collect::<BTreeSet<_>>();
    let mut continuation_edges = BTreeSet::new();
    for (index, exit) in plan.exits.iter().enumerate() {
        if plan.lexical_control.is_none()
            && matches!(
                exit.continuation,
                ExitContinuation::LoopLatch { .. }
                    | ExitContinuation::LoopBreak { .. }
                    | ExitContinuation::FunctionReturn { .. }
            )
        {
            return Err(replay_error(
                function,
                "lexical exit continuation has no lexical-control certificate",
            ));
        }
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
        validate_finalizer_actions(function, &exit.finalize_in_order, storage, leaves)?;
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
            ExitContinuation::CommitResult { source } => {
                validate_result_source(program, function, source, storage, leaves, return_operands)?
            }
            ExitContinuation::FunctionReturn { edge, source, .. } => {
                validate_owned_edge(function, exit.from, *edge, &mut continuation_edges)?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(replay_error(
                        function,
                        "function-return continuation edge is conditional",
                    ));
                }
                validate_result_source(program, function, source, storage, leaves, return_operands)?
            }
            ExitContinuation::LoopBreak { edge, .. } => {
                validate_owned_edge(function, exit.from, *edge, &mut continuation_edges)?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(replay_error(
                        function,
                        "loop-break continuation edge is conditional",
                    ));
                }
            }
            ExitContinuation::LoopLatch { .. } => {}
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
        if plan.lexical_control.is_some() && region.normal_scope_end == UNRESOLVED_NORMAL_END {
            // Authenticated as abrupt-only by the lexical certificate replay.
            continue;
        }
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

/// Each finalizer names its exact cleanup leaf, lifecycle and variant case path.
/// Shared by exits and lexical execution-boundary certificates.
fn validate_finalizer_actions(
    function: &ResolvedFunction,
    actions: &[super::FinalizeAction],
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
) -> Result<(), Diagnostic> {
    let mut finalized = BTreeSet::new();
    for action in actions {
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
    Ok(())
}

/// Shared by the ordinary commit and every lexical function-return source.
fn validate_result_source(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    source: &CleanupResultSource,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    return_operands: &BTreeSet<ExpressionId>,
) -> Result<(), Diagnostic> {
    match source {
        CleanupResultSource::Scalar { expression } => {
            if type_needs_drop(program, function, &function.return_type)? {
                return Err(replay_error(
                    function,
                    "droppable function result uses a scalar result commit",
                ));
            }
            if expression != &function.body.id && !return_operands.contains(expression) {
                return Err(replay_error(
                    function,
                    "scalar result commit does not publish the function body",
                ));
            }
        }
        CleanupResultSource::Owned { storage: result } => {
            if !matches!(
                function.return_type,
                ResolvedType::Nominal { .. }
                    | ResolvedType::Bytes
                    | ResolvedType::String
                    | ResolvedType::StringMap
                    | ResolvedType::OnceFunction
                    | ResolvedType::OnceFunctionI64
                    | ResolvedType::OnceFunctionI64Pair
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
        ExitContinuation::LoopLatch { target } => {
            if let Some(leaves) =
                lexical_cert::owned_continue_leaves(function, exit.id, &source_chain)?
            {
                return Ok(leaves);
            }
            let region = lexical_cert::loop_region(plan, target).ok_or_else(|| {
                replay_error(function, "loop latch names an unknown lexical loop")
            })?;
            if !source_chain.contains(&region) {
                return Err(replay_error(
                    function,
                    "loop latch exit does not originate inside its loop region",
                ));
            }
            Ok(source_chain
                .into_iter()
                .take_while(|candidate| *candidate != region)
                .collect())
        }
        ExitContinuation::Continue(edge)
        | ExitContinuation::LoopBreak { edge, .. }
        | ExitContinuation::FunctionReturn { edge, .. } => {
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

fn validate_typed_control_skeleton_with_index<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    mut continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<(), Diagnostic> {
    if function.cleanup_plan.lexical_control.is_none()
        && factored::validate_with_index_storage(
            program,
            function,
            budget,
            helpers,
            continuations.as_deref_mut(),
        )?
    {
        return Ok(());
    }
    let mut expected =
        hir_skeleton_paths_with_index_storage(program, function, budget, helpers, continuations)?;
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

#[cfg(test)]
fn hir_skeleton_paths(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    hir_skeleton_paths_with_index(program, function, budget, None)
}
fn hir_skeleton_paths_with_index(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    hir_skeleton_paths_with_index_storage(program, function, budget, helpers, None)
}
fn hir_skeleton_paths_with_index_storage<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    let mut work = SkeletonWork::checked_with_index(program, function, budget, helpers)?;
    work.continuations = continuations;
    let mut paths = work.singleton_path(empty_expr_path(), "HIR root path")?;
    for contract in &function.requires {
        paths = sequence_expression(program, function, paths, contract, &mut work)?;
        paths = split_contract(paths, contract, &mut work)?;
    }
    paths = sequence_expression(program, function, paths, &function.body, &mut work)?;
    let has_residual = paths.iter().any(|path| path.residual);
    let owned_result = nested_shape::type_needs_drop_with_budget(
        program,
        function,
        &function.return_type,
        work.budget,
    )?;
    if has_residual && !owned_result {
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
                    SkeletonObservation::StageCopyResult(CopySource::Body {
                        expression,
                        instance,
                    }),
                    "body result staging",
                )?;
            }
            path.residual = false;
        }
    }
    if owned_result {
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
    if has_residual && owned_result {
        let provisional = CleanupPlace::whole(StorageId::ProvisionalResult);
        for path in &mut paths {
            if path.residual {
                if path.owned_source.as_ref() != Some(&provisional) {
                    return Err(replay_error(
                        function,
                        "owned postfix `?` residual does not stage the provisional result",
                    ));
                }
                path.residual = false;
            }
        }
    }
    // A lexical `return` has already staged its result and left its contour;
    // it now runs every postcondition exactly like a normally completed body.
    lexical_skeleton::release_returns(&mut paths);
    for contract in &function.ensures {
        paths = sequence_expression(program, function, paths, contract, &mut work)?;
        paths = split_contract(paths, contract, &mut work)?;
    }
    let mut completed = Vec::new();
    for path in paths {
        let terminal = lexical_skeleton::terminal(&path);
        work.push_skeleton_path(
            &mut completed,
            SkeletonPath {
                observations: path.observations,
                terminal,
            },
            "completed HIR skeleton path",
        )?;
    }
    Ok(completed)
}

fn sequence_expression<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    prefixes: Vec<ExprSkeletonPath>,
    expression: &'hir ResolvedExpr,
    work: &mut SkeletonWork<'hir, '_>,
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
    #[cfg(test)]
    if path_array::main696::selected() {
        return path_array::main696::sequence_skeleton_paths(prefixes, suffixes, work);
    }
    let prefixes = work.merged_paths(prefixes)?;
    let suffixes = work.merged_suffixes(suffixes)?;
    let share_prefix = work.shares_prefix_tail(&suffixes)?;
    let mut combined = Vec::new();
    let minimum = path_array::count!(
        work,
        true,
        if suffixes.is_empty() {
            0
        } else {
            prefixes.len()
        },
        "sequenced path capacity"
    );
    path_array::select!(first, minimum, &combined, work);
    work.budget.reserve_bytes(
        work.function,
        final_output::HIR_SELECTION_CONTROLS,
        "HIR single-suffix selection controls",
    )?;
    let single = suffixes.len() == 1;
    for mut prefix in prefixes {
        if prefix.failed || prefix.residual {
            path_array::push!(
                work,
                first,
                minimum,
                &mut combined,
                prefix,
                "short-circuited skeleton path"
            );
            continue;
        }
        if share_prefix {
            work.share_prefix_tail(&mut prefix.observations)?;
        }
        if single {
            work.budget.reserve_bytes(
                work.function,
                final_output::HIR_MOVE_CONTROLS,
                "HIR single-suffix move controls",
            )?;
            let suffix = &suffixes[0];
            work.charge(1, "skeleton prefix clone")?;
            note_skeleton_materialization();
            let observations = prefix.observations;
            let observations = work.extend_owned_observations(
                observations,
                &suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source = work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            path_array::push!(
                work,
                first,
                minimum,
                &mut combined,
                ExprSkeletonPath {
                    observations,
                    owned_source,
                    failed: suffix.failed,
                    residual: suffix.residual,
                    continuation: lexical_skeleton::Continuation::sequenced(
                        &prefix.continuation,
                        &suffix.continuation,
                    ),
                },
                "sequenced skeleton path",
            );
            continue;
        }
        for suffix in suffixes.iter() {
            let observations =
                work.clone_observations(&prefix.observations, "skeleton prefix clone")?;
            let observations = work.extend_owned_observations(
                observations,
                &suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source = work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            path_array::push!(
                work,
                first,
                minimum,
                &mut combined,
                ExprSkeletonPath {
                    observations,
                    owned_source,
                    failed: suffix.failed,
                    residual: suffix.residual,
                    continuation: lexical_skeleton::Continuation::sequenced(
                        &prefix.continuation,
                        &suffix.continuation,
                    ),
                },
                "sequenced skeleton path",
            );
        }
    }
    Ok(combined)
}

use path_summary::has_active_paths;

fn expression_skeleton<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    expression: &'hir ResolvedExpr,
    work: &mut SkeletonWork<'hir, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let (lease, mut frames) = if let Some(storage) = work.continuations.as_mut() {
        if !storage.matches(program) {
            return Err(replay_error(
                function,
                "typed-HIR continuation storage belongs to another program",
            ));
        }
        storage.acquire(function, work.budget)?
    } else {
        (
            continuation_storage::Lease::Local,
            continuation_storage::fresh(function, work.budget)?,
        )
    };
    // No directory borrow spans the runner: guards re-enter this same Storage.
    // A panic still unwinds to invocation Drop, as in the original lease owner.
    let result = expression_skeleton_run(program, function, expression, work, &mut frames);
    frames.clear(); // Drop every owned continuation payload before reuse.
    if let Some(storage) = work.continuations.as_mut() {
        storage.release(lease, frames);
    }
    result
}
fn expression_skeleton_run<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    expression: &'hir ResolvedExpr,
    work: &mut SkeletonWork<'hir, '_>,
    frames: &mut Vec<continuation_storage::Frame<'hir>>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    use continuation_storage::{Frame, OwnedStep};

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

    macro_rules! schedule_scalar {
        ($next:expr, $frames:ident, $produced:ident) => {{
            match $next {
                scalar_match::Next::Guard(state) => {
                    let guard = state.expression();
                    push_frame!($frames, Frame::ScalarGuard(state));
                    push_frame!($frames, Frame::Eval(guard));
                }
                scalar_match::Next::Value(state) => {
                    let value = state.expression();
                    push_frame!($frames, Frame::ScalarValue(state));
                    push_frame!($frames, Frame::Eval(value));
                }
                scalar_match::Next::Done(paths) => $produced = Some(paths),
            }
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
                ResolvedStatement::LexicalWhile {
                    condition,
                    body,
                    latch,
                    ..
                } => {
                    push(
                        frames,
                        Frame::LexicalLoopCondition {
                            prefixes: paths,
                            expression,
                            statements,
                            tail,
                            index: next,
                            condition,
                            body,
                            latch,
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

    push_frame!(frames, Frame::Eval(expression));
    let mut produced: Option<Vec<ExprSkeletonPath>> = None;
    /// Lexical loops whose body is being evaluated, innermost last. A `continue`
    /// reads its loop's generated advancement from here.
    struct ActiveLoop<'a> {
        condition: &'a ExpressionId,
        true_match: Option<&'a ResolvedExpr>,
        advance: Option<&'a ResolvedExpr>,
        step: OwnedStep<'a>,
    }
    let mut active_loops: Vec<ActiveLoop<'_>> = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::FinishCopyRegion { previous } => {
                let paths = produced
                    .take()
                    .expect("Copy region expression paths retained");
                let paths = work.merged_paths(paths)?;
                work.active_copy_region = previous;
                produced = Some(paths);
            }
            Frame::RenewalPrefix(expression) => {
                let mut paths = produced.take().expect("renewal RHS paths retained");
                renewal::prepend_reservation(function, expression, &mut paths, work)?;
                produced = Some(paths);
            }
            Frame::Eval(expression) => {
                debug_assert!(produced.is_none());
                if work.budget.merge_paths
                    && function.cleanup_plan.lexical_control.is_some()
                    && work.active_copy_region.is_none()
                    && factored::copy_regions::eligible(program, function, expression, work.budget)?
                {
                    // This expression starts from its own empty path. Close its
                    // scope before the caller sequences any retained prefix.
                    let identity = work.clone_payload(&expression.id, "Copy region origin")?;
                    let previous = work.active_copy_region.replace(identity);
                    push_frame!(frames, Frame::FinishCopyRegion { previous });
                }
                if work.renewal_binding(&expression.id)?.is_some() {
                    push_frame!(frames, Frame::RenewalPrefix(expression));
                }
                if strings::owns_clone(expression)
                    && !work.string_owner_moves.contains(&expression.id)
                    && !work.string_condition_reads.contains(&expression.id)
                {
                    produced = Some(if allocation_paths::current(program, function).is_some() {
                        allocation_paths::string_paths(expression, work)?
                    } else {
                        strings::paths(expression, work)?
                    });
                    continue;
                }

                match &expression.kind {
                    ResolvedExprKind::LexicalExit {
                        kind,
                        target,
                        value,
                    } => match (kind, value) {
                        (crate::ast::LexicalExitKind::Return, Some(operand)) => {
                            push_frame!(frames, Frame::LexicalReturn { expression });
                            push_frame!(frames, Frame::Eval(operand));
                        }
                        (crate::ast::LexicalExitKind::Break, None) => {
                            produced = Some(lexical_skeleton::exit_paths(expression, work)?);
                        }
                        (crate::ast::LexicalExitKind::Continue, None) => {
                            let crate::hir::ControlTarget::Loop { site, .. } = target else {
                                return Err(replay_error(
                                    function,
                                    "continue does not target a lexical loop",
                                ));
                            };
                            let active = active_loops
                                .iter()
                                .rev()
                                .find(|active| active.condition == site)
                                .ok_or_else(|| {
                                    replay_error(function, "continue names no active lexical loop")
                                })?;
                            let (advance, step) = (active.advance, active.step);
                            match advance {
                                None => {
                                    produced =
                                        Some(lexical_skeleton::exit_paths(expression, work)?);
                                }
                                Some(advance) => {
                                    push_frame!(
                                        frames,
                                        Frame::ContinueAdvance { expression, step }
                                    );
                                    push_frame!(frames, Frame::Eval(advance));
                                }
                            }
                        }
                        _ => {
                            return Err(replay_error(
                                function,
                                "lexical exit has an inconsistent kind or operand",
                            ));
                        }
                    },
                    ResolvedExprKind::Closure { .. } if !expression.ty.is_once_function() => {
                        produced = Some(
                            work.singleton_path(empty_expr_path(), "copy closure skeleton path")?,
                        );
                    }
                    ResolvedExprKind::FunctionReference { .. }
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
                    | ResolvedExprKind::BorrowedUtf8Literal(_) => {
                        produced =
                            Some(work.singleton_path(empty_expr_path(), "literal skeleton path")?);
                    }
                    ResolvedExprKind::Place(place) => {
                        let owned_source = if !work.string_condition_reads.contains(&expression.id)
                            && expression.ownership == OwnershipMode::Own
                            && type_needs_drop(program, function, &expression.ty)?
                        {
                            Some(cleanup_place_from_hir(function, place, work)?)
                        } else {
                            None
                        };
                        produced = Some(work.singleton_path(
                            ExprSkeletonPath {
                                observations: Observations::default(),
                                owned_source,
                                failed: false,
                                residual: false,
                                continuation: lexical_skeleton::Continuation::Normal,
                            },
                            "place skeleton path",
                        )?);
                    }
                    ResolvedExprKind::BorrowPlace { .. } => {
                        produced =
                            Some(work.singleton_path(empty_expr_path(), "borrow skeleton path")?);
                    }
                    // Row Loan v1 borrows two exact places and owns no state.
                    ResolvedExprKind::RowLoan { args, .. } => {
                        if !super::row_loan::exact_places(args) {
                            return Err(replay_error(
                                function,
                                "row loan skeleton children are not exact places",
                            ));
                        }
                        produced =
                            Some(work.singleton_path(empty_expr_path(), "row loan skeleton path")?);
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
                                states: typed_storage::call_root(work)?,
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
                    // Resumable Effects v1 (issue #204): `yield`'s request
                    // is an admitted Copy scalar (`hir::resolve_yield`
                    // enforces this), so it is transparent to the skeleton
                    // in exactly the same way an upcast is -- reusing the
                    // same no-op marker rather than adding a second one.
                    ResolvedExprKind::Yield { request } => {
                        push_frame!(frames, Frame::UpcastPassthrough);
                        push_frame!(frames, Frame::Eval(request));
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
                    ResolvedExprKind::Invoke { .. } | ResolvedExprKind::Closure { .. } => {
                        typed_storage::unsupported_provider(work)?;
                        let (_, args, params) =
                            crate::hir::function_value::cleanup_call(expression)?;
                        work.charge(1, "indirect call skeleton root state")?;
                        let states = typed_storage::call_root(work)?;
                        if let Some(argument) = args.first() {
                            push_frame!(
                                frames,
                                Frame::CallArgument {
                                    expression,
                                    params: params.into(),
                                    args,
                                    index: 0,
                                    states
                                }
                            );
                            push_frame!(frames, Frame::Eval(argument));
                        } else {
                            produced = Some(finish_call_states(
                                program, function, expression, states, work,
                            )?);
                        }
                    }
                    ResolvedExprKind::VecFieldRead { args, .. } => {
                        let params = if work.budget.selected() {
                            typed_storage::field_params(program, expression, work)?
                        } else {
                            resolved_call_params(
                                program,
                                function,
                                expression,
                                crate::vec_field::operation_id(),
                                None,
                                &[],
                            )?
                        };
                        work.charge(1, "scoped vector read skeleton root state")?;
                        let states = typed_storage::call_root(work)?;
                        if let Some(argument) = args.first() {
                            push_frame!(
                                frames,
                                Frame::CallArgument {
                                    expression,
                                    params: params.into(),
                                    args,
                                    index: 0,
                                    states
                                }
                            );
                            push_frame!(frames, Frame::Eval(argument));
                        } else {
                            produced = Some(finish_call_states(
                                program, function, expression, states, work,
                            )?);
                        }
                    }
                    ResolvedExprKind::LiteralFormat { args, .. } => {
                        typed_storage::unsupported_provider(work)?;
                        let params = resolved_call_params(
                            program,
                            function,
                            expression,
                            crate::literal_format::operation_id(),
                            None,
                            &[],
                        )?;
                        work.charge(1, "literal format skeleton root state")?;
                        let states = typed_storage::call_root(work)?;
                        if let Some(argument) = args.first() {
                            push_frame!(
                                frames,
                                Frame::CallArgument {
                                    expression,
                                    params: params.into(),
                                    args,
                                    index: 0,
                                    states
                                }
                            );
                            push_frame!(frames, Frame::Eval(argument));
                        } else {
                            produced = Some(finish_call_states(
                                program, function, expression, states, work,
                            )?);
                        }
                    }
                    ResolvedExprKind::Call {
                        callee,
                        instance,
                        args,
                        type_arguments,
                    } => {
                        #[cfg(test)]
                        let held_params = match program.borrowed_helper_lease(function) {
                            Some(lease) => {
                                Some(lease.call_params(program, function, expression, work)?)
                            }
                            None => None,
                        };
                        #[cfg(not(test))]
                        let held_params: Option<
                            Vec<crate::hir::ResolvedParam>,
                        > = None;
                        let params: resolved_call::Params<'_> = if let Some(params) = held_params {
                            params.into()
                        } else {
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
                            let vec_intrinsic = instance
                                .is_none()
                                .then(|| crate::vec_ops::by_id(callee.as_str()))
                                .flatten();
                            let box_intrinsic = instance
                                .is_none()
                                .then(|| crate::box_ops::by_id(callee.as_str()))
                                .flatten();
                            if instance.is_none()
                                && crate::map_ops::by_id(callee.as_str()).is_some()
                            {
                                typed_storage::unsupported_provider(work)?;
                                resolved_call_params(
                                    program,
                                    function,
                                    expression,
                                    callee,
                                    None,
                                    type_arguments,
                                )?
                                .into()
                            } else if let Some(op) = string_intrinsic {
                                if work.budget.selected()
                                    || !resolved_call::ordinary_borrows_enabled()
                                {
                                    typed_storage::static_params(
                                        op.id(),
                                        op.param_types(),
                                        op.param_names(),
                                        |i, _| op.param_ownership(i),
                                        work,
                                    )
                                    .map(resolved_call::Params::Owned)?
                                } else {
                                    work.budget.reserve_bytes(
                                        function,
                                        0,
                                        "intrinsic parameter slots",
                                    )?;
                                    resolved_call::Params::String(op)
                                }
                            } else if let Some(op) = str_intrinsic {
                                if work.budget.selected()
                                    || !resolved_call::ordinary_borrows_enabled()
                                {
                                    typed_storage::static_params(
                                        op.id(),
                                        op.param_types(),
                                        op.param_names(),
                                        |_, ty| {
                                            if *ty == ResolvedType::Str {
                                                OwnershipMode::Borrow
                                            } else {
                                                OwnershipMode::Value
                                            }
                                        },
                                        work,
                                    )
                                    .map(resolved_call::Params::Owned)?
                                } else {
                                    work.budget.reserve_bytes(
                                        function,
                                        0,
                                        "intrinsic parameter slots",
                                    )?;
                                    resolved_call::Params::Str(op)
                                }
                            } else if let Some(op) = byte_intrinsic {
                                if work.budget.selected()
                                    || !resolved_call::ordinary_borrows_enabled()
                                {
                                    typed_storage::static_params(
                                        op.id(),
                                        op.param_types(),
                                        op.param_names(),
                                        |i, _| op.param_ownership(i),
                                        work,
                                    )
                                    .map(resolved_call::Params::Owned)?
                                } else {
                                    work.budget.reserve_bytes(
                                        function,
                                        0,
                                        "intrinsic parameter slots",
                                    )?;
                                    resolved_call::Params::Byte(op)
                                }
                            } else if let Some(op) = host_io_intrinsic {
                                typed_storage::unsupported_provider(work)?;
                                crate::host_io_ops::resolved_params(op).into()
                            } else if instance.is_none()
                                && (crate::iterator_ops::by_id(callee.as_str()).is_some()
                                    || crate::list_ops::by_id(callee.as_str()).is_some()
                                    || callee.as_str() == crate::stdin_stream_ops::EOF_ID)
                            {
                                typed_storage::unsupported_provider(work)?;
                                resolved_call_params(
                                    program,
                                    function,
                                    expression,
                                    callee,
                                    None,
                                    type_arguments,
                                )?
                                .into()
                            } else if let Some(op) = vec_intrinsic {
                                if work.budget.selected() {
                                    typed_storage::vector_params(program, expression, work)?.into()
                                } else {
                                    let [element] = type_arguments.as_slice() else {
                                        return Err(replay_error(
                                            function,
                                            "bounded Vec skeleton call has incorrect type arity",
                                        ));
                                    };
                                    resolved_call::vector_params(
                                        program.declarations(),
                                        op,
                                        element,
                                        callee,
                                    )
                                    .into()
                                }
                            } else if let Some(op) = box_intrinsic {
                                typed_storage::unsupported_provider(work)?;
                                let [element] = type_arguments.as_slice() else {
                                    return Err(replay_error(
                                        function,
                                        "bounded Box skeleton call has incorrect type arity",
                                    ));
                                };
                                crate::box_ops::resolved_params(op, element).into()
                            } else {
                                let target = program
                                    .resolve_call_target(callee, instance.as_ref())
                                    .ok_or_else(|| {
                                        replay_error(
                                            function,
                                            format_args!("unknown skeleton callee `{callee}`"),
                                        )
                                    })?;
                                if work.budget.selected()
                                    || !resolved_call::ordinary_borrows_enabled()
                                {
                                    work.clone_payload(&target.params, "callee parameter clone")?
                                        .into()
                                } else {
                                    work.budget.reserve_bytes(
                                        function,
                                        0,
                                        "callee parameter clone",
                                    )?;
                                    resolved_call::Params::Borrowed(&target.params)
                                }
                            }
                        };
                        work.charge(1, "call skeleton root state")?;
                        let states = typed_storage::call_root(work)?;
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
                    ResolvedExprKind::NativeRustImportCall(call)
                        if super::native_rust::owns(expression) =>
                    {
                        typed_storage::unsupported_provider(work)?;
                        let params = resolved_call_params(
                            program,
                            function,
                            expression,
                            &call.import,
                            None,
                            &[],
                        )?;
                        work.charge(1, "owned native call skeleton root state")?;
                        let states = typed_storage::call_root(work)?;
                        if let Some(argument) = call.args.first() {
                            let lend = super::native_rust::lends_string_place(
                                expression,
                                argument,
                                params[0].ownership,
                            );
                            push_frame!(
                                frames,
                                Frame::CallArgument {
                                    expression,
                                    params: params.into(),
                                    args: &call.args,
                                    index: 0,
                                    states,
                                }
                            );
                            if lend {
                                produced = Some(
                                    work.singleton_path(empty_expr_path(), "native String loan")?,
                                );
                            } else {
                                push_frame!(frames, Frame::Eval(argument));
                            }
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
                        typed_storage::unsupported_provider(work)?;
                        let params = crate::command_io_ops::resolved_params(call.operation);
                        work.charge(1, "host-command skeleton root state")?;
                        let states = typed_storage::call_root(work)?;
                        if let Some(argument) = call.args.first() {
                            push_frame!(
                                frames,
                                Frame::CallArgument {
                                    expression,
                                    params: params.into(),
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
                    ResolvedExprKind::Block { statements, tail }
                    | ResolvedExprKind::VecConstructor {
                        statements, tail, ..
                    } => {
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
                                ResolvedStatement::LexicalWhile {
                                    condition,
                                    body,
                                    latch,
                                    ..
                                } => {
                                    push_frame!(
                                        frames,
                                        Frame::LexicalLoopCondition {
                                            prefixes: paths,
                                            expression,
                                            statements,
                                            tail,
                                            index: 0,
                                            condition,
                                            body,
                                            latch,
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
                                arms
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
                    let lend = super::native_rust::lends_string_place(
                        expression,
                        &args[next],
                        params.get(next).expect("next call parameter").ownership,
                    );
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
                    if lend {
                        produced =
                            Some(work.singleton_path(empty_expr_path(), "native String loan")?);
                    } else {
                        push_frame!(frames, Frame::Eval(&args[next]));
                    }
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
                let op = crate::byte_ops::ByteOp::Range;
                let params = if work.budget.selected() || !resolved_call::ordinary_borrows_enabled()
                {
                    typed_storage::static_params(
                        op.id(),
                        op.param_types(),
                        op.param_names(),
                        |i, _| op.param_ownership(i),
                        work,
                    )
                    .map(resolved_call::Params::Owned)?
                } else {
                    work.budget
                        .reserve_bytes(function, 0, "intrinsic parameter slots")?;
                    resolved_call::Params::Byte(op)
                };
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
                    ResolvedStatement::CopyDiscard { .. } | ResolvedStatement::Unsafe { .. } => {
                        // Unsafe boundaries bind and own nothing here: their ordinary block body was evaluated as this statement's value expression.
                        if let Some(settled) = advance_block_value(
                            function, frames, work, expression, statements, tail, index, paths,
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
                            function, frames, work, expression, statements, tail, index, paths,
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
                // The skip branch joins the body branch at the loop continuation without further observations.
                for path in joined.into_iter().chain(false_prefixes) {
                    work.push_expr_path(&mut results, path, "while join path")?;
                }
                if let Some(settled) = advance_block_value(
                    function, frames, work, expression, statements, tail, index, results,
                )? {
                    produced = Some(settled);
                }
            }
            Frame::LexicalReturn { expression } => {
                let mut paths = produced.take().expect("return operand path retained");
                if type_needs_drop(program, function, &function.return_type)? {
                    // The owned result moves into provisional storage at the exit
                    // site, before any contour cleanup or postcondition runs.
                    let at = work.clone_owned(&expression.id, "lexical return site clone")?;
                    paths = transfer_completed_paths(
                        function,
                        paths,
                        at,
                        CleanupPlace::whole(StorageId::ProvisionalResult),
                        "owned lexical return",
                        work,
                    )?;
                }
                produced = Some(lexical_skeleton::mark_return(paths, expression, work)?);
            }
            Frame::ContinueAdvance { expression, step } => {
                let advance = produced.take().expect("continue advancement path retained");
                let ResolvedExprKind::LexicalExit { target, .. } = &expression.kind else {
                    return Err(replay_error(
                        function,
                        "continue advancement frame lost its exit",
                    ));
                };
                let prefix = lexical_skeleton::continue_prefix(expression, work)?;
                let mut advanced = sequence_skeleton_paths(prefix, &advance, work)?;
                if let Some((assignment, binding, arm)) = step {
                    // Only the Yield tail runs. Its owned Step moves into the
                    // Yield block at the arm identity, then into the match at the
                    // same identity, then renews the Step binding at the match.
                    let arm_at =
                        work.clone_owned(&arm.id, "yield block transfer identity clone")?;
                    let arm_place = temporary_place(arm, work)?;
                    advanced = transfer_completed_paths(
                        function,
                        advanced,
                        arm_at,
                        arm_place,
                        "owning continue Yield block",
                        work,
                    )?;
                    advanced = finish_owned(program, function, assignment, arm, advanced, work)?;
                    let at = work.clone_owned(&assignment.id, "step renewal expression clone")?;
                    let binding = work.clone_owned(&binding.id, "step renewal binding clone")?;
                    advanced = transfer_completed_paths(
                        function,
                        advanced,
                        at,
                        CleanupPlace::whole(StorageId::Value(binding)),
                        "owned iterator step renewal",
                        work,
                    )?;
                }
                produced = Some(lexical_skeleton::finish_continue(advanced, target, work)?);
            }
            Frame::LexicalLoopCondition {
                prefixes,
                expression,
                statements,
                tail,
                index,
                condition,
                body,
                latch,
            } => {
                let condition_paths = produced.take().expect("loop condition path retained");
                let prefixed = sequence_skeleton_paths(prefixes, &condition_paths, work)?;
                let (results, true_prefixes, false_prefixes) =
                    split_boolean_prefixes(prefixed, &condition.id, work)?;
                work.budget
                    .reserve_vec(function, &mut active_loops, 1, "active lexical loops")?;
                active_loops.push(ActiveLoop {
                    condition: &condition.id,
                    true_match: crate::hir::iterator_loop::true_body_match(condition, body, latch),
                    advance: lexical_hir::generated_advance(body, latch),
                    step: if matches!(latch, crate::hir::LoopLatchKind::OwnedIteratorStep { .. }) {
                        lexical_hir::owned_step_assignment(body).and_then(
                            |(assignment, binding)| {
                                lexical_hir::yield_arm(body)
                                    .map(|(_, arm)| (assignment, binding, arm))
                            },
                        )
                    } else {
                        None
                    },
                });
                push_frame!(
                    frames,
                    Frame::LexicalLoopBody {
                        expression,
                        statements,
                        tail,
                        index,
                        results,
                        true_prefixes,
                        false_prefixes,
                        condition,
                    }
                );
                push_frame!(frames, Frame::Eval(body));
            }
            Frame::LexicalLoopBody {
                expression,
                statements,
                tail,
                index,
                mut results,
                true_prefixes,
                false_prefixes,
                condition,
            } => {
                active_loops.pop();
                let body_paths = produced.take().expect("loop body path retained");
                let joined = sequence_skeleton_paths(true_prefixes, &body_paths, work)?;
                work.budget
                    .reserve_clone(function, &work.owner, "lexical loop owner clone")?;
                let owner = work
                    .owner
                    .clone()
                    .ok_or_else(|| replay_error(function, "lexical loop has no execution owner"))?;
                let target = crate::hir::ControlTarget::Loop {
                    owner,
                    site: work.clone_owned(&condition.id, "loop target site clone")?,
                };
                let settled = lexical_skeleton::settle_loop_body(joined, &target, work)?;
                for path in settled.into_iter().chain(false_prefixes) {
                    work.push_expr_path(&mut results, path, "lexical loop join path")?;
                }
                if let Some(settled) = advance_block_value(
                    function, frames, work, expression, statements, tail, index, results,
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
                    typed_storage::push_projection(
                        &mut destination,
                        case,
                        "variant case projection clone",
                        work,
                    )?;
                    typed_storage::push_projection(
                        &mut destination,
                        &field.field,
                        "variant field projection clone",
                        work,
                    )?;
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
                    work.budget.reserve_vec(
                        function,
                        &mut destination.projections,
                        1,
                        "field projection slots",
                    )?;
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
                        format_args!("record update repeats field `{}`", field.field),
                    ));
                }
                let suffixes = produced.take().expect("update field path retained");
                let mut paths = sequence_skeleton_paths(paths, &suffixes, work)?;
                if needs_cleanup
                    && field.value.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &field.value.ty)?
                {
                    let mut destination = temporary_place(expression, work)?;
                    work.budget.reserve_vec(
                        function,
                        &mut destination.projections,
                        1,
                        "field projection slots",
                    )?;
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
                        work.budget.reserve_vec(
                            function,
                            &mut source.projections,
                            1,
                            "projection slots",
                        )?;
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
            Frame::ScalarGuard(state) => {
                let paths = produced.take().expect("scalar guard paths retained");
                let next = state.finish(paths, work)?;
                schedule_scalar!(next, frames, produced);
            }
            Frame::ScalarValue(state) => {
                let paths = produced.take().expect("scalar value paths retained");
                let next = state.finish(program, paths, work)?;
                schedule_scalar!(next, frames, produced);
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
                // Scalar decisions authenticate each selected arm and guard.
                if crate::hir::is_refutable_match_scalar(&scrutinee.ty) {
                    let next = scalar_match::State::new(
                        program,
                        function,
                        expression,
                        scrutinee,
                        arms,
                        scrutinee_paths,
                        work,
                    )?
                    .next(work)?;
                    schedule_scalar!(next, frames, produced);
                    continue;
                }
                let is_record =
                    validate_match_shape(program, function, expression, scrutinee, arms, work)?;
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
                                typed_storage::unsupported_provider(work)?;
                                if record_destructure::contains_nested(
                                    program.declarations(),
                                    instance,
                                    fields,
                                ) {
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
                                        let mut field_source = work
                                            .clone_payload(&source, "nested match source clone")?;
                                        for field in binding.path {
                                            work.budget.reserve_vec(
                                                function,
                                                &mut field_source.projections,
                                                1,
                                                "nested match projection slots",
                                            )?;
                                            field_source.projections.push(field);
                                        }
                                        typed_storage::push_observation(
                                            &mut path.observations,
                                            SkeletonObservation::Transfer {
                                                at: expression.id.clone(),
                                                source: field_source,
                                                destination: CleanupPlace::whole(StorageId::Value(
                                                    binding.binding,
                                                )),
                                            },
                                            work,
                                            "nested record transfer observation",
                                        )?;
                                    }
                                    continue;
                                }
                                // Declaration order independently authenticates transfers.
                                let declarations = program
                                    .declarations()
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
                                    let mut projected =
                                        work.clone_payload(&source, "record match source clone")?;
                                    work.budget.reserve_vec(
                                        function,
                                        &mut projected.projections,
                                        1,
                                        "record match projection slots",
                                    )?;
                                    let field_identity = work
                                        .clone_payload(&field.field, "record match field clone")?;
                                    projected.projections.push(field_identity);
                                    typed_storage::push_observation(
                                        &mut path.observations,
                                        SkeletonObservation::Transfer {
                                            at: expression.id.clone(),
                                            source: projected,
                                            destination: CleanupPlace::whole(StorageId::Value(
                                                binding.id.clone(),
                                            )),
                                        },
                                        work,
                                        "record transfer observation",
                                    )?;
                                }
                            } else {
                                // A borrowed match observes a named owned or borrowed place without moving any cleanup epoch. The match expression itself
                                // cannot forward that place as an owned result.
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
                // Both original arm expressions have been independently scanned.
                // Project only the impossible Done runtime path out at the join.
                let done_in_true_body = index == 0
                    && active_loops.last().is_some_and(|context| {
                        context
                            .true_match
                            .is_some_and(|value| std::ptr::eq(value, expression))
                    });
                let next_remaining = if done_in_true_body {
                    remaining
                } else {
                    finish_match_arm(
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
                    )?
                };
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
    program: &dyn crate::cleanup::ReplayContext,
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
) -> Result<Option<StagedCopyResultSource>, Diagnostic> {
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
        let declaration = program.declarations().declaration(id).ok_or_else(|| {
            replay_error(
                function,
                format_args!("postfix `?` references unknown `{id}`"),
            )
        })?;
        if declaration.identity_origin != IdentityOrigin::CompilerOwned {
            return Err(replay_error(
                function,
                format_args!("postfix `?` reference `{id}` is not compiler-owned"),
            ));
        }
    }
    let source_arguments = replay_result_arguments(function, &operand.ty, result)?;
    let target_arguments = replay_result_arguments(function, residual_type, result)?;
    let exact_owned = exact_owned_try(source_arguments, target_arguments);
    if source_arguments.len() != 2
        || target_arguments.len() != 2
        || (!exact_owned
            && source_arguments
                .iter()
                .chain(target_arguments.iter())
                .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)))
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
        let facts = paid_type_facts::flags(program.declarations(), function, ty, work.budget)?
            .ok_or_else(|| {
                replay_error(function, "postfix `?` Result instance has no type facts")
            })?;
        let valid = if exact_owned {
            !facts.copy && facts.sized && !facts.contains_resource && facts.needs_drop
        } else {
            facts.copy && facts.sized && !facts.contains_resource && !facts.needs_drop
        };
        if !valid {
            return Err(replay_error(
                function,
                "postfix `?` is outside its exact Result cleanup slice",
            ));
        }
    }
    if exact_owned {
        let expected_ownership = if expression.ty == ResolvedType::Bytes {
            OwnershipMode::Own
        } else {
            OwnershipMode::Value
        };
        if expression.ownership != expected_ownership {
            return Err(replay_error(
                function,
                "owned postfix `?` has inconsistent ownership or instance identity",
            ));
        }
        return Ok(None);
    }
    Ok(Some(StagedCopyResultSource::TryResidual {
        expression: work.clone_owned(&expression.id, "try source expression clone")?,
        operand: work.clone_owned(&operand.id, "try source operand clone")?,
        source_instance: work.clone_owned(&operand.ty, "try source instance clone")?,
        target_instance: work.clone_owned(residual_type, "try target instance clone")?,
        result: work.clone_owned(result, "try Result identity clone")?,
        ok_case: work.clone_owned(ok_case, "try Ok identity clone")?,
        ok_field: work.clone_owned(ok_field, "try Ok field clone")?,
        err_case: work.clone_owned(err_case, "try Err identity clone")?,
        err_field: work.clone_owned(err_field, "try Err field clone")?,
    }))
}

#[allow(clippy::too_many_arguments)]
fn authenticated_try_option_stage_source(
    program: &dyn crate::cleanup::ReplayContext,
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
        let declaration = program.declarations().declaration(id).ok_or_else(|| {
            replay_error(
                function,
                format_args!("Option postfix `?` references unknown `{id}`"),
            )
        })?;
        if declaration.identity_origin != IdentityOrigin::CompilerOwned {
            return Err(replay_error(
                function,
                format_args!("Option postfix `?` reference `{id}` is not compiler-owned"),
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
        let facts = paid_type_facts::flags(program.declarations(), function, ty, work.budget)?
            .ok_or_else(|| {
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

fn validate_match_shape(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arms: &[ResolvedMatchArm],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        return lease.match_shape(program, function, expression, work.budget);
    }
    let _ = work;
    validate_match_skeleton_shape(program, function, expression, scrutinee, arms)
}

fn validate_match_skeleton_shape(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arms: &[ResolvedMatchArm],
) -> Result<bool, Diagnostic> {
    if type_needs_drop(program, function, &expression.ty)?
        && !record_destructure::admits_owned_match_result(
            program, function, expression, scrutinee, arms,
        )
    {
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
            .declarations()
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
        | ResolvedType::StringMap
        | ResolvedType::TypeParameter { .. }
        | ResolvedType::OnceFunction
        | ResolvedType::OnceFunctionI64
        | ResolvedType::OnceFunctionI64Pair
        | ResolvedType::MutFunctionI64
        | ResolvedType::Function { .. } => false,
        ResolvedType::Never => {
            return Err(replay_error(
                function,
                "match scrutinee cannot have terminal flow type",
            ));
        }
    };
    let is_variant = match &scrutinee.ty {
        ResolvedType::Nominal { declaration, .. } => program
            .declarations()
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
            let declarations = program
                .declarations()
                .record_fields(record)
                .ok_or_else(|| {
                    replay_error(function, "explicit record match has no field inventory")
                })?;
            let ResolvedMatchPattern::Record { instance, .. } = &arm.pattern else {
                unreachable!("record pattern matched above")
            };
            let ResolvedType::Nominal { arguments, .. } = instance else {
                return Err(replay_error(
                    function,
                    "explicit record match instance is not nominal",
                ));
            };
            if record_destructure::contains_nested(program.declarations(), instance, fields) {
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
                let field_ty = nested_shape::substitute_type_current(
                    program,
                    function,
                    &declaration.ty,
                    record,
                    arguments,
                )?;
                let field = fields
                    .iter()
                    .find(|field| field.field == declaration.id)
                    .ok_or_else(|| {
                        replay_error(function, "explicit record pattern is incomplete")
                    })?;
                let expected = if type_needs_drop(program, function, &field_ty)? {
                    expected_scrutinee_ownership
                } else {
                    OwnershipMode::Value
                };
                match &field.pattern {
                    ResolvedRecordMatchFieldPattern::Binding(binding)
                        if binding.ty == field_ty && binding.ownership == expected => {}
                    ResolvedRecordMatchFieldPattern::Wildcard
                        if *mode == crate::hir::ResolvedMatchMode::Borrow
                            || !type_needs_drop(program, function, &field_ty)? => {}
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
fn finish_match_arm<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    expression: &'hir ResolvedExpr,
    scrutinee: &'hir ResolvedExpr,
    arms: &'hir [ResolvedMatchArm],
    index: usize,
    remaining: Vec<ExprSkeletonPath>,
    arm_paths: &[ExprSkeletonPath],
    results: &mut Vec<ExprSkeletonPath>,
    is_record: bool,
    work: &mut SkeletonWork<'hir, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    if arms[index].guard.is_some() {
        return guarded_variant::finish_arm(
            program,
            function,
            expression,
            scrutinee,
            &arms[index],
            index + 1 == arms.len(),
            remaining,
            arm_paths,
            results,
            work,
        );
    }
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
            let selected = finish_owned(
                program,
                function,
                expression,
                &arms[index].value,
                selected,
                work,
            )?;
            append_expr_paths(results, selected, work, "match selected result")?;
            continue;
        }
        let cases = arms[index].pattern.case_iter().ok_or_else(|| {
            replay_error(
                function,
                "wildcard match arm must be the final exhaustive arm",
            )
        })?;
        for case in cases {
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
            let selected = finish_owned(
                program,
                function,
                expression,
                &arms[index].value,
                selected,
                work,
            )?;
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
        }
        work.push_expr_path(&mut next_remaining, path, "match remaining scrutinee path")?;
    }
    Ok(next_remaining)
}

fn prepare_selected_match_path(
    program: &dyn crate::cleanup::ReplayContext,
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
            let source = work
                .clone_payload(&path.owned_source, "selected match owned source clone")?
                .ok_or_else(|| {
                    replay_error(function, "owned variant match has no HIR cleanup source")
                })?;
            for field in fields {
                if !type_needs_drop(program, function, &field.binding.ty)? {
                    continue;
                }
                let mut field_source =
                    work.clone_owned(&source, "owned variant match source clone")?;
                typed_storage::push_projection(
                    &mut field_source,
                    case,
                    "owned variant match case clone",
                    work,
                )?;
                typed_storage::push_projection(
                    &mut field_source,
                    &field.field,
                    "owned variant match field clone",
                    work,
                )?;
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

/// Clone a guarded scalar-match prefix under its existing payload reservation.
fn clone_expr_path_shallow(
    path: &ExprSkeletonPath,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<ExprSkeletonPath, Diagnostic> {
    work.clone_payload(path, "scalar match guarded prefix clone")
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
    #[cfg(test)]
    if path_array::main696::selected() {
        return path_array::main696::append_expr_paths(target, paths, work, phase);
    }
    let minimum = path_array::count!(work, false, paths.len(), phase);
    path_array::select!(first, minimum, target, work);
    for path in paths {
        path_array::push!(work, first, minimum, target, path, phase);
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
    program: &dyn crate::cleanup::ReplayContext,
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
    let declarations = program
        .declarations()
        .record_fields(record)
        .ok_or_else(|| {
            replay_error(
                function,
                format_args!("record update has unknown record `{record}`"),
            )
        })?;
    let arguments = record_destructure::update::concrete_arguments(function, expression, record)?;
    for field in declarations {
        let field_ty =
            nested_shape::substitute_type_current(program, function, &field.ty, record, arguments)?;
        if replaced.contains(&field.id) || !type_needs_drop(program, function, &field_ty)? {
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
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    operand_paths: Vec<ExprSkeletonPath>,
    option: bool,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let (operand, success_case, success_field, source) = if option {
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
            None,
            Some(authenticated_try_option_stage_source(
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
            )?),
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
            Some(ok_field),
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
        if source.is_none() && expression.ownership == OwnershipMode::Own {
            let mut payload = success.owned_source.take().ok_or_else(|| {
                replay_error(function, "owned postfix `?` success has no cleanup source")
            })?;
            payload
                .projections
                .push(work.clone_owned(success_case, "try success case projection")?);
            payload.projections.push(work.clone_owned(
                success_field.expect("owned Result try has an Ok field"),
                "try success field projection",
            )?);
            let destination = temporary_place(expression, work)?;
            let at = work.clone_owned(&expression.id, "try success transfer identity")?;
            let observed_destination =
                work.clone_owned(&destination, "try success transfer destination")?;
            work.push_observation(
                &mut success,
                SkeletonObservation::Transfer {
                    at,
                    source: payload,
                    destination: observed_destination,
                },
                "owned try success transfer",
            )?;
            success.owned_source = Some(destination);
        } else if source.is_none() {
            success.owned_source = None;
        }
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
        if let Some(source) = &source {
            let staged_source = work.clone_owned(source, "try staged-result source clone")?;
            compact_stage::prepare!(work.function, work.budget, &staged_source);
            let staged_source = CopySource::new(work.function, work.budget, staged_source)?;
            work.push_observation(
                &mut residual,
                SkeletonObservation::StageCopyResult(staged_source),
                "try residual staging observation",
            )?;
        } else {
            let owned_source = residual.owned_source.take().ok_or_else(|| {
                replay_error(function, "owned postfix `?` residual has no cleanup source")
            })?;
            let (owned_source, destination) =
                owned_try_residual_places(function, expression, owned_source, work)?;
            let result_place =
                owned_try_residual_result_place(program, function, expression, &destination)?;
            let at = work.clone_owned(&expression.id, "try residual transfer identity")?;
            let observed_destination =
                work.clone_owned(&destination, "try residual transfer destination")?;
            work.push_observation(
                &mut residual,
                SkeletonObservation::Transfer {
                    at,
                    source: owned_source,
                    destination: observed_destination,
                },
                "owned try residual transfer",
            )?;
            residual.owned_source = Some(result_place);
        }
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
    program: &dyn crate::cleanup::ReplayContext,
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
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    parameter: resolved_call::Param<'_>,
    argument: &ResolvedExpr,
    index: usize,
    states: Vec<CallSkeletonState>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<CallSkeletonState>, Diagnostic> {
    let states = work.merged_call_states(states)?;
    let suffixes = work.merged_suffixes(suffixes)?;
    let share_prefix = work.shares_prefix_tail(&suffixes)?;
    let mut next = Vec::new();
    for (mut prefix, commits) in states {
        if prefix.failed || prefix.residual {
            work.charge(1, "short-circuited call state push")?;
            note_skeleton_materialization();
            work.budget
                .reserve_vec(function, &mut next, 1, "short circuit call slots")?;
            next.push((prefix, commits));
            continue;
        }
        if share_prefix {
            work.share_prefix_tail(&mut prefix.observations)?;
        }
        for suffix in suffixes.iter() {
            let observations =
                work.clone_observations(&prefix.observations, "call prefix clone")?;
            let observations = work.extend_owned_observations(
                observations,
                &suffix.observations,
                "call suffix clone",
            )?;
            let owned_source =
                work.clone_owned(&suffix.owned_source, "call suffix owned-source clone")?;
            let mut path = ExprSkeletonPath {
                observations,
                owned_source,
                failed: suffix.failed,
                residual: suffix.residual,
                continuation: lexical_skeleton::Continuation::sequenced(
                    &prefix.continuation,
                    &suffix.continuation,
                ),
            };
            let mut path_commits = work.clone_owned(&commits, "call commit-state clone")?;
            if !path.failed
                && !path.residual
                && parameter.ownership == OwnershipMode::Own
                && type_needs_drop(program, function, parameter.ty)?
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
                work.budget.reserve_vec(
                    function,
                    &mut path_commits,
                    1,
                    "staged call commit slots",
                )?;
                path_commits.push((parameter_index, epoch));
            }
            work.charge(1, "call state push")?;
            note_skeleton_materialization();
            work.budget
                .reserve_vec(function, &mut next, 1, "call state slots")?;
            next.push((path, path_commits));
        }
    }
    Ok(next)
}

fn split_status_paths(
    paths: Vec<ExprSkeletonPath>,
    source: StatusSourceId,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    #[cfg(test)]
    if path_array::main696::selected() {
        return path_array::main696::split_status_paths(paths, source, work);
    }
    let mut results = Vec::new();
    let minimum = path_array::count!(work, false, paths.len(), "status path capacity");
    path_array::select!(first, minimum, &results, work);
    for mut path in paths {
        if path.failed || path.residual {
            path_array::push!(
                work,
                first,
                minimum,
                &mut results,
                path,
                "short-circuited status path"
            );
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
        path_array::push!(
            work,
            first,
            minimum,
            &mut results,
            failure,
            "status failure path"
        );
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
        path_array::push!(
            work,
            first,
            minimum,
            &mut results,
            path,
            "status success path"
        );
    }
    Ok(results)
}

fn split_contract(
    paths: Vec<ExprSkeletonPath>,
    contract: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    #[cfg(test)]
    if path_array::main696::selected() {
        return path_array::main696::split_contract(paths, contract, work);
    }
    let mut results = Vec::new();
    let minimum = path_array::count!(work, false, paths.len(), "contract path capacity");
    path_array::select!(first, minimum, &results, work);
    for mut path in paths {
        if path.failed || path.residual {
            path_array::push!(
                work,
                first,
                minimum,
                &mut results,
                path,
                "short-circuited contract path"
            );
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
        path_array::push!(
            work,
            first,
            minimum,
            &mut results,
            failure,
            "contract failure path"
        );
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
        path_array::push!(
            work,
            first,
            minimum,
            &mut results,
            path,
            "contract success path"
        );
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
            replay_error(
                function,
                format_args!("{description} has no HIR cleanup source"),
            )
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
        (plan.entry, Vec::<std::rc::Rc<SkeletonObservation>>::new()),
        "cleanup-plan root state push",
    )?;
    let mut paths = Vec::new();
    while let Some((block, mut observations)) = queue.pop_front() {
        let block = &plan.blocks[block.0 as usize];
        // Charge only work performed at this block. The previous charge used the entire accumulated observation length on every linear block, turning a
        // depth-D skewed conditional into artificial O(D^3) work. Observation history is copied only at a real branch, charged below immediately before
        // each clone.
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
                CleanupTransition::ReserveRenewal { at, binding } => {
                    let observation = SkeletonObservation::ReserveRenewal {
                        at: skeleton_clone(budget, function, at, "renewal expression")?,
                        binding: skeleton_clone(budget, function, binding, "renewal binding")?,
                    };
                    skeleton_push(
                        budget,
                        function,
                        &mut observations,
                        observation,
                        "renewal reservation",
                    )?;
                }
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                }
                | CleanupTransition::Renew {
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
                    compact_stage::prepare!(function, budget, &source);
                    let source = CopySource::new(function, budget, source)?;
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
                budget.reserve_bytes(
                    function,
                    final_output::PLAN_SELECTION_CONTROLS,
                    "plan final-output selection controls",
                )?;
                if edges.len() == 2 && observations.len() < observations.capacity() {
                    let edge = &plan.edges[edges[0].0 as usize];
                    let observation = final_output::observation!(edge, budget, function);
                    budget.reserve_bytes(
                        function,
                        branch_clone::inline_controls::<std::rc::Rc<SkeletonObservation>>(),
                        "branch clone receiving controls",
                    )?;
                    // The branch producer has no Result<Vec<_>, Diagnostic>
                    // receiver or owned observation forwarding argument. Its
                    // nested work/census/payment Results retain their charges.
                    let mut branch;
                    if !budget.pays_allocations() {
                        branch = skeleton_clone(
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
                    } else {
                        budget.charge_skeleton(
                            function,
                            1,
                            "cleanup-plan skeleton branch clone",
                        )?;
                        budget.reserve_clone(
                            function,
                            &observations,
                            "cleanup-plan skeleton branch clone",
                        )?;
                        note_skeleton_materialization();
                        // Original push work precedes every extra backing debit.
                        budget.charge_skeleton(function, 1, "plan branch observation push")?;
                        note_skeleton_materialization();
                        let capacity = if observations.is_empty() {
                            1
                        } else {
                            if observations.len() > usize::MAX / 2 {
                                return Err(budget.budget_refusal(
                                    function,
                                    "cleanup replay branch capacity overflowed",
                                    "",
                                ));
                            }
                            observations.len() * 2
                        };
                        budget.reserve_array::<std::rc::Rc<SkeletonObservation>>(
                            function,
                            capacity - observations.len(),
                            "plan branch observation push",
                        )?;
                        budget.reserve_bytes(
                            function,
                            physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(
                                &observation,
                            ),
                            "plan branch observation push",
                        )?;
                        branch = Vec::with_capacity(capacity);
                        branch.extend(observations.iter().cloned());
                        branch.push(observation.into());
                    }
                    skeleton_queue_push(
                        budget,
                        function,
                        &mut queue,
                        (edge.to, branch),
                        "plan branch state push",
                    )?;
                    let edge = &plan.edges[edges[1].0 as usize];
                    let observation = final_output::observation!(edge, budget, function);
                    budget.reserve_bytes(
                        function,
                        final_output::PLAN_MOVE_CONTROLS,
                        "plan final-output move controls",
                    )?;
                    budget.charge_skeleton(function, 1, "cleanup-plan skeleton branch clone")?;
                    note_skeleton_materialization();
                    #[cfg(test)]
                    final_output::record_move(observations.len(), observations.capacity());
                    let mut branch = observations;
                    budget.charge_skeleton(function, 1, "plan branch observation push")?;
                    note_skeleton_materialization();
                    budget.reserve_bytes(function, 0, "plan branch observation push")?;
                    budget.reserve_bytes(
                        function,
                        physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(
                            &observation,
                        ),
                        "plan branch observation push",
                    )?;
                    branch.push(observation.into());
                    skeleton_queue_push(
                        budget,
                        function,
                        &mut queue,
                        (edge.to, branch),
                        "plan branch state push",
                    )?;
                    continue;
                }
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
                    budget.reserve_bytes(
                        function,
                        branch_clone::inline_controls::<std::rc::Rc<SkeletonObservation>>(),
                        "branch clone receiving controls",
                    )?;
                    // The branch producer has no Result<Vec<_>, Diagnostic>
                    // receiver or owned observation forwarding argument. Its
                    // nested work/census/payment Results retain their charges.
                    let mut branch;
                    if !budget.pays_allocations() {
                        branch = skeleton_clone(
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
                    } else {
                        budget.charge_skeleton(
                            function,
                            1,
                            "cleanup-plan skeleton branch clone",
                        )?;
                        budget.reserve_clone(
                            function,
                            &observations,
                            "cleanup-plan skeleton branch clone",
                        )?;
                        note_skeleton_materialization();
                        // Original push work precedes every extra backing debit.
                        budget.charge_skeleton(function, 1, "plan branch observation push")?;
                        note_skeleton_materialization();
                        let capacity = if observations.is_empty() {
                            1
                        } else {
                            if observations.len() > usize::MAX / 2 {
                                return Err(budget.budget_refusal(
                                    function,
                                    "cleanup replay branch capacity overflowed",
                                    "",
                                ));
                            }
                            observations.len() * 2
                        };
                        budget.reserve_array::<std::rc::Rc<SkeletonObservation>>(
                            function,
                            capacity - observations.len(),
                            "plan branch observation push",
                        )?;
                        budget.reserve_bytes(
                            function,
                            physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(
                                &observation,
                            ),
                            "plan branch observation push",
                        )?;
                        branch = Vec::with_capacity(capacity);
                        branch.extend(observations.iter().cloned());
                        branch.push(observation.into());
                    }
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
                match &exit.continuation {
                    ExitContinuation::LoopBreak { edge, .. }
                    | ExitContinuation::FunctionReturn { edge, .. } => {
                        if let Some(marker) =
                            lexical_skeleton::plan_exit_observation(budget, function, exit.id)?
                        {
                            skeleton_push(
                                budget,
                                function,
                                &mut observations,
                                marker,
                                "plan lexical exit observation push",
                            )?;
                        }
                        skeleton_queue_push(
                            budget,
                            function,
                            &mut queue,
                            (plan.edges[edge.0 as usize].to, observations),
                            "plan lexical continuation state push",
                        )?;
                    }
                    ExitContinuation::LoopLatch { target } => {
                        let marker = match lexical_skeleton::plan_exit_observation(
                            budget, function, exit.id,
                        )? {
                            Some(marker) => marker,
                            None => SkeletonObservation::LoopLatch {
                                target: skeleton_clone(
                                    budget,
                                    function,
                                    target,
                                    "plan loop-latch target clone",
                                )?,
                            },
                        };
                        skeleton_push(
                            budget,
                            function,
                            &mut observations,
                            marker,
                            "plan loop-latch observation push",
                        )?;
                        match lexical_cert::latch_resume(plan, exit.id) {
                            Some(entry) => skeleton_queue_push(
                                budget,
                                function,
                                &mut queue,
                                (entry, observations),
                                "plan generated-latch state push",
                            )?,
                            None => skeleton_push(
                                budget,
                                function,
                                &mut paths,
                                SkeletonPath {
                                    observations: observations.into(),
                                    terminal: SkeletonTerminal::LoopLatch,
                                },
                                "plan loop-latch path push",
                            )?,
                        }
                    }
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
                                observations: observations.into(),
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
                            observations: observations.into(),
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

fn expression_has_nested_record_destructure(
    program: &dyn crate::cleanup::ReplayContext,
    expression: &ResolvedExpr,
) -> bool {
    expression_has_kind(expression, |kind| {
        matches!(
            kind,
            ResolvedExprKind::Match {
                mode: crate::hir::ResolvedMatchMode::Own | crate::hir::ResolvedMatchMode::Borrow,
                arms,
                ..
            } if arms.iter().any(|arm| matches!(
                &arm.pattern,
                ResolvedMatchPattern::Record { instance, fields, .. }
                    if record_destructure::contains_nested(program.declarations(), instance, fields)
            ))
        )
    })
}

#[cfg(test)]
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

#[cfg(test)]
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
            ExitContinuation::Continue(edge)
            | ExitContinuation::LoopBreak { edge, .. }
            | ExitContinuation::FunctionReturn { edge, .. } => {
                edge_references[edge.0 as usize] += 1
            }
            ExitContinuation::ReturnFailure { source } => {
                status_references.insert(source.clone());
            }
            ExitContinuation::CommitResult { .. }
            | ExitContinuation::ReturnUnit
            | ExitContinuation::LoopLatch { .. } => {}
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
            ExitContinuation::Continue(edge)
            | ExitContinuation::LoopBreak { edge, .. }
            | ExitContinuation::FunctionReturn { edge, .. } => vec![*edge],
            ExitContinuation::LoopLatch { .. } => {
                // A generated advancement proof fragment has no CFG edge; its
                // certified entry is the only implicit successor.
                return lexical_cert::latch_resume(plan, *exit)
                    .into_iter()
                    .collect();
            }
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
    expressions: &BTreeMap<ExpressionId, Option<CallFact<'_>>>,
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
    leaves: &Leaves,
) -> Result<Vec<LivenessFlagId>, Diagnostic> {
    if !storage.contains(&place.storage) {
        return Err(replay_error(
            function,
            "cleanup place references unknown storage",
        ));
    }
    let under = leaves.under(place);
    if under.is_empty() {
        return Err(replay_error(
            function,
            "cleanup place has no droppable leaf",
        ));
    }
    Ok(under)
}

fn require_expression(
    function: &ResolvedFunction,
    expressions: &BTreeMap<ExpressionId, Option<CallFact<'_>>>,
    expression: &ExpressionId,
) -> Result<(), Diagnostic> {
    if expressions.contains_key(expression) {
        Ok(())
    } else {
        Err(replay_error(
            function,
            format_args!("cleanup plan references unknown expression `{expression}`"),
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
            format_args!(
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
    u32::try_from(index).map_err(|_| replay_error(function, format_args!("too many {what}s")))
}

/// Borrowing message bridge for `replay_error`. Nothing is owned until the
/// route is chosen: the ordinary route reproduces the original allocations, the
/// test-only funded route counts and prepays before constructing any text.
enum ReplayMessage<'a> {
    Borrowed(&'a str),
    Owned(String),
    Formatted(std::fmt::Arguments<'a>),
}

impl<'a> From<&'a str> for ReplayMessage<'a> {
    fn from(message: &'a str) -> Self {
        Self::Borrowed(message)
    }
}

impl<'a> From<&'a String> for ReplayMessage<'a> {
    fn from(message: &'a String) -> Self {
        Self::Borrowed(message.as_str())
    }
}

impl From<String> for ReplayMessage<'_> {
    fn from(message: String) -> Self {
        Self::Owned(message)
    }
}

impl<'a> From<std::fmt::Arguments<'a>> for ReplayMessage<'a> {
    fn from(message: std::fmt::Arguments<'a>) -> Self {
        Self::Formatted(message)
    }
}

impl ReplayMessage<'_> {
    /// The original owned argument: borrowed text converts as `Into<String>`
    /// did, an owned String moves unchanged, arguments format once as the
    /// original pre-call `format!` did.
    fn into_original(self) -> String {
        match self {
            Self::Borrowed(message) => message.into(),
            Self::Owned(message) => message,
            Self::Formatted(arguments) => std::fmt::format(arguments),
        }
    }
}

fn replay_error<'a>(
    function: &ResolvedFunction,
    message: impl Into<ReplayMessage<'a>>,
) -> Diagnostic {
    let message = message.into();
    #[cfg(test)]
    match allocation::authority_for_function(function) {
        Ok(None) => {}
        Ok(Some(lease)) => return funded_replay_error(lease, function, message),
        Err(_) => {
            // A stale or expired subject never falls back to the production
            // ledger or the ordinary allocation route.
            return Diagnostic::io_static(
                "SPX-H006",
                "oracle invocation authority refused a replay diagnostic",
            );
        }
    }
    replay_error_old(function, message.into_original())
}

/// Funded route. Borrowed and formatted messages are counted and prepaid before
/// any diagnostic text exists. An already owned String was allocated by the
/// caller before this call and cannot be prepaid: the account records that as a
/// sticky physical failure so the live hook fails visibly, never a free pass.
#[cfg(test)]
fn funded_replay_error(
    lease: allocation::OracleLease,
    function: &ResolvedFunction,
    message: ReplayMessage<'_>,
) -> Diagnostic {
    let result = match &message {
        ReplayMessage::Borrowed(text) => {
            allocation::replay_error_funded(lease, function, format_args!("{text}"))
        }
        ReplayMessage::Formatted(arguments) => {
            allocation::replay_error_funded(lease, function, *arguments)
        }
        ReplayMessage::Owned(text) => {
            lease.fail_physical("replay diagnostic message was allocated before its prepayment");
            allocation::replay_error_funded(lease, function, format_args!("{text}"))
        }
    };
    match result {
        Ok(diagnostic) => diagnostic,
        Err(failure) => failure.into_diagnostic(),
    }
}

/// Original `replay_error` body, unchanged and independent of the bridge.
fn replay_error_old(function: &ResolvedFunction, message: impl Into<String>) -> Diagnostic {
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
mod oracle_controls;

#[cfg(test)]
#[path = "replay_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "nested_owned_records_tests.rs"]
mod nested_owned_records_tests;

#[cfg(test)]
#[path = "nested_record_destructure_tests.rs"]
mod nested_record_destructure_tests;

#[cfg(test)]
pub(super) mod phase_measurement;

#[cfg(test)]
mod live_oracle;
#[cfg(test)]
pub(crate) use live_oracle::with_financed_test_oracles;

#[cfg(test)]
pub(crate) use factored::EpsilonAppCensus;
