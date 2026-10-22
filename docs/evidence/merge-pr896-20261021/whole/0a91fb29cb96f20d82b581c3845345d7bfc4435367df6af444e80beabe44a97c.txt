//! Same-ledger per-call receiving for the closed reference-coverage family.
use super::{
    expression_facts, CleanupTerminator, CleanupTransition, EdgeCondition, ExitContinuation,
    ReplayBudget, ResolvedFunction, StatusSourceId,
};
use crate::cleanup_plan::renewal_profile::child_refusal::{ReferenceCoverage, Refusal};
use crate::cleanup_plan::replay::selected_context::child::{ArrayReceiving, NodeReceiving, Permit};
use std::collections::BTreeSet;
type Error = expression_facts::errors::EngineError;

pub(in super::super) fn validate(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    let permit = Permit::profile::<()>(function, budget)?;
    // These macros introduce no function/Result forwarding receiver. Every
    // actual wrapper invocation first pays its exact additional receiving slots.
    // The reservation/charge primitives are the existing paid-budget boundary;
    // only their genuine prepaid failure Diagnostic enters Owned here.
    macro_rules! receive_array {
        ($count:expr, $phase:literal) => {{
            match budget.reserve_array::<ArrayReceiving<'_>>(
                function,
                1,
                "selected reference array receiving",
            ) {
                Ok(()) => {}
                Err(error) => return Err(Error::Owned(error)),
            }
            match budget.charge(function, 9, "selected reference array receiving") {
                Ok(()) => {}
                Err(error) => return Err(Error::Owned(error)),
            }
            match budget.child_reserve_array::<usize>(function, $count, $phase) {
                Ok(()) => {}
                Err(error) => return permit.resolve(Err(error)),
            }
        }};
    }
    macro_rules! insert_status {
        ($set:ident, $source:expr) => {{
            // Same original membership test precedes each conditional node.
            // A duplicate creates no wrapper, receiving bank or node.
            if !$set.contains($source) {
                match budget.reserve_array::<NodeReceiving<'_>>(
                    function,
                    1,
                    "selected reference node receiving",
                ) {
                    Ok(()) => {}
                    Err(error) => return Err(Error::Owned(error)),
                }
                match budget.charge(function, 8, "selected reference node receiving") {
                    Ok(()) => {}
                    Err(error) => return Err(Error::Owned(error)),
                }
                match budget.child_reserve_btree_node::<&StatusSourceId, ()>(
                    function,
                    "status reference set",
                ) {
                    Ok(()) => {}
                    Err(error) => return permit.resolve(Err(error)),
                }
                $set.insert($source);
            }
        }};
    }
    let plan = &function.cleanup_plan;
    receive_array!(plan.edges.len(), "edge reference vector");
    let mut edge_references = vec![0_usize; plan.edges.len()];
    receive_array!(plan.exits.len(), "exit reference vector");
    let mut exit_references = vec![0_usize; plan.exits.len()];
    let mut status_references = BTreeSet::new();
    for block in &plan.blocks {
        for transition in &block.transitions {
            if let CleanupTransition::SelectFailure { source } = transition {
                insert_status!(status_references, source);
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => edge_references[edge.0 as usize] += 1,
            CleanupTerminator::Branch(edges) => {
                for edge in edges {
                    edge_references[edge.0 as usize] += 1;
                }
            }
            CleanupTerminator::Exit(exit) => exit_references[exit.0 as usize] += 1,
        }
    }
    for edge in &plan.edges {
        match &edge.condition {
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                insert_status!(status_references, source);
            }
            EdgeCondition::Always
            | EdgeCondition::BooleanResult(_, _)
            | EdgeCondition::VariantCase { .. }
            | EdgeCondition::ArmSelected { .. } => {}
        }
    }
    for exit in &plan.exits {
        match &exit.continuation {
            ExitContinuation::Continue(edge)
            | ExitContinuation::LoopBreak { edge, .. }
            | ExitContinuation::FunctionReturn { edge, .. } => {
                edge_references[edge.0 as usize] += 1
            }
            ExitContinuation::ReturnFailure { source } => {
                insert_status!(status_references, source);
            }
            ExitContinuation::CommitResult { .. }
            | ExitContinuation::ReturnUnit
            | ExitContinuation::LoopLatch { .. } => {}
        }
    }
    if edge_references.iter().any(|count| *count != 1) {
        return permit.resolve(Err(Refusal::ReferenceCoverage(
            ReferenceCoverage::EdgeOwners,
        )
        .into()));
    }
    if exit_references.iter().any(|count| *count != 1) {
        return permit.resolve(Err(Refusal::ReferenceCoverage(
            ReferenceCoverage::ExitOwners,
        )
        .into()));
    }
    let mut declared_statuses = BTreeSet::new();
    for source in &plan.status_sources {
        insert_status!(declared_statuses, &source.id);
    }
    if status_references != declared_statuses {
        return permit.resolve(Err(Refusal::ReferenceCoverage(
            ReferenceCoverage::StatusDeclarations,
        )
        .into()));
    }
    Ok(())
}
