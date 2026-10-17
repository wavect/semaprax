//! Lexical one-successor output-vector transfer after the first original work.

// Unchanged fees remain owned and compiled at replay::sequence_vector.
macro_rules! transfer {
    ($prefixes:ident, $suffixes:ident, $work:ident, $share:ident, $precharged:ident, $frame_bytes:ident) => {{
        $work.budget.reserve_bytes(
            $work.function,
            $frame_bytes,
            "single-successor vector receiving frame",
        )?;
        let mut scratch = Observations::default();
        for slot in &mut $prefixes {
            if slot.failed || slot.residual {
                if $precharged {
                    $precharged = false;
                } else {
                    $work.charge(1, "short-circuited skeleton path")?;
                    note_skeleton_materialization();
                }
                $work.budget.reserve_array::<ExprSkeletonPath>(
                    $work.function,
                    0,
                    "short-circuited skeleton path",
                )?;
                continue;
            }
            if $precharged {
                $precharged = false;
            } else {
                if $share {
                    $work.share_prefix_tail(&mut slot.observations)?;
                }
                $work.budget.reserve_bytes(
                    $work.function,
                    $crate::cleanup_plan::replay::final_output::HIR_MOVE_CONTROLS,
                    "HIR single-suffix move controls",
                )?;
                $work.charge(1, "skeleton prefix clone")?;
                note_skeleton_materialization();
            }
            let suffix = &$suffixes[0];
            std::mem::swap(&mut scratch, &mut slot.observations);
            // Exact surviving general owned extension, including DAG behavior,
            // nested Results, physical growth, first refusal and logical work.
            scratch = $work.extend_owned_observations(
                scratch,
                &suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source =
                $work.clone_owned(suffix.owned_source, "sequenced skeleton owned-source clone")?;
            let continuation =
                lexical_skeleton::Continuation::sequenced(&slot.continuation, suffix.continuation);
            $work.charge(1, "sequenced skeleton path")?;
            note_skeleton_materialization();
            $work.budget.reserve_array::<ExprSkeletonPath>(
                $work.function,
                0,
                "sequenced skeleton path",
            )?;
            std::mem::swap(&mut scratch, &mut slot.observations);
            // Same remaining-prefix Drop sequence: owned source, then
            // continuation, after successful publication. Nothing fallible or
            // allocating occurs between these assignments and the next item.
            slot.owned_source = owned_source;
            slot.failed = suffix.failed;
            slot.residual = suffix.residual;
            slot.continuation = continuation;
        }
        return Ok($prefixes);
    }};
}
pub(super) use transfer;

macro_rules! reuse {
    ($prefixes:ident, $suffixes:ident, $work:ident, $share:ident, $minimum:ident, $precharged:ident) => {{
        // Original first-item acquisition, borrowed until the branch decision.
        // The existing minimum/first predicate proves a nonempty owned vector.
        {
            let mut input = $prefixes.iter_mut();
            let Some(prefix) = input.next() else {
                unreachable!("nonempty path minimum")
            };
            if prefix.failed || prefix.residual {
                #[cfg(test)]
                $crate::cleanup_plan::replay::sequence_vector::funded_tests::record_first($work);
                $work.charge(1, "short-circuited skeleton path")?;
                $crate::cleanup_plan::replay::note_skeleton_materialization();
            } else {
                if $share {
                    $work.share_prefix_tail(&mut prefix.observations)?;
                }
                $work.budget.reserve_bytes(
                    $work.function,
                    $crate::cleanup_plan::replay::final_output::HIR_MOVE_CONTROLS,
                    "HIR single-suffix move controls",
                )?;
                #[cfg(test)]
                $crate::cleanup_plan::replay::sequence_vector::funded_tests::record_first($work);
                $work.charge(1, "skeleton prefix clone")?;
                $crate::cleanup_plan::replay::note_skeleton_materialization();
            }
        }
        $precharged = true;
        $work.budget.reserve_bytes(
            $work.function,
            $crate::cleanup_plan::replay::sequence_vector::QUERY,
            "single-successor vector selection",
        )?;
        if !$work.budget.selected() {
            if $minimum <= MAX_REPLAY_PATHS {
                let capacity = $prefixes.capacity();
                match $minimum.checked_next_power_of_two() {
                    Some(expected) if expected == capacity => {
                            match capacity.checked_mul(std::mem::size_of::<ExprSkeletonPath>()) {
                                Some(bytes) => {
                                    match $minimum.checked_mul($crate::cleanup_plan::replay::sequence_vector::SWAP_CONTROLS) {
                                        Some(swap_bytes) => {
                                            match swap_bytes.checked_add($crate::cleanup_plan::replay::sequence_vector::FRAME) {
                                                Some(frame_bytes) => {
                                                    match frame_bytes.checked_add($crate::cleanup_plan::replay::sequence_vector::QUERY) {
                                                        Some(min_saved) if bytes >= min_saved => {
                                                            $crate::cleanup_plan::replay::owned_suffix_878::sequence_vector::transfer!(
                                                                $prefixes, $suffixes, $work, $share, $precharged, frame_bytes);
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                                None => {}
                                            }
                                        }
                                        None => {}
                                    }
                                }
                                None => {}
                            }
                    }
                    _ => {}
                }
            }
        }
    }};
}
pub(super) use reuse;

// Exactly the old first stopped push after its work/materialization have run.
macro_rules! stopped_after_work {
    ($work:ident, $first:ident, $minimum:ident, $target:ident, $path:ident) => {{
        $work.budget.reserve_bytes(
            $work.function,
            $crate::cleanup_plan::replay::path_array::capacity_controls(),
            "short-circuited skeleton path",
        )?;
        let capacity = match $minimum.checked_next_power_of_two() {
            Some(capacity) => capacity,
            None => {
                return Err($work.budget.budget_refusal(
                    $work.function,
                    "cleanup replay minimum path capacity overflowed",
                    "",
                ))
            }
        };
        $work.budget.reserve_vec(
            $work.function,
            &mut $target,
            capacity,
            "short-circuited skeleton path",
        )?;
        $target.push($path);
        $first = false;
    }};
}
pub(super) use stopped_after_work;
