//! Private independently rebuilt rows, consumed once by the canonical receiver.
//! The attachment is compared only; it never selects storage or derived rows.
pub(crate) use super::paid_materialize::ordered_union::merge;
#[cfg(test)]
pub(crate) use super::paid_materialize::with_materializer_stage;
use super::*;
use crate::bounded_output::cfg_reservation::{CfgReservation, Refused};

pub(crate) struct Source<'hir, 'payer> {
    owner: &'payer CfgReservation,
    program: &'hir ResolvedProgram,
    function: &'hir ResolvedFunction,
    cfg: Cfg<'hir>,
    drafts: Vec<PaidDraft>,
    edge_live: Vec<Vec<LoanId>>,
    starts: Vec<Vec<LoanId>>,
    kills: Vec<Vec<LoanId>>,
}

/// One borrowed whole-CFG descriptor, not a per-endpoint callback or output row.
pub(crate) struct Parts<'a> {
    pub(crate) points: &'a [LoanProgramPoint],
    pub(crate) edges: &'a [(u16, u16)],
    pub(crate) live: &'a [Vec<LoanId>],
    pub(crate) starts: &'a [Vec<LoanId>],
    pub(crate) kills: &'a [Vec<LoanId>],
    pub(crate) successors: &'a [u16],
    pub(crate) predecessor_offsets: &'a [u16],
    pub(crate) predecessors: &'a [u16],
}
pub(crate) struct LoanView<'a> {
    pub(crate) id: LoanId,
    pub(crate) site: &'a ExpressionId,
    pub(crate) cause: &'a LoanCause,
    pub(crate) phase: LoanPointPhase,
}

// Actual new terminal call tuple, distinct from Source ownership/drop.
type EmitArgs<'a, 'p, F> = (
    Cfg<'a>,
    Vec<PaidDraft>,
    Vec<Option<LoanId>>,
    Vec<Vec<LoanId>>,
    Vec<Vec<u16>>,
    &'p CfgReservation,
    &'a ResolvedProgram,
    &'a ResolvedFunction,
    F,
);

/// Cold gateway pays its own checked arithmetic and every receiver transport
/// before the receiver or Source is constructed. These are additional roles;
/// no removed endpoint backing is treated as a call/Result credit.
pub(super) fn acquire<F>(payer: &CfgReservation) -> Result<(), Refused> {
    payer.reserve(
        // New exact-program/function/ledger check call tuple and returned bool,
        // plus the three pointer comparisons inside it, before any arguments.
        2 * std::mem::size_of::<(
            &Source<'_, '_>,
            &ResolvedProgram,
            &ResolvedFunction,
            &CfgReservation,
        )>() + 2 * std::mem::size_of::<bool>()
            + 2 * std::mem::size_of::<(*const ResolvedProgram, *const ResolvedProgram)>()
            + 2 * std::mem::size_of::<(*const ResolvedFunction, *const ResolvedFunction)>()
            + 2 * std::mem::size_of::<(*const CfgReservation, *const CfgReservation)>()
            + 6 * std::mem::size_of::<bool>()
            + 2 * std::mem::size_of::<(&mut Source<'_, '_>, &mut Vec<Place>)>()
            + 2 * std::mem::size_of::<Result<(), Diagnostic>>()
            + 2 * std::mem::size_of::<EmitArgs<'_, '_, F>>()
            + 8 * std::mem::size_of::<&Source<'_, '_>>()
            + 12 * std::mem::size_of::<usize>()
            + 4 * std::mem::size_of::<Option<usize>>()
            + 8 * std::mem::size_of::<Result<(), Diagnostic>>()
            + 8 * std::mem::size_of::<Result<(), Refused>>()
            + 8 * std::mem::size_of::<F>()
            + 8 * std::mem::size_of::<&CfgReservation>()
            + 4 * std::mem::size_of::<Source<'_, '_>>()
            + 4 * std::mem::size_of::<Parts<'_>>()
            + 8 * std::mem::size_of::<bool>(),
    )
}

impl Source<'_, '_> {
    pub(crate) fn is_current(
        &self,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        owner: &CfgReservation,
    ) -> bool {
        std::ptr::eq(self.program, program)
            && std::ptr::eq(self.function, function)
            && std::ptr::eq(self.owner, owner)
            && owner.reserve(0).is_ok()
    }
    pub(crate) fn loan_count(&self) -> usize {
        self.drafts.len()
    }
    pub(crate) fn loan(&self, index: usize) -> LoanView<'_> {
        let draft = &self.drafts[index];
        LoanView {
            id: LoanId(index as u16),
            site: &draft.site,
            cause: &draft.cause,
            phase: self.cfg.points[draft.start as usize].phase,
        }
    }
    pub(crate) fn parts(&self) -> Parts<'_> {
        let (successors, predecessor_offsets, predecessors) = self.cfg.adjacency.borrowed_parts();
        Parts {
            points: &self.cfg.points,
            edges: &self.cfg.edges,
            live: &self.edge_live,
            starts: &self.starts,
            kills: &self.kills,
            successors,
            predecessor_offsets,
            predecessors,
        }
    }
    pub(crate) fn move_origins(&mut self, output: &mut Vec<Place>) -> Result<(), Diagnostic> {
        self.owner.reserve(0).map_err(|_| storage::marker())?;
        // Original final receiving extend, now moving the independently owned
        // draft origin. No intermediate lookup or origin vector is produced.
        output.extend(self.drafts.drain(..).map(|draft| draft.origin));
        Ok(())
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "The independent terminal producer transfers its exact original inventories and one paid receiving closure."
)]
pub(super) fn emit<'a, 'p, F>(
    cfg: Cfg<'a>,
    drafts: Vec<PaidDraft>,
    parents: Vec<Option<LoanId>>,
    edge_live: Vec<Vec<LoanId>>,
    termination_edges: Vec<Vec<u16>>,
    payer: &'p CfgReservation,
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    receive: F,
) -> Result<(), Diagnostic>
where
    F: FnOnce(Option<&mut Source<'a, 'p>>, bool) -> Result<(), Diagnostic>,
{
    payer.reserve(0).map_err(|_| storage::marker())?;
    // LoanView return/receiving and whole row comparison, end-iterator/Option,
    // origin Drain/Map/next/returned Place and actual draft drop roles. This is
    // deliberately not a rebate of the old output aggregate or clone payment.
    let loan_roles = 12 * std::mem::size_of::<LoanView<'_>>()
        + 6 * std::mem::size_of::<(&Source<'_, '_>, usize)>()
        + 12 * std::mem::size_of::<usize>()
        + 4 * std::mem::size_of::<PaidDraft>()
        + 4 * std::mem::size_of::<Option<PaidDraft>>()
        + 4 * std::mem::size_of::<Place>()
        + 16 * std::mem::size_of::<usize>()
        + 16 * std::mem::size_of::<bool>()
        + 8 * std::mem::size_of::<Option<&LoanProgramPoint>>();
    let bytes = loan_roles.checked_mul(drafts.len()).ok_or_else(|| {
        let _ = payer.arithmetic();
        storage::marker()
    })?;
    payer.reserve(bytes).map_err(|_| storage::marker())?;
    // Endpoint/edge comparisons use borrowed rows, scalar cursor and sticky
    // bools only. The two original ordered_union calls and fees remain real.
    let endpoint_roles = 16 * std::mem::size_of::<bool>()
        + 4 * std::mem::size_of::<usize>()
        + 2 * std::mem::size_of::<Option<&LoanEndpoint>>();
    let edge_roles = 8 * std::mem::size_of::<bool>() + 2 * std::mem::size_of::<Option<&LoanEdge>>();
    let bytes = endpoint_roles
        .checked_mul(cfg.points.len())
        .and_then(|bytes| {
            edge_roles
                .checked_mul(cfg.edges.len())
                .and_then(|edges| bytes.checked_add(edges))
        })
        .ok_or_else(|| {
            let _ = payer.arithmetic();
            storage::marker()
        })?;
    payer.reserve(bytes).map_err(|_| storage::marker())?;
    // Preserve original start/kill directory construction and payments; only
    // the complete final Loan/Endpoint/Edge materialization is absent.
    let schema = if cfg
        .expressions
        .iter()
        .any(|e| matches!(e.kind, ResolvedExprKind::LexicalExit { .. }))
    {
        LOAN_PLAN_SCHEMA_V3
    } else if drafts.iter().any(|d| d.cause == LoanCause::TraversalFreeze) {
        LOAN_PLAN_SCHEMA_V2
    } else {
        LOAN_PLAN_SCHEMA_V1
    };
    let attached = &function.loan_plan;
    let mut matched = attached.schema == schema;
    matched &= attached.loans.len() == drafts.len();
    matched &= attached.endpoints.len() == cfg.points.len();
    matched &= attached.edges.len() == cfg.edges.len();
    for (index, draft) in drafts.iter().enumerate() {
        let end_edges = &termination_edges[index];
        if end_edges.is_empty() {
            return Err(error("loan has no bounded CFG termination edge"));
        }
        let start = &cfg.points[draft.start as usize];
        if let Some(loan) = attached.loans.get(index) {
            matched &= loan.id == LoanId(index as u16);
            matched &= loan.site == draft.site;
            matched &= loan.origin == draft.origin;
            matched &= loan.parent == parents[index];
            matched &= loan.start == *start;
            matched &= loan.end_edges == *end_edges;
            matched &= loan.cause == draft.cause;
            let mut previous = None;
            let mut ordinal = 0usize;
            for edge in end_edges {
                let point = &cfg.points[cfg.edges[*edge as usize].1 as usize];
                if previous != Some(point) {
                    matched &= loan.ends.get(ordinal) == Some(point);
                    ordinal += 1;
                }
                previous = Some(point);
            }
            matched &= loan.ends.len() == ordinal;
        } else {
            matched = false;
        }
    }
    payer
        .reserve(2 * std::mem::size_of::<Vec<Vec<LoanId>>>())
        .map_err(|_| storage::marker())?;
    let mut starts = storage::filled(cfg.points.len(), Vec::<LoanId>::new(), Some(payer))?;
    let mut kills = storage::filled(cfg.points.len(), Vec::<LoanId>::new(), Some(payer))?;
    for (index, draft) in drafts.iter().enumerate() {
        let start = &cfg.points[draft.start as usize];
        let end_edges = &termination_edges[index];
        storage::push(
            &mut starts[*cfg.nodes.get(start).expect("start point retained") as usize],
            LoanId(index as u16),
            Some(payer),
        )?;
        for edge in end_edges {
            storage::push(
                &mut kills[cfg.edges[*edge as usize].1 as usize],
                LoanId(index as u16),
                Some(payer),
            )?;
        }
    }
    // Every retained draft origin comes from clone_place's exact vector or
    // funded::resolve's exact count allocation (which rejects excess capacity).
    // This source does not admit arbitrary externally constructed Drafts.
    // Captured owner is rechecked immediately before handing over the only
    // mutable Source loan. Attachment mismatch does not change this handoff.
    payer.reserve(0).map_err(|_| storage::marker())?;
    let mut source = Source {
        owner: payer,
        program,
        function,
        cfg,
        drafts,
        edge_live,
        starts,
        kills,
    };
    receive(Some(&mut source), matched)
}

#[cfg(test)]
pub(crate) fn with_work_limit<T>(limit: usize, operation: impl FnOnce() -> T) -> T {
    funded_flat::with_inventory_work_limit(limit, operation)
}
