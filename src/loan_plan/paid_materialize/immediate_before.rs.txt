//! Same canonical output order; actual result/temporary/replacement payment.
use super::*;

pub(super) fn materialize<S>(
    cfg: Cfg<'_>,
    drafts: Vec<CfgDraft<S>>,
    parents: Vec<Option<LoanId>>,
    edge_live: Vec<Vec<LoanId>>,
    termination_edges: Vec<Vec<u16>>,
    payer: &crate::bounded_output::cfg_reservation::CfgReservation,
) -> Result<LoanPlan, Diagnostic> {
    // The draft count is exact. Result collection need not retain spare
    // geometric capacity in the authoritative physical loan inventory.
    payer
        .reserve(std::mem::size_of::<LoanPlan>())
        .map_err(|_| storage::marker())?;
    let mut loans = cfg_directory::vector(drafts.len(), payer)?;
    for (index, draft) in drafts.iter().enumerate() {
        let end_edges = storage::clone_vec(&termination_edges[index], Some(payer))?;
        if end_edges.is_empty() {
            return Err(error("loan has no bounded CFG termination edge"));
        }
        let mut ends = cfg_directory::vector(end_edges.len(), payer)?;
        ends.extend(
            end_edges
                .iter()
                .map(|edge| cfg.points[cfg.edges[*edge as usize].1 as usize].clone()),
        );
        ends.dedup();
        loans.push(Loan {
            id: LoanId(index as u16),
            site: draft.site.clone(),
            origin: storage::clone_place(&draft.origin, Some(payer))?,
            parent: parents[index],
            start: cfg.points[draft.start as usize].clone(),
            ends,
            end_edges,
            cause: draft.cause.clone(),
        });
    }
    payer
        .reserve(2 * std::mem::size_of::<Vec<Vec<LoanId>>>())
        .map_err(|_| storage::marker())?;
    let mut starts = storage::filled(cfg.points.len(), Vec::<LoanId>::new(), Some(payer))?;
    let mut kills = storage::filled(cfg.points.len(), Vec::<LoanId>::new(), Some(payer))?;
    for loan in &loans {
        storage::push(
            &mut starts[*cfg.nodes.get(&loan.start).expect("start point retained") as usize],
            loan.id,
            Some(payer),
        )?;
        for edge in &loan.end_edges {
            let to = cfg.edges[*edge as usize].1;
            storage::push(&mut kills[to as usize], loan.id, Some(payer))?;
        }
    }
    let mut endpoints = cfg_directory::vector(cfg.points.len(), payer)?;
    for (node, point) in cfg.points.iter().enumerate() {
        // Both producers append LoanId in increasing loan-index order. Merge
        // borrowed edge rows; never sort or repair canonical plan vectors.
        let live_before = ordered_union::merge(
            cfg.predecessors(node)
                .iter()
                .filter_map(|from| cfg.edges.binary_search(&(*from, node as u16)).ok())
                .map(|edge| edge_live[edge].as_slice()),
            payer,
        )?;
        let live_after = ordered_union::merge(
            cfg.successor_edges(node)
                .map(|edge| cfg.edges[edge as usize].1)
                .filter_map(|to| cfg.edges.binary_search(&(node as u16, to)).ok())
                .map(|edge| edge_live[edge].as_slice()),
            payer,
        )?;
        endpoints.push(LoanEndpoint {
            point: point.clone(),
            live_before: compact_ids::from_vec_paid(live_before, payer)?,
            starts: compact_ids::from_vec_paid(
                storage::clone_vec(&starts[node], Some(payer))?,
                payer,
            )?,
            kills: compact_ids::from_vec_paid(
                storage::clone_vec(&kills[node], Some(payer))?,
                payer,
            )?,
            live_after: compact_ids::from_vec_paid(live_after, payer)?,
        });
    }
    let mut edges = cfg_directory::vector(cfg.edges.len(), payer)?;
    for ((from, to), live) in cfg.edges.iter().zip(edge_live) {
        edges.push(LoanEdge {
            from: *from,
            to: *to,
            live: compact_ids::from_vec_paid(live, payer)?,
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

#[cfg(test)]
mod before;
mod ordered_union;
#[cfg(test)]
mod tests;
