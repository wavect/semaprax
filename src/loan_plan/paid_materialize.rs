//! Same canonical output order; actual result/temporary/replacement payment.
use super::*;

// Inline the successful append: no intermediate owned Diagnostic Results.
// The actual immutable final Box fields and all prior IDs remain complete.
macro_rules! append_endpoint_id {
    ($ids:expr, $id:expr, $payer:expr) => {{
        let ids: &mut Box<[LoanId]> = $ids;
        let id = $id;
        let payer = $payer;
        let count = match ids.len().checked_add(1) {
            Some(count) => count,
            None => {
                payer.arithmetic();
                return Err(storage::marker());
            }
        };
        match payer.array::<LoanId>(count) {
            Ok(()) => (),
            Err(_) => return Err(storage::marker()),
        }
        let requested = match count.checked_mul(std::mem::size_of::<LoanId>()) {
            Some(bytes) => bytes,
            None => {
                payer.arithmetic();
                return Err(storage::marker());
            }
        };
        let mut values = Vec::new();
        if values.try_reserve_exact(count).is_err() {
            payer.allocation(requested);
            return Err(storage::marker());
        }
        if values.capacity() != count {
            let excess = values
                .capacity()
                .saturating_sub(count)
                .saturating_mul(std::mem::size_of::<LoanId>());
            let _ = payer.capacity(requested, excess);
            return Err(storage::marker());
        }
        values.extend_from_slice(ids);
        values.push(id);
        *ids = values.into_boxed_slice();
    }};
}

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
            starts: Box::new([]),
            kills: Box::new([]),
            live_after: compact_ids::from_vec_paid(live_after, payer)?,
        });
    }
    // Publish the same dense endpoints first, then append IDs in the original
    // loan/termination-edge order. No temporary per-point Vec directory exists.
    payer
        .reserve(ENDPOINT_LOOP_CONTROLS)
        .map_err(|_| storage::marker())?;
    for loan in &loans {
        payer
            .reserve(ENDPOINT_APPEND_CONTROLS)
            .map_err(|_| storage::marker())?;
        append_endpoint_id!(
            &mut endpoints[*cfg.nodes.get(&loan.start).expect("start point retained") as usize]
                .starts,
            loan.id,
            payer
        );
        for edge in &loan.end_edges {
            payer
                .reserve(ENDPOINT_APPEND_CONTROLS)
                .map_err(|_| storage::marker())?;
            let to = cfg.edges[*edge as usize].1;
            append_endpoint_id!(&mut endpoints[to as usize].kills, loan.id, payer);
        }
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

// Actual additional borrowed loop controls. No owning IR/result storage bank.
const ENDPOINT_LOOP_CONTROLS: usize = 2 * std::mem::size_of::<std::slice::Iter<'static, Loan>>()
    + 4 * std::mem::size_of::<&Loan>()
    + 2 * std::mem::size_of::<std::slice::Iter<'static, u16>>()
    + 4 * std::mem::size_of::<&u16>();
const ENDPOINT_APPEND_CONTROLS: usize = 4 * std::mem::size_of::<&mut Box<[LoanId]>>()
    + 4 * std::mem::size_of::<&crate::bounded_output::cfg_reservation::CfgReservation>()
    + 4 * std::mem::size_of::<LoanId>()
    + 4 * std::mem::size_of::<Vec<LoanId>>()
    + 2 * std::mem::size_of::<Result<(), crate::bounded_output::cfg_reservation::Refused>>()
    + 2 * std::mem::size_of::<Result<(), std::collections::TryReserveError>>()
    + 4 * std::mem::size_of::<Box<[LoanId]>>()
    + 12 * std::mem::size_of::<usize>()
    + 4 * std::mem::size_of::<Option<usize>>()
    + 4 * std::mem::size_of::<&[LoanId]>()
    + 2 * std::mem::size_of::<Option<&u16>>()
    + 4 * std::mem::size_of::<bool>();

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
#[cfg(test)]
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

// Finite test stages reuse the existing panic-safe materializer director.
#[cfg(test)]
pub(crate) fn with_materializer_stage<T>(before_8086: bool, operation: impl FnOnce() -> T) -> T {
    owned_inputs_tests::with_materializer_stage(before_8086, operation)
}
