//! Conservative event-capacity preflight for native conformance tracing.
//!
//! The native trace buffer is caller-provided and must not overflow after an
//! owned argument crosses the ABI boundary.  This module computes a
//! deterministic upper bound from the already validated, acyclic cleanup
//! plan.  It does not enable resource lowering or emit trace events.

#![cfg_attr(not(test), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet};

use crate::cleanup_plan::{
    BlockId, CleanupBlock, CleanupEdge, CleanupPlan, CleanupTerminator, EdgeId, ExitContinuation,
    ExitTarget, ExitTargetId, CLEANUP_PLAN_SCHEMA_V2,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ResolvedFunction, ResolvedProgram, ResolvedResourceDropKind,
    ResolvedTypeDeclarationKind,
};

const TRANSITION_EVENTS: u32 = 1;
const TRIVIAL_FINALIZER_EVENTS: u32 = 2;
const IMPORTED_FINALIZER_EVENTS: u32 = 4;
const RESULT_COMMIT_EVENTS: u32 = 1;

/// Return a conservative event capacity for one validated cleanup plan.
///
/// Each reachable transition emits one event.  A trivial finalizer emits
/// `finalize_begin` and `finalize_end`; an imported finalizer additionally
/// emits `import_begin` and success-only `import_end`.  A result commit emits
/// one event.  Sequential costs are summed and mutually exclusive branches
/// take their maximum.  False finalizer guards may make an actual trace
/// shorter, never longer.
pub(super) fn required_event_capacity(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<u32, Diagnostic> {
    required_event_capacity_for_plan(program, &function.cleanup_plan)
}

fn required_event_capacity_for_plan(
    program: &ResolvedProgram,
    plan: &CleanupPlan,
) -> Result<u32, Diagnostic> {
    if plan.schema != CLEANUP_PLAN_SCHEMA_V2 {
        return Err(trace_error(format!(
            "native trace preflight requires cleanup schema `{CLEANUP_PLAN_SCHEMA_V2}`, found `{}`",
            plan.schema
        )));
    }

    let blocks = unique_index(&plan.blocks, |block| block.id, "block")?;
    let edges = unique_index(&plan.edges, |edge| edge.id, "edge")?;
    let exits = unique_index(&plan.exits, |exit| exit.id, "exit")?;
    validate_references(plan, &blocks, &edges, &exits)?;
    let lifecycle_weights = lifecycle_weights(program)?;

    let calculator = CapacityCalculator {
        blocks,
        edges,
        exits,
        lifecycle_weights,
    };
    calculator.capacity_from_block(plan.entry)
}

struct CapacityCalculator<'a> {
    blocks: BTreeMap<BlockId, &'a CleanupBlock>,
    edges: BTreeMap<EdgeId, &'a CleanupEdge>,
    exits: BTreeMap<ExitTargetId, &'a ExitTarget>,
    lifecycle_weights: BTreeMap<DeclarationId, u32>,
}

impl CapacityCalculator<'_> {
    fn capacity_from_block(&self, entry: BlockId) -> Result<u32, Diagnostic> {
        let postorder = self.reachable_postorder(entry)?;
        let mut capacities = BTreeMap::new();
        for id in postorder {
            let block = self.block(id)?;
            let transitions = u32::try_from(block.transitions.len())
                .map_err(|_| trace_error(format!("block {} has too many transitions", id.0)))?;
            let transition_capacity =
                checked_mul(transitions, TRANSITION_EVENTS, "transition event capacity")?;
            let continuation_capacity = match &block.terminator {
                CleanupTerminator::Goto(edge) => {
                    let target = self.edge_target(id, *edge)?;
                    capacity_at(&capacities, target)?
                }
                CleanupTerminator::Branch(edges) => {
                    if edges.is_empty() {
                        return Err(trace_error(format!(
                            "cleanup branch at block {} has no edges",
                            id.0
                        )));
                    }
                    let mut maximum = 0;
                    for edge in edges {
                        let target = self.edge_target(id, *edge)?;
                        maximum = maximum.max(capacity_at(&capacities, target)?);
                    }
                    maximum
                }
                CleanupTerminator::Exit(exit) => self.exit_capacity(id, *exit, &capacities)?,
            };
            capacities.insert(
                id,
                checked_add(
                    transition_capacity,
                    continuation_capacity,
                    "block event capacity",
                )?,
            );
        }
        capacity_at(&capacities, entry)
    }

    fn reachable_postorder(&self, entry: BlockId) -> Result<Vec<BlockId>, Diagnostic> {
        #[derive(Clone, Copy)]
        enum Step {
            Enter(BlockId),
            Finish(BlockId),
        }

        const VISITING: u8 = 1;
        const DONE: u8 = 2;

        let mut marks = BTreeMap::new();
        let mut postorder = Vec::new();
        let mut stack = vec![Step::Enter(entry)];
        while let Some(step) = stack.pop() {
            match step {
                Step::Enter(id) => match marks.get(&id).copied() {
                    Some(DONE) => continue,
                    Some(VISITING) => {
                        return Err(trace_error(format!(
                            "native trace preflight found a reachable cleanup cycle at block {}",
                            id.0
                        )));
                    }
                    None => {
                        self.block(id)?;
                        marks.insert(id, VISITING);
                        stack.push(Step::Finish(id));
                        for successor in self.successors(id)?.into_iter().rev() {
                            match marks.get(&successor).copied() {
                                Some(VISITING) => {
                                    return Err(trace_error(format!(
                                        "native trace preflight found a reachable cleanup cycle at block {}",
                                        successor.0
                                    )));
                                }
                                Some(DONE) => {}
                                None => stack.push(Step::Enter(successor)),
                                Some(_) => unreachable!("only traversal marks one and two exist"),
                            }
                        }
                    }
                    Some(_) => unreachable!("only traversal marks one and two exist"),
                },
                Step::Finish(id) => {
                    marks.insert(id, DONE);
                    postorder.push(id);
                }
            }
        }
        Ok(postorder)
    }

    fn successors(&self, id: BlockId) -> Result<Vec<BlockId>, Diagnostic> {
        let block = self.block(id)?;
        match &block.terminator {
            CleanupTerminator::Goto(edge) => Ok(vec![self.edge_target(id, *edge)?]),
            CleanupTerminator::Branch(edges) => {
                if edges.is_empty() {
                    return Err(trace_error(format!(
                        "cleanup branch at block {} has no edges",
                        id.0
                    )));
                }
                edges
                    .iter()
                    .map(|edge| self.edge_target(id, *edge))
                    .collect()
            }
            CleanupTerminator::Exit(exit) => {
                let exit = self.exit(id, *exit)?;
                match &exit.continuation {
                    ExitContinuation::Continue(edge) => Ok(vec![self.edge_target(id, *edge)?]),
                    ExitContinuation::CommitResult { .. }
                    | ExitContinuation::ReturnFailure { .. }
                    | ExitContinuation::ReturnUnit => Ok(Vec::new()),
                }
            }
        }
    }

    fn block(&self, id: BlockId) -> Result<&CleanupBlock, Diagnostic> {
        self.blocks
            .get(&id)
            .copied()
            .ok_or_else(|| trace_error(format!("cleanup entry references missing block {}", id.0)))
    }

    fn edge_target(&self, block: BlockId, id: EdgeId) -> Result<BlockId, Diagnostic> {
        let edge = self.edges.get(&id).copied().ok_or_else(|| {
            trace_error(format!("cleanup block references missing edge {}", id.0))
        })?;
        if edge.from != block {
            return Err(trace_error(format!(
                "cleanup edge {} starts at block {}, not block {}",
                id.0, edge.from.0, block.0
            )));
        }
        Ok(edge.to)
    }

    fn exit(&self, block: BlockId, id: ExitTargetId) -> Result<&ExitTarget, Diagnostic> {
        let exit = self.exits.get(&id).copied().ok_or_else(|| {
            trace_error(format!("cleanup block references missing exit {}", id.0))
        })?;
        if exit.from != block {
            return Err(trace_error(format!(
                "cleanup exit {} starts at block {}, not block {}",
                id.0, exit.from.0, block.0
            )));
        }
        Ok(exit)
    }

    fn exit_capacity(
        &self,
        block: BlockId,
        id: ExitTargetId,
        capacities: &BTreeMap<BlockId, u32>,
    ) -> Result<u32, Diagnostic> {
        let exit = self.exit(block, id)?;
        let mut capacity = 0;
        for action in &exit.finalize_in_order {
            let weight = self
                .lifecycle_weights
                .get(&action.lifecycle_id)
                .copied()
                .ok_or_else(|| {
                    trace_error(format!(
                        "cleanup exit {} references unknown lifecycle `{}`",
                        id.0, action.lifecycle_id
                    ))
                })?;
            capacity = checked_add(capacity, weight, "finalizer event capacity")?;
        }

        let continuation = match &exit.continuation {
            ExitContinuation::Continue(edge) => {
                let target = self.edge_target(block, *edge)?;
                capacity_at(capacities, target)?
            }
            ExitContinuation::CommitResult { .. } => RESULT_COMMIT_EVENTS,
            ExitContinuation::ReturnFailure { .. } | ExitContinuation::ReturnUnit => 0,
        };
        checked_add(capacity, continuation, "exit event capacity")
    }
}

fn capacity_at(capacities: &BTreeMap<BlockId, u32>, id: BlockId) -> Result<u32, Diagnostic> {
    capacities.get(&id).copied().ok_or_else(|| {
        trace_error(format!(
            "native trace postorder omitted reachable block {}",
            id.0
        ))
    })
}

fn validate_references(
    plan: &CleanupPlan,
    blocks: &BTreeMap<BlockId, &CleanupBlock>,
    edges: &BTreeMap<EdgeId, &CleanupEdge>,
    exits: &BTreeMap<ExitTargetId, &ExitTarget>,
) -> Result<(), Diagnostic> {
    if !blocks.contains_key(&plan.entry) {
        return Err(trace_error(format!(
            "cleanup entry references missing block {}",
            plan.entry.0
        )));
    }
    let regions = plan
        .regions
        .iter()
        .map(|region| region.id)
        .collect::<BTreeSet<_>>();
    if regions.len() != plan.regions.len() {
        return Err(trace_error("cleanup plan has a duplicate region identity"));
    }
    for region in &plan.regions {
        if let Some(parent) = region.parent {
            if !regions.contains(&parent) {
                return Err(trace_error(format!(
                    "cleanup region {} references missing parent region {}",
                    region.id.0, parent.0
                )));
            }
        }
        if !exits.contains_key(&region.normal_scope_end) {
            return Err(trace_error(format!(
                "cleanup region {} references missing normal-scope exit {}",
                region.id.0, region.normal_scope_end.0
            )));
        }
    }
    for block in blocks.values() {
        if !regions.contains(&block.region) {
            return Err(trace_error(format!(
                "cleanup block {} references missing region {}",
                block.id.0, block.region.0
            )));
        }
    }
    for edge in edges.values() {
        if !blocks.contains_key(&edge.from) || !blocks.contains_key(&edge.to) {
            return Err(trace_error(format!(
                "cleanup edge {} references a missing endpoint",
                edge.id.0
            )));
        }
    }
    for exit in exits.values() {
        if !blocks.contains_key(&exit.from) {
            return Err(trace_error(format!(
                "cleanup exit {} references missing source block {}",
                exit.id.0, exit.from.0
            )));
        }
        for region in &exit.leaves_regions {
            if !regions.contains(region) {
                return Err(trace_error(format!(
                    "cleanup exit {} references missing region {}",
                    exit.id.0, region.0
                )));
            }
        }
    }
    Ok(())
}

fn lifecycle_weights(
    program: &ResolvedProgram,
) -> Result<BTreeMap<DeclarationId, u32>, Diagnostic> {
    let known_imports = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .map(|import| import.id.clone())
        .collect::<BTreeSet<_>>();
    let mut weights = BTreeMap::new();
    for declaration in &program.types {
        let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
            continue;
        };
        let weight = match &drop.kind {
            ResolvedResourceDropKind::Trivial => TRIVIAL_FINALIZER_EVENTS,
            ResolvedResourceDropKind::Imported { import, .. } => {
                if !known_imports.contains(import) {
                    return Err(trace_error(format!(
                        "resource lifecycle `{}` references unknown import `{import}`",
                        drop.id
                    )));
                }
                IMPORTED_FINALIZER_EVENTS
            }
        };
        if weights.insert(drop.id.clone(), weight).is_some() {
            return Err(trace_error(format!(
                "duplicate resource lifecycle identity `{}`",
                drop.id
            )));
        }
    }
    Ok(weights)
}

fn unique_index<'a, T, Id: Copy + Ord>(
    values: &'a [T],
    id: impl Fn(&T) -> Id,
    kind: &str,
) -> Result<BTreeMap<Id, &'a T>, Diagnostic> {
    let mut index = BTreeMap::new();
    for value in values {
        if index.insert(id(value), value).is_some() {
            return Err(trace_error(format!(
                "cleanup plan has a duplicate {kind} identity"
            )));
        }
    }
    Ok(index)
}

fn checked_add(left: u32, right: u32, context: &str) -> Result<u32, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| trace_error(format!("{context} exceeds u32")))
}

fn checked_mul(left: u32, right: u32, context: &str) -> Result<u32, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| trace_error(format!("{context} exceeds u32")))
}

fn trace_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B103", message)
}

#[cfg(test)]
#[path = "native_trace/tests.rs"]
mod tests;
