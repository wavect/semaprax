//! Fused history clone and its known immediate push, with paid new controls.
use super::*;
use std::mem::size_of;

// The producer is physically in the enumerator. Its outer Vec Result and
// owned observation argument no longer exist; all nested Result controls stay.
// One budget/function reference pair pays the caller's reservation boundary.
// The payload-reference term remains for the direct clone census argument.
pub(super) fn inline_controls<T: Clone>() -> usize {
    size_of::<Result<(), Diagnostic>>() // caller payment result
        + 4 * size_of::<Result<(), Diagnostic>>() // clone/push work, clone census, conversion
        + 2 * size_of::<Result<(), Diagnostic>>() // backing wrapper and nested byte payment
        + size_of::<Result<usize, Diagnostic>>() // checked array size conversion
        + size_of::<Option<usize>>()
        + 2 * size_of::<Vec<T>>() // initializer and the enumerator's named branch
        + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>()
        + size_of::<&Vec<T>>() // direct clone census argument
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<&mut Vec<T>>() + size_of::<&[T]>()
        + 2 * size_of::<std::slice::Iter<'_, T>>()
        + 2 * size_of::<std::iter::Cloned<std::slice::Iter<'_, T>>>()
}

// Paid by the enumerator before evaluating the new helper's arguments. The
// original clone/push controls are not credited against these actual new sites.
#[cfg(test)]
pub(super) fn controls<T: Clone, V>() -> usize {
    size_of::<Result<Vec<T>, Diagnostic>>() // fused receiving result
        + size_of::<Result<(), Diagnostic>>() // caller payment result
        + 4 * size_of::<Result<(), Diagnostic>>() // clone/push work, clone census, conversion
        + 2 * size_of::<Result<(), Diagnostic>>() // backing wrapper and nested byte payment
        + size_of::<Result<usize, Diagnostic>>() // reserve_array checked byte-size conversion
        + size_of::<Option<usize>>() // reserve_array checked multiplication
        + 2 * size_of::<Vec<T>>() // initializer and named local receiver
        + 2 * size_of::<&mut ReplayBudget>() + 2 * size_of::<&ResolvedFunction>()
        + size_of::<&Vec<T>>() + size_of::<V>() // new argument receivers
        + 2 * size_of::<&str>() // fixed phase receivers
        + 2 * size_of::<usize>() // controls() receiving value and capacity local
        + 2 * size_of::<bool>() // empty and arithmetic overflow branch receivers
        + 2 * size_of::<&mut Vec<T>>() // extend and push method receivers
        + size_of::<&[T]>() // iter slice receiver
        + 2 * size_of::<std::slice::Iter<'_, T>>() // iterator initializer and cloned receiver
        + 2 * size_of::<std::iter::Cloned<std::slice::Iter<'_, T>>>() // initializer and extend receiver
}

#[cfg(test)]
#[track_caller]
pub(super) fn with_observation<T: Clone + ClonePayload, V: physical_clone::IntoReplay<T>>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    values: &Vec<T>,
    observation: V,
) -> Result<Vec<T>, Diagnostic> {
    if !budget.pays_allocations() {
        let mut branch = skeleton_clone(
            budget,
            function,
            values,
            "cleanup-plan skeleton branch clone",
        )?;
        skeleton_push(
            budget,
            function,
            &mut branch,
            observation,
            "plan branch observation push",
        )?;
        return Ok(branch);
    }
    budget.charge_skeleton(function, 1, "cleanup-plan skeleton branch clone")?;
    budget.reserve_clone(function, values, "cleanup-plan skeleton branch clone")?;
    note_skeleton_materialization();
    // The original push work refusal must win before any extra capacity debit.
    budget.charge_skeleton(function, 1, "plan branch observation push")?;
    note_skeleton_materialization();
    let capacity = if values.is_empty() {
        1
    } else {
        if values.len() > usize::MAX / 2 {
            return Err(budget.budget_refusal(
                function,
                "cleanup replay branch capacity overflowed",
                "",
            ));
        }
        values.len() * 2
    };
    budget.reserve_array::<T>(
        function,
        capacity - values.len(),
        "plan branch observation push",
    )?;
    budget.reserve_bytes(
        function,
        physical_clone::conversion_bytes::<T, _>(&observation),
        "plan branch observation push",
    )?;
    let mut result = Vec::with_capacity(capacity);
    result.extend(values.iter().cloned());
    result.push(observation.into());
    Ok(result)
}

#[cfg(test)]
mod before;
#[cfg(test)]
use before::{for_push, main_replay_input, plan_skeleton_paths};
#[cfg(test)]
mod funded_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod inline_tests;

#[cfg(test)]
mod before_inline;
