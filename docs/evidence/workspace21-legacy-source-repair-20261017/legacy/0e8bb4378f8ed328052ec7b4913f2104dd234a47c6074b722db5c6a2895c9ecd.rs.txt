//! Reuse the existing ownership-state set for an identity partition.
//! The complete old key clone and every downstream join/replay check remain.
use super::*;
use std::collections::btree_map::IntoValues;
use std::mem::size_of;

type States = BTreeSet<Box<PathState>>;
pub(super) enum Groups {
    One(Option<States>),
    Many(IntoValues<ControlKey, States>),
}
impl Iterator for Groups {
    type Item = States;
    fn next(&mut self) -> Option<States> {
        match self {
            Self::One(value) => value.take(),
            Self::Many(values) => values.next(),
        }
    }
}

// Full new producer/caller iterator owners and outward/receiving Results;
// helper input, One payload, old-map transfer and Many iterator temporaries;
// call/receiving references and the singleton-selection/borrow controls.
// No old owner/header is credited against any of these complete layouts.
const RECEIVER_BYTES: usize = 2 * size_of::<Groups>()
    + 2 * size_of::<Result<Groups, Diagnostic>>()
    + size_of::<States>()
    + size_of::<Option<States>>()
    + 2 * size_of::<BTreeMap<ControlKey, States>>()
    + size_of::<IntoValues<ControlKey, States>>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<Option<&'static PathState>>()
    + size_of::<&PathState>()
    + 2 * size_of::<bool>();

pub(super) fn partition(
    function: &ResolvedFunction,
    states: States,
    budget: &mut ReplayBudget,
) -> Result<Groups, Diagnostic> {
    if budget.pays_allocations() {
        budget.reserve_bytes(function, RECEIVER_BYTES, "ownership group receiving owners")?;
    }
    let singleton = states.len() == 1 && budget.pays_allocations();
    #[cfg(test)]
    let singleton = singleton && !tests::original_selected();
    if singleton {
        if let Some(state) = states.first().map(Box::as_ref) {
            let key = if storage::atomic_controls::selected(state, budget) {
                let payload = storage::atomic_controls::reserve(function, state, budget)?;
                storage::atomic_controls::copy(payload)
            } else {
                storage::reserve_control_clone(function, state, budget)?;
                (
                    state.pending_failure.clone(),
                    state.selected_failure.clone(),
                    state.staged_copy_result.clone(),
                    state.published,
                    state.returned.clone(),
                )
            };
            // IntoValues drops the old key before yielding this sole group.
            // Keep its full construction, work and payload charges here too.
            drop(key);
            budget.reserve_bytes(function, 0, "incoming ownership state authority")?;
        }
        return Ok(Groups::One(Some(states)));
    }
    let grouped = {
        let mut groups = BTreeMap::<ControlKey, BTreeSet<Box<PathState>>>::new();
        for state in states {
            let key = if storage::atomic_controls::selected(&state, budget) {
                let payload = storage::atomic_controls::reserve(function, &state, budget)?;
                storage::atomic_controls::copy(payload)
            } else {
                storage::reserve_control_clone(function, &state, budget)?;
                (
                    state.pending_failure.clone(),
                    state.selected_failure.clone(),
                    state.staged_copy_result.clone(),
                    state.published,
                    state.returned.clone(),
                )
            };
            if !groups.contains_key(&key) {
                storage::reserve_btree_node::<ControlKey, BTreeSet<Box<PathState>>>(
                    function,
                    budget,
                    "ownership control groups",
                )?;
            }
            let group = groups.entry(key).or_default();
            boxed_states::insert_held(group, state, &mut |bytes, phase| {
                budget.reserve_bytes(function, bytes, phase)
            })?;
        }
        groups
    };
    Ok(Groups::Many(grouped.into_values()))
}

#[cfg(test)]
pub(super) mod source_custody;
#[cfg(test)]
mod tests;
