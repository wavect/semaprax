//! Deterministic checked-work traversal helpers for the shared-loan planner.

use std::collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque};

use crate::diagnostic::Diagnostic;

use super::{charge, Cfg, LoanId, WorkCounter};

const MAX_CACHED_REACHABILITY_STARTS: usize = 8;

pub(super) type EdgeLiveness = (Vec<Vec<LoanId>>, Vec<Vec<u16>>);

#[derive(Default)]
pub(super) struct ReachabilityCache {
    entries: BTreeMap<u16, Vec<bool>>,
    insertion_order: VecDeque<u16>,
}

impl ReachabilityCache {
    fn retain(
        &mut self,
        cfg: &Cfg<'_>,
        start: u16,
        work: &mut WorkCounter,
    ) -> Result<(), Diagnostic> {
        let inserted = match self.entries.entry(start) {
            Entry::Occupied(_) => false,
            Entry::Vacant(entry) => {
                let mut reachable = vec![false; cfg.points.len()];
                let mut pending = vec![start];
                while let Some(node) = pending.pop() {
                    charge(work)?;
                    if !reachable[node as usize] {
                        reachable[node as usize] = true;
                        pending.extend(cfg.successors[node as usize].iter().rev().copied());
                    }
                }
                entry.insert(reachable);
                true
            }
        };
        if inserted {
            self.insertion_order.push_back(start);
            if self.insertion_order.len() > MAX_CACHED_REACHABILITY_STARTS {
                let oldest = self
                    .insertion_order
                    .pop_front()
                    .expect("an overflowing reachability cache has an oldest start");
                self.entries.remove(&oldest);
            }
        }
        Ok(())
    }
}

pub(super) fn live_nodes(
    cfg: &Cfg<'_>,
    start: u16,
    seeds: &BTreeSet<u16>,
    reachable_by_start: &mut ReachabilityCache,
    work: &mut WorkCounter,
) -> Result<BTreeSet<u16>, Diagnostic> {
    reachable_by_start.retain(cfg, start, work)?;
    let reachable = reachable_by_start
        .entries
        .get(&start)
        .expect("the checked traversal cached this start");
    let mut live = BTreeSet::new();
    let mut pending = seeds
        .iter()
        .filter(|seed| reachable[**seed as usize])
        .copied()
        .collect::<Vec<_>>();
    pending.push(start);
    while let Some(node) = pending.pop() {
        charge(work)?;
        if !reachable[node as usize] || !live.insert(node) || node == start {
            continue;
        }
        pending.extend(cfg.predecessors[node as usize].iter().rev().copied());
    }
    Ok(live)
}

pub(super) fn edge_liveness(
    cfg: &Cfg<'_>,
    live: &[BTreeSet<u16>],
    work: &mut WorkCounter,
) -> Result<EdgeLiveness, Diagnostic> {
    let mut edge_live = vec![Vec::<LoanId>::new(); cfg.edges.len()];
    let mut termination_edges = vec![Vec::<u16>::new(); live.len()];
    for (loan_index, nodes) in live.iter().enumerate() {
        let id = LoanId(loan_index as u16);
        for from in nodes {
            for edge_index in &cfg.successor_edges[*from as usize] {
                charge(work)?;
                let (_, to) = cfg.edges[*edge_index as usize];
                if nodes.contains(&to) {
                    edge_live[*edge_index as usize].push(id);
                } else {
                    termination_edges[loan_index].push(*edge_index);
                }
            }
        }
    }
    Ok((edge_live, termination_edges))
}
