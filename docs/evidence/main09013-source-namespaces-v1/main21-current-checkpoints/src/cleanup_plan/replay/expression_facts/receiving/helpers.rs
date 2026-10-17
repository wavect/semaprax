//! Complete paid transitive receiving results; original root helpers stay compiled.
use super::*;
mod matching;
mod regions;
pub(super) use matching::{exit_regions_match, EXIT_SELECTOR_BYTES};
pub(super) use regions::expected_exit_regions;

pub(super) fn validate_place(
    function: &ResolvedFunction,
    place: &CleanupPlace,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<Vec<LivenessFlagId>, Diagnostic> {
    if !budget.pays_allocations() {
        return crate::cleanup_plan::replay::validate_place(function, place, storage, leaves);
    }
    budget.reserve_bytes(function, 0, "cleanup place receiving")?;
    budget.charge(function, 0, "cleanup place receiving")?;
    if !storage.contains(&place.storage) {
        return Err(replay_error(
            function,
            "cleanup place references unknown storage",
        ));
    }
    // Nested helper return and this helper's initialized/receiving result overlap.
    budget.reserve_bytes(
        function,
        2 * std::mem::size_of::<&mut ReplayBudget>()
            + std::mem::size_of::<Result<Vec<LivenessFlagId>, Diagnostic>>()
            + std::mem::size_of::<Vec<LivenessFlagId>>(),
        "cleanup place receiving",
    )?;
    budget.charge(function, 4, "cleanup place receiving")?;
    let under = leaves.under_with_budget(function, place, budget)?;
    if under.is_empty() {
        return Err(replay_error(
            function,
            "cleanup place has no droppable leaf",
        ));
    }
    Ok(under)
}

#[cfg(test)]
mod before_remaining;
#[cfg(test)]
mod tests;
