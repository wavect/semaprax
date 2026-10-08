//! Deterministic checked-work traversal helpers for the shared-loan planner.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;

use super::{charge, Cfg, LoanId, WorkCounter};

pub(super) fn live_nodes(
    cfg: &Cfg<'_>,
    start: u16,
    seeds: &BTreeSet<u16>,
    reachable_by_start: &mut BTreeMap<u16, BTreeSet<u16>>,
    work: &mut WorkCounter,
) -> Result<BTreeSet<u16>, Diagnostic> {
    if !reachable_by_start.contains_key(&start) {
        let mut reachable = BTreeSet::new();
        let mut pending = vec![start];
        while let Some(node) = pending.pop() {
            charge(work)?;
            if reachable.insert(node) {
                pending.extend(cfg.successors[node as usize].iter().rev().copied());
            }
        }
        reachable_by_start.insert(start, reachable);
    }
    let reachable = reachable_by_start
        .get(&start)
        .expect("the checked traversal cached this start");
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

pub(super) fn edge_liveness(
    cfg: &Cfg<'_>,
    live: &[BTreeSet<u16>],
    work: &mut WorkCounter,
) -> Result<(Vec<Vec<LoanId>>, Vec<Vec<u16>>), Diagnostic> {
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
