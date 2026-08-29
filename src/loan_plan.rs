//! Bounded target-neutral shared-loan proof attachment.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::BinaryOp;
use crate::diagnostic::Diagnostic;
use crate::hir::{
    ExpressionId, OwnershipMode, Place, PlaceProjection, ResolvedExpr, ResolvedExprKind,
    ResolvedFunction, ResolvedMatchPattern, ResolvedProgram, ResolvedRecordMatchFieldPattern,
    ResolvedStatement, ResolvedType, ValueId,
};

pub const LOAN_PLAN_SCHEMA_V1: &str = "semaprax.loan-plan.v1";
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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LoanProgramPoint {
    pub expression: ExpressionId,
    pub phase: LoanPointPhase,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LoanCause {
    SliceView,
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
    pub live_before: Vec<LoanId>,
    pub starts: Vec<LoanId>,
    /// Edge-qualified terminations are authoritative in [`Loan::end_edges`];
    /// this is the deterministic node-level union summary.
    pub kills: Vec<LoanId>,
    /// Union summary over outgoing CFG edges.
    pub live_after: Vec<LoanId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanEdge {
    /// Dense endpoint index of the CFG source point.
    pub from: u16,
    /// Dense endpoint index of the CFG destination point.
    pub to: u16,
    /// Exact simultaneous loans live on this one control-flow edge.
    pub live: Vec<LoanId>,
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
    if !has_own_root_candidate(program, function)? {
        return Ok(LoanPlan::empty_v1());
    }
    build_cfg_plan(program, function)
}

/// Exact retained heap capacity owned by a nonempty plan. The inline
/// `LoanPlan` carrier is intentionally excluded so callers can account it in
/// the layout domain that owns the containing function.
pub(crate) fn owned_capacity_bytes(plan: &LoanPlan) -> Option<usize> {
    fn add(total: &mut usize, bytes: usize) -> Option<()> {
        *total = total.checked_add(bytes)?;
        Some(())
    }
    fn point_bytes(point: &LoanProgramPoint) -> usize {
        point.expression.as_str().len()
    }
    fn place_bytes(place: &Place) -> Option<usize> {
        let mut bytes = place.root.as_str().len();
        add(
            &mut bytes,
            place
                .projections
                .capacity()
                .checked_mul(std::mem::size_of::<PlaceProjection>())?,
        )?;
        for projection in &place.projections {
            match projection {
                PlaceProjection::Field(field) => add(&mut bytes, field.as_str().len())?,
                PlaceProjection::VariantField { case, field } => {
                    add(&mut bytes, case.as_str().len())?;
                    add(&mut bytes, field.as_str().len())?;
                }
            }
        }
        Some(bytes)
    }

    let mut bytes = plan
        .loans
        .capacity()
        .checked_mul(std::mem::size_of::<Loan>())?;
    add(
        &mut bytes,
        plan.endpoints
            .capacity()
            .checked_mul(std::mem::size_of::<LoanEndpoint>())?,
    )?;
    add(
        &mut bytes,
        plan.edges
            .capacity()
            .checked_mul(std::mem::size_of::<LoanEdge>())?,
    )?;
    for loan in &plan.loans {
        add(&mut bytes, loan.site.as_str().len())?;
        add(&mut bytes, place_bytes(&loan.origin)?)?;
        add(&mut bytes, point_bytes(&loan.start))?;
        add(
            &mut bytes,
            loan.ends
                .capacity()
                .checked_mul(std::mem::size_of::<LoanProgramPoint>())?,
        )?;
        for end in &loan.ends {
            add(&mut bytes, point_bytes(end))?;
        }
        add(
            &mut bytes,
            loan.end_edges
                .capacity()
                .checked_mul(std::mem::size_of::<u16>())?,
        )?;
    }
    for endpoint in &plan.endpoints {
        add(&mut bytes, point_bytes(&endpoint.point))?;
        for ids in [
            &endpoint.live_before,
            &endpoint.starts,
            &endpoint.kills,
            &endpoint.live_after,
        ] {
            add(
                &mut bytes,
                ids.capacity().checked_mul(std::mem::size_of::<LoanId>())?,
            )?;
        }
    }
    for edge in &plan.edges {
        add(
            &mut bytes,
            edge.live
                .capacity()
                .checked_mul(std::mem::size_of::<LoanId>())?,
        )?;
    }
    Some(bytes)
}

#[derive(Clone)]
struct CfgDraft {
    site: ExpressionId,
    origin: Place,
    parent_root: Option<ValueId>,
    binding: Option<ValueId>,
    start: u16,
    seeds: BTreeSet<u16>,
    cause: LoanCause,
}

struct Cfg<'a> {
    points: Vec<LoanProgramPoint>,
    nodes: BTreeMap<LoanProgramPoint, u16>,
    expressions: Vec<&'a ResolvedExpr>,
    edges: Vec<(u16, u16)>,
    successors: Vec<Vec<u16>>,
    predecessors: Vec<Vec<u16>>,
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
        if let ResolvedExprKind::Match { arms, .. } = &expression.kind {
            for arm in arms {
                inventory_pattern_ownership(&arm.pattern, &mut ownership);
            }
        }
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Let { binding, value, .. } = statement {
                    ownership.insert(binding.id.clone(), binding.ownership);
                    if binding.ty == ResolvedType::SliceU8 {
                        if let Some(place) = expression_place(value) {
                            aliases.insert(binding.id.clone(), place);
                            bound.insert(value.id.clone(), binding.id.clone());
                        }
                    }
                }
            }
        }
    }

    let mut root_ownership = BTreeMap::<ValueId, bool>::new();
    for expression in expressions {
        let view = match &expression.kind {
            ResolvedExprKind::BorrowPlace { place, .. } => Some(place.clone()),
            ResolvedExprKind::ByteRange { source, .. } => expression_place(source),
            ResolvedExprKind::Place(place) if bound.contains_key(&expression.id) => {
                Some(place.clone())
            }
            _ => None,
        };
        if let Some(place) = view {
            if ultimate_root_is_own(&place.root, &aliases, &ownership, &mut root_ownership)? {
                return Ok(true);
            }
        }
        if let ResolvedExprKind::Call {
            callee,
            instance,
            args,
            ..
        } = &expression.kind
        {
            let target = program.resolve_call_target(callee, instance.as_ref());
            for (index, argument) in args.iter().enumerate() {
                let borrowed = target.map_or_else(
                    || argument.ownership == OwnershipMode::Borrow,
                    |target| {
                        target
                            .params
                            .get(index)
                            .is_some_and(|parameter| parameter.ownership == OwnershipMode::Borrow)
                    },
                );
                if borrowed {
                    if let Some(place) = expression_place(argument) {
                        if ultimate_root_is_own(
                            &place.root,
                            &aliases,
                            &ownership,
                            &mut root_ownership,
                        )? {
                            return Ok(true);
                        }
                    }
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
                if ultimate_root_is_own(&place.root, &aliases, &ownership, &mut root_ownership)? {
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
            break ownership.get(&current) == Some(&OwnershipMode::Own);
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
    let mut work = 0usize;
    let cfg = build_cfg(function, &mut work)?;
    let mut aliases = BTreeMap::<ValueId, Place>::new();
    let mut bound = BTreeMap::<ExpressionId, ValueId>::new();
    let mut ownership = function
        .params
        .iter()
        .map(|param| (param.id.clone(), param.ownership))
        .collect::<BTreeMap<_, _>>();
    for expression in &cfg.expressions {
        charge(&mut work)?;
        if let ResolvedExprKind::Match { arms, .. } = &expression.kind {
            for arm in arms {
                inventory_pattern_ownership(&arm.pattern, &mut ownership);
            }
        }
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Let { binding, value, .. } = statement {
                    ownership.insert(binding.id.clone(), binding.ownership);
                    if binding.ty == ResolvedType::SliceU8 {
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
        charge(&mut work)?;
        let view = match &expression.kind {
            ResolvedExprKind::BorrowPlace { place, .. } => Some(place.clone()),
            ResolvedExprKind::ByteRange { source, .. } => expression_place(source),
            ResolvedExprKind::Place(place) if bound.contains_key(&expression.id) => {
                Some(place.clone())
            }
            _ => None,
        };
        if let Some(place) = view {
            drafts.push(CfgDraft {
                site: expression.id.clone(),
                origin: resolve_origin(&aliases, place.clone(), &mut work)?,
                parent_root: aliases
                    .contains_key(&place.root)
                    .then(|| place.root.clone()),
                binding: bound.get(&expression.id).cloned(),
                start: cfg.node(expression, LoanPointPhase::Before)?,
                seeds: BTreeSet::new(),
                cause: LoanCause::SliceView,
            });
        }
        if let ResolvedExprKind::Call {
            callee,
            instance,
            args,
            ..
        } = &expression.kind
        {
            let target = program.resolve_call_target(callee, instance.as_ref());
            for (index, argument) in args.iter().enumerate() {
                let borrowed = target.map_or_else(
                    || argument.ownership == OwnershipMode::Borrow,
                    |target| {
                        target
                            .params
                            .get(index)
                            .is_some_and(|parameter| parameter.ownership == OwnershipMode::Borrow)
                    },
                );
                if !borrowed {
                    continue;
                }
                let place = expression_place(argument)
                    .ok_or_else(|| error("borrowed call lacks an exact place origin"))?;
                drafts.push(CfgDraft {
                    site: expression.id.clone(),
                    origin: resolve_origin(&aliases, place.clone(), &mut work)?,
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
        }
        if let ResolvedExprKind::Match {
            mode: crate::hir::ResolvedMatchMode::Borrow,
            scrutinee,
            arms,
        } = &expression.kind
        {
            let place = expression_place(scrutinee)
                .ok_or_else(|| error("borrow match lacks an exact place origin"))?;
            let origin = resolve_origin(&aliases, place.clone(), &mut work)?;
            for (index, arm) in arms.iter().enumerate() {
                let entry = arm.guard.as_deref().unwrap_or(&arm.value);
                drafts.push(CfgDraft {
                    site: expression.id.clone(),
                    origin: origin.clone(),
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

    drafts.retain(|draft| ownership.get(&draft.origin.root) == Some(&OwnershipMode::Own));
    // Borrow-only roots do not need an authenticated loan attachment. Do not
    // retain the transient CFG built to classify candidates: legacy HIR stays
    // heap-identical at this boundary and only real own-root loans carry the
    // endpoint/edge proof.
    if drafts.is_empty() {
        return Ok(LoanPlan::empty_v1());
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
    let parents = drafts
        .iter()
        .map(|draft| {
            draft
                .parent_root
                .clone()
                .and_then(|root| resolve_parent(&aliases, &binding_loans, root))
        })
        .collect::<Vec<_>>();
    let mut live = drafts
        .iter()
        .map(|draft| live_nodes(&cfg, draft.start, &draft.seeds, &mut work))
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
            live[parent] = live_nodes(&cfg, drafts[parent].start, &seeds, &mut work)?;
            changed |= live[parent].len() != before;
        }
        if !changed {
            break;
        }
    }
    reject_cfg_overlaps(program, &cfg, &aliases, &drafts, &live, &mut work)?;

    let mut edge_live = vec![Vec::<LoanId>::new(); cfg.edges.len()];
    let mut termination_edges = vec![Vec::<u16>::new(); drafts.len()];
    for (loan_index, nodes) in live.iter().enumerate() {
        let id = LoanId(loan_index as u16);
        for (edge_index, (from, to)) in cfg.edges.iter().copied().enumerate() {
            charge(&mut work)?;
            if nodes.contains(&from) && nodes.contains(&to) {
                edge_live[edge_index].push(id);
            } else if nodes.contains(&from) && !nodes.contains(&to) {
                termination_edges[loan_index].push(edge_index as u16);
            }
        }
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

fn build_cfg<'a>(function: &'a ResolvedFunction, work: &mut usize) -> Result<Cfg<'a>, Diagnostic> {
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
            ResolvedExprKind::Block { statements, tail } => {
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
                        } => {
                            edge_set.insert((
                                node(condition, LoanPointPhase::After)?,
                                node(body, LoanPointPhase::Before)?,
                            ));
                            edge_set.insert((
                                node(condition, LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ));
                            edge_set.insert((
                                node(body, LoanPointPhase::After)?,
                                node(condition, LoanPointPhase::Before)?,
                            ));
                        }
                        _ => {
                            edge_set.insert((
                                node(statement_exit(statement), LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ));
                        }
                    }
                }
                edge_set.insert((node(tail, LoanPointPhase::After)?, after));
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
                edge_set.insert((node(then_branch, LoanPointPhase::After)?, after));
                edge_set.insert((node(else_branch, LoanPointPhase::After)?, after));
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
                    edge_set.insert((node(&arm.value, LoanPointPhase::After)?, after));
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
                let children = evaluation_children(expression);
                if let Some(first) = children.first() {
                    edge_set.insert((before, node(first, LoanPointPhase::Before)?));
                    for pair in children.windows(2) {
                        edge_set.insert((
                            node(pair[0], LoanPointPhase::After)?,
                            node(pair[1], LoanPointPhase::Before)?,
                        ));
                    }
                    edge_set.insert((
                        node(children.last().expect("nonempty"), LoanPointPhase::After)?,
                        after,
                    ));
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
    let mut successors = vec![Vec::new(); points.len()];
    let mut predecessors = vec![Vec::new(); points.len()];
    for (from, to) in &edges {
        successors[*from as usize].push(*to);
        predecessors[*to as usize].push(*from);
    }
    Ok(Cfg {
        points,
        nodes,
        expressions,
        edges,
        successors,
        predecessors,
    })
}

fn statement_entry(statement: &ResolvedStatement) -> &ResolvedExpr {
    match statement {
        ResolvedStatement::Let { value, .. } | ResolvedStatement::Assign { value, .. } => value,
        ResolvedStatement::Unsafe { body, .. } => body,
        ResolvedStatement::While { condition, .. } => condition,
    }
}

fn statement_exit(statement: &ResolvedStatement) -> &ResolvedExpr {
    match statement {
        ResolvedStatement::Let { value, .. } | ResolvedStatement::Assign { value, .. } => value,
        ResolvedStatement::Unsafe { body, .. } => body,
        ResolvedStatement::While { body, .. } => body,
    }
}

fn evaluation_children(expression: &ResolvedExpr) -> Vec<&ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => args.iter().collect(),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().collect(),
        ResolvedExprKind::HostCommandCall(call) => call.args.iter().collect(),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => vec![source, start, end],
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => vec![value],
        ResolvedExprKind::Binary { left, right, .. } => vec![left, right],
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.iter().map(|field| &field.value).collect()
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => std::iter::once(base.as_ref())
            .chain(fields.iter().map(|field| &field.value))
            .collect(),
        ResolvedExprKind::Block { .. }
        | ResolvedExprKind::If { .. }
        | ResolvedExprKind::Match { .. }
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
        | ResolvedExprKind::BorrowPlace { .. } => Vec::new(),
    }
}

fn live_nodes(
    cfg: &Cfg<'_>,
    start: u16,
    seeds: &BTreeSet<u16>,
    work: &mut usize,
) -> Result<BTreeSet<u16>, Diagnostic> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(node) = pending.pop() {
        charge(work)?;
        if reachable.insert(node) {
            pending.extend(cfg.successors[node as usize].iter().rev().copied());
        }
    }
    let mut live = BTreeSet::new();
    let mut pending = seeds
        .iter()
        .filter(|seed| reachable.contains(seed))
        .copied()
        .collect::<Vec<_>>();
    pending.push(start);
    while let Some(node) = pending.pop() {
        charge(work)?;
        if !reachable.contains(&node) || !live.insert(node) || node == start {
            continue;
        }
        pending.extend(cfg.predecessors[node as usize].iter().rev().copied());
    }
    Ok(live)
}

fn reject_cfg_overlaps(
    program: &ResolvedProgram,
    cfg: &Cfg<'_>,
    aliases: &BTreeMap<ValueId, Place>,
    drafts: &[CfgDraft],
    live: &[BTreeSet<u16>],
    work: &mut usize,
) -> Result<(), Diagnostic> {
    let mut nonconsuming = BTreeSet::new();
    for expression in &cfg.expressions {
        match &expression.kind {
            ResolvedExprKind::Call {
                callee,
                instance,
                args,
                ..
            } => {
                let target = program.resolve_call_target(callee, instance.as_ref());
                for (index, argument) in args.iter().enumerate() {
                    let borrowed = target.map_or_else(
                        || argument.ownership == OwnershipMode::Borrow,
                        |target| {
                            target.params.get(index).is_some_and(|parameter| {
                                parameter.ownership == OwnershipMode::Borrow
                            })
                        },
                    );
                    if borrowed {
                        nonconsuming.insert(argument.id.clone());
                    }
                }
            }
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
                )?;
            }
        }
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Assign {
                    binding,
                    field,
                    value,
                    ..
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
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn reject_overlap_at(
    node: u16,
    place: &Place,
    drafts: &[CfgDraft],
    live: &[BTreeSet<u16>],
) -> Result<(), Diagnostic> {
    if drafts.iter().zip(live).any(|(loan, live)| {
        live.contains(&node)
            && loan.origin.root == place.root
            && (place.projections.starts_with(&loan.origin.projections)
                || loan.origin.projections.starts_with(&place.projections))
    }) {
        Err(error(
            "move, mutation, or transfer overlaps an active shared loan",
        ))
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
    let loans = drafts
        .iter()
        .enumerate()
        .map(|(index, draft)| {
            let end_edges = termination_edges[index].clone();
            if end_edges.is_empty() {
                return Err(error("loan has no bounded CFG termination edge"));
            }
            let mut ends = end_edges
                .iter()
                .map(|edge| cfg.points[cfg.edges[*edge as usize].1 as usize].clone())
                .collect::<Vec<_>>();
            ends.dedup();
            Ok(Loan {
                id: LoanId(index as u16),
                site: draft.site.clone(),
                origin: draft.origin.clone(),
                parent: parents[index],
                start: cfg.points[draft.start as usize].clone(),
                ends,
                end_edges,
                cause: draft.cause.clone(),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let mut starts = vec![Vec::<LoanId>::new(); cfg.points.len()];
    let mut kills = vec![Vec::<LoanId>::new(); cfg.points.len()];
    for loan in &loans {
        starts[*cfg.nodes.get(&loan.start).expect("start point retained") as usize].push(loan.id);
        for edge in &loan.end_edges {
            let to = cfg.edges[*edge as usize].1;
            kills[to as usize].push(loan.id);
        }
    }
    let endpoints = cfg
        .points
        .iter()
        .enumerate()
        .map(|(node, point)| {
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
            LoanEndpoint {
                point: point.clone(),
                live_before: live_before.into_iter().collect(),
                starts: starts[node].clone(),
                kills: kills[node].clone(),
                live_after: live_after.into_iter().collect(),
            }
        })
        .collect();
    let edges = cfg
        .edges
        .iter()
        .zip(edge_live)
        .map(|((from, to), live)| LoanEdge {
            from: *from,
            to: *to,
            live,
        })
        .collect();
    Ok(LoanPlan {
        schema: LOAN_PLAN_SCHEMA_V1,
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
            return Err(error(format!(
                "function `{}` has forged shared-loan evidence",
                function.id
            )));
        }
    }
    Ok(())
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
    mut place: Place,
    work: &mut usize,
) -> Result<Place, Diagnostic> {
    let mut seen = BTreeSet::new();
    while let Some(alias) = aliases.get(&place.root) {
        charge(work)?;
        if !seen.insert(place.root.clone()) {
            return Err(error("loan alias provenance contains a cycle"));
        }
        let mut projections = alias.projections.clone();
        projections.extend(place.projections);
        place = Place {
            root: alias.root.clone(),
            projections,
        };
    }
    Ok(place)
}

fn expression_place(expression: &ResolvedExpr) -> Option<Place> {
    match &expression.kind {
        ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. } => {
            Some(place.clone())
        }
        ResolvedExprKind::ByteRange { source, .. } => expression_place(source),
        _ => None,
    }
}

fn point(expression: &ResolvedExpr, phase: LoanPointPhase) -> LoanProgramPoint {
    LoanProgramPoint {
        expression: expression.id.clone(),
        phase,
    }
}

fn charge(work: &mut usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_add(1)
        .ok_or_else(|| error("loan checked-work counter overflows"))?;
    if *work > MAX_LOAN_PLAN_WORK_V1 {
        return Err(error("loan analysis exceeds 1,000,000 checked work"));
    }
    Ok(())
}

fn push_children<'a>(expression: &'a ResolvedExpr, pending: &mut Vec<&'a ResolvedExpr>) {
    match &expression.kind {
        ResolvedExprKind::Block { statements, tail } => {
            pending.push(tail);
            for statement in statements.iter().rev() {
                for index in (0..statement.child_count()).rev() {
                    if let Some(child) = statement.child(index) {
                        pending.push(child);
                    }
                }
            }
        }
        ResolvedExprKind::Call { args, .. } => pending.extend(args.iter().rev()),
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
        | ResolvedExprKind::Upcast { source: value } => pending.push(value),
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
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
    }
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    const FIXTURE: &str = r#"
module test.loan_plan_cfg;
@id("bytes.take") fn take(value: own Bytes) -> i64 { 1 }
@id("loan.run")
fn run(input: borrow Slice<u8>, outer: bool, inner: bool) -> i64 {
    let owned = bytes_copy(input);
    let view = bytes_as_slice(owned);
    let observed = if outer {
        if inner { byte_len(view) > 0usize && byte_len(view) < 9usize } else { false }
    } else { false };
    take(owned)
}
@id("app.main") fn main() -> i64 { 0 }
"#;

    fn fixture() -> ResolvedProgram {
        let ast = crate::parse(FIXTURE, Path::new("loan-plan-cfg.spx")).unwrap();
        assert!(crate::verify::verify(&ast).is_empty());
        crate::hir::resolve(&ast).unwrap()
    }

    fn run_mutation(name: &str, mut mutate: impl FnMut(&mut LoanPlan, &ResolvedFunction)) {
        let mut program = fixture();
        let index = program
            .functions
            .iter()
            .position(|function| function.id.as_str() == "loan.run")
            .unwrap();
        let snapshot = program.functions[index].clone();
        mutate(&mut program.functions[index].loan_plan, &snapshot);
        assert!(crate::hir::validate_core(&program).is_ok());
        let error = match crate::hir::validate(&program) {
            Err(error) => error,
            Ok(()) => panic!("mutation `{name}` unexpectedly validated"),
        };
        assert_eq!(error.code, "SPX-H006");
    }

    #[test]
    fn nested_and_lazy_paths_have_edge_qualified_terminations() {
        let program = fixture();
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "loan.run")
            .unwrap();
        let view = function
            .loan_plan
            .loans
            .iter()
            .find(|loan| loan.cause == LoanCause::SliceView && loan.parent.is_none())
            .unwrap();
        assert!(
            view.end_edges.len() >= 3,
            "nested/lazy exits are edge-qualified"
        );
        assert!(view
            .end_edges
            .iter()
            .all(|edge| (*edge as usize) < function.loan_plan.edges.len()));
    }

    #[test]
    fn every_attached_plan_surface_is_replayed_exactly() {
        run_mutation("schema", |plan, _| plan.schema = "forged");
        run_mutation("id", |plan, _| plan.loans[0].id = LoanId(255));
        run_mutation("site", |plan, function| {
            plan.loans[0].site = function.body.id.clone()
        });
        run_mutation("origin", |plan, function| {
            plan.loans[0].origin.root = function.params[0].id.clone()
        });
        run_mutation("parent", |plan, _| {
            plan.loans[0].parent = Some(plan.loans[0].id)
        });
        run_mutation("start", |plan, _| {
            plan.loans[0].start.phase = LoanPointPhase::After
        });
        run_mutation("ends", |plan, _| plan.loans[0].ends.clear());
        run_mutation("end_edges", |plan, _| plan.loans[0].end_edges.clear());
        run_mutation("endpoint starts", |plan, _| {
            plan.endpoints
                .iter_mut()
                .find(|endpoint| !endpoint.starts.is_empty())
                .unwrap()
                .starts
                .clear()
        });
        run_mutation("endpoint live", |plan, _| {
            plan.endpoints[0].live_after.push(LoanId(255))
        });
        run_mutation("edge live", |plan, _| {
            plan.edges
                .iter_mut()
                .find(|edge| !edge.live.is_empty())
                .unwrap()
                .live
                .clear()
        });
        run_mutation("omission", |plan, _| {
            plan.loans.pop();
        });
    }

    #[test]
    fn own_match_payload_loans_are_canonical_and_cannot_be_omitted() {
        let source = r#"
module test.loan_plan_owned_match;

@id("loan.consume")
fn consume(value: own Bytes) -> i64 { 1 }

@id("loan.inspect")
fn inspect(input: own Option<Bytes>) -> i64 {
    match own input {
        Option::None {} => 0,
        Option::Some { value: bytes } => {
            let observed = if byte_len(bytes_as_slice(bytes)) == 1usize { 1 } else { 0 };
            consume(bytes) + observed
        },
    }
}

@id("app.main") fn main() -> i64 { 0 }
"#;
        let ast = crate::parse(source, Path::new("loan-plan-owned-match.spx")).unwrap();
        assert!(crate::verify::verify(&ast).is_empty());
        let mut program = crate::hir::resolve(&ast).expect("owned match payload loan resolves");
        let index = program
            .functions
            .iter()
            .position(|function| function.id.as_str() == "loan.inspect")
            .unwrap();
        let function = &program.functions[index];
        let match_expression = match &function.body.kind {
            ResolvedExprKind::Match { .. } => &function.body,
            ResolvedExprKind::Block { tail, .. } => tail,
            _ => &function.body,
        };
        let ResolvedExprKind::Match { arms, .. } = &match_expression.kind else {
            panic!("fixture body remains a match")
        };
        let ResolvedMatchPattern::Variant { fields, .. } = &arms[1].pattern else {
            panic!("Some arm retains its variant payload")
        };
        let owned_payload = &fields[0].binding;
        assert_eq!(owned_payload.ownership, OwnershipMode::Own);
        assert!(function
            .loan_plan
            .loans
            .iter()
            .any(|loan| loan.origin.root == owned_payload.id));

        program.functions[index].loan_plan = LoanPlan::empty_v1();
        assert!(crate::hir::validate_core(&program).is_ok());
        let error = crate::hir::validate(&program)
            .expect_err("an owned match payload loan cannot be omitted from hostile HIR");
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.contains("forged shared-loan evidence"));
    }

    #[test]
    fn option_try_residual_edge_terminates_a_normal_path_loan() {
        // Source admission currently rejects `?` with a live owned byte
        // carrier. Exercise the defensive HIR planner directly so that future
        // admission cannot inherit a CFG that lacks the residual-return path.
        let program = fixture();
        let mut function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "loan.run")
            .unwrap()
            .clone();
        let execution = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
        let operand_id = {
            let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
                panic!("fixture body remains a block")
            };
            let ResolvedStatement::Let {
                value: observed, ..
            } = &mut statements[2]
            else {
                panic!("third statement remains the observed branch")
            };
            let ResolvedExprKind::If { condition, .. } = &mut observed.kind else {
                panic!("observed value remains an if")
            };
            let mut operand = (**condition).clone();
            operand.id = ExpressionId::new(&execution, "body.s2.value.condition.operand");
            let operand_id = operand.id.clone();
            condition.kind = ResolvedExprKind::TryOption {
                operand: Box::new(operand),
                option: crate::hir::DeclarationId::new("prelude.option"),
                some_case: crate::hir::DeclarationId::new("prelude.option.some"),
                some_field: crate::hir::DeclarationId::new("prelude.option.some.value"),
                none_case: crate::hir::DeclarationId::new("prelude.option.none"),
                residual_type: ResolvedType::Bool,
            };
            operand_id
        };

        let plan = build_plan(&program, &function).expect("defensive TryOption CFG builds");
        let from = plan
            .endpoints
            .iter()
            .position(|endpoint| {
                endpoint.point.expression == operand_id
                    && endpoint.point.phase == LoanPointPhase::After
            })
            .unwrap() as u16;
        let to = plan
            .endpoints
            .iter()
            .position(|endpoint| {
                endpoint.point.expression == function.body.id
                    && endpoint.point.phase == LoanPointPhase::After
            })
            .unwrap() as u16;
        let residual_edge = plan
            .edges
            .iter()
            .position(|edge| edge.from == from && edge.to == to)
            .expect("TryOption must retain an immediate residual-return edge")
            as u16;
        let loan = plan
            .loans
            .iter()
            .find(|loan| loan.cause == LoanCause::SliceView && loan.parent.is_none())
            .unwrap();
        assert!(loan.end_edges.contains(&residual_edge));
    }

    #[test]
    fn loan_limit_accepts_256_and_rejects_257() {
        fn source(count: usize) -> String {
            let mut source = String::from(
                "module test.loan_limit;\n@id(\"loan.limit\") fn limit(input: borrow Slice<u8>) -> i64 {\nlet owned = bytes_copy(input);\n",
            );
            for index in 0..count {
                source.push_str(&format!("let view{index} = bytes_as_slice(owned);\n"));
            }
            source.push_str("0\n}\n@id(\"app.main\") fn main() -> i64 { 0 }\n");
            source
        }
        let accepted = crate::parse(&source(256), Path::new("loan-limit-256.spx")).unwrap();
        assert!(crate::verify::verify(&accepted).is_empty());
        if let Err(diagnostics) = crate::hir::resolve(&accepted) {
            panic!("256-loan boundary rejected: {diagnostics:?}");
        }
        let rejected = crate::parse(&source(257), Path::new("loan-limit-257.spx")).unwrap();
        assert!(crate::verify::verify(&rejected).is_empty());
        assert!(crate::hir::resolve(&rejected)
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-H006"));
    }

    #[test]
    fn loan_free_function_above_cfg_point_bound_preserves_legacy_admission() {
        let program = fixture();
        let mut function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap()
            .clone();
        let seed = match &function.body.kind {
            ResolvedExprKind::Block { tail, .. } => (**tail).clone(),
            _ => panic!("resolved fixture body must be a block"),
        };
        let execution = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
        function.requires = (0..2_100)
            .map(|index| {
                let mut expression = seed.clone();
                expression.id = ExpressionId::new(&execution, &format!("preflight.{index}"));
                expression
            })
            .collect();

        let plan = build_plan(&program, &function)
            .expect("loan-free preflight must run before the 4,096-point CFG bound");
        assert_eq!(plan.schema, LOAN_PLAN_SCHEMA_V1);
        assert!(plan.loans.is_empty());
        assert!(plan.endpoints.is_empty());
        assert!(plan.edges.is_empty());
    }
}
