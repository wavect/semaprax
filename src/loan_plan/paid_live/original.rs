//! Exact original checked-work/LIFO/edge order with caller-paid seed/live sets.
use super::*;
#[cfg(test)]
#[path = "tests.rs"]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
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

pub(super) fn live_nodes(
    cfg: &Cfg<'_>,
    loan: LoanId,
    start: u16,
    seeds: &cfg_directory::Set<u16>,
    reachable: &work::Reachability,
    work: &mut WorkCounter,
) -> Result<cfg_directory::Set<u16>, Diagnostic> {
    let payer = work.allocation.ok_or_else(storage::marker)?;
    let mut live = cfg_directory::Set::new(payer)?;
    let mut pending = Vec::new();
    for seed in seeds.iter(payer)? {
        charge(work)?;
        if reachable.contains(loan, *seed) {
            super::storage::push(&mut pending, *seed, work.allocation)?;
        }
    }
    super::storage::push(&mut pending, start, work.allocation)?;
    while let Some(node) = pending.pop() {
        charge(work)?;
        if !reachable.contains(loan, node)
            || !cfg_directory::set_insert(&mut live, node, work.allocation)?
            || node == start
        {
            continue;
        }
        super::storage::extend(
            &mut pending,
            cfg.predecessors(node as usize).iter().rev().copied(),
            work.allocation,
        )?;
    }
    Ok(live)
}

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
