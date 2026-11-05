//! Identical CFG construction/order with explicit caller-paid actual storage.
#![allow(
    clippy::duplicate_mod,
    reason = "Complete independently compiled before builders retain their original modules and fixtures."
)]
use super::*;

#[allow(
    dead_code,
    reason = "Retain the complete independently compiled pre-phase-pair builder."
)]
#[path = "phase_pairs_before.rs"]
mod phase_pairs_before;
#[allow(
    dead_code,
    reason = "Complete independently compiled pre-shared-directory builder."
)]
#[path = "root_directory_before.rs"]
mod root_directory_before;

#[cfg(test)]
pub(super) fn phase_pairs_before_for_test<'a>(
    function: &'a ResolvedFunction,
    work: &mut WorkCounter,
) -> Result<Cfg<'a>, Diagnostic> {
    phase_pairs_before::build(function, work)
}

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
    let mut expression_roots = cfg_directory::fresh(payer)?;
    let mut root_by_expression = cfg_directory::Map::<ExpressionId, Option<[u16; 2]>>::new(payer)?;
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
            None,
            work.allocation,
        )?;
        storage::push(&mut expressions, expression, work.allocation)?;
        storage::push(&mut expression_roots, root, work.allocation)?;
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
    // Move the complete discovery directory; do not clone keys or root IR.
    payer
        .reserve(2 * std::mem::size_of::<cfg_directory::Map<ExpressionId, Option<[u16; 2]>>>())
        .map_err(|_| storage::marker())?;
    let mut nodes = root_by_expression;
    for expression in &expressions {
        // MAX_LOAN_ENDPOINTS_V1 above bounds the complete pair below 4096.
        // Price the real pair's initializer/argument/receiving copies and both
        // actual index values; Map independently pays its complete actual node.
        payer
            .reserve(4 * std::mem::size_of::<Option<[u16; 2]>>() + 4 * std::mem::size_of::<u16>())
            .map_err(|_| storage::marker())?;
        let before =
            u16::try_from(points.len()).map_err(|_| error("CFG point identity overflows"))?;
        for phase in [LoanPointPhase::Before, LoanPointPhase::After] {
            let point = point(expression, phase);
            storage::push(&mut points, point, work.allocation)?;
        }
        cfg_directory::map_insert(
            &mut nodes,
            expression.id.clone(),
            Some([before, before + 1]),
            work.allocation,
        )?;
    }
    // The full ordinary point lookup remains the sole interface in CFG assembly.
    // Pay the actual complete enum's initializer and receiving value before it
    // owns the same complete phase-pair map that the final Cfg will retain.
    payer
        .reserve(2 * std::mem::size_of::<cfg_directory::Nodes>())
        .map_err(|_| storage::marker())?;
    let nodes = cfg_directory::Nodes::PhaseRoots(nodes);
    let node = |expression: &ResolvedExpr, phase: LoanPointPhase| {
        nodes
            .get(&point(expression, phase))
            .copied()
            .ok_or_else(|| error("CFG child point is not indexed"))
    };
    let root_after = |expression: &ResolvedExpr| {
        let before = nodes
            .get(&point(expression, LoanPointPhase::Before))
            .ok_or_else(|| error("CFG expression lacks a function-exit root"))?;
        let root = expression_roots
            .get(usize::from(*before) / 2)
            .ok_or_else(|| error("CFG expression lacks a function-exit root"))?;
        nodes
            .get(&LoanProgramPoint {
                expression: root.id.clone(),
                phase: LoanPointPhase::After,
            })
            .copied()
            .ok_or_else(|| error("CFG function-exit root is not indexed"))
    };
    drop(points_stage);
    let _edges_stage = crate::bounded_output::reservation_stage_guard(
        crate::bounded_output::ReservationStage::LoanCfgEdges,
    );
    let mut edge_pairs = cfg_directory::fresh::<(u16, u16)>(payer)?;
    for expression in &expressions {
        charge(work)?;
        let before = node(expression, LoanPointPhase::Before)?;
        let after = node(expression, LoanPointPhase::After)?;
        match &expression.kind {
            ResolvedExprKind::LexicalExit { kind, value, .. } => {
                if let Some(value) = value {
                    storage::push(
                        &mut edge_pairs,
                        (before, node(value, LoanPointPhase::Before)?),
                        work.allocation,
                    )?;
                    storage::push(
                        &mut edge_pairs,
                        (node(value, LoanPointPhase::After)?, after),
                        work.allocation,
                    )?;
                } else {
                    storage::push(&mut edge_pairs, (before, after), work.allocation)?;
                }
                let destination = if *kind == crate::ast::LexicalExitKind::Return {
                    root_after(expression)?
                } else {
                    node(
                        lexical_exit::destination(expression, &expressions, work)?,
                        LoanPointPhase::Before,
                    )?
                };
                storage::push(&mut edge_pairs, (after, destination), work.allocation)?;
            }
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                let entry = statements
                    .first()
                    .map(statement_entry)
                    .unwrap_or(tail.as_ref());
                storage::push(
                    &mut edge_pairs,
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
                            storage::push(
                                &mut edge_pairs,
                                (
                                    node(condition, LoanPointPhase::After)?,
                                    node(body, LoanPointPhase::Before)?,
                                ),
                                work.allocation,
                            )?;
                            storage::push(
                                &mut edge_pairs,
                                (
                                    node(condition, LoanPointPhase::After)?,
                                    node(next, LoanPointPhase::Before)?,
                                ),
                                work.allocation,
                            )?;
                            if body.ty != ResolvedType::Never {
                                storage::push(
                                    &mut edge_pairs,
                                    (
                                        node(body, LoanPointPhase::After)?,
                                        node(condition, LoanPointPhase::Before)?,
                                    ),
                                    work.allocation,
                                )?;
                            }
                        }
                        _ => {
                            storage::push(
                                &mut edge_pairs,
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
                    storage::push(
                        &mut edge_pairs,
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
                storage::push(
                    &mut edge_pairs,
                    (before, node(condition, LoanPointPhase::Before)?),
                    work.allocation,
                )?;
                storage::push(
                    &mut edge_pairs,
                    (
                        node(condition, LoanPointPhase::After)?,
                        node(then_branch, LoanPointPhase::Before)?,
                    ),
                    work.allocation,
                )?;
                storage::push(
                    &mut edge_pairs,
                    (
                        node(condition, LoanPointPhase::After)?,
                        node(else_branch, LoanPointPhase::Before)?,
                    ),
                    work.allocation,
                )?;
                if then_branch.ty != ResolvedType::Never {
                    storage::push(
                        &mut edge_pairs,
                        (node(then_branch, LoanPointPhase::After)?, after),
                        work.allocation,
                    )?;
                }
                if else_branch.ty != ResolvedType::Never {
                    storage::push(
                        &mut edge_pairs,
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
                storage::push(
                    &mut edge_pairs,
                    (before, node(left, LoanPointPhase::Before)?),
                    work.allocation,
                )?;
                storage::push(
                    &mut edge_pairs,
                    (
                        node(left, LoanPointPhase::After)?,
                        node(right, LoanPointPhase::Before)?,
                    ),
                    work.allocation,
                )?;
                storage::push(
                    &mut edge_pairs,
                    (node(left, LoanPointPhase::After)?, after),
                    work.allocation,
                )?;
                storage::push(
                    &mut edge_pairs,
                    (node(right, LoanPointPhase::After)?, after),
                    work.allocation,
                )?;
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                storage::push(
                    &mut edge_pairs,
                    (before, node(scrutinee, LoanPointPhase::Before)?),
                    work.allocation,
                )?;
                for (index, arm) in arms.iter().enumerate() {
                    let entry = arm.guard.as_deref().unwrap_or(&arm.value);
                    storage::push(
                        &mut edge_pairs,
                        (
                            node(scrutinee, LoanPointPhase::After)?,
                            node(entry, LoanPointPhase::Before)?,
                        ),
                        work.allocation,
                    )?;
                    if let Some(guard) = &arm.guard {
                        storage::push(
                            &mut edge_pairs,
                            (
                                node(guard, LoanPointPhase::After)?,
                                node(&arm.value, LoanPointPhase::Before)?,
                            ),
                            work.allocation,
                        )?;
                        if let Some(next) = arms.get(index + 1) {
                            let next = next.guard.as_deref().unwrap_or(&next.value);
                            storage::push(
                                &mut edge_pairs,
                                (
                                    node(guard, LoanPointPhase::After)?,
                                    node(next, LoanPointPhase::Before)?,
                                ),
                                work.allocation,
                            )?;
                        }
                    }
                    if arm.value.ty != ResolvedType::Never {
                        storage::push(
                            &mut edge_pairs,
                            (node(&arm.value, LoanPointPhase::After)?, after),
                            work.allocation,
                        )?;
                    }
                }
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                let operand_after = node(operand, LoanPointPhase::After)?;
                storage::push(
                    &mut edge_pairs,
                    (before, node(operand, LoanPointPhase::Before)?),
                    work.allocation,
                )?;
                storage::push(&mut edge_pairs, (operand_after, after), work.allocation)?;
                // `?` has two semantic successors: normal unwrapping and an
                // immediate residual return from this contract/body root.
                // Keeping the residual edge distinct makes a later loan use
                // live only on the normal path and terminates it on return.
                storage::push(
                    &mut edge_pairs,
                    (operand_after, root_after(expression)?),
                    work.allocation,
                )?;
            }
            _ => {
                let mut children = evaluation_children(expression);
                if let Some(first) = children.next() {
                    storage::push(
                        &mut edge_pairs,
                        (before, node(first, LoanPointPhase::Before)?),
                        work.allocation,
                    )?;
                    let mut last = first;
                    for next in children {
                        storage::push(
                            &mut edge_pairs,
                            (
                                node(last, LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ),
                            work.allocation,
                        )?;
                        last = next;
                    }
                    storage::push(
                        &mut edge_pairs,
                        (node(last, LoanPointPhase::After)?, after),
                        work.allocation,
                    )?;
                } else {
                    storage::push(&mut edge_pairs, (before, after), work.allocation)?;
                }
            }
        }
    }
    // These are private CFG metadata edges. Their prior AVL collector emitted
    // lexicographic unique pairs; preserve that complete publication order.
    payer
        .reserve(
            2 * std::mem::size_of::<&mut [(u16, u16)]>()
                + 2 * std::mem::size_of::<&mut Vec<(u16, u16)>>()
                + std::mem::size_of::<usize>()
                + std::mem::size_of::<std::cmp::Ordering>(),
        )
        .map_err(|_| storage::marker())?;
    edge_pairs.sort_unstable();
    edge_pairs.dedup();
    let edges = edge_pairs;
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
        nodes,
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
