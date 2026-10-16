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

// Finished paths have a distinct element layout. Keep the existing expression
// constructor unchanged, fund this complete typed receiver, and require its
// smallest eligible saved backing to cover the new receiving layout.
fn completed_receiving_bytes() -> usize {
    use std::mem::size_of;
    2 * size_of::<Result<(), Diagnostic>>()
        + size_of::<&mut SkeletonWork<'_, '_>>()
        + size_of::<&mut Vec<SkeletonPath>>()
        + size_of::<&ResolvedFunction>()
        + size_of::<&mut ReplayBudget>()
        + size_of::<&str>()
        + 5 * size_of::<usize>()
        + 2 * size_of::<Option<usize>>()
        + 6 * size_of::<bool>()
}

#[track_caller]
pub(super) fn completed_minimum(
    paths: &mut Vec<SkeletonPath>,
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
    if count < 8
        || paths.capacity() != 0
        || !work.budget.pays_allocations()
        || count.checked_next_power_of_two().is_some_and(|capacity| {
            capacity - 1 < completed_receiving_bytes().div_ceil(std::mem::size_of::<SkeletonPath>())
        })
    {
        return Ok(());
    }
    work.budget
        .reserve_array::<u8>(work.function, completed_receiving_bytes(), phase)?;
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
#[path = "before.rs"]
mod before;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "completed_tests.rs"]
mod completed_tests;
#[cfg(test)]
#[path = "final_before.rs"]
mod final_before;

#[path="current.rs"]
mod current;
use current::{append_expr_paths,sequence_skeleton_paths,split_status_paths,split_contract};
