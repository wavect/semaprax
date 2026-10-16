use super::*;
pub(in crate::cleanup_plan::replay) fn sequence_skeleton_paths(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let prefixes = work.merged_paths(prefixes)?;
    let suffixes = work.merged_suffixes(suffixes)?;
    let share_prefix = work.shares_prefix_tail(&suffixes)?;
    if sequence_vector::can_reuse(sequence_vector::Candidate {
        prefixes: &prefixes,
        suffixes: &suffixes,
        budget: work.budget,
    }) {
        work.budget.reserve_array::<u8>(
            work.function,
            sequence_vector::receiving_bytes(),
            "single-successor receiving frame",
        )?;
        return sequence_vector::single(prefixes, &suffixes, work, share_prefix);
    }
    let mut combined = Vec::new();
    if !suffixes.is_empty() {
        path_array::minimum(
            &mut combined,
            prefixes.len(),
            work,
            "sequenced path capacity",
        )?;
    }
    for mut prefix in prefixes {
        if prefix.failed || prefix.residual {
            work.push_expr_path(&mut combined, prefix, "short-circuited skeleton path")?;
            continue;
        }
        if share_prefix {
            work.share_prefix_tail(&mut prefix.observations)?;
        }
        // This is the original for-loop's Iter/Option state. Earlier
        // successors borrow the prefix; the last one can take it intact.
        let mut remaining_suffixes = suffixes.iter();
        while let Some(suffix) = remaining_suffixes.next() {
            let observations = if sequence_prefix::can_take(sequence_prefix::Candidate {
                prefix: &prefix.observations,
                suffix: &suffix.observations,
                remaining_suffixes: remaining_suffixes.len(),
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
            work.push_expr_path(
                &mut combined,
                ExprSkeletonPath {
                    observations,
                    owned_source,
                    failed: suffix.failed,
                    residual: suffix.residual,
                    continuation: lexical_skeleton::Continuation::sequenced(
                        &prefix.continuation,
                        &suffix.continuation,
                    ),
                },
                "sequenced skeleton path",
            )?;
        }
    }
    Ok(combined)
}
