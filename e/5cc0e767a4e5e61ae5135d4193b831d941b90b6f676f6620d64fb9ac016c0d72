//! Transfer paths that the original lexical merger always keeps separate.
//! No path, observation, key or continuation is inspected or reconstructed.
use super::*;
use std::mem::size_of;

// Complete new receiving results, vector owners, borrowed selection arguments,
// acquisition result and fixed controls. Removed owners supply no credit.
pub(super) const RECEIVING_BYTES: usize =
    2 * size_of::<Result<Vec<ExprSkeletonPath>, Diagnostic>>()
        + 2 * size_of::<Vec<ExprSkeletonPath>>()
        + size_of::<Result<(), Diagnostic>>()
        + 2 * size_of::<(&SkeletonWork<'static, 'static>, &Vec<ExprSkeletonPath>)>()
        + 3 * size_of::<usize>()
        + 3 * size_of::<bool>()
        + 3 * size_of::<Option<usize>>()
        + size_of::<(&Vec<ExprSkeletonPath>, usize)>();

pub(super) fn can_reuse(work: &SkeletonWork<'_, '_>, paths: &Vec<ExprSkeletonPath>) -> bool {
    #[cfg(test)]
    if work.budget.oracle_lease().is_some() {
        return false;
    }
    work.budget.pays_builder()
        && !work.budget.selected()
        && work.budget.merge_paths
        && work.function.cleanup_plan.lexical_control.is_some()
        && work.active_copy_region.is_none()
        && paths.len() >= 2
        && paths.len() <= MAX_REPLAY_PATHS
        // Keep bounded spare backing, rather than retaining a shrunken large Vec.
        && paths.len().checked_next_power_of_two()
            .is_some_and(|bound| paths.capacity() <= bound)
        // Every isolated path occupies one group and one output slot. For
        // n >= 2 the original exact-growth group Vec also paid its first
        // one-slot backing before its final backing. No such debit is refunded.
        && paths.len().checked_add(1)
            .and_then(|groups| groups.checked_mul(size_of::<(ExprSkeletonPath, Vec<Observations>)>()))
            .and_then(|groups| paths.len().checked_mul(size_of::<ExprSkeletonPath>())
                .and_then(|output| groups.checked_add(output)))
            .is_some_and(|removed| removed > RECEIVING_BYTES)
}

#[cfg(test)]
mod tests;
