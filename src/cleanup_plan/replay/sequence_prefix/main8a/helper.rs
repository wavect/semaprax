//! A pure borrowed shape probe; the existing receiver owns fallible transfer.
use super::*;

pub(in crate::cleanup_plan::replay) struct Candidate<'a> {
    pub(in crate::cleanup_plan::replay) prefix: &'a Observations,
    pub(in crate::cleanup_plan::replay) suffix: &'a Observations,
    /// Successors still pending after the currently borrowed successor.
    pub(in crate::cleanup_plan::replay) remaining_suffixes: usize,
    pub(in crate::cleanup_plan::replay) budget: &'a ReplayBudget,
}
// Both complete incoming/callee headers and both boolean result slots. The
// transfer stays in the existing sequencing receiver, whose observation/result
// carriers already existed in the original clone. No new fallible sum crosses
// this probe. The old vector clone alone must cover this complete new layout.
pub(in crate::cleanup_plan::replay) const RECEIVING_BYTES: usize =
    2 * std::mem::size_of::<Candidate<'static>>() + 2 * std::mem::size_of::<bool>();
const ITEM_BYTES: usize = std::mem::size_of::<std::rc::Rc<SkeletonObservation>>();
const MINIMUM_ITEMS: usize = RECEIVING_BYTES.div_ceil(ITEM_BYTES);

pub(in crate::cleanup_plan::replay) fn can_take(input: Candidate<'_>) -> bool {
    #[cfg(test)]
    if tests::original_selected() {
        return false;
    }
    #[cfg(test)]
    if input.budget.oracle_lease().is_some() {
        return false;
    }
    input.budget.pays_allocations()
        && !input.budget.selected()
        && input.remaining_suffixes == 0
        && input.prefix.items.len() >= MINIMUM_ITEMS
        && (input.suffix.prefix.is_some()
            || input.prefix.items.capacity() == input.prefix.items.len()
            || input.suffix.items.len() <= input.prefix.items.capacity() - input.prefix.items.len())
}

#[cfg(test)]
#[path = "before.rs"]
mod before;
#[cfg(test)]
#[path = "wrapper_context.rs"]
mod before_wrapper;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
