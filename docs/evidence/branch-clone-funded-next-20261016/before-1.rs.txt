//! A known next push can acquire its existing growth capacity at clone time.
//! This removes the intermediate clone backing; future capacity stays the same.
use super::*;

#[track_caller]
pub(super) fn for_push<T: Clone + ClonePayload>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    values: &Vec<T>,
    phase: &str,
) -> Result<Vec<T>, Diagnostic> {
    #[cfg(test)]
    if tests::original_selected() {
        return skeleton_clone(budget, function, values, phase);
    }
    if !budget.pays_allocations() {
        return skeleton_clone(budget, function, values, phase);
    }
    budget.charge_skeleton(function, 1, phase)?;
    // Keep the complete old clone census, including all nested owned backing
    // and its independently funded oracle work. Nothing is repriced or waived.
    budget.reserve_clone(function, values, phase)?;
    let capacity = values
        .len()
        .checked_add(1)
        .and_then(|required| physical_storage::replay_growth_capacity(values.len(), required))
        .ok_or_else(|| {
            budget.budget_refusal(function, "cleanup replay branch capacity overflowed", "")
        })?;
    budget.reserve_array::<T>(function, capacity - values.len(), phase)?;
    note_skeleton_materialization();
    let mut result = Vec::with_capacity(capacity);
    result.extend(values.iter().cloned());
    Ok(result)
}

#[cfg(test)]
mod tests;
