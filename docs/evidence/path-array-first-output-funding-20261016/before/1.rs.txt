//! Skip intermediate empty-result backings when output has a known minimum.
//! The original push loop still produces and meters every path. Acquiring the
//! same power-of-two capacity early preserves every subsequent growth target.
use super::*;

fn receiving_bytes() -> usize {
    use std::mem::size_of;
    2 * size_of::<Result<(), Diagnostic>>()
        + size_of::<&mut SkeletonWork<'_, '_>>()
        + size_of::<&mut Vec<ExprSkeletonPath>>()
        + size_of::<&ResolvedFunction>()
        + size_of::<&mut ReplayBudget>()
        + size_of::<&str>()
        + 3 * size_of::<usize>()
        + size_of::<Option<usize>>()
        + 4 * size_of::<bool>()
}

#[track_caller]
pub(super) fn minimum(
    paths: &mut Vec<ExprSkeletonPath>,
    count: usize,
    work: &mut SkeletonWork<'_, '_>,
    phase: &str,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    if tests::original_selected() {
        return Ok(());
    }
    // Selection inspects existing headers only; it allocates and traverses
    // nothing. Small outputs retain their complete original physical route.
    if count < 8 || paths.capacity() != 0 || !work.budget.pays_allocations() {
        return Ok(());
    }
    work.budget
        .reserve_array::<u8>(work.function, receiving_bytes(), phase)?;
    let capacity = count.checked_next_power_of_two().ok_or_else(|| {
        work.budget.budget_refusal(
            work.function,
            "cleanup replay minimum path capacity overflowed",
            "",
        )
    })?;
    work.budget
        .reserve_vec(work.function, paths, capacity, phase)
}

#[cfg(test)]
mod before;
#[cfg(test)]
mod tests;
