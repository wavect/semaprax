//! A header-proved lower bound for the unchanged sequencing loop.
use super::*;
use std::mem::size_of;

struct Candidate<'a> {
    prefixes: &'a [ExprSkeletonPath],
    suffixes: &'a [ExprSkeletonPath],
    budget: &'a ReplayBudget,
}

// Both complete incoming/probe and fallible return carriers, the receiving
// borrows, checked arithmetic slots and first-path probe. The skipped vector
// backing alone must cover this overlap; the old minimum fee is not credit.
fn receiving_bytes() -> usize {
    3 * size_of::<Result<(), Diagnostic>>()
        + 2 * size_of::<Candidate<'static>>()
        + 2 * size_of::<(
            &mut Vec<ExprSkeletonPath>,
            &[ExprSkeletonPath],
            &[ExprSkeletonPath],
            &mut SkeletonWork<'_, '_>,
            &str,
        )>()
        + 2 * size_of::<Option<usize>>()
        + size_of::<(
            [usize; 4],
            [Option<usize>; 6],
            Option<&ExprSkeletonPath>,
            [bool; 4],
        )>()
}

fn lower_bound(input: Candidate<'_>) -> Option<usize> {
    #[cfg(test)]
    if tests::original_selected() || super::tests::original_selected() {
        return None;
    }
    #[cfg(test)]
    if input.budget.oracle_lease().is_some() {
        return None;
    }
    if !input.budget.pays_allocations()
        || input.budget.selected()
        || input.prefixes.len() < 8
        || input.suffixes.len() < 2
    {
        return None;
    }
    let first = input.prefixes.first()?;
    if first.failed || first.residual {
        return None;
    }
    // Every prefix produces at least one path. This first active prefix
    // produces all S suffixes, proving P + S - 1 without scanning a tail.
    let count = input
        .prefixes
        .len()
        .checked_add(input.suffixes.len().checked_sub(1)?)?;
    if count > MAX_REPLAY_PATHS {
        return None;
    }
    let previous = input.prefixes.len().checked_next_power_of_two()?;
    let capacity = count.checked_next_power_of_two()?;
    let removed = capacity.checked_sub(previous)?;
    (removed.checked_mul(size_of::<ExprSkeletonPath>())? >= receiving_bytes()).then_some(count)
}

pub(in crate::cleanup_plan::replay) fn minimum(
    paths: &mut Vec<ExprSkeletonPath>,
    prefixes: &[ExprSkeletonPath],
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
    phase: &str,
) -> Result<(), Diagnostic> {
    let count = if paths.capacity() == 0 {
        lower_bound(Candidate {
            prefixes,
            suffixes,
            budget: work.budget,
        })
    } else {
        None
    };
    match count {
        Some(count) => {
            work.budget.reserve_array::<u8>(
                work.function,
                receiving_bytes(),
                "sequenced minimum receiving frame",
            )?;
            super::minimum(paths, count, work, phase)
        }
        None => super::minimum(paths, prefixes.len(), work, phase),
    }
}

#[cfg(test)]
mod before;
#[cfg(test)]
mod tests;
