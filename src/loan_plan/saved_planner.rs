//! Saved original shared-loan planner, kept as an independent test oracle.
//!
//! Bodies below are the planner at commit c8a5d30a6, relocated verbatim: the
//! three nested `Vec<Vec<u16>>` adjacency tables in this module's own `Cfg`,
//! `build_plan` -> `build_cfg_plan_counted` -> `materialize_cfg_plan`, the
//! original `Vec`-returning `evaluation_children`, the original move-to-front
//! `PointQueue` traversal in `work`, and the Cfg-coupled draft helpers in
//! `draft`. The common public `LoanPlan`, `Loan`, endpoint/edge types, ids,
//! `CfgDraft`, `WorkCounter` and `charge` are imported, not redefined, so one
//! plan value and one work ledger compare exactly.
//!
//! Shared by explicit note, because none of it reads a `Cfg` and it did not
//! change in the flat-adjacency tranche: `has_own_root_candidate`,
//! `inventory_pattern_ownership`, `reject_overlap_at`, `resolve_parent`,
//! `resolve_origin`, `expression_place`, `statement_entry`, `statement_exit`,
//! `point`, `push_children`, `lexical_exit::destination`,
//! `native_view::{inventory_aliases, match_parents}`, `compact_ids`,
//! `traversal::inspect_counted` and `guidance::work_limit`. Those are the
//! unchanged inspect/shape logic; the oracle's independence is the adjacency,
//! traversal, queue, child-collection and materialization above.
//!
//! None of this is reachable from production builds and it grants no proof or
//! validation authority.

use super::*;
use super::nested_candidate_before::has_own_root_candidate;

mod cfg_build;
mod draft;
mod work;

pub(super) use cfg_build::{build_cfg, evaluation_children};

pub(super) struct Cfg<'a> {
    pub(super) points: Vec<LoanProgramPoint>,
    pub(super) nodes: BTreeMap<LoanProgramPoint, u16>,
    pub(super) expressions: Vec<&'a ResolvedExpr>,
    pub(super) edges: Vec<(u16, u16)>,
    pub(super) successors: Vec<Vec<u16>>,
    pub(super) successor_edges: Vec<Vec<u16>>,
    pub(super) predecessors: Vec<Vec<u16>>,
}

impl Cfg<'_> {
    pub(super) fn node(
        &self,
        expression: &ResolvedExpr,
        phase: LoanPointPhase,
    ) -> Result<u16, Diagnostic> {
        self.nodes
            .get(&point(expression, phase))
            .copied()
            .ok_or_else(|| error("CFG references an unindexed expression point"))
    }
}

pub(super) fn build_plan(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<LoanPlan, Diagnostic> {
    reject_new_statement_profile(function)?;
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

fn build_cfg_plan(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<LoanPlan, Diagnostic> {
    let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
    build_cfg_plan_counted(program, function, &mut work)
}

/// The saved planner's exact consumed work count, mirroring the production
/// test entry point so both can be driven at identical limits.
pub(super) fn build_cfg_plan_with_work_limit(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    limit: usize,
) -> (Result<LoanPlan, Diagnostic>, usize) {
    let mut work = WorkCounter::new(limit);
    let result = build_cfg_plan_counted(program, function, &mut work);
    (result, work.used)
}

pub(super) fn build_cfg_plan_counted(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    work: &mut WorkCounter,
) -> Result<LoanPlan, Diagnostic> {
    reject_new_statement_profile(function)?;
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

    let mut drafts = Vec::<CfgDraft>::new();
    for expression in &cfg.expressions {
        charge(work)?;
        if let Some(draft) = draft::draft(program, expression, &cfg, work)? {
            drafts.push(draft);
        }
        if let Some(draft) = draft::draft_bytes(expression, &cfg, &aliases, work)? {
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
                    && cfg.successors[index].is_empty()
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
        for edge in cfg.predecessors[node]
            .iter()
            .filter_map(|from| cfg.edges.binary_search(&(*from, node as u16)).ok())
        {
            live_before.extend(edge_live[edge].iter().copied());
        }
        let mut live_after = BTreeSet::new();
        for edge in cfg.successors[node]
            .iter()
            .filter_map(|to| cfg.edges.binary_search(&(node as u16, *to)).ok())
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

// Test-only feature exclusion. This allocation-free current feature census is
// used only to reject statements absent from c8a, never to reconstruct a plan.
// The saved receiver's original planning/work bodies remain the oracle for its
// admitted fixtures; it cannot silently inherit new statement_entry semantics.
fn reject_new_statement_profile(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    if crate::hir::copy_discard_function_uses(function) {
        return Err(error("saved c8a loan planner does not admit CopyDiscard"));
    }
    Ok(())
}
