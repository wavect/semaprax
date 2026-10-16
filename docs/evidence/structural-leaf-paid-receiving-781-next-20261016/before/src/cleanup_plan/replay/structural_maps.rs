//! Selected constructor accounting for primitive structural replay indexes.
//! Independently rebuilt inventory remains the authority; attached shapes are
//! validated here; selected direct records retain exact field projections.
use super::*;

#[cfg(test)]
pub(super) fn validate_slots(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeSet<StorageId>, Diagnostic> {
    validate_slots_core::<Diagnostic>(function, budget)
}

pub(super) fn validate_slots_core<E: semantic_refusal::Error>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeSet<StorageId>, E> {
    let mut storage = BTreeSet::new();
    for (index, slot) in function.cleanup_plan.slots.iter().enumerate() {
        let expected = u32_index(function, index, "cleanup slot")?;
        if slot.id.0 != expected || slot.storage_index != expected {
            return Err(E::structural(
                function,
                semantic_refusal::Atom::NoncontiguousSlot(index),
            ));
        }
        if !storage.contains(&slot.storage) {
            budget.reserve_btree_node::<StorageId, ()>(function, "slot storage index")?;
            budget.reserve_clone(function, &slot.storage, "slot storage identity")?;
        }
        if !storage.insert(slot.storage.clone()) {
            return Err(E::structural(
                function,
                semantic_refusal::Atom::RepeatedStorage(slot.storage.clone()),
            ));
        }
    }
    Ok(storage)
}

pub(super) fn validate_regions(
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    if plan.regions.is_empty() {
        return Err(replay_error(function, "cleanup plan has no root region"));
    }
    let mut assigned = BTreeSet::new();
    for (index, region) in plan.regions.iter().enumerate() {
        let expected = u32_index(function, index, "cleanup region")?;
        if region.id.0 != expected {
            return Err(replay_error(
                function,
                "cleanup region IDs are not contiguous",
            ));
        }
        match (index, region.parent) {
            (0, None) => {}
            (0, Some(_)) | (_, None) => {
                return Err(replay_error(
                    function,
                    "cleanup region tree has an invalid root",
                ));
            }
            (_, Some(parent)) if usize::try_from(parent.0).ok().is_some_and(|p| p < index) => {}
            _ => {
                return Err(replay_error(
                    function,
                    "cleanup region parent must precede its child",
                ));
            }
        }
        for item in &region.slots {
            if !storage.contains(item) {
                return Err(replay_error(
                    function,
                    "cleanup region references unknown storage",
                ));
            }
            if !assigned.contains(item) {
                budget.reserve_btree_node::<StorageId, ()>(function, "region storage index")?;
                budget.reserve_clone(function, item, "region storage identity")?;
            }
            if !assigned.insert(item.clone()) {
                return Err(replay_error(
                    function,
                    "cleanup storage is assigned to more than one region",
                ));
            }
        }
        let unresolved =
            plan.lexical_control.is_some() && region.normal_scope_end == UNRESOLVED_NORMAL_END;
        if !unresolved
            && usize::try_from(region.normal_scope_end.0)
                .ok()
                .is_none_or(|exit| exit >= plan.exits.len())
        {
            return Err(replay_error(
                function,
                "cleanup region references an unknown normal-scope exit",
            ));
        }
    }
    if &assigned != storage {
        return Err(replay_error(
            function,
            "cleanup plan has storage not assigned to exactly one region",
        ));
    }
    Ok(())
}

pub(super) fn collect_leaves(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Leaves, Diagnostic> {
    let mut leaves = BTreeMap::new();
    let mut places = BTreeSet::new();
    for slot in &function.cleanup_plan.slots {
        flat_records::collect_slot_leaves(function, slot, &mut leaves, &mut places, budget)?;
    }
    for (expected, flag) in leaves.keys().enumerate() {
        if flag.0 != u32_index(function, expected, "liveness flag")? {
            return Err(replay_error(
                function,
                "cleanup liveness flags are not contiguous from zero",
            ));
        }
    }
    Leaves::new_with_budget(function, leaves, budget)
}

pub(super) fn validate_reference_coverage(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    budget.reserve_array::<usize>(function, plan.edges.len(), "edge reference vector")?;
    let mut edge_references = vec![0_usize; plan.edges.len()];
    budget.reserve_array::<usize>(function, plan.exits.len(), "exit reference vector")?;
    let mut exit_references = vec![0_usize; plan.exits.len()];
    let mut status_references = BTreeSet::new();

    for block in &plan.blocks {
        for transition in &block.transitions {
            if let CleanupTransition::SelectFailure { source } = transition {
                insert_status(function, &mut status_references, source, budget)?;
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
                insert_status(function, &mut status_references, source, budget)?;
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
                insert_status(function, &mut status_references, source, budget)?;
            }
            ExitContinuation::CommitResult { .. }
            | ExitContinuation::ReturnUnit
            | ExitContinuation::LoopLatch { .. } => {}
        }
    }
    if edge_references.iter().any(|count| *count != 1) {
        return Err(replay_error(
            function,
            "each cleanup edge must be referenced by exactly one control-flow owner",
        ));
    }
    if exit_references.iter().any(|count| *count != 1) {
        return Err(replay_error(
            function,
            "each cleanup exit must be referenced by exactly one block",
        ));
    }
    let mut declared_statuses = BTreeSet::new();
    for source in &plan.status_sources {
        insert_status(function, &mut declared_statuses, &source.id, budget)?;
    }
    if status_references != declared_statuses {
        return Err(replay_error(
            function,
            "declared and referenced cleanup status sources differ",
        ));
    }
    Ok(())
}

fn insert_status<'plan>(
    function: &ResolvedFunction,
    statuses: &mut BTreeSet<&'plan StatusSourceId>,
    source: &'plan StatusSourceId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !statuses.contains(source) {
        // Both sets are local to validation and all identities belong to the
        // immutable plan. Reference order still compares every identity field.
        budget.reserve_btree_node::<&StatusSourceId, ()>(function, "status reference set")?;
        statuses.insert(source);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

pub(super) mod paid_reference;
