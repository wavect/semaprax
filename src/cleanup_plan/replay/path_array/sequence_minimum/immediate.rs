//! Complete immediate paid receiver, including every original transfer and work operation.
use super::super::super::*;
pub(super) fn sequence_skeleton_paths(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    #[cfg(test)]
    if path_array_main44::selected() {
        return path_array_main44::current_sequence(prefixes, suffixes, work);
    }
    #[cfg(test)]
    if sequence_vector_maina3::selected() {
        return sequence_vector_maina3::current_sequence(prefixes, suffixes, work);
    }
    #[cfg(test)]
    if path_array::main696::selected() {
        return path_array::main696::sequence_skeleton_paths(prefixes, suffixes, work);
    }
    let mut prefixes = work.merged_paths(prefixes)?;
    let suffixes = work.merged_suffixes(suffixes)?;
    let share_prefix = work.shares_prefix_tail(&suffixes)?;
    let mut combined = Vec::new();
    let minimum = path_array::count!(
        work,
        true,
        if suffixes.is_empty() {
            0
        } else {
            prefixes.len()
        },
        "sequenced path capacity"
    );
    path_array::select!(first, minimum, &combined, work);
    work.budget.reserve_bytes(
        work.function,
        final_output::HIR_SELECTION_CONTROLS,
        "HIR single-suffix selection controls",
    )?;
    let single = suffixes.len() == 1;
    let mut precharged = false;
    if single && first {
        sequence_vector::reuse!(prefixes, suffixes, work, share_prefix, minimum, precharged);
    }
    for mut prefix in prefixes {
        if prefix.failed || prefix.residual {
            if precharged {
                precharged = false;
                sequence_vector::stopped_after_work!(work, first, minimum, combined, prefix);
                continue;
            }
            path_array::push!(
                work,
                first,
                minimum,
                &mut combined,
                prefix,
                "short-circuited skeleton path"
            );
            continue;
        }
        if !precharged && share_prefix {
            work.share_prefix_tail(&mut prefix.observations)?;
        }
        if single {
            if precharged {
                precharged = false;
            } else {
                work.budget.reserve_bytes(
                    work.function,
                    final_output::HIR_MOVE_CONTROLS,
                    "HIR single-suffix move controls",
                )?;
                work.charge(1, "skeleton prefix clone")?;
                note_skeleton_materialization();
            }
            let suffix = &suffixes[0];
            let observations = prefix.observations;
            let observations = work.extend_owned_observations(
                observations,
                &suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source = work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            path_array::push!(
                work,
                first,
                minimum,
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
            );
            continue;
        }
        // The first original clone work debit wins before optional metadata.
        // Iter/next are the original borrowed for-loop state; empty input does
        // not introduce either a work debit or a selector payment.
        let mut pending = suffixes.iter();
        let Some(first_suffix) = pending.next() else {
            continue;
        };
        #[cfg(test)]
        final_output::record_final_frontier(0, work);
        work.charge(1, "skeleton prefix clone")?;
        work.budget.reserve_bytes(
            work.function,
            final_output::FINAL_SPLIT_CONTROLS,
            "HIR final-suffix split controls",
        )?;
        let Some((last, earlier)) = pending.as_slice().split_last() else {
            continue;
        };
        // Lexical remainder of the original first clone: no second charge and
        // no newly forwarded Result<Observations, Diagnostic>.
        work.budget
            .reserve_clone(work.function, &prefix.observations, "skeleton prefix clone")?;
        note_skeleton_materialization();
        let observations = prefix.observations.clone();
        let observations = work.extend_owned_observations(
            observations,
            &first_suffix.observations,
            "skeleton suffix clone",
        )?;
        let owned_source = work.clone_owned(
            &first_suffix.owned_source,
            "sequenced skeleton owned-source clone",
        )?;
        path_array::sequence_minimum::push!(
            work,
            first,
            minimum,
            pending,
            &mut combined,
            ExprSkeletonPath {
                observations,
                owned_source,
                failed: first_suffix.failed,
                residual: first_suffix.residual,
                continuation: lexical_skeleton::Continuation::sequenced(
                    &prefix.continuation,
                    &first_suffix.continuation,
                ),
            },
            "sequenced skeleton path",
        );
        for suffix in earlier.iter() {
            let observations =
                work.clone_observations(&prefix.observations, "skeleton prefix clone")?;
            let observations = work.extend_owned_observations(
                observations,
                &suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source = work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            path_array::push!(
                work,
                first,
                minimum,
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
            );
        }
        // The last original clone also debits work before any header query.
        #[cfg(test)]
        final_output::record_final_frontier(1, work);
        work.charge(1, "skeleton prefix clone")?;
        work.budget.reserve_bytes(
            work.function,
            final_output::FINAL_HEADER_CONTROLS,
            "HIR final-suffix header controls",
        )?;
        let len = prefix.observations.items.len();
        let capacity = prefix.observations.items.capacity();
        let suffix_len = last.observations.items.len();
        let take = if work.budget.pays_allocations() {
            if work.budget.selected() || len < final_output::FINAL_MINIMUM_ITEMS {
                false
            } else if last.observations.prefix.is_some() || capacity == len {
                true
            } else {
                suffix_len <= capacity - len
            }
        } else {
            false
        };
        let observations = if take {
            work.budget.reserve_bytes(
                work.function,
                final_output::FINAL_MOVE_CONTROLS,
                "HIR final-suffix move controls",
            )?;
            note_skeleton_materialization();
            prefix.observations
        } else {
            work.budget.reserve_clone(
                work.function,
                &prefix.observations,
                "skeleton prefix clone",
            )?;
            note_skeleton_materialization();
            prefix.observations.clone()
        };
        let observations = work.extend_owned_observations(
            observations,
            &last.observations,
            "skeleton suffix clone",
        )?;
        let owned_source =
            work.clone_owned(&last.owned_source, "sequenced skeleton owned-source clone")?;
        path_array::push!(
            work,
            first,
            minimum,
            &mut combined,
            ExprSkeletonPath {
                observations,
                owned_source,
                failed: last.failed,
                residual: last.residual,
                continuation: lexical_skeleton::Continuation::sequenced(
                    &prefix.continuation,
                    &last.continuation,
                ),
            },
            "sequenced skeleton path",
        );
    }
    Ok(combined)
}
