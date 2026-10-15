//! Saved original per-point adjacency, traversals and child collection.
//!
//! This is the independent oracle for the flat adjacency: nested `Vec<Vec<u16>>`
//! tables filled by one push per sorted edge, the original traversal drivers
//! with their exact charge order, and the original `Vec`-returning child list.
//! None of it is reachable from production builds.
//!
//! This is a partial oracle: its reachability queue is a plain vector that
//! reproduces the move-to-front order, not the original intrusive `PointQueue`.
//! The exact original planner, queue included, is `saved_planner`; this module
//! stays as a second, structurally different reference for the same tables.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind};

use super::{charge, LoanId, WorkCounter};

type EdgeLiveness = (Vec<Vec<LoanId>>, Vec<Vec<u16>>);

pub(super) struct LegacyAdjacency {
    pub(super) successors: Vec<Vec<u16>>,
    pub(super) successor_edges: Vec<Vec<u16>>,
    pub(super) predecessors: Vec<Vec<u16>>,
}

impl LegacyAdjacency {
    pub(super) fn from_edges(points: usize, edges: &[(u16, u16)]) -> Self {
        let mut successors = vec![Vec::new(); points];
        let mut successor_edges = vec![Vec::new(); points];
        let mut predecessors = vec![Vec::new(); points];
        for (edge, (from, to)) in edges.iter().enumerate() {
            successors[*from as usize].push(*to);
            successor_edges[*from as usize].push(edge as u16);
            predecessors[*to as usize].push(*from);
        }
        Self {
            successors,
            successor_edges,
            predecessors,
        }
    }
}

pub(super) struct LegacyReachability {
    by_point: Vec<u64>,
    words: usize,
}

impl LegacyReachability {
    pub(super) fn contains(&self, loan: usize, node: usize) -> bool {
        self.by_point[node * self.words + loan / 64] & (1u64 << (loan % 64)) != 0
    }
}

// The production queue is an intrusive move-to-front stack. Here the same
// order is a plain vector whose last element is the front.
fn push_front(stack: &mut Vec<u16>, point: u16) {
    if stack.last() == Some(&point) {
        return;
    }
    stack.retain(|queued| *queued != point);
    stack.push(point);
}

pub(super) fn reachability(
    adjacency: &LegacyAdjacency,
    points: usize,
    starts: &[u16],
    work: &mut WorkCounter,
) -> Result<LegacyReachability, Diagnostic> {
    let words = starts.len().div_ceil(u64::BITS as usize);
    let mut by_point = vec![0_u64; words * points];
    let mut stack = Vec::new();
    for _ in &by_point {
        charge(work)?;
    }
    for (loan, start) in starts.iter().copied().enumerate().rev() {
        charge(work)?;
        by_point[start as usize * words + loan / 64] |= 1u64 << (loan % 64);
        push_front(&mut stack, start);
    }
    while let Some(from) = stack.pop() {
        charge(work)?;
        for to in adjacency.successors[from as usize].iter().rev() {
            let mut changed = false;
            for word in 0..words {
                charge(work)?;
                let before = by_point[*to as usize * words + word];
                let after = before | by_point[from as usize * words + word];
                by_point[*to as usize * words + word] = after;
                changed |= before != after;
            }
            if changed {
                charge(work)?;
                push_front(&mut stack, *to);
            }
        }
    }
    Ok(LegacyReachability { by_point, words })
}

pub(super) fn live_nodes(
    adjacency: &LegacyAdjacency,
    reachable: &LegacyReachability,
    loan: usize,
    start: u16,
    seeds: &BTreeSet<u16>,
    work: &mut WorkCounter,
) -> Result<BTreeSet<u16>, Diagnostic> {
    let mut live = BTreeSet::new();
    let mut pending = Vec::new();
    for seed in seeds {
        charge(work)?;
        if reachable.contains(loan, *seed as usize) {
            pending.push(*seed);
        }
    }
    pending.push(start);
    while let Some(node) = pending.pop() {
        charge(work)?;
        if !reachable.contains(loan, node as usize) || !live.insert(node) || node == start {
            continue;
        }
        pending.extend(adjacency.predecessors[node as usize].iter().rev().copied());
    }
    Ok(live)
}

pub(super) fn edge_liveness(
    adjacency: &LegacyAdjacency,
    edges: &[(u16, u16)],
    live: &[BTreeSet<u16>],
    work: &mut WorkCounter,
) -> Result<EdgeLiveness, Diagnostic> {
    let mut edge_live = vec![Vec::<LoanId>::new(); edges.len()];
    let mut termination_edges = vec![Vec::<u16>::new(); live.len()];
    for (loan_index, nodes) in live.iter().enumerate() {
        let id = LoanId(loan_index as u16);
        for from in nodes {
            for edge_index in &adjacency.successor_edges[*from as usize] {
                charge(work)?;
                let (_, to) = edges[*edge_index as usize];
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

pub(super) fn evaluation_children(expression: &ResolvedExpr) -> Vec<&ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::LexicalExit { value, .. } => value.as_deref().into_iter().collect(),
        ResolvedExprKind::Closure { captures, .. } => {
            captures.iter().map(|capture| &capture.value).collect()
        }
        ResolvedExprKind::FunctionReference { .. } => Vec::new(),
        ResolvedExprKind::Invoke { callable, args } => std::iter::once(callable.as_ref())
            .chain(args.iter())
            .collect(),
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. }
        | ResolvedExprKind::RowLoan { args, .. } => args.iter().collect(),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().collect(),
        ResolvedExprKind::HostCommandCall(call) => call.args.iter().collect(),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => vec![source, start, end],
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => vec![value],
        ResolvedExprKind::Binary { left, right, .. } => vec![left, right],
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.iter().map(|field| &field.value).collect()
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => std::iter::once(base.as_ref())
            .chain(fields.iter().map(|field| &field.value))
            .collect(),
        ResolvedExprKind::Block { .. }
        | ResolvedExprKind::VecConstructor { .. }
        | ResolvedExprKind::If { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::BorrowedUtf8Literal(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => Vec::new(),
    }
}
