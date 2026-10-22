//! Owned caller suffix retention with borrowed representative metadata.
use super::*;
#[cfg(test)]
mod before;
mod sequence_vector;
#[cfg(test)]
mod tests;

struct Frame {
    // Field order is intentional on every preparation refusal.
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: Vec<ExprSkeletonPath>,
}
struct View<'a> {
    observations: &'a Observations,
    owned_source: &'a Option<CleanupPlace>,
    failed: bool,
    residual: bool,
    continuation: &'a lexical_skeleton::Continuation,
}

// New dispatch/receiving owners, rows, borrowed view assembly and match/return
// carriers. Existing sequence/payload/work/union fees remain in their bodies.
pub(super) const QUERY: usize = std::mem::size_of::<Result<(), Diagnostic>>()
    + 2 * std::mem::size_of::<&mut ReplayBudget>()
    + 2 * std::mem::size_of::<&ResolvedFunction>()
    + 2 * std::mem::size_of::<&str>()
    + 2 * std::mem::size_of::<&Vec<ExprSkeletonPath>>()
    + 2 * std::mem::size_of::<usize>()
    + 4 * std::mem::size_of::<bool>();

// Per original suffix: actual/formal clone helper arguments and returned,
// received, then stored observation carriers. Acquired before moving inputs.
pub(super) const ROW_CLONE_CONTROLS: usize =
    2 * std::mem::size_of::<(&mut SkeletonWork<'static, 'static>, &Observations, &str)>()
        + 2 * std::mem::size_of::<Result<Observations, Diagnostic>>()
        + std::mem::size_of::<Observations>(); // SearchComputation is physically absent from the new lexical search.

pub(super) const CONTROLS: usize = 2 * std::mem::size_of::<Vec<Observations>>()
    + 2 * std::mem::size_of::<std::vec::IntoIter<Observations>>()
    + 2 * std::mem::size_of::<std::iter::Enumerate<std::vec::IntoIter<Observations>>>()
    + 2 * std::mem::size_of::<std::slice::Iter<'static, ExprSkeletonPath>>()
    + std::mem::size_of::<Frame>()
    + 4 * std::mem::size_of::<Vec<ExprSkeletonPath>>()
    + 2 * std::mem::size_of::<Result<Vec<ExprSkeletonPath>, Diagnostic>>()
    + 2 * std::mem::size_of::<Result<Vec<(usize, Observations)>, Diagnostic>>()
    + 4 * std::mem::size_of::<Result<(), Diagnostic>>()
    + 6 * std::mem::size_of::<&mut SkeletonWork<'static, 'static>>()
    + 6 * std::mem::size_of::<&ResolvedFunction>()
    + 6 * std::mem::size_of::<&str>()
    + 12 * std::mem::size_of::<usize>()
    + 12 * std::mem::size_of::<bool>()
    + 4 * std::mem::size_of::<Vec<(usize, Observations)>>()
    + 4 * std::mem::size_of::<Vec<View<'static>>>()
    + 4 * std::mem::size_of::<std::slice::Iter<'static, (usize, Observations)>>()
    + 8 * std::mem::size_of::<View<'static>>();

// This macro acquires the complete new dispatch before moving either owned
// argument. On its own admission refusal, explicitly preserve the old callee's
// prefix-before-original-suffix release order rather than caller-local order.
macro_rules! dispatch {
    ($prefixes:ident, $suffixes:ident, $work:ident) => {{
        #[cfg(test)]
        if path_array_main44::selected()
            || sequence_vector_maina3::selected()
            || path_array::main696::selected()
        {
            return sequence_skeleton_paths($prefixes, &$suffixes, $work);
        }
        match $work.budget.reserve_bytes(
            $work.function,
            $crate::cleanup_plan::replay::owned_suffix::QUERY,
            "owned suffix dispatch query controls",
        ) {
            Ok(()) => {}
            Err(error) => {
                drop($prefixes);
                drop($suffixes);
                return Err(error);
            }
        }
        if $work.budget.merge_paths && $suffixes.len() > 1 {
            match $work.budget.reserve_bytes(
                $work.function,
                $crate::cleanup_plan::replay::owned_suffix::CONTROLS.saturating_add(
                    $crate::cleanup_plan::replay::owned_suffix::ROW_CLONE_CONTROLS
                        .saturating_mul($suffixes.len()),
                ),
                "owned suffix retention receiving controls",
            ) {
                Ok(()) => {}
                Err(error) => {
                    drop($prefixes);
                    drop($suffixes);
                    return Err(error);
                }
            }
            return $crate::cleanup_plan::replay::owned_suffix::sequence(
                $prefixes, $suffixes, $work,
            );
        }
    }};
}
pub(super) use dispatch;

pub(super) fn sequence(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut frame = Frame { prefixes, suffixes };
    let prefixes = work.merged_paths(std::mem::take(&mut frame.prefixes))?;
    // Retain the merged prefix owner first while suffix preparation can fail.
    frame.prefixes = prefixes;
    let rows = rows(&frame.suffixes, work)?;
    work.budget.reserve_array::<View<'_>>(
        work.function,
        rows.len(),
        "owned suffix borrowed views",
    )?;
    let mut views = Vec::with_capacity(rows.len());
    for (index, observations) in &rows {
        let original = &frame.suffixes[*index];
        views.push(View {
            observations,
            owned_source: &original.owned_source,
            failed: original.failed,
            residual: original.residual,
            continuation: &original.continuation,
        });
    }
    // The original suffix carrier remains outside the complete sequence core,
    // as it did outside sequence_skeleton_paths in sequence_expression.
    let prefixes = std::mem::take(&mut frame.prefixes);
    sequence_core(prefixes, &views, work)
}

fn rows(
    originals: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<(usize, Observations)>, Diagnostic> {
    // Match the old suffix preparation phase: every independent observation
    // clone is attempted before the path-state merge work debit. Metadata and
    // the full ExprSkeletonPath copy array are absent, not charged as credit.
    work.budget.reserve_array::<Observations>(
        work.function,
        originals.len(),
        "factored suffix path copies",
    )?;
    let mut copies = Vec::with_capacity(originals.len());
    for path in originals {
        copies.push(work.clone_payload(&path.observations, "factored suffix path backing")?);
    }
    work.charge(originals.len(), "factored path-state merge")?;
    work.budget.reserve_bytes(
        work.function,
        2 * std::mem::size_of::<Vec<usize>>()
            + std::mem::size_of::<Vec<(usize, Vec<Observations>)>>()
            + skeleton_work::merge_search::OWNER_CONTROLS,
        "owned suffix merge headers",
    )?;
    skeleton_work::merge_search::construct!(search);
    let mut order = Vec::<usize>::new();
    let mut groups = Vec::<(usize, Vec<Observations>)>::new();
    for (index, observations) in copies.into_iter().enumerate() {
        let path = &originals[index];
        if !work.lexical_merge_normal(path) {
            work.budget
                .reserve_vec(work.function, &mut groups, 1, "isolated lexical path")?;
            work.budget.reserve_array::<Observations>(
                work.function,
                1,
                "owned suffix isolated observation",
            )?;
            groups.push((index, vec![observations]));
            continue;
        }
        skeleton_work::merge_search::search!(
            work,
            search,
            order,
            skeleton_work::merge_search::ROW_INDEX,
            skeleton_work::merge_search::PATH_COMPARE,
            skeleton_work::merge_keys::compare_paths(&originals[groups[search.group].0], path)
        );
        if search.found {
            let group = order[search.position];
            work.budget.reserve_vec(
                work.function,
                &mut groups[group].1,
                1,
                "factored merge members",
            )?;
            groups[group].1.push(observations);
        } else {
            work.budget.reserve_vec(
                work.function,
                &mut order,
                1,
                "factored borrowed merge indices",
            )?;
            work.budget
                .reserve_vec(work.function, &mut groups, 1, "factored merge groups")?;
            work.budget.reserve_array::<Observations>(
                work.function,
                1,
                "factored first merge member",
            )?;
            order.insert(search.position, groups.len());
            groups.push((index, vec![observations]));
        }
    }
    work.budget.reserve_array::<(usize, Observations)>(
        work.function,
        groups.len(),
        "owned suffix merged rows",
    )?;
    let mut result = Vec::with_capacity(groups.len());
    for (index, mut members) in groups {
        let observations = if members.len() > 1 {
            if work.budget.selected() {
                work.union_observations(members)?
            } else {
                work.ordinary_union(members)?
            }
        } else {
            members.pop().expect("one original suffix member")
        };
        result.push((index, observations));
    }
    Ok(result)
}

fn sequence_core(
    mut prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[View<'_>],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let share_prefix = if suffixes.len() < 2 {
        false
    } else {
        work.charge(suffixes.len(), "factored suffix fanout inspection")?;
        suffixes
            .iter()
            .any(|suffix| suffix.observations.prefix.is_some())
    };
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
                suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source =
                work.clone_owned(suffix.owned_source, "sequenced skeleton owned-source clone")?;
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
                        suffix.continuation,
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
        // Lexical remainder of the original first clone: no second charge and
        // no newly forwarded Result<Observations, Diagnostic>.
        let observations = skeleton_work::prefix_fusion::after_work!(
            work,
            prefix.observations,
            *first_suffix.observations
        );
        let owned_source = work.clone_owned(
            first_suffix.owned_source,
            "sequenced skeleton owned-source clone",
        )?;
        path_array::sequence_minimum::optimized::push!(
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
                    first_suffix.continuation,
                ),
            },
            "sequenced skeleton path",
        );
        // Preserve the original first-output remaining-count receiver: only
        // after that publication may the last suffix leave this iterator.
        let Some(last) = pending.next_back() else {
            continue;
        };
        for suffix in pending {
            let observations =
                work.clone_observations(&prefix.observations, "skeleton prefix clone")?;
            let observations = work.extend_owned_observations(
                observations,
                suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source =
                work.clone_owned(suffix.owned_source, "sequenced skeleton owned-source clone")?;
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
                        suffix.continuation,
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
            last.observations,
            "skeleton suffix clone",
        )?;
        let owned_source =
            work.clone_owned(last.owned_source, "sequenced skeleton owned-source clone")?;
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
                    last.continuation,
                ),
            },
            "sequenced skeleton path",
        );
    }
    Ok(combined)
}
