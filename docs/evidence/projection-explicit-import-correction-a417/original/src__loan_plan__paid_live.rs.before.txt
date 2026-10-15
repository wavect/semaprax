//! Exact original checked-work/LIFO/edge order with caller-paid seed/live sets.
use super::*;
#[cfg(test)]
mod tests;
pub(super) trait Membership {
    fn contains(&self, node: &u16) -> bool;
}
impl Membership for BTreeSet<u16> {
    fn contains(&self, node: &u16) -> bool {
        BTreeSet::contains(self, node)
    }
}
impl Membership for cfg_directory::Set<u16> {
    fn contains(&self, node: &u16) -> bool {
        cfg_directory::Set::contains(self, node)
    }
}

#[cfg(test)]
pub(super) fn live_nodes_with_pending(
    cfg: &Cfg<'_>,
    loan: LoanId,
    start: u16,
    seeds: &cfg_directory::Set<u16>,
    reachable: &work::Reachability,
    work: &mut WorkCounter,
    pending: &mut Vec<u16>,
) -> Result<cfg_directory::Set<u16>, Diagnostic> {
    let payer = work.allocation.ok_or_else(storage::marker)?;
    let mut live = cfg_directory::Set::new(payer)?;
    for seed in seeds.iter(payer)? {
        charge(work)?;
        if reachable.contains(loan, *seed) {
            super::storage::push(pending, *seed, work.allocation)?;
        }
    }
    super::storage::push(pending, start, work.allocation)?;
    while let Some(node) = pending.pop() {
        charge(work)?;
        if !reachable.contains(loan, node)
            || !cfg_directory::set_insert(&mut live, node, work.allocation)?
            || node == start
        {
            continue;
        }
        super::storage::extend(
            pending,
            cfg.predecessors(node as usize).iter().rev().copied(),
            work.allocation,
        )?;
    }
    Ok(live)
}

#[cfg(test)]
pub(super) fn edge_liveness(
    cfg: &Cfg<'_>,
    live: &[cfg_directory::Set<u16>],
    work: &mut WorkCounter,
) -> Result<work::EdgeLiveness, Diagnostic> {
    let payer = work.allocation.ok_or_else(storage::marker)?;
    let mut edge_live =
        super::storage::filled(cfg.edges.len(), Vec::<LoanId>::new(), work.allocation)?;
    let mut termination_edges =
        super::storage::filled(live.len(), Vec::<u16>::new(), work.allocation)?;
    for (loan_index, nodes) in live.iter().enumerate() {
        let id = LoanId(loan_index as u16);
        for from in nodes.iter(payer)? {
            for edge_index in cfg.successor_edges(*from as usize) {
                charge(work)?;
                let (_, to) = cfg.edges[edge_index as usize];
                if nodes.contains(&to) {
                    super::storage::push(&mut edge_live[edge_index as usize], id, work.allocation)?;
                } else {
                    super::storage::push(
                        &mut termination_edges[loan_index],
                        edge_index,
                        work.allocation,
                    )?;
                }
            }
        }
    }
    Ok((edge_live, termination_edges))
}

// Existing standalone owners retain their original one-visit scope.
#[cfg(test)]
pub(super) fn live_nodes(
    cfg: &Cfg<'_>,
    loan: LoanId,
    start: u16,
    seeds: &cfg_directory::Set<u16>,
    reachable: &work::Reachability,
    work: &mut WorkCounter,
) -> Result<cfg_directory::Set<u16>, Diagnostic> {
    live_nodes_with_pending(cfg, loan, start, seeds, reachable, work, &mut Vec::new())
}

#[cfg(test)]
#[path = "paid_live/original.rs"]
pub(super) mod original;
#[cfg(test)]
#[path = "paid_live/reuse_tests.rs"]
mod reuse_tests;

#[cfg(test)]
#[path = "paid_live/funded_before.rs"]
mod funded_before;

// The complete prior funded facade delegates only to its complete prior plan;
// its source/lexical checks and semantic/physical publication remain unchanged.
#[cfg(test)]
mod before_plan {
    use super::*;
    pub(super) fn build(
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        work: &mut WorkCounter,
    ) -> Result<LoanPlan, Diagnostic> {
        paid_plan::build_before_live_reuse(program, function, work)
    }
}

// Lexical test bridges preserve the original helper's private signatures and
// entire bodies; they only permit the complete old planner to call them.
#[cfg(test)]
pub(super) mod before_plan_live {
    use super::*;
    pub(in crate::loan_plan) fn live_nodes(
        cfg: &Cfg<'_>,
        loan: LoanId,
        start: u16,
        seeds: &cfg_directory::Set<u16>,
        reachable: &work::Reachability,
        work: &mut WorkCounter,
    ) -> Result<cfg_directory::Set<u16>, Diagnostic> {
        original::live_nodes(cfg, loan, start, seeds, reachable, work)
    }
    pub(in crate::loan_plan) fn edge_liveness(
        cfg: &Cfg<'_>,
        live: &[cfg_directory::Set<u16>],
        work: &mut WorkCounter,
    ) -> Result<work::EdgeLiveness, Diagnostic> {
        original::edge_liveness(cfg, live, work)
    }
}

// Ordinary funded receiver; complete AVL functions above remain independent.
pub(super) mod dense;
