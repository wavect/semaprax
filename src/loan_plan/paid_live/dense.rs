//! Private live scratch only: exact point-domain bits, canonical ascending keys.
use super::Membership;
use super::super::{Cfg, Diagnostic, LoanId, WorkCounter, cfg_directory, charge, storage, work};
use crate::bounded_output::cfg_reservation::CfgReservation;
use std::mem::size_of;

pub(in crate::loan_plan) struct Nodes {
    words: Vec<u64>,
    domain_words: usize,
    len: usize,
}
impl Nodes {
    fn new(points: usize, payer: &CfgReservation) -> Result<Self, Diagnostic> {
        payer
            .reserve(2 * size_of::<Self>() + size_of::<Result<Self, Diagnostic>>())
            .map_err(|_| storage::marker())?;
        // The real CFG bound is checked by its producer. Keep this private
        // carrier total even for forged direct inputs; u16 keys are unchanged.
        let domain_words = points.min(usize::from(u16::MAX) + 1).div_ceil(64);
        Ok(Self {
            words: Vec::new(),
            domain_words,
            len: 0,
        })
    }
    fn insert(&mut self, key: u16, payer: &CfgReservation) -> Result<bool, Diagnostic> {
        payer.reserve(0).map_err(|_| storage::marker())?;
        let index = usize::from(key) / 64;
        if index >= self.words.len() {
            // Malformed out-of-domain keys retain the old set's membership,
            // rather than silently disappearing or acquiring a new quota.
            let count = self.domain_words.max(index + 1);
            payer
                .reserve(size_of::<Vec<u64>>() + size_of::<Result<Vec<u64>, Diagnostic>>())
                .map_err(|_| storage::marker())?;
            let mut words = storage::filled(count, 0u64, Some(payer))?;
            words[..self.words.len()].copy_from_slice(&self.words);
            self.words = words;
        }
        let bit = 1u64 << (key % 64);
        let fresh = self.words[index] & bit == 0;
        self.words[index] |= bit;
        self.len += usize::from(fresh);
        Ok(fresh)
    }
    pub(in crate::loan_plan) fn contains(&self, key: &u16) -> bool {
        self.words
            .get(usize::from(*key) / 64)
            .is_some_and(|word| word & (1u64 << (*key % 64)) != 0)
    }
    pub(in crate::loan_plan) fn len(&self) -> usize {
        self.len
    }
    pub(in crate::loan_plan) fn remove(
        &mut self,
        key: &u16,
        payer: &CfgReservation,
    ) -> Result<bool, Diagnostic> {
        payer.reserve(0).map_err(|_| storage::marker())?;
        let Some(word) = self.words.get_mut(usize::from(*key) / 64) else {
            return Ok(false);
        };
        let bit = 1u64 << (*key % 64);
        let present = *word & bit != 0;
        *word &= !bit;
        self.len -= usize::from(present);
        Ok(present)
    }
    pub(in crate::loan_plan) fn values(
        &self,
        payer: &CfgReservation,
    ) -> Result<Keys<'_>, Diagnostic> {
        payer
            .reserve(size_of::<Keys<'_>>() + size_of::<Result<Keys<'_>, Diagnostic>>())
            .map_err(|_| storage::marker())?;
        Ok(Keys {
            words: &self.words,
            index: 0,
            bits: self.words.first().copied().unwrap_or(0),
        })
    }
    #[cfg(test)]
    fn into_vec(self, payer: &CfgReservation) -> Result<Vec<u16>, Diagnostic> {
        let mut values = cfg_directory::vector(self.len, payer)?;
        values.extend(self.values(payer)?);
        Ok(values)
    }
}
impl Membership for Nodes {
    fn contains(&self, node: &u16) -> bool {
        self.contains(node)
    }
}
pub(in crate::loan_plan) struct Keys<'a> {
    words: &'a [u64],
    index: usize,
    bits: u64,
}
impl Iterator for Keys<'_> {
    type Item = u16;
    fn next(&mut self) -> Option<u16> {
        loop {
            if self.index >= self.words.len() {
                return None;
            }
            if self.bits != 0 {
                let bit = self.bits.trailing_zeros() as usize;
                self.bits &= self.bits - 1;
                return Some((self.index * 64 + bit) as u16);
            }
            self.index += 1;
            self.bits = *self.words.get(self.index)?;
        }
    }
}

pub(in crate::loan_plan) fn live_nodes_with_pending(
    cfg: &Cfg<'_>,
    loan: LoanId,
    start: u16,
    seeds: &cfg_directory::Set<u16>,
    reachable: &work::Reachability,
    work: &mut WorkCounter,
    pending: &mut Vec<u16>,
) -> Result<Nodes, Diagnostic> {
    let payer = work.allocation.ok_or_else(storage::marker)?;
    let mut live = Nodes::new(cfg.points.len(), payer)?;
    for seed in seeds.iter(payer)? {
        charge(work)?;
        if reachable.contains(loan, *seed) {
            storage::push(pending, *seed, work.allocation)?;
        }
    }
    storage::push(pending, start, work.allocation)?;
    while let Some(node) = pending.pop() {
        charge(work)?;
        if !reachable.contains(loan, node) || !live.insert(node, payer)? || node == start {
            continue;
        }
        storage::extend(
            pending,
            cfg.predecessors(node as usize).iter().rev().copied(),
            work.allocation,
        )?;
    }
    Ok(live)
}

pub(in crate::loan_plan) fn edge_liveness(
    cfg: &Cfg<'_>,
    live: &[Nodes],
    work: &mut WorkCounter,
) -> Result<work::EdgeLiveness, Diagnostic> {
    let payer = work.allocation.ok_or_else(storage::marker)?;
    let mut edge_live = storage::filled(cfg.edges.len(), Vec::<LoanId>::new(), work.allocation)?;
    let mut termination_edges = storage::filled(live.len(), Vec::<u16>::new(), work.allocation)?;
    for (loan_index, nodes) in live.iter().enumerate() {
        let id = LoanId(loan_index as u16);
        for from in nodes.values(payer)? {
            for edge_index in cfg.successor_edges(from as usize) {
                charge(work)?;
                let (_, to) = cfg.edges[edge_index as usize];
                if nodes.contains(&to) {
                    storage::push(&mut edge_live[edge_index as usize], id, work.allocation)?;
                } else {
                    storage::push(
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

#[cfg(test)]
mod tests;
