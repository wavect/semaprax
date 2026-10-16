//! Transfer an already paid prefix only when its single successor owns it.
//! Probe existing headers only; all other lanes retain the complete clone.
use super::path_array::main696 as path_array;
use super::{
    note_skeleton_materialization, Diagnostic, Observations, ReplayBudget, ResolvedFunction,
};
use main_current::sequence_skeleton_paths;

fn receiving_bytes() -> usize {
    use std::mem::size_of;
    2 * size_of::<Result<Option<Observations>, Diagnostic>>()
        + size_of::<&ResolvedFunction>()
        + size_of::<&mut ReplayBudget>()
        + size_of::<&mut Observations>()
        + size_of::<&Observations>()
        + size_of::<&str>()
        + size_of::<Observations>()
        + 3 * size_of::<usize>()
        + size_of::<bool>()
}
fn minimum_items() -> usize {
    receiving_bytes().div_ceil(std::mem::size_of::<std::rc::Rc<SkeletonObservation>>())
}

pub(super) fn try_take(
    prefix: &mut Observations,
    suffix: &Observations,
    suffix_count: usize,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<Option<Observations>, Diagnostic> {
    #[cfg(test)]
    if tests::original_selected() {
        return Ok(None);
    }
    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        return Ok(None);
    }
    // The old clone alone costs at least the complete new receiving layout.
    // Reusing spare storage cannot introduce a larger growth acquisition. A
    // DAG suffix freezes the prefix unchanged instead of extending its tail.
    // These are allocation-free shape probes, not a new fact/cache authority.
    if !budget.pays_allocations()
        || budget.selected()
        || suffix_count != 1
        || prefix.items.len() < minimum_items()
        || (suffix.prefix.is_none()
            && suffix.items.len() > prefix.items.capacity() - prefix.items.len())
    {
        return Ok(None);
    }
    // Same logical clone operation and materialization as the old receiver.
    // Its actual vector clone and clone cursor are absent: every observation
    // and the immutable prefix transfer intact. Existing backing stays paid.
    budget.charge_skeleton(function, 1, phase)?;
    budget.reserve_array::<u8>(function, receiving_bytes(), phase)?;
    note_skeleton_materialization();
    Ok(Some(std::mem::take(prefix)))
}

#[cfg(test)]
mod before;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod main_current;
#[cfg(test)]
mod shipping_tests;
