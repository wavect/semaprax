//! Identical CFG construction/order with explicit caller-paid actual storage.
use super::*;

pub(super) fn build<'a>(
    function: &'a ResolvedFunction,
    work: &mut WorkCounter,
) -> Result<Cfg<'a>, Diagnostic> {
    let payer = work.allocation.ok_or_else(storage::marker)?;
    let discovery_stage = crate::bounded_output::reservation_stage_guard(
        crate::bounded_output::ReservationStage::LoanCfgDiscovery,
    );
    let mut roots = cfg_directory::fresh(payer)?;
    storage::extend(
        &mut roots,
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(function.ensures.iter()),
        work.allocation,
    )?;
    let mut expressions = cfg_directory::fresh(payer)?;
    let mut root_by_expression = cfg_directory::Map::<ExpressionId, ExpressionId>::new(payer)?;
    let mut pending = cfg_directory::fresh(payer)?;
    storage::extend(
        &mut pending,
        roots.iter().rev().map(|root| (*root, *root)),
        work.allocation,
    )?;
    let mut child_scratch: Option<Vec<&'a ResolvedExpr>> = None;
    while let Some((expression, root)) = pending.pop() {
        charge(work)?;
        // Both old directories received identical keys at each completed
        // iteration. Keep the old set insertion's zero-byte authentication
        // before its semantic duplicate check, including selected refusal.
        payer.reserve(0).map_err(|_| storage::marker())?;
        if root_by_expression.contains_key(&expression.id) {
            return Err(error("CFG contains a duplicate expression identity"));
        }
        cfg_directory::map_insert(
            &mut root_by_expression,
            expression.id.clone(),
            root.id.clone(),
            work.allocation,
        )?;
        storage::push(&mut expressions, expression, work.allocation)?;
        // Acquire at the original child frontier, after work/identity checks.
        // Only borrowed references enter this one invocation-owned scratch.
        if child_scratch.is_none() {
            child_scratch = Some(cfg_directory::fresh(payer)?);
        } else {
            payer.reserve(0).map_err(|_| storage::marker())?;
        }
        let children = child_scratch.as_mut().expect("child scratch acquired");
        children.clear();
        paid_children::push_children(expression, children, work.allocation)?;
        storage::extend(
            &mut pending,
            children.iter().copied().map(|child| (child, root)),
            work.allocation,
        )?;
        children.clear();
    }
    // No scratch backing is retained in the returned CFG or point phase.
    drop(child_scratch);
    drop(discovery_stage);
    let points_stage = crate::bounded_output::reservation_stage_guard(
        crate::bounded_output::ReservationStage::LoanCfgPoints,
    );
    let node_count = expressions
        .len()
        .checked_mul(2)
        .ok_or_else(|| error("CFG point count overflows"))?;
    if node_count > MAX_LOAN_ENDPOINTS_V1 {
        return Err(error("function exceeds 4,096 loan program points"));
    }
    let mut points = cfg_directory::vector(node_count, payer)?;
    let mut nodes = cfg_directory::Map::new(payer)?;
    for expression in &expressions {
        for phase in [LoanPointPhase::Before, LoanPointPhase::After] {
            let point = point(expression, phase);
            let id =
                u16::try_from(points.len()).map_err(|_| error("CFG point identity overflows"))?;
            cfg_directory::map_insert(&mut nodes, point.clone(), id, work.allocation)?;
            storage::push(&mut points, point, work.allocation)?;
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
    drop(points_stage);
    let _edges_stage = crate::bounded_output::reservation_stage_guard(
        crate::bounded_output::ReservationStage::LoanCfgEdges,
    );
    let mut edge_set = cfg_directory::Set::<(u16, u16)>::new(payer)?;
    for expression in &expressions {
        charge(work)?;
        let before = node(expression, LoanPointPhase::Before)?;
        let after = node(expression, LoanPointPhase::After)?;
        match &expression.kind {
            ResolvedExprKind::LexicalExit { kind, value, .. } => {
                if let Some(value) = value {
                    cfg_directory::set_insert(
                        &mut edge_set,
                        (before, node(value, LoanPointPhase::Before)?),
                        work.allocation,
                    )?;
                    cfg_directory::set_insert(
                        &mut edge_set,
                        (node(value, LoanPointPhase::After)?, after),
                        work.allocation,
                    )?;
                } else {
                    cfg_directory::set_insert(&mut edge_set, (before, after), work.allocation)?;
                }
                let destination = if *kind == crate::ast::LexicalExitKind::Return {
                    root_after(expression)?
                } else {
                    node(
                        lexical_exit::destination(expression, &expressions, work)?,
                        LoanPointPhase::Before,
                    )?
                };
                cfg_directory::set_insert(&mut edge_set, (after, destination), work.allocation)?;
            }
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                let entry = statements
                    .first()
                    .map(statement_entry)
                    .unwrap_or(tail.as_ref());
                cfg_directory::set_insert(
                    &mut edge_set,
                    (before, node(entry, LoanPointPhase::Before)?),
                    work.allocation,
                )?;
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
                            cfg_directory::set_insert(
                                &mut edge_set,
                                (
                                    node(condition, LoanPointPhase::After)?,
                                    node(body, LoanPointPhase::Before)?,
                                ),
                                work.allocation,
                            )?;
                            cfg_directory::set_insert(
                                &mut edge_set,
                                (
                                    node(condition, LoanPointPhase::After)?,
                                    node(next, LoanPointPhase::Before)?,
                                ),
                                work.allocation,
                            )?;
                            if body.ty != ResolvedType::Never {
                                cfg_directory::set_insert(
                                    &mut edge_set,
                                    (
                                        node(body, LoanPointPhase::After)?,
                                        node(condition, LoanPointPhase::Before)?,
                                    ),
                                    work.allocation,
                                )?;
                            }
                        }
                        _ => {
                            cfg_directory::set_insert(
                                &mut edge_set,
                                (
                                    node(statement_exit(statement), LoanPointPhase::After)?,
                                    node(next, LoanPointPhase::Before)?,
                                ),
                                work.allocation,
                            )?;
                        }
                    }
                }
                if tail.ty != ResolvedType::Never {
                    cfg_directory::set_insert(
                        &mut edge_set,
                        (node(tail, LoanPointPhase::After)?, after),
                        work.allocation,
                    )?;
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                cfg_directory::set_insert(
                    &mut edge_set,
                    (before, node(condition, LoanPointPhase::Before)?),
                    work.allocation,
                )?;
                cfg_directory::set_insert(
                    &mut edge_set,
                    (
                        node(condition, LoanPointPhase::After)?,
                        node(then_branch, LoanPointPhase::Before)?,
                    ),
                    work.allocation,
                )?;
                cfg_directory::set_insert(
                    &mut edge_set,
                    (
                        node(condition, LoanPointPhase::After)?,
                        node(else_branch, LoanPointPhase::Before)?,
                    ),
                    work.allocation,
                )?;
                if then_branch.ty != ResolvedType::Never {
                    cfg_directory::set_insert(
                        &mut edge_set,
                        (node(then_branch, LoanPointPhase::After)?, after),
                        work.allocation,
                    )?;
                }
                if else_branch.ty != ResolvedType::Never {
                    cfg_directory::set_insert(
                        &mut edge_set,
                        (node(else_branch, LoanPointPhase::After)?, after),
                        work.allocation,
                    )?;
                }
            }
            ResolvedExprKind::Binary {
                op: BinaryOp::And | BinaryOp::Or,
                left,
                right,
            } => {
                cfg_directory::set_insert(
                    &mut edge_set,
                    (before, node(left, LoanPointPhase::Before)?),
                    work.allocation,
                )?;
                cfg_directory::set_insert(
                    &mut edge_set,
                    (
                        node(left, LoanPointPhase::After)?,
                        node(right, LoanPointPhase::Before)?,
                    ),
                    work.allocation,
                )?;
                cfg_directory::set_insert(
                    &mut edge_set,
                    (node(left, LoanPointPhase::After)?, after),
                    work.allocation,
                )?;
                cfg_directory::set_insert(
                    &mut edge_set,
                    (node(right, LoanPointPhase::After)?, after),
                    work.allocation,
                )?;
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                cfg_directory::set_insert(
                    &mut edge_set,
                    (before, node(scrutinee, LoanPointPhase::Before)?),
                    work.allocation,
                )?;
                for (index, arm) in arms.iter().enumerate() {
                    let entry = arm.guard.as_deref().unwrap_or(&arm.value);
                    cfg_directory::set_insert(
                        &mut edge_set,
                        (
                            node(scrutinee, LoanPointPhase::After)?,
                            node(entry, LoanPointPhase::Before)?,
                        ),
                        work.allocation,
                    )?;
                    if let Some(guard) = &arm.guard {
                        cfg_directory::set_insert(
                            &mut edge_set,
                            (
                                node(guard, LoanPointPhase::After)?,
                                node(&arm.value, LoanPointPhase::Before)?,
                            ),
                            work.allocation,
                        )?;
                        if let Some(next) = arms.get(index + 1) {
                            let next = next.guard.as_deref().unwrap_or(&next.value);
                            cfg_directory::set_insert(
                                &mut edge_set,
                                (
                                    node(guard, LoanPointPhase::After)?,
                                    node(next, LoanPointPhase::Before)?,
                                ),
                                work.allocation,
                            )?;
                        }
                    }
                    if arm.value.ty != ResolvedType::Never {
                        cfg_directory::set_insert(
                            &mut edge_set,
                            (node(&arm.value, LoanPointPhase::After)?, after),
                            work.allocation,
                        )?;
                    }
                }
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                let operand_after = node(operand, LoanPointPhase::After)?;
                cfg_directory::set_insert(
                    &mut edge_set,
                    (before, node(operand, LoanPointPhase::Before)?),
                    work.allocation,
                )?;
                cfg_directory::set_insert(&mut edge_set, (operand_after, after), work.allocation)?;
                // `?` has two semantic successors: normal unwrapping and an
                // immediate residual return from this contract/body root.
                // Keeping the residual edge distinct makes a later loan use
                // live only on the normal path and terminates it on return.
                cfg_directory::set_insert(
                    &mut edge_set,
                    (operand_after, root_after(expression)?),
                    work.allocation,
                )?;
            }
            _ => {
                let mut children = evaluation_children(expression);
                if let Some(first) = children.next() {
                    cfg_directory::set_insert(
                        &mut edge_set,
                        (before, node(first, LoanPointPhase::Before)?),
                        work.allocation,
                    )?;
                    let mut last = first;
                    for next in children {
                        cfg_directory::set_insert(
                            &mut edge_set,
                            (
                                node(last, LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ),
                            work.allocation,
                        )?;
                        last = next;
                    }
                    cfg_directory::set_insert(
                        &mut edge_set,
                        (node(last, LoanPointPhase::After)?, after),
                        work.allocation,
                    )?;
                } else {
                    cfg_directory::set_insert(&mut edge_set, (before, after), work.allocation)?;
                }
            }
        }
    }
    let edges = edge_set.into_vec(payer)?;
    if edges.len() > MAX_LOAN_EDGES_V1 {
        return Err(error("function exceeds 4,096 loan CFG edges"));
    }
    payer
        .reserve(std::mem::size_of::<adjacency::Adjacency>())
        .map_err(|_| storage::marker())?;
    let adjacency =
        adjacency::Adjacency::build_with_storage(points.len(), &edges, work.allocation)?;
    payer
        .reserve(std::mem::size_of::<Cfg<'_>>())
        .map_err(|_| storage::marker())?;
    Ok(Cfg {
        points,
        nodes: cfg_directory::Nodes::Funded(nodes),
        expressions,
        edges,
        adjacency,
    })
}

// Option uses Vec's non-null niche; the single inline carrier stays identical.
const _: () = assert!(
    std::mem::size_of::<Option<Vec<&ResolvedExpr>>>() == std::mem::size_of::<Vec<&ResolvedExpr>>()
);

#[cfg(test)]
#[path = "paid_cfg/original.rs"]
mod original;
#[cfg(test)]
#[path = "paid_cfg/tests.rs"]
mod tests;

// The complete old paid planner uses its old CFG receiver, not this optimization.
#[cfg(test)]
pub(super) fn build_original<'a>(
    function: &'a ResolvedFunction,
    work: &mut WorkCounter,
) -> Result<Cfg<'a>, Diagnostic> {
    if work.allocation.is_some() {
        original::build(function, work)
    } else {
        super::build_cfg(function, work)
    }
}

#[cfg(test)]
#[path = "paid_cfg/rootmap_tests.rs"]
mod rootmap_tests;
#[cfg(test)]
#[path = "paid_cfg/seen_before.rs"]
mod seen_before;
