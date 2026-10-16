//! A pure borrowed shape probe; the existing receiver owns fallible transfer.
use super::*;

pub(super) struct Candidate<'a> {
    pub(super) prefix: &'a Observations,
    pub(super) suffix: &'a Observations,
    pub(super) suffix_count: usize,
    pub(super) budget: &'a ReplayBudget,
}
// Both complete incoming/callee headers and both boolean result slots. The
// transfer stays in the existing sequencing receiver, whose observation/result
// carriers already existed in the original clone. No new fallible sum crosses
// this probe. The old vector clone alone must cover this complete new layout.
pub(super) const RECEIVING_BYTES: usize =
    2 * std::mem::size_of::<Candidate<'static>>() + 2 * std::mem::size_of::<bool>();
const ITEM_BYTES: usize = std::mem::size_of::<std::rc::Rc<SkeletonObservation>>();
const MINIMUM_ITEMS: usize = RECEIVING_BYTES.div_ceil(ITEM_BYTES);

pub(super) fn can_take(input: Candidate<'_>) -> bool {
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
        && input.suffix_count == 1
        && input.prefix.items.len() >= MINIMUM_ITEMS
        && (input.suffix.prefix.is_some()
            || input.suffix.items.len() <= input.prefix.items.capacity() - input.prefix.items.len())
}

#[cfg(test)]
mod before;
#[cfg(test)]
mod before_wrapper;
#[cfg(test)]
mod tests;
