//! Same canonical output order; actual result/temporary/replacement payment.
use super::*;

pub(super) fn materialize<S>(
    cfg: Cfg<'_>,
    drafts: Vec<CfgDraft<S>>,
    parents: Vec<Option<LoanId>>,
    edge_live: Vec<Vec<LoanId>>,
    mut termination_edges: Vec<Vec<u16>>,
    payer: &crate::bounded_output::cfg_reservation::CfgReservation,
) -> Result<LoanPlan, Diagnostic> {
    #[cfg(test)]
    if owned_inputs_tests::use_immediate_old() {
        return immediate_before::materialize(
            cfg,
            drafts,
            parents,
            edge_live,
            termination_edges,
            payer,
        );
    }
    // The draft count is exact. Result collection need not retain spare
    // geometric capacity in the authoritative physical loan inventory.
    payer
        .reserve(std::mem::size_of::<LoanPlan>())
        .map_err(|_| storage::marker())?;
    let mut loans = cfg_directory::vector(drafts.len(), payer)?;
    // These are new consuming transports, not a discount of the old borrowed
    // loop. Pay both IntoIterator receivers, its returned iterator, enumerate,
    // the held/drop iterator and the reservation's scalar/result receivers.
    payer
        .array::<DraftCold<S>>(1)
        .map_err(|_| storage::marker())?;
    let mut drafts = drafts.into_iter().enumerate();
    loop {
        // next's mutable loans, Option/pair, owned draft binding, pattern and
        // terminal/drop roles exist even on the exhausted turn. Acquire first.
        payer
            .array::<DraftTurn<'_, S>>(1)
            .map_err(|_| storage::marker())?;
        let Some((index, draft)) = drafts.next() else {
            break;
        };
        // Index at the SAME per-draft frontier: malformed short input must not
        // silently truncate a zip or jump ahead of an earlier missing-end error.
        // mem::take constructs only a new empty Vec; the original row moves.
        payer
            .array::<TakeRow<'_, u16>>(1)
            .map_err(|_| storage::marker())?;
        let end_edges = std::mem::take(&mut termination_edges[index]);
        // The producer grows termination rows geometrically (minimum four).
        // Old clone_vec published exact capacity. Preserve that lifetime bound:
        // pay the complete replacement WHILE old backing is still live, with
        // all new Box/Vec receivers acquired before any selector/conversion.
        payer
            .array::<TerminationCompact<'_>>(1)
            .map_err(|_| storage::marker())?;
        let end_edges = if end_edges.capacity() != end_edges.len() {
            if !end_edges.is_empty() {
                payer
                    .array::<u16>(end_edges.len())
                    .map_err(|_| storage::marker())?;
            }
            end_edges.into_boxed_slice().into_vec()
        } else {
            end_edges
        };
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
            origin: draft.origin,
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
    // Both row directories have exactly one row per CFG point. No row is
    // cloned and no capacity is inferred from an attached authoritative plan.
    payer
        .array::<EndpointCold>(1)
        .map_err(|_| storage::marker())?;
    let mut starts = starts.into_iter();
    let mut kills = kills.into_iter();
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
            starts: {
                payer
                    .array::<EndpointTurn<'_>>(1)
                    .map_err(|_| storage::marker())?;
                compact_ids::from_vec_paid(
                    starts.next().expect("one start row per CFG point"),
                    payer,
                )?
            },
            kills: {
                payer
                    .array::<EndpointTurn<'_>>(1)
                    .map_err(|_| storage::marker())?;
                compact_ids::from_vec_paid(
                    kills.next().expect("one kill row per CFG point"),
                    payer,
                )?
            },
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

// Complete new transport maps. Tuples below are layout invoices only. Repeated
// fields name distinct actual/formal/returned/held/drop occurrences; surviving
// point/site/cause clones, compact replacement and old result/storage fees stay.
type DraftIterator<S> = std::iter::Enumerate<std::vec::IntoIter<CfgDraft<S>>>;
type PaidTurn = (
    usize,
    usize,
    // Layout-only actual/formal captured-owner loans; no static loan is made.
    &'static crate::bounded_output::cfg_reservation::CfgReservation,
    &'static crate::bounded_output::cfg_reservation::CfgReservation,
    Result<(), crate::bounded_output::cfg_reservation::Refused>,
    Result<(), crate::bounded_output::cfg_reservation::Refused>,
    Result<(), Diagnostic>,
    Result<(), Diagnostic>,
);
type DraftCold<S> = (
    Vec<CfgDraft<S>>,
    Vec<CfgDraft<S>>,
    std::vec::IntoIter<CfgDraft<S>>,
    std::vec::IntoIter<CfgDraft<S>>,
    DraftIterator<S>,
    DraftIterator<S>,
    PaidTurn,
);
type DraftTurn<'a, S> = (
    usize,
    usize,
    bool,
    Option<(usize, CfgDraft<S>)>,
    Option<(usize, CfgDraft<S>)>,
    (usize, CfgDraft<S>),
    CfgDraft<S>,
    &'a mut DraftIterator<S>,
    &'a mut DraftIterator<S>,
    PaidTurn,
);
type TakeRow<'a, T> = (
    usize,
    usize,
    &'a mut Vec<Vec<T>>,
    &'a mut Vec<Vec<T>>, // indexing actual/formal
    &'a mut Vec<T>,
    &'a mut Vec<T>,
    &'a mut Vec<T>,
    &'a mut Vec<T>,
    Vec<T>,
    Vec<T>,
    Vec<T>, // default empty, returned row, retained/drop row
    PaidTurn,
);
// Only type layouts are evaluated: no tariff tuple is materialized.
// Actual/formal Vec and Box receivers, both returned/held/drop carriers,
// len/capacity loans/operands, both predicates and two reservation Results.
type TerminationCompact<'a> = (
    Vec<u16>,
    Vec<u16>,
    Vec<u16>,
    Vec<u16>,
    Box<[u16]>,
    Box<[u16]>,
    Box<[u16]>,
    Box<[u16]>,
    &'a Vec<u16>,
    &'a Vec<u16>,
    &'a Vec<u16>,
    &'a Vec<u16>,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    bool,
    bool,
    PaidTurn,
    PaidTurn,
);
type EndpointCold = (
    Vec<Vec<LoanId>>,
    Vec<Vec<LoanId>>,
    Vec<Vec<LoanId>>,
    Vec<Vec<LoanId>>,
    std::vec::IntoIter<Vec<LoanId>>,
    std::vec::IntoIter<Vec<LoanId>>,
    std::vec::IntoIter<Vec<LoanId>>,
    std::vec::IntoIter<Vec<LoanId>>,
    PaidTurn,
);
type EndpointTurn<'a> = (
    usize,
    usize,
    bool,
    &'a mut std::vec::IntoIter<Vec<LoanId>>,
    &'a mut std::vec::IntoIter<Vec<LoanId>>,
    Option<Vec<LoanId>>,
    Option<Vec<LoanId>>,
    Vec<LoanId>,
    Vec<LoanId>,
    Vec<LoanId>,
    Vec<LoanId>,
    PaidTurn,
);

#[cfg(test)]
mod before;
#[cfg(test)]
mod immediate_before;
pub(in crate::loan_plan) mod ordered_union;
#[cfg(test)]
mod owned_inputs_tests;
#[cfg(test)]
mod tests;
