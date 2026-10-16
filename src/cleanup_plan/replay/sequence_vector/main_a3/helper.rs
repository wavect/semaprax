//! Reuse a same-capacity owned input vector for exactly one successor per path.
//! The original fallback remains complete. This route clones/transfers the same
//! path fields and keeps the original work/materialization operations.
use super::*;
use std::mem::size_of;

pub(super) struct Candidate<'a> {
    pub(super) prefixes: &'a Vec<ExprSkeletonPath>,
    pub(super) suffixes: &'a [ExprSkeletonPath],
    pub(super) budget: &'a ReplayBudget,
}
// Complete caller/callee and fallible-operation carriers, incoming/output/
// replacement path headers, the vector/borrowed iterator and probe state.
// Existing work operations retain their own backing reservations. Pay this
// additional receiving overlap before moving the input owner. The removed
// vector alone must cover it; the old minimum's fee is not counted as credit.
pub(super) fn receiving_bytes() -> usize {
    2 * size_of::<Result<Vec<ExprSkeletonPath>, Diagnostic>>()
        + 6 * size_of::<Result<ExprSkeletonPath, Diagnostic>>()
        + size_of::<Vec<ExprSkeletonPath>>()
        + size_of::<std::slice::IterMut<'_, ExprSkeletonPath>>()
        + 3 * size_of::<ExprSkeletonPath>()
        + 2 * size_of::<Candidate<'static>>()
        + size_of::<(
            &mut SkeletonWork<'_, '_>,
            &ExprSkeletonPath,
            &mut ExprSkeletonPath,
        )>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<bool>()
}

pub(super) fn can_reuse(input: Candidate<'_>) -> bool {
    #[cfg(test)]
    if tests::original_selected() || path_array::original_route_selected() {
        return false;
    }
    #[cfg(test)]
    if input.budget.oracle_lease().is_some() {
        return false;
    }
    input.budget.pays_allocations()
        && !input.budget.selected()
        && input.suffixes.len() == 1
        && input.prefixes.len() >= 8
        && input.prefixes.len() <= MAX_REPLAY_PATHS
        && input.prefixes.len().checked_next_power_of_two() == Some(input.prefixes.capacity())
        && input
            .prefixes
            .capacity()
            .checked_mul(size_of::<ExprSkeletonPath>())
            .is_some_and(|bytes| bytes >= receiving_bytes())
}

pub(super) fn single(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
    share_prefix: bool,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut paths = prefixes;
    for slot in &mut paths {
        if slot.failed || slot.residual {
            work.charge(1, "short-circuited skeleton path")?;
            note_skeleton_materialization();
            work.budget.reserve_array::<ExprSkeletonPath>(
                work.function,
                0,
                "short-circuited skeleton path",
            )?;
            continue;
        }
        // The empty replacement owns no backing. The original prefix remains
        // held through all the same fallible operations as the old receiver.
        let mut prefix = std::mem::replace(slot, empty_expr_path());
        if share_prefix {
            work.share_prefix_tail(&mut prefix.observations)?;
        }
        let suffix = &suffixes[0];
        let observations = if sequence_prefix::can_take(sequence_prefix::Candidate {
            prefix: &prefix.observations,
            suffix: &suffix.observations,
            remaining_suffixes: 0,
            budget: work.budget,
        }) {
            // Retain the original clone's work and materialization, while
            // funding the pure probe before transferring any owned field.
            work.charge(1, "skeleton prefix clone")?;
            work.budget.reserve_array::<u8>(
                work.function,
                sequence_prefix::RECEIVING_BYTES,
                "skeleton prefix clone",
            )?;
            note_skeleton_materialization();
            std::mem::take(&mut prefix.observations)
        } else {
            work.clone_observations(&prefix.observations, "skeleton prefix clone")?
        };
        let observations = work.extend_owned_observations(
            observations,
            &suffix.observations,
            "skeleton suffix clone",
        )?;
        let owned_source = work.clone_owned(
            &suffix.owned_source,
            "sequenced skeleton owned-source clone",
        )?;
        let path = ExprSkeletonPath {
            observations,
            owned_source,
            failed: suffix.failed,
            residual: suffix.residual,
            continuation: lexical_skeleton::Continuation::sequenced(
                &prefix.continuation,
                &suffix.continuation,
            ),
        };
        work.charge(1, "sequenced skeleton path")?;
        note_skeleton_materialization();
        work.budget.reserve_array::<ExprSkeletonPath>(
            work.function,
            0,
            "sequenced skeleton path",
        )?;
        *slot = path;
    }
    Ok(paths)
}

#[cfg(test)]
#[path = "before.rs"]
mod before;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
