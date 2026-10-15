//! Bounded target-neutral shared-loan proof attachment.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{BinaryOp, Span};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    ExpressionId, OwnershipMode, Place, PlaceProjection, ResolvedExpr, ResolvedExprKind,
    ResolvedFunction, ResolvedMatchPattern, ResolvedProgram, ResolvedRecordMatchFieldPattern,
    ResolvedStatement, ResolvedType, ValueId,
};

pub(crate) use storage::PlannerError as FundedError;

pub(crate) fn build_plan_with_reservation(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    payer: &crate::bounded_output::cfg_reservation::CfgReservation,
) -> Result<LoanPlan, FundedError> {
    funded_flat::build(program, function, payer)
}

pub const LOAN_PLAN_SCHEMA_V1: &str = "semaprax.loan-plan.v1";
pub const LOAN_PLAN_SCHEMA_V2: &str = "semaprax.loan-plan.v2";
pub const LOAN_PLAN_SCHEMA_V3: &str = "semaprax.loan-plan.v3";
pub const MAX_LOANS_PER_FUNCTION_V1: usize = 256;
pub const MAX_LOAN_ENDPOINTS_V1: usize = 4_096;
pub const MAX_LOAN_EDGES_V1: usize = 4_096;
pub const MAX_LOAN_PLAN_WORK_V1: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LoanId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LoanPointPhase {
    Before,
    After,
}

mod adjacency;
#[cfg(test)]
mod boundary_tests;
mod call_arguments;
#[cfg(test)]
mod call_arguments_legacy;
mod cfg_directory;
mod children;
#[cfg(test)]
mod children_tests;
mod compact_ids;
use call_arguments::borrowed_call_arguments;
use children::evaluation_children;
mod directory_access;
mod funded_flat;
#[cfg(test)]
mod funded_flat_legacy;
mod guidance;
#[cfg(test)]
mod legacy_adjacency;
mod lexical_exit;
mod native_view;
#[cfg(test)]
mod native_view_legacy;
mod origin;
pub(crate) mod owned_capacity;
mod paid_candidate;
#[cfg(test)]
mod paid_candidate_before;
#[cfg(test)]
mod paid_candidate_legacy;
mod paid_cfg;
mod paid_children;
mod paid_cursor;
mod paid_live;
mod paid_materialize;
#[cfg(test)]
mod paid_materialize_legacy;
mod paid_patterns;
#[cfg(test)]
mod paid_patterns_legacy;
mod paid_plan;
#[cfg(test)]
mod paid_plan_legacy;
mod paid_predicates;
#[cfg(test)]
mod paid_predicates_legacy;
mod paid_substitute;
mod provisional_result;
mod record_vec_borrow;
#[cfg(test)]
mod saved_planner;
#[cfg(test)]
mod shared_identity_tests;
mod storage;
pub(crate) mod traversal;
#[cfg(test)]
mod whole_oracle_tests;
mod work;

pub(crate) use work::{REACHABILITY_BYTES_PER_EXPRESSION, REACHABILITY_FIXED_BYTES};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LoanProgramPoint {
    pub expression: ExpressionId,
    pub phase: LoanPointPhase,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LoanCause {
    SliceView,
    StrView,
    TraversalFreeze,
    BorrowedCall { argument: u16 },
    MatchBorrow { arm: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Loan {
    pub id: LoanId,
    /// Stable semantic borrow site, distinct from the path-specific start.
    pub site: ExpressionId,
    pub origin: Place,
    pub parent: Option<LoanId>,
    pub start: LoanProgramPoint,
    pub ends: Vec<LoanProgramPoint>,
    /// Canonical CFG edges on which this loan stops being live. Each entry is
    /// an index into [`LoanPlan::edges`] and disambiguates equal join points.
    pub end_edges: Vec<u16>,
    pub cause: LoanCause,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanEndpoint {
    pub point: LoanProgramPoint,
    /// Union summary over incoming CFG edges. [`LoanEdge::live`] is the
    /// authoritative path-exact carrier at joins.
    pub live_before: Box<[LoanId]>,
    pub starts: Box<[LoanId]>,
    /// Edge-qualified terminations are authoritative in [`Loan::end_edges`];
    /// this is the deterministic node-level union summary.
    pub kills: Box<[LoanId]>,
    /// Union summary over outgoing CFG edges.
    pub live_after: Box<[LoanId]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanEdge {
    /// Dense endpoint index of the CFG source point.
    pub from: u16,
    /// Dense endpoint index of the CFG destination point.
    pub to: u16,
    /// Exact simultaneous loans live on this one control-flow edge.
    pub live: Box<[LoanId]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanPlan {
    pub schema: &'static str,
    pub loans: Vec<Loan>,
    pub endpoints: Vec<LoanEndpoint>,
    pub edges: Vec<LoanEdge>,
}

impl LoanPlan {
    pub(crate) fn unresolved() -> Self {
        Self {
            schema: "unresolved",
            loans: Vec::new(),
            endpoints: Vec::new(),
            edges: Vec::new(),
        }
    }

    fn empty_v1() -> Self {
        Self {
            schema: LOAN_PLAN_SCHEMA_V1,
            loans: Vec::new(),
            endpoints: Vec::new(),
            edges: Vec::new(),
        }
    }
}

pub fn build_plan(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<LoanPlan, Diagnostic> {
    let lexical = crate::hir::lexical_exit::requires(function)?;
    if lexical {
        let owner = crate::hir::lexical_exit::owner(program, function)?;
        crate::hir::lexical_exit::validate_function(program, function, &owner)?;
    }
    has_own_root_candidate(program, function)
        .and_then(|candidate| {
            if candidate {
                build_cfg_plan(program, function)
            } else {
                let mut plan = LoanPlan::empty_v1();
                if lexical {
                    plan.schema = LOAN_PLAN_SCHEMA_V3;
                }
                Ok(plan)
            }
        })
        .map_err(|diagnostic| guidance::work_limit(diagnostic, function))
}

/// Exact retained heap capacity owned by a nonempty plan. The inline
/// `LoanPlan` carrier is intentionally excluded so callers can account it in
/// the layout domain that owns the containing function.
pub(crate) fn owned_capacity_bytes(plan: &LoanPlan) -> Option<usize> {
    owned_capacity::owned_capacity_bytes(plan)
}

#[derive(Clone)]
struct CfgDraft<S = BTreeSet<u16>> {
    site: ExpressionId,
    origin: Place,
    origin_span: Span,
    parent_root: Option<ValueId>,
    binding: Option<ValueId>,
    start: u16,
    seeds: S,
    cause: LoanCause,
}

type PaidDraft = CfgDraft<cfg_directory::Set<u16>>;

struct Cfg<'a> {
    points: Vec<LoanProgramPoint>,
    nodes: cfg_directory::Nodes,
    expressions: Vec<&'a ResolvedExpr>,
    edges: Vec<(u16, u16)>,
    adjacency: adjacency::Adjacency,
}

fn has_own_root_candidate(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<bool, Diagnostic> {
    let roots = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(function.ensures.iter())
        .collect::<Vec<_>>();
    let mut expressions = Vec::new();
    let mut pending = roots.iter().rev().copied().collect::<Vec<_>>();
    while let Some(expression) = pending.pop() {
        expressions.push(expression);
        push_children(expression, &mut pending);
    }

    let mut aliases = BTreeMap::<ValueId, Place>::new();
    let mut bound = BTreeMap::<ExpressionId, ValueId>::new();
    let mut ownership = function
        .params
        .iter()
        .map(|param| (param.id.clone(), param.ownership))
        .collect::<BTreeMap<_, _>>();
    for expression in &expressions {
        native_view::inventory_aliases(program, expression, &mut aliases);
        if let ResolvedExprKind::Match { arms, .. } = &expression.kind {
            for arm in arms {
                inventory_pattern_ownership(&arm.pattern, &mut ownership);
            }
        }
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Let { binding, value, .. } = statement {
                    ownership.insert(binding.id.clone(), binding.ownership);
                    if matches!(binding.ty, ResolvedType::SliceU8 | ResolvedType::Str) {
                        if let Some(place) = expression_place(value) {
                            aliases.insert(binding.id.clone(), place);
                            bound.insert(value.id.clone(), binding.id.clone());
                        }
                    }
                }
            }
        }
    }

    let provisional = provisional_result::root(function);
    let mut root_ownership = BTreeMap::<ValueId, bool>::new();
    for expression in expressions {
        if let Some(proof) = traversal::inspect(program, expression)? {
            if expression_place(proof.source)
                .is_some_and(|place| ownership.get(&place.root) == Some(&OwnershipMode::Own))
            {
                return Ok(true);
            }
        }
        // A borrowed byte traversal freezes its view's ultimate owner.
        if let Some(region) = crate::hir::byte_traversal_region(expression)? {
            if let Some(place) = expression_place(region.view) {
                if ultimate_root_is_own(
                    &place.root,
                    &aliases,
                    &ownership,
                    &mut root_ownership,
                    provisional,
                )? {
                    return Ok(true);
                }
            }
        }
        let view = match &expression.kind {
            ResolvedExprKind::Call { .. } => {
                crate::vec_ops::string_slot::place(expression).cloned()
            }
            ResolvedExprKind::BorrowPlace { place, .. } => Some(place.clone()),
            ResolvedExprKind::VecFieldRead { args, .. }
                if matches!(
                    expression.ty,
                    crate::hir::ResolvedType::Str | crate::hir::ResolvedType::SliceU8
                ) =>
            {
                args.first().and_then(expression_place)
            }
            ResolvedExprKind::ByteRange { source, .. } => expression_place(source),
            ResolvedExprKind::NativeRustImportCall(call)
                if matches!(
                    call.result,
                    crate::hir::ResolvedImportResultKind::BorrowedStr { .. }
                ) =>
            {
                call.args.first().and_then(expression_place)
            }
            ResolvedExprKind::Place(place) if bound.contains_key(&expression.id) => {
                Some(place.clone())
            }
            _ => None,
        };
        if let Some(place) = view {
            if ultimate_root_is_own(
                &place.root,
                &aliases,
                &ownership,
                &mut root_ownership,
                provisional,
            )? {
                return Ok(true);
            }
        }
        for (_, argument) in borrowed_call_arguments(program, expression) {
            if let Some(place) = expression_place(argument) {
                if ultimate_root_is_own(
                    &place.root,
                    &aliases,
                    &ownership,
                    &mut root_ownership,
                    provisional,
                )? {
                    return Ok(true);
                }
            }
        }
        if let ResolvedExprKind::Match {
            mode: crate::hir::ResolvedMatchMode::Borrow,
            scrutinee,
            ..
        } = &expression.kind
        {
            if let Some(place) = expression_place(scrutinee) {
                if ultimate_root_is_own(
                    &place.root,
                    &aliases,
                    &ownership,
                    &mut root_ownership,
                    provisional,
                )? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn ultimate_root_is_own(
    root: &ValueId,
    aliases: &BTreeMap<ValueId, Place>,
    ownership: &BTreeMap<ValueId, OwnershipMode>,
    memo: &mut BTreeMap<ValueId, bool>,
    provisional: Option<&ValueId>,
) -> Result<bool, Diagnostic> {
    if let Some(result) = memo.get(root) {
        return Ok(*result);
    }
    let mut current = root.clone();
    let mut chain = Vec::new();
    let mut seen = BTreeSet::new();
    let result = loop {
        if let Some(result) = memo.get(&current) {
            break *result;
        }
        if !seen.insert(current.clone()) {
            return Err(error("shared-loan alias provenance contains a cycle"));
        }
        let Some(parent) = aliases.get(&current) else {
            break ownership.get(&current) == Some(&OwnershipMode::Own)
                || provisional == Some(&current);
        };
        chain.push(current);
        current = parent.root.clone();
    };
    memo.insert(current, result);
    for alias in chain {
        memo.insert(alias, result);
    }
    Ok(result)
}

impl Cfg<'_> {
    fn node(&self, expression: &ResolvedExpr, phase: LoanPointPhase) -> Result<u16, Diagnostic> {
        self.nodes
            .get(&point(expression, phase))
            .copied()
            .ok_or_else(|| error("CFG references an unindexed expression point"))
    }
}

fn build_cfg_plan(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<LoanPlan, Diagnostic> {
    let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
    build_cfg_plan_counted(program, function, &mut work)
}

#[derive(Clone, Copy)]
struct WorkCounter<'payer> {
    allocation: Option<&'payer crate::bounded_output::cfg_reservation::CfgReservation>,
    #[cfg(test)]
    uncached_reachability: bool,
    used: usize,
    limit: usize,
}

impl<'payer> WorkCounter<'payer> {
    fn new(limit: usize) -> Self {
        Self {
            allocation: None,
            used: 0,
            limit,
            #[cfg(test)]
            uncached_reachability: false,
        }
    }
}

/// Build through the production planner while retaining the exact consumed
/// work count for boundary evidence. Focused tests may lower the limit to prove
/// that the first unit past an exact boundary fails closed; this entry point is
/// absent from production builds.
#[cfg(test)]
fn build_cfg_plan_with_work_limit(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    limit: usize,
) -> (Result<LoanPlan, Diagnostic>, usize) {
    let mut work = WorkCounter::new(limit);
    let result = build_cfg_plan_counted(program, function, &mut work);
    (result, work.used)
}

fn build_cfg_plan_counted(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    work: &mut WorkCounter,
) -> Result<LoanPlan, Diagnostic> {
    let cfg = build_cfg(function, work)?;
    let mut aliases = BTreeMap::<ValueId, Place>::new();
    let mut bound = BTreeMap::<ExpressionId, ValueId>::new();
    let mut ownership = function
        .params
        .iter()
        .map(|param| (param.id.clone(), param.ownership))
        .collect::<BTreeMap<_, _>>();
    for expression in &cfg.expressions {
        charge(work)?;
        native_view::inventory_aliases(program, expression, &mut aliases);
        if let ResolvedExprKind::Match { arms, .. } = &expression.kind {
            for arm in arms {
                inventory_pattern_ownership(&arm.pattern, &mut ownership);
            }
        }
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Let { binding, value, .. } = statement {
                    ownership.insert(binding.id.clone(), binding.ownership);
                    if matches!(binding.ty, ResolvedType::SliceU8 | ResolvedType::Str) {
                        if let Some(place) = expression_place(value) {
                            aliases.insert(binding.id.clone(), place);
                            bound.insert(value.id.clone(), binding.id.clone());
                        }
                    }
                }
            }
        }
    }

    let mut byte_census = crate::hir::ByteTraversalSlot::new(program, function)?;
    let mut drafts = Vec::<CfgDraft>::new();
    for expression in &cfg.expressions {
        charge(work)?;
        if let Some(draft) = traversal::draft(program, expression, &cfg, work)? {
            drafts.push(draft);
        }
        if let Some(draft) = traversal::draft_bytes_in(
            program,
            function,
            &mut byte_census,
            expression,
            &cfg,
            &aliases,
            work,
        )? {
            drafts.push(draft);
        }
        let view = match &expression.kind {
            ResolvedExprKind::Call { .. } => {
                crate::vec_ops::string_slot::place(expression).cloned()
            }
            ResolvedExprKind::BorrowPlace { place, .. } => Some(place.clone()),
            ResolvedExprKind::VecFieldRead { args, .. }
                if matches!(
                    expression.ty,
                    crate::hir::ResolvedType::Str | crate::hir::ResolvedType::SliceU8
                ) =>
            {
                args.first().and_then(expression_place)
            }
            ResolvedExprKind::ByteRange { source, .. } => expression_place(source),
            ResolvedExprKind::NativeRustImportCall(call)
                if matches!(
                    call.result,
                    crate::hir::ResolvedImportResultKind::BorrowedStr { .. }
                ) =>
            {
                call.args.first().and_then(expression_place)
            }
            ResolvedExprKind::Place(place) if bound.contains_key(&expression.id) => {
                Some(place.clone())
            }
            _ => None,
        };
        if let Some(place) = view {
            drafts.push(CfgDraft {
                site: expression.id.clone(),
                origin: resolve_origin(&aliases, place.clone(), work)?,
                origin_span: expression.span,
                parent_root: aliases
                    .contains_key(&place.root)
                    .then(|| place.root.clone()),
                binding: bound.get(&expression.id).cloned(),
                start: cfg.node(expression, LoanPointPhase::Before)?,
                seeds: BTreeSet::new(),
                cause: if expression.ty == ResolvedType::Str {
                    LoanCause::StrView
                } else {
                    LoanCause::SliceView
                },
            });
        }
        for (index, argument) in borrowed_call_arguments(program, expression) {
            let place = expression_place(argument)
                .ok_or_else(|| error("borrowed call lacks an exact place origin"))?;
            drafts.push(CfgDraft {
                site: expression.id.clone(),
                origin: resolve_origin(&aliases, place.clone(), work)?,
                origin_span: argument.span,
                parent_root: aliases
                    .contains_key(&place.root)
                    .then(|| place.root.clone()),
                binding: None,
                start: cfg.node(expression, LoanPointPhase::Before)?,
                seeds: [cfg.node(expression, LoanPointPhase::After)?]
                    .into_iter()
                    .collect(),
                cause: LoanCause::BorrowedCall {
                    argument: u16::try_from(index)
                        .map_err(|_| error("borrowed argument index overflows"))?,
                },
            });
        }
        if let ResolvedExprKind::Match {
            mode: crate::hir::ResolvedMatchMode::Borrow,
            scrutinee,
            arms,
        } = &expression.kind
        {
            let place = expression_place(scrutinee)
                .ok_or_else(|| error("borrow match lacks an exact place origin"))?;
            let origin = resolve_origin(&aliases, place.clone(), work)?;
            for (index, arm) in arms.iter().enumerate() {
                let entry = arm.guard.as_deref().unwrap_or(&arm.value);
                drafts.push(CfgDraft {
                    site: expression.id.clone(),
                    origin: origin.clone(),
                    origin_span: scrutinee.span,
                    parent_root: aliases
                        .contains_key(&place.root)
                        .then(|| place.root.clone()),
                    binding: None,
                    start: cfg.node(entry, LoanPointPhase::Before)?,
                    seeds: [cfg.node(&arm.value, LoanPointPhase::After)?]
                        .into_iter()
                        .collect(),
                    cause: LoanCause::MatchBorrow {
                        arm: u16::try_from(index)
                            .map_err(|_| error("match arm index overflows"))?,
                    },
                });
            }
        }
    }

    // Independent HIR scope validation admits this exact result identity only
    // in ensures; it is held owned storage until contract settlement completes.
    drafts.retain(|draft| {
        ownership.get(&draft.origin.root) == Some(&OwnershipMode::Own)
            || provisional_result::root(function) == Some(&draft.origin.root)
    });
    // Borrow-only roots do not need an authenticated loan attachment. Do not
    // retain the transient CFG built to classify candidates: legacy HIR stays
    // heap-identical at this boundary and only real own-root loans carry the
    // endpoint/edge proof.
    if drafts.is_empty() {
        let mut plan = LoanPlan::empty_v1();
        if cfg
            .expressions
            .iter()
            .any(|e| matches!(e.kind, ResolvedExprKind::LexicalExit { .. }))
        {
            plan.schema = LOAN_PLAN_SCHEMA_V3;
        }
        return Ok(plan);
    }
    if drafts.len() > MAX_LOANS_PER_FUNCTION_V1 {
        return Err(error("function exceeds 256 shared loans"));
    }

    let binding_loans = drafts
        .iter()
        .enumerate()
        .filter_map(|(index, draft)| {
            draft
                .binding
                .as_ref()
                .map(|binding| (binding.clone(), LoanId(index as u16)))
        })
        .collect::<BTreeMap<_, _>>();
    for expression in &cfg.expressions {
        let ResolvedExprKind::Place(place) = &expression.kind else {
            continue;
        };
        if let Some(id) = binding_loans.get(&place.root) {
            drafts[id.0 as usize]
                .seeds
                .insert(cfg.node(expression, LoanPointPhase::After)?);
        }
    }
    let match_parents = native_view::match_parents(&cfg.expressions, &drafts);
    let parents = drafts
        .iter()
        .map(|draft| {
            draft.parent_root.clone().and_then(|root| {
                resolve_parent(&aliases, &binding_loans, root.clone())
                    .or_else(|| resolve_parent(&aliases, &match_parents, root))
            })
        })
        .collect::<Vec<_>>();
    #[cfg(not(test))]
    let reachable = work::Reachability::build(&cfg, drafts.iter().map(|draft| draft.start), work)?;
    #[cfg(test)]
    let reachable = if work.uncached_reachability {
        work::Reachability::uncached(&cfg, drafts.iter().map(|draft| draft.start), work)?
    } else {
        work::Reachability::build(&cfg, drafts.iter().map(|draft| draft.start), work)?
    };
    let mut live = drafts
        .iter()
        .enumerate()
        .map(|(index, draft)| {
            work::live_nodes(
                &cfg,
                LoanId(index as u16),
                draft.start,
                &draft.seeds,
                &reachable,
                work,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    for _ in 0..=drafts.len() {
        let mut changed = false;
        for child in (0..drafts.len()).rev() {
            let Some(parent) = parents[child] else {
                continue;
            };
            let parent = parent.0 as usize;
            let before = live[parent].len();
            let mut seeds = drafts[parent].seeds.clone();
            seeds.extend(live[child].iter().copied());
            live[parent] = work::live_nodes(
                &cfg,
                LoanId(parent as u16),
                drafts[parent].start,
                &seeds,
                &reachable,
                work,
            )?;
            changed |= live[parent].len() != before;
        }
        if !changed {
            break;
        }
    }
    reject_cfg_overlaps(program, &cfg, &aliases, &drafts, &live, work)?;

    let (mut edge_live, mut termination_edges) = work::edge_liveness(&cfg, &live, work)?;
    // A contract/body root has no outgoing CFG edge. A synchronous call
    // ending at that root completes on its incoming After edge, rather than
    // needing a nonexistent successor. Only admit this formerly rejected
    // terminal shape; already admitted v1 plans retain identical bytes.
    if termination_edges.iter().any(Vec::is_empty) {
        let mut terminal_roots = BTreeSet::new();
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(function.ensures.iter())
        {
            charge(work)?;
            terminal_roots.insert(cfg.node(root, LoanPointPhase::After)?);
        }
        for nodes in &mut live {
            for (index, point) in cfg.points.iter().enumerate() {
                charge(work)?;
                if point.phase == LoanPointPhase::After
                    && cfg.successor_edges(index).is_empty()
                    && terminal_roots.contains(&(index as u16))
                {
                    nodes.remove(&(index as u16));
                }
            }
        }
        for (child, parent) in parents.iter().enumerate() {
            if let Some(parent) = parent {
                for node in &live[child] {
                    charge(work)?;
                    if !live[parent.0 as usize].contains(node) {
                        return Err(error("terminal loan child escapes its parent lifetime"));
                    }
                }
            }
        }
        // Recompute edge proof only for a previously rejected terminal shape.
        (edge_live, termination_edges) = work::edge_liveness(&cfg, &live, work)?;
    }
    materialize_cfg_plan(cfg, drafts, parents, edge_live, termination_edges)
}

fn inventory_pattern_ownership(
    pattern: &ResolvedMatchPattern,
    ownership: &mut BTreeMap<ValueId, OwnershipMode>,
) {
    fn record_field(
        pattern: &ResolvedRecordMatchFieldPattern,
        ownership: &mut BTreeMap<ValueId, OwnershipMode>,
    ) {
        match pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding) => {
                ownership.insert(binding.id.clone(), binding.ownership);
            }
            ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
                for field in fields {
                    record_field(&field.pattern, ownership);
                }
            }
            ResolvedRecordMatchFieldPattern::Wildcard => {}
        }
    }

    match pattern {
        ResolvedMatchPattern::Variant { fields, .. } => {
            for field in fields {
                ownership.insert(field.binding.id.clone(), field.binding.ownership);
            }
        }
        ResolvedMatchPattern::Record { fields, .. } => {
            for field in fields {
                record_field(&field.pattern, ownership);
            }
        }
        ResolvedMatchPattern::Binding(binding) => {
            ownership.insert(binding.id.clone(), binding.ownership);
        }
        ResolvedMatchPattern::Or(alternatives) => {
            for alternative in alternatives {
                inventory_pattern_ownership(alternative, ownership);
            }
        }
        ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => {}
    }
}

fn build_cfg<'a>(
    function: &'a ResolvedFunction,
    work: &mut WorkCounter,
) -> Result<Cfg<'a>, Diagnostic> {
    if work.allocation.is_some() {
        return paid_cfg::build(function, work);
    }
    let roots = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(function.ensures.iter())
        .collect::<Vec<_>>();
    let mut expressions = Vec::new();
    let mut seen = BTreeSet::new();
    let mut root_by_expression = BTreeMap::<ExpressionId, ExpressionId>::new();
    let mut pending = roots
        .iter()
        .rev()
        .map(|root| (*root, *root))
        .collect::<Vec<_>>();
    while let Some((expression, root)) = pending.pop() {
        charge(work)?;
        if !seen.insert(expression.id.clone()) {
            return Err(error("CFG contains a duplicate expression identity"));
        }
        root_by_expression.insert(expression.id.clone(), root.id.clone());
        expressions.push(expression);
        let mut children = Vec::new();
        push_children(expression, &mut children);
        pending.extend(children.into_iter().map(|child| (child, root)));
    }
    let node_count = expressions
        .len()
        .checked_mul(2)
        .ok_or_else(|| error("CFG point count overflows"))?;
    if node_count > MAX_LOAN_ENDPOINTS_V1 {
        return Err(error("function exceeds 4,096 loan program points"));
    }
    let mut points = Vec::with_capacity(node_count);
    let mut nodes = BTreeMap::new();
    for expression in &expressions {
        for phase in [LoanPointPhase::Before, LoanPointPhase::After] {
            let point = point(expression, phase);
            let id =
                u16::try_from(points.len()).map_err(|_| error("CFG point identity overflows"))?;
            nodes.insert(point.clone(), id);
            points.push(point);
        }
    }
    let node = |expression: &ResolvedExpr, phase: LoanPointPhase| {
        nodes
            .get(&point(expression, phase))
            .copied()
            .ok_or_else(|| error("CFG child point is not indexed"))
    };
    let root_after = |expression: &ResolvedExpr| {
        let root = root_by_expression
            .get(&expression.id)
            .ok_or_else(|| error("CFG expression lacks a function-exit root"))?;
        nodes
            .get(&LoanProgramPoint {
                expression: root.clone(),
                phase: LoanPointPhase::After,
            })
            .copied()
            .ok_or_else(|| error("CFG function-exit root is not indexed"))
    };
    let mut edge_set = BTreeSet::<(u16, u16)>::new();
    for expression in &expressions {
        charge(work)?;
        let before = node(expression, LoanPointPhase::Before)?;
        let after = node(expression, LoanPointPhase::After)?;
        match &expression.kind {
            ResolvedExprKind::LexicalExit { kind, value, .. } => {
                if let Some(value) = value {
                    edge_set.insert((before, node(value, LoanPointPhase::Before)?));
                    edge_set.insert((node(value, LoanPointPhase::After)?, after));
                } else {
                    edge_set.insert((before, after));
                }
                let destination = if *kind == crate::ast::LexicalExitKind::Return {
                    root_after(expression)?
                } else {
                    node(
                        lexical_exit::destination(expression, &expressions, work)?,
                        LoanPointPhase::Before,
                    )?
                };
                edge_set.insert((after, destination));
            }
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                let entry = statements
                    .first()
                    .map(statement_entry)
                    .unwrap_or(tail.as_ref());
                edge_set.insert((before, node(entry, LoanPointPhase::Before)?));
                for (index, statement) in statements.iter().enumerate() {
                    let next = statements
                        .get(index + 1)
                        .map(statement_entry)
                        .unwrap_or(tail.as_ref());
                    match statement {
                        ResolvedStatement::While {
                            condition, body, ..
                        }
                        | ResolvedStatement::LexicalWhile {
                            condition, body, ..
                        } => {
                            edge_set.insert((
                                node(condition, LoanPointPhase::After)?,
                                node(body, LoanPointPhase::Before)?,
                            ));
                            edge_set.insert((
                                node(condition, LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ));
                            if body.ty != ResolvedType::Never {
                                edge_set.insert((
                                    node(body, LoanPointPhase::After)?,
                                    node(condition, LoanPointPhase::Before)?,
                                ));
                            }
                        }
                        _ => {
                            edge_set.insert((
                                node(statement_exit(statement), LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ));
                        }
                    }
                }
                if tail.ty != ResolvedType::Never {
                    edge_set.insert((node(tail, LoanPointPhase::After)?, after));
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                edge_set.insert((before, node(condition, LoanPointPhase::Before)?));
                edge_set.insert((
                    node(condition, LoanPointPhase::After)?,
                    node(then_branch, LoanPointPhase::Before)?,
                ));
                edge_set.insert((
                    node(condition, LoanPointPhase::After)?,
                    node(else_branch, LoanPointPhase::Before)?,
                ));
                if then_branch.ty != ResolvedType::Never {
                    edge_set.insert((node(then_branch, LoanPointPhase::After)?, after));
                }
                if else_branch.ty != ResolvedType::Never {
                    edge_set.insert((node(else_branch, LoanPointPhase::After)?, after));
                }
            }
            ResolvedExprKind::Binary {
                op: BinaryOp::And | BinaryOp::Or,
                left,
                right,
            } => {
                edge_set.insert((before, node(left, LoanPointPhase::Before)?));
                edge_set.insert((
                    node(left, LoanPointPhase::After)?,
                    node(right, LoanPointPhase::Before)?,
                ));
                edge_set.insert((node(left, LoanPointPhase::After)?, after));
                edge_set.insert((node(right, LoanPointPhase::After)?, after));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                edge_set.insert((before, node(scrutinee, LoanPointPhase::Before)?));
                for (index, arm) in arms.iter().enumerate() {
                    let entry = arm.guard.as_deref().unwrap_or(&arm.value);
                    edge_set.insert((
                        node(scrutinee, LoanPointPhase::After)?,
                        node(entry, LoanPointPhase::Before)?,
                    ));
                    if let Some(guard) = &arm.guard {
                        edge_set.insert((
                            node(guard, LoanPointPhase::After)?,
                            node(&arm.value, LoanPointPhase::Before)?,
                        ));
                        if let Some(next) = arms.get(index + 1) {
                            let next = next.guard.as_deref().unwrap_or(&next.value);
                            edge_set.insert((
                                node(guard, LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ));
                        }
                    }
                    if arm.value.ty != ResolvedType::Never {
                        edge_set.insert((node(&arm.value, LoanPointPhase::After)?, after));
                    }
                }
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                let operand_after = node(operand, LoanPointPhase::After)?;
                edge_set.insert((before, node(operand, LoanPointPhase::Before)?));
                edge_set.insert((operand_after, after));
                // `?` has two semantic successors: normal unwrapping and an
                // immediate residual return from this contract/body root.
                // Keeping the residual edge distinct makes a later loan use
                // live only on the normal path and terminates it on return.
                edge_set.insert((operand_after, root_after(expression)?));
            }
            _ => {
                let mut children = evaluation_children(expression);
                if let Some(first) = children.next() {
                    edge_set.insert((before, node(first, LoanPointPhase::Before)?));
                    let mut last = first;
                    for next in children {
                        edge_set.insert((
                            node(last, LoanPointPhase::After)?,
                            node(next, LoanPointPhase::Before)?,
                        ));
                        last = next;
                    }
                    edge_set.insert((node(last, LoanPointPhase::After)?, after));
                } else {
                    edge_set.insert((before, after));
                }
            }
        }
    }
    let edges = edge_set.into_iter().collect::<Vec<_>>();
    if edges.len() > MAX_LOAN_EDGES_V1 {
        return Err(error("function exceeds 4,096 loan CFG edges"));
    }
    let adjacency =
        adjacency::Adjacency::build_with_storage(points.len(), &edges, work.allocation)?;
    Ok(Cfg {
        points,
        nodes: cfg_directory::Nodes::Legacy(nodes),
        expressions,
        edges,
        adjacency,
    })
}

fn statement_entry(statement: &ResolvedStatement) -> &ResolvedExpr {
    match statement {
        ResolvedStatement::Let { value, .. }
        | ResolvedStatement::Assign { value, .. }
        | ResolvedStatement::CopyDiscard { value, .. } => value,
        ResolvedStatement::Unsafe { body, .. } => body,
        ResolvedStatement::While { condition, .. }
        | ResolvedStatement::LexicalWhile { condition, .. } => condition,
    }
}

fn statement_exit(statement: &ResolvedStatement) -> &ResolvedExpr {
    match statement {
        ResolvedStatement::Let { value, .. }
        | ResolvedStatement::Assign { value, .. }
        | ResolvedStatement::CopyDiscard { value, .. } => value,
        ResolvedStatement::Unsafe { body, .. } => body,
        ResolvedStatement::While { body, .. } | ResolvedStatement::LexicalWhile { body, .. } => {
            body
        }
    }
}

fn reject_cfg_overlaps(
    program: &ResolvedProgram,
    cfg: &Cfg<'_>,
    aliases: &BTreeMap<ValueId, Place>,
    drafts: &[CfgDraft],
    live: &[BTreeSet<u16>],
    work: &mut WorkCounter,
) -> Result<(), Diagnostic> {
    let mut nonconsuming = BTreeSet::new();
    for expression in &cfg.expressions {
        for (_, argument) in borrowed_call_arguments(program, expression) {
            nonconsuming.insert(argument.id.clone());
        }
        match &expression.kind {
            ResolvedExprKind::Match {
                mode: crate::hir::ResolvedMatchMode::Borrow,
                scrutinee,
                ..
            } => {
                nonconsuming.insert(scrutinee.id.clone());
            }
            ResolvedExprKind::ByteRange { source, .. } => {
                nonconsuming.insert(source.id.clone());
            }
            _ => {}
        }
    }
    for expression in &cfg.expressions {
        charge(work)?;
        if let ResolvedExprKind::Place(place) = &expression.kind {
            if expression.ownership == OwnershipMode::Own && !nonconsuming.contains(&expression.id)
            {
                let place = resolve_origin(aliases, place.clone(), work)?;
                reject_overlap_at(
                    cfg.node(expression, LoanPointPhase::Before)?,
                    &place,
                    drafts,
                    live,
                    expression.span,
                )?;
            }
        }
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Assign {
                    binding,
                    field,
                    value,
                    span,
                } = statement
                {
                    let mut projections = Vec::new();
                    if let Some(field) = field {
                        projections.push(PlaceProjection::Field(field.clone()));
                    }
                    let place = resolve_origin(
                        aliases,
                        Place {
                            root: binding.id.clone(),
                            projections,
                        },
                        work,
                    )?;
                    reject_overlap_at(
                        cfg.node(value, LoanPointPhase::After)?,
                        &place,
                        drafts,
                        live,
                        *span,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn reject_overlap_at<S, N: paid_live::Membership>(
    node: u16,
    place: &Place,
    drafts: &[CfgDraft<S>],
    live: &[N],
    span: Span,
) -> Result<(), Diagnostic> {
    if let Some((loan, _)) = drafts.iter().zip(live).find(|(loan, live)| {
        live.contains(&node)
            && loan.origin.root == place.root
            && (place.projections.starts_with(&loan.origin.projections)
                || loan.origin.projections.starts_with(&place.projections))
    }) {
        let message = "move, mutation, or transfer overlaps an active shared loan";
        // Keep the conflicting operation as the primary location. Hand-built
        // or hostile HIR may have no source provenance, represented by the
        // default span; never fabricate either location.
        let mut diagnostic = if span.start < span.end && span.line > 0 && span.column > 0 {
            Diagnostic::error("SPX-H006", message, span)
        } else {
            error(message)
        };
        if loan.origin_span.start < loan.origin_span.end
            && loan.origin_span.line > 0
            && loan.origin_span.column > 0
        {
            diagnostic = diagnostic.with_help(format!(
                "the active shared loan begins at {}:{}; end its uses before moving, mutating, or transferring the owner",
                loan.origin_span.line, loan.origin_span.column
            ));
        }
        Err(diagnostic)
    } else {
        Ok(())
    }
}

fn materialize_cfg_plan(
    cfg: Cfg<'_>,
    drafts: Vec<CfgDraft>,
    parents: Vec<Option<LoanId>>,
    edge_live: Vec<Vec<LoanId>>,
    termination_edges: Vec<Vec<u16>>,
) -> Result<LoanPlan, Diagnostic> {
    // The draft count is exact. Result collection need not retain spare
    // geometric capacity in the authoritative physical loan inventory.
    let mut loans = Vec::with_capacity(drafts.len());
    for (index, draft) in drafts.iter().enumerate() {
        let end_edges = termination_edges[index].clone();
        if end_edges.is_empty() {
            return Err(error("loan has no bounded CFG termination edge"));
        }
        let mut ends = end_edges
            .iter()
            .map(|edge| cfg.points[cfg.edges[*edge as usize].1 as usize].clone())
            .collect::<Vec<_>>();
        ends.dedup();
        loans.push(Loan {
            id: LoanId(index as u16),
            site: draft.site.clone(),
            origin: draft.origin.clone(),
            parent: parents[index],
            start: cfg.points[draft.start as usize].clone(),
            ends,
            end_edges,
            cause: draft.cause.clone(),
        });
    }
    let mut starts = vec![Vec::<LoanId>::new(); cfg.points.len()];
    let mut kills = vec![Vec::<LoanId>::new(); cfg.points.len()];
    for loan in &loans {
        starts[*cfg.nodes.get(&loan.start).expect("start point retained") as usize].push(loan.id);
        for edge in &loan.end_edges {
            let to = cfg.edges[*edge as usize].1;
            kills[to as usize].push(loan.id);
        }
    }
    let mut endpoints = Vec::with_capacity(cfg.points.len());
    for (node, point) in cfg.points.iter().enumerate() {
        let mut live_before = BTreeSet::new();
        for edge in cfg
            .predecessors(node)
            .iter()
            .filter_map(|from| cfg.edges.binary_search(&(*from, node as u16)).ok())
        {
            live_before.extend(edge_live[edge].iter().copied());
        }
        let mut live_after = BTreeSet::new();
        for edge in cfg
            .successors(node)
            .filter_map(|to| cfg.edges.binary_search(&(node as u16, to)).ok())
        {
            live_after.extend(edge_live[edge].iter().copied());
        }
        endpoints.push(LoanEndpoint {
            point: point.clone(),
            live_before: compact_ids::from_vec(live_before.into_iter().collect())?,
            starts: compact_ids::from_vec(starts[node].clone())?,
            kills: compact_ids::from_vec(kills[node].clone())?,
            live_after: compact_ids::from_vec(live_after.into_iter().collect())?,
        });
    }
    let mut edges = Vec::with_capacity(cfg.edges.len());
    for ((from, to), live) in cfg.edges.iter().zip(edge_live) {
        edges.push(LoanEdge {
            from: *from,
            to: *to,
            live: compact_ids::from_vec(live)?,
        });
    }
    let schema = if cfg
        .expressions
        .iter()
        .any(|e| matches!(e.kind, ResolvedExprKind::LexicalExit { .. }))
    {
        LOAN_PLAN_SCHEMA_V3
    } else if loans
        .iter()
        .any(|loan| loan.cause == LoanCause::TraversalFreeze)
    {
        LOAN_PLAN_SCHEMA_V2
    } else {
        LOAN_PLAN_SCHEMA_V1
    };
    Ok(LoanPlan {
        schema,
        loans,
        endpoints,
        edges,
    })
}

pub fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        if function.loan_plan != build_plan(program, function)? {
            return Err(forged_evidence(function));
        }
    }
    Ok(())
}

// The original owned diagnostic policy is shared with the invocation-local
// deferred comparison. This renderer grants no proof or validation authority.
pub(crate) fn forged_evidence(function: &ResolvedFunction) -> Diagnostic {
    error(format!(
        "function `{}` has forged shared-loan evidence",
        function.id
    ))
}

fn resolve_parent(
    aliases: &BTreeMap<ValueId, Place>,
    bindings: &BTreeMap<ValueId, LoanId>,
    mut root: ValueId,
) -> Option<LoanId> {
    let mut seen = BTreeSet::new();
    while seen.insert(root.clone()) {
        if let Some(id) = bindings.get(&root) {
            return Some(*id);
        }
        root = aliases.get(&root)?.root.clone();
    }
    None
}

fn resolve_origin(
    aliases: &BTreeMap<ValueId, Place>,
    place: Place,
    work: &mut WorkCounter,
) -> Result<Place, Diagnostic> {
    origin::resolve(aliases, place, work)
}

fn expression_place(expression: &ResolvedExpr) -> Option<Place> {
    match &expression.kind {
        ResolvedExprKind::Call { .. } => crate::vec_ops::string_slot::place(expression).cloned(),
        ResolvedExprKind::VecFieldRead { args, .. } | ResolvedExprKind::RowLoan { args, .. } => {
            args.first().and_then(expression_place)
        }
        ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. } => {
            Some(place.clone())
        }
        ResolvedExprKind::ByteRange { source, .. } => expression_place(source),
        ResolvedExprKind::NativeRustImportCall(call)
            if matches!(
                call.result,
                crate::hir::ResolvedImportResultKind::BorrowedStr { .. }
            ) =>
        {
            call.args.first().and_then(expression_place)
        }
        _ => None,
    }
}

fn point(expression: &ResolvedExpr, phase: LoanPointPhase) -> LoanProgramPoint {
    LoanProgramPoint {
        expression: expression.id.clone(),
        phase,
    }
}

fn charge(work: &mut WorkCounter) -> Result<(), Diagnostic> {
    work.used = work
        .used
        .checked_add(1)
        .ok_or_else(|| error("loan checked-work counter overflows"))?;
    if work.used > work.limit {
        return Err(error("loan analysis exceeds 1,000,000 checked work"));
    }
    Ok(())
}

fn push_children<'a>(expression: &'a ResolvedExpr, pending: &mut Vec<&'a ResolvedExpr>) {
    match &expression.kind {
        ResolvedExprKind::LexicalExit { value, .. } => {
            pending.extend(value.as_deref());
        }
        ResolvedExprKind::Closure { captures, .. } => {
            pending.extend(captures.iter().rev().map(|capture| &capture.value))
        }
        ResolvedExprKind::FunctionReference { .. } => {}
        ResolvedExprKind::Invoke { callable, args } => {
            pending.extend(args.iter().rev());
            pending.push(callable);
        }
        ResolvedExprKind::Block { statements, tail }
        | ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } => {
            pending.push(tail);
            for statement in statements.iter().rev() {
                for index in (0..statement.child_count()).rev() {
                    if let Some(child) = statement.child(index) {
                        pending.push(child);
                    }
                }
            }
        }
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. }
        | ResolvedExprKind::RowLoan { args, .. } => pending.extend(args.iter().rev()),
        ResolvedExprKind::NativeRustImportCall(call) => pending.extend(call.args.iter().rev()),
        ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter().rev()),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            pending.push(end);
            pending.push(start);
            pending.push(source);
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => pending.push(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            pending.push(right);
            pending.push(left);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            pending.push(else_branch);
            pending.push(then_branch);
            pending.push(condition);
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value));
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            for arm in arms.iter().rev() {
                pending.push(&arm.value);
                if let Some(guard) = &arm.guard {
                    pending.push(guard);
                }
            }
            pending.push(scrutinee);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value));
            pending.push(base);
        }
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
        | ResolvedExprKind::BorrowedUtf8Literal(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
    }
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

#[cfg(test)]
#[path = "loan_plan/tests.rs"]
mod tests;
