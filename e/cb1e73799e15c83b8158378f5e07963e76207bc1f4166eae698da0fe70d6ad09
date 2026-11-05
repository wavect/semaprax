//! Flat private CFG adjacency over the canonical sorted edge vector.
//!
//! The edge vector is sorted by `(from, to)` and is never reordered here.
//! Successors and successor edge indices of a point are therefore one
//! contiguous run of that vector, addressed by offsets alone. Predecessors are
//! stored in one entry vector in edge-index order, the order per-edge pushes
//! produced.

use std::ops::Range;

use crate::diagnostic::Diagnostic;

use super::{error, Cfg};

pub(super) struct Adjacency {
    successor_offsets: Vec<u16>,
    predecessor_offsets: Vec<u16>,
    predecessors: Vec<u16>,
}

impl Adjacency {
    #[cfg(test)]
    pub(super) fn complete_storage_matches_for_test(&self, other: &Self) -> bool {
        self.successor_offsets == other.successor_offsets
            && self.predecessor_offsets == other.predecessor_offsets
            && self.predecessors == other.predecessors
    }
    #[cfg(test)]
    pub(super) fn build(points: usize, edges: &[(u16, u16)]) -> Result<Self, Diagnostic> {
        Self::build_with_storage(points, edges, None)
    }

    pub(super) fn build_with_storage(
        points: usize,
        edges: &[(u16, u16)],
        payer: Option<&crate::bounded_output::cfg_reservation::CfgReservation>,
    ) -> Result<Self, Diagnostic> {
        let edge_count = u16::try_from(edges.len())
            .map_err(|_| error("function exceeds 4,096 loan CFG edges"))?;
        debug_assert!(edges.windows(2).all(|pair| pair[0] <= pair[1]));
        let mut successor_offsets = super::storage::vector(points + 1, payer)?;
        let mut cursor = 0;
        for node in 0..=points {
            while cursor < edges.len() && (edges[cursor].0 as usize) < node {
                cursor += 1;
            }
            successor_offsets.push(cursor as u16);
        }
        // Count, convert to end offsets, then fill from the last edge so each
        // bucket keeps ascending edge order; every slot ends at its start.
        let mut predecessor_offsets = super::storage::filled(points + 1, 0_u16, payer)?;
        for (_, to) in edges {
            predecessor_offsets[*to as usize] += 1;
        }
        let mut end = 0;
        for slot in &mut predecessor_offsets[..points] {
            end += *slot;
            *slot = end;
        }
        predecessor_offsets[points] = edge_count;
        let mut predecessors = super::storage::filled(edges.len(), 0_u16, payer)?;
        for (from, to) in edges.iter().rev() {
            let slot = &mut predecessor_offsets[*to as usize];
            *slot -= 1;
            predecessors[*slot as usize] = *from;
        }
        Ok(Self {
            successor_offsets,
            predecessor_offsets,
            predecessors,
        })
    }
}

impl Cfg<'_> {
    /// Edge indices leaving `node`, ascending.
    pub(super) fn successor_edges(&self, node: usize) -> Range<u16> {
        self.adjacency.successor_offsets[node]..self.adjacency.successor_offsets[node + 1]
    }

    /// Targets of the edges leaving `node`, in edge order.
    pub(super) fn successors(&self, node: usize) -> impl DoubleEndedIterator<Item = u16> + '_ {
        let edges = self.successor_edges(node);
        self.edges[edges.start as usize..edges.end as usize]
            .iter()
            .map(|(_, to)| *to)
    }

    /// Sources of the edges entering `node`, in edge order.
    pub(super) fn predecessors(&self, node: usize) -> &[u16] {
        let offsets = &self.adjacency.predecessor_offsets;
        &self.adjacency.predecessors[offsets[node] as usize..offsets[node + 1] as usize]
    }
}
