//! Paid no-growth output backing reuse; complete independent enumerator retained.
use super::*;
use std::mem::size_of;
use physical_budget::ByteFailure;

// New query and cold fallback frames. The first borrowed acquisition substitutes
// for the first original consuming acquisition; its original work precedes this
// bank. Active-first cold probes retain exact DAG sharing and first clone work,
// and run no active reuse. Extra iterator/next roles are separately priced.
pub(super) const QUERY: usize = 2 * size_of::<Result<(), ByteFailure>>()
    + 4 * size_of::<&mut ReplayBudget>() + 4 * size_of::<&ResolvedFunction>()
    + 4 * size_of::<&str>()
    + 2 * size_of::<std::slice::IterMut<'static, ExprSkeletonPath>>()
    + 2 * size_of::<Option<&mut ExprSkeletonPath>>() + size_of::<&mut ExprSkeletonPath>()
    + 2 * size_of::<std::slice::Iter<'static, ExprSkeletonPath>>()
    + 2 * size_of::<Option<&ExprSkeletonPath>>() + size_of::<&ExprSkeletonPath>()
    + 2 * size_of::<std::vec::IntoIter<ExprSkeletonPath>>()
    + 2 * size_of::<Option<ExprSkeletonPath>>()
    + 10 * size_of::<Option<usize>>() + 32 * size_of::<usize>()
    + 16 * size_of::<bool>() + 4 * size_of::<&Vec<ExprSkeletonPath>>()
    + 2 * size_of::<&ReplayBudget>() + ORACLE_QUERY_CONTROLS
    + ReplayBudget::WARM_PREPARE_CONTROLS + ReplayBudget::ORACLE_SELECTOR_CONTROLS;

// Every reached scan member: returned/matched Some reference, bound reference,
// both stop flags, their lazy combination and the accumulated eligibility flag.
pub(super) const SCAN_ROW: usize = 2 * size_of::<Option<&ExprSkeletonPath>>()
    + size_of::<&ExprSkeletonPath>() + 4 * size_of::<bool>();
// The cursor's comparison, addition and publication are new; stopped pushes'
// original work and complete ordinary reserve_vec calls remain independently.
pub(super) const CURSOR_ROW: usize = 4 * size_of::<usize>() + 2 * size_of::<bool>()
    + 2 * size_of::<&mut PrefixBuffer<ExprSkeletonPath>>();

// New owned guard/cleanup and pop/return receiving transports, including the
// unwind drain range/iterator. The original active body is copied lexically,
// with its original work, all clone/extend/push receivers and output counts.
pub(super) const FRAME: usize = 2 * size_of::<PrefixBuffer<ExprSkeletonPath>>()
    + 4 * size_of::<Vec<ExprSkeletonPath>>()
    + 2 * size_of::<Option<ExprSkeletonPath>>() + size_of::<ExprSkeletonPath>()
    + 2 * size_of::<std::vec::Drain<'static, ExprSkeletonPath>>()
    + 2 * size_of::<std::ops::RangeFrom<usize>>()
    + 4 * size_of::<&mut Vec<ExprSkeletonPath>>()
    + 2 * size_of::<&mut PrefixBuffer<ExprSkeletonPath>>()
    + 2 * size_of::<std::slice::Iter<'static, ExprSkeletonPath>>()
    + 2 * size_of::<Option<&ExprSkeletonPath>>()
    + 2 * size_of::<Option<(&ExprSkeletonPath, &[ExprSkeletonPath])>>()
    + 2 * size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<Result<Vec<ExprSkeletonPath>, Diagnostic>>()
    + 8 * size_of::<usize>() + 6 * size_of::<bool>()
    // One additional scan-work invocation, with full returning work/Oracle
    // receiving controls. This expression remains subject to peer census.
    + 4 * size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<Result<(), ByteFailure>>()
    + 4 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * size_of::<(&mut SkeletonWork<'static, 'static>, usize, &str)>()
    + 4 * size_of::<Option<usize>>()
    + ORACLE_WORK_CONTROLS + 2 * ReplayBudget::WARM_PREPARE_CONTROLS
    + 8 * size_of::<bool>() + 8 * size_of::<usize>();

#[cfg(test)]
const ORACLE_WORK_CONTROLS: usize = allocation::OracleLease::WORK_CALL_CONTROLS
    + allocation::OracleLease::BYTE_CALL_CONTROLS;
#[cfg(not(test))]
const ORACLE_WORK_CONTROLS: usize = 0;
#[cfg(test)]
const ORACLE_QUERY_CONTROLS: usize = allocation::OracleLease::BYTE_CALL_CONTROLS;
#[cfg(not(test))]
const ORACLE_QUERY_CONTROLS: usize = 0;

// Before all stopped paths have been published, the old consuming loop drops
// current + unprocessed tail BEFORE its combined output. Retained backing must
// preserve exactly that order on ordinary errors AND unwinds. Once the last
// active path has been popped, all remaining slots are published output and
// normal Vec order is the old order. The active local then drops first.
pub(super) struct PrefixBuffer<T> {
    pub(super) paths: Vec<T>,
    pub(super) published: usize,
    pub(super) has_unprocessed: bool,
}
impl<T> Drop for PrefixBuffer<T> {
    fn drop(&mut self) {
        if self.has_unprocessed {
            drop(self.paths.drain(self.published..));
        }
    }
}

macro_rules! reuse {
    ($prefixes:ident, $suffixes:ident, $work:ident, $sharing:ident, $minimum:ident, $precharged:ident, $first:ident, $single:ident) => {{
        let stopped;
        {
            let mut input = $prefixes.iter_mut();
            let Some(prefix) = input.next() else { unreachable!("original nonempty minimum") };
            stopped = prefix.failed || prefix.residual;
            if stopped {
                $work.charge(1, "short-circuited skeleton path")?;
                note_skeleton_materialization();
            } else {
                // Preserve the exact old pre-clone ordering on a cold miss.
                skeleton_work::dag_storage::share_prefix!($work, $sharing, prefix.observations);
                let mut pending = $suffixes.iter();
                let Some(_) = pending.next() else { unreachable!("multiple suffixes") };
                #[cfg(test)]
                final_output::record_final_frontier(0, $work);
                $work.charge(1, "skeleton prefix clone")?;
            }
        }
        $precharged = true;
        $crate::cleanup_plan::replay::trailing_active::observe_frontier($work,0,$crate::cleanup_plan::replay::trailing_active::QUERY);
        physical_budget::compact_try!($work.budget.reserve_compact_bytes(
            $work.function, $crate::cleanup_plan::replay::trailing_active::QUERY,
            "trailing-active reuse query controls"));
        let oracle = $work.budget.oracle_lease();
        if oracle.is_some() && stopped && !$work.budget.selected() {
            let n = $prefixes.len();
            if let Some(required) = n.checked_sub(1).and_then(|count| count.checked_add($suffixes.len())) {
                if required <= MAX_REPLAY_PATHS && required <= $prefixes.capacity() {
                    if let Some(old_capacity) = $minimum.checked_next_power_of_two() {
                        if let Some(old_bytes) = old_capacity.checked_mul(std::mem::size_of::<ExprSkeletonPath>()) {
                            if let Some(rows) = n.checked_mul($crate::cleanup_plan::replay::trailing_active::SCAN_ROW
                                + $crate::cleanup_plan::replay::trailing_active::CURSOR_ROW) {
                                if let Some(frame) = rows.checked_add($crate::cleanup_plan::replay::trailing_active::FRAME) {
                                    if let Some(total) = frame.checked_add($crate::cleanup_plan::replay::trailing_active::QUERY) {
                                        // Input capacity is not a saving: compare the exact old
                                        // minimum output allocation against ALL new controls.
                                        if old_bytes > total {
                                            $crate::cleanup_plan::replay::trailing_active::observe_frontier($work,1,frame);
                                            physical_budget::compact_try!($work.budget.reserve_compact_bytes(
                                                $work.function, frame, "trailing-active scan and cleanup controls"));
                                            $work.charge(n, "trailing-active shape scan")?;
                                            let (last, stopped_prefix) = $prefixes.split_last().unwrap();
                                            let mut eligible = !last.failed && !last.residual;
                                            for path in stopped_prefix { eligible &= path.failed || path.residual; }
                                            if eligible {
                                                $crate::cleanup_plan::replay::trailing_active::transfer!(
                                                    $prefixes, $suffixes, $work, $sharing, $minimum,
                                                    $precharged, $first, $single);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }};
}
pub(super) use reuse;

macro_rules! transfer {
    ($prefixes:ident, $suffixes:ident, $work:ident, $sharing:ident, $minimum:ident, $precharged:ident, $first:ident, $single:ident) => {{
        let stopped_count = $prefixes.len() - 1;
        let mut buffer = $crate::cleanup_plan::replay::trailing_active::PrefixBuffer {
            paths: $prefixes, published: 0, has_unprocessed: true,
        };
        while buffer.published < stopped_count {
            if $precharged { $precharged = false; }
            else { $work.charge(1, "short-circuited skeleton path")?; note_skeleton_materialization(); }
            if $first {
                $work.budget.reserve_bytes($work.function, path_array::capacity_controls(), "short-circuited skeleton path")?;
                // Keep the original capacity arithmetic and failure selection;
                // only its physically absent output backing is not requested.
                let _capacity = match $minimum.checked_next_power_of_two() {
                    Some(capacity) => capacity,
                    None => return Err($work.budget.budget_refusal($work.function,
                        "cleanup replay minimum path capacity overflowed", "")),
                };
            }
            $work.budget.reserve_vec($work.function, &mut buffer.paths, 0, "short-circuited skeleton path")?;
            buffer.published += 1;
            $first = false;
        }
        let mut prefix = buffer.paths.pop().unwrap();
        buffer.has_unprocessed = false;
        {
            let mut combined = &mut buffer.paths;
            'active: {
        if !$precharged {
            skeleton_work::dag_storage::share_prefix!($work, $sharing, prefix.observations);
        }
        if $single {
            if $precharged {
                $precharged = false;
            } else {
                final_output::compact::hir_move!($work);
                final_output::compact::hir_work!($work);
                note_skeleton_materialization();
            }
            let suffix = &$suffixes[0];
            let observations = prefix.observations;
            let observations = $work.extend_owned_observations(
                observations,
                &suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source = $work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            compact_path::push!(
                $work,
                $first,
                $minimum,
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
            break 'active;
        }
        // The $first original clone $work debit wins before optional metadata.
        // Iter/next are the original borrowed for-loop state; empty input does
        // not introduce either a $work debit or a selector payment.
        let mut pending = $suffixes.iter();
        let Some(first_suffix) = pending.next() else {
            break 'active;
        };
        #[cfg(test)]
        final_output::record_final_frontier(0, $work);
        $work.charge(1, "skeleton prefix clone")?;
        $work.budget.reserve_bytes(
            $work.function,
            final_output::FINAL_SPLIT_CONTROLS,
            "HIR final-suffix split controls",
        )?;
        skeleton_work::enumerated_prefix::share!($work, prefix);
        // Lexical remainder of the original $first clone: no second charge and
        // no newly forwarded Result<Observations, Diagnostic>.
        let observations = skeleton_work::prefix_fusion::after_work!(
            $work,
            prefix.observations,
            first_suffix.observations
        );
        let owned_source = $work.clone_owned(
            &first_suffix.owned_source,
            "sequenced skeleton owned-source clone",
        )?;
        path_array::sequence_minimum::optimized::push!(
            $work,
            $first,
            $minimum,
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
        // Preserve the original $first-output remaining-count receiver: only
        // after that publication may the last suffix leave this iterator.
        let Some(last) = pending.next_back() else {
            break 'active;
        };
        for suffix in pending {
            let observations =
                $work.clone_observations(&prefix.observations, "skeleton prefix clone")?;
            let observations = $work.extend_owned_observations(
                observations,
                &suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source = $work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            compact_path::push!(
                $work,
                $first,
                $minimum,
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
        // The last original clone also debits $work before any header query.
        #[cfg(test)]
        final_output::record_final_frontier(1, $work);
        $work.charge(1, "skeleton prefix clone")?;
        $work.budget.reserve_bytes(
            $work.function,
            final_output::FINAL_HEADER_CONTROLS,
            "HIR final-suffix header controls",
        )?;
        let len = prefix.observations.items.len();
        let capacity = prefix.observations.items.capacity();
        let suffix_len = last.observations.items.len();
        let take = if $work.budget.pays_allocations() {
            if $work.budget.selected() || len < final_output::FINAL_MINIMUM_ITEMS {
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
            $work.budget.reserve_bytes(
                $work.function,
                final_output::FINAL_MOVE_CONTROLS,
                "HIR final-suffix move controls",
            )?;
            note_skeleton_materialization();
            prefix.observations
        } else {
            $work.budget.reserve_clone(
                $work.function,
                &prefix.observations,
                "skeleton prefix clone",
            )?;
            note_skeleton_materialization();
            prefix.observations.clone()
        };
        let observations = $work.extend_owned_observations(
            observations,
            &last.observations,
            "skeleton suffix clone",
        )?;
        let owned_source =
            $work.clone_owned(&last.owned_source, "sequenced skeleton owned-source clone")?;
        compact_path::push!(
            $work,
            $first,
            $minimum,
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
        }
        let output = std::mem::take(&mut buffer.paths);
        return Ok(output);
    }};
}
pub(super) use transfer;

#[cfg(test)]
thread_local! { static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
pub(super) fn before_selected() -> bool { BEFORE.with(|v| v.get()) }
#[cfg(test)]
fn with_before<T>(operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore { fn drop(&mut self) { BEFORE.with(|v| v.set(self.0)); } }
    let restore = Restore(BEFORE.with(|v| v.replace(true)));
    let output = operation();drop(restore);output
}
#[cfg(test)]
mod tests;

// ENTIRE immediate predecessor: namespace/visibility only; all original
// selectors, setup, general/single/multiple loops and failure gates retained.
#[cfg(test)]
pub(super) fn before_sequence_skeleton_paths(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    #[cfg(test)]
    if skeleton_work::enumerated_prefix::original_selected() {
        return skeleton_work::enumerated_prefix::old_sequence(prefixes, suffixes, work);
    }
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
    let prefixes = ordinary_plan_history::completed_fork::emission::dispatch!(
        prefixes, suffixes, work
    );
    let mut prefixes = work.merged_paths(prefixes)?;
    let suffixes = work.merged_suffixes(suffixes)?;
    let share_prefix = work.prefix_tail_sharing(&suffixes)?;
    let mut combined = Vec::new();
    let minimum = compact_path::count!(
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
    final_output::compact::hir_selection!(work);
    let single = suffixes.len() == 1;
    let mut precharged = false;
    if single && first {
        sequence_vector::reuse!(prefixes, suffixes, work, token share_prefix, minimum, precharged);
    }
    for mut prefix in prefixes {
        if prefix.failed || prefix.residual {
            if precharged {
                precharged = false;
                sequence_vector::stopped_after_work!(work, first, minimum, combined, prefix);
                continue;
            }
            compact_path::push!(
                work,
                first,
                minimum,
                &mut combined,
                prefix,
                "short-circuited skeleton path"
            );
            continue;
        }
        if !precharged {
            skeleton_work::dag_storage::share_prefix!(work, share_prefix, prefix.observations);
        }
        if single {
            if precharged {
                precharged = false;
            } else {
                final_output::compact::hir_move!(work);
                final_output::compact::hir_work!(work);
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
            compact_path::push!(
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
        skeleton_work::enumerated_prefix::share!(work, prefix);
        // Lexical remainder of the original first clone: no second charge and
        // no newly forwarded Result<Observations, Diagnostic>.
        let observations = skeleton_work::prefix_fusion::after_work!(
            work,
            prefix.observations,
            first_suffix.observations
        );
        let owned_source = work.clone_owned(
            &first_suffix.owned_source,
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
                    &first_suffix.continuation,
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
                &suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source = work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            compact_path::push!(
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
        compact_path::push!(
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

// Fixed test instrumentation only: records an existing account's charge at the
// two NEW positive requests. It cannot grant budget, reset an account, replace
// the payer, alter admission, or suppress the actual ordinary reserve call.
thread_local! {static FRONTIERS: std::cell::Cell<Option<[usize;4]>>=const{std::cell::Cell::new(None)};}
pub(super) fn observe_frontier(work:&SkeletonWork<'_, '_>,index:usize,requested:usize) {
    FRONTIERS.with(|slot|{if let Some(mut rows)=slot.get(){let receipt=work.budget.oracle_lease().unwrap().receipt().unwrap();rows[index*2]=receipt.charged;rows[index*2+1]=requested;slot.set(Some(rows));}});
}
#[cfg(test)]
fn with_frontiers<R>(body:impl FnOnce()->R)->(R,[usize;4]) {
    struct Reset;impl Drop for Reset {fn drop(&mut self){FRONTIERS.with(|s|s.set(None));}}
    FRONTIERS.with(|s|{assert!(s.get().is_none());s.set(Some([0;4]));});let reset=Reset;let result=body();let rows=FRONTIERS.with(|s|s.get().unwrap());drop(reset);(result,rows)
}
