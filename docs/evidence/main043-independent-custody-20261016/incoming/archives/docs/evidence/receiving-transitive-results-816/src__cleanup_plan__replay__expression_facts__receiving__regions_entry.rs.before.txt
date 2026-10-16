//! Paid region/entry receiving directories; unchanged full membership/place checks.
use super::*;

pub(in crate::cleanup_plan::replay) fn validate_regions(
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return crate::cleanup_plan::replay::validate_regions(function, storage);
    }
    let plan = &function.cleanup_plan;
    if plan.regions.is_empty() {
        return Err(replay_error(function, "cleanup plan has no root region"));
    }
    let mut assigned = Directory::new(function, budget, "cleanup region assignment directory")?;
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
            if !assigned.insert(function, item, budget)? {
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
    // Every inserted key was checked against this immutable storage set, and
    // repeated keys already returned the original duplicate error. Therefore
    // assigned is a subset with unique full keys; equal cardinality is exact
    // set equality, not an identity/display-name shortcut.
    if assigned.len() != storage.len() {
        return Err(replay_error(
            function,
            "cleanup plan has storage not assigned to exactly one region",
        ));
    }
    Ok(())
}

pub(in crate::cleanup_plan::replay) fn validate_entry(
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return crate::cleanup_plan::replay::validate_entry(function, storage, leaves);
    }
    let plan = &function.cleanup_plan;
    if usize::try_from(plan.entry.0)
        .ok()
        .is_none_or(|entry| entry >= plan.blocks.len())
    {
        return Err(replay_error(function, "cleanup entry block is unknown"));
    }
    let mut flags = Directory::new(function, budget, "cleanup entry flag directory")?;
    for place in &plan.entry_state.live_owned_parameters {
        let under = validate_place(function, place, storage, leaves)?;
        for flag in &under {
            if !flags.insert(function, *flag, budget)? {
                return Err(replay_error(
                    function,
                    "cleanup entry places overlap or repeat liveness flags",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
