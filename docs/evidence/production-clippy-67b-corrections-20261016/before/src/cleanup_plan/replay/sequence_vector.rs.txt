//! Lexical one-successor output-vector transfer after the first original work.
use super::{Diagnostic, ExprSkeletonPath, Observations, ReplayBudget, ResolvedFunction};
use std::mem::size_of;

// Paid after the first original stopped-push/active-clone work. The first
// IterMut/Option view substitutes for the original consuming loop acquisition;
// it carries no owned replacement, probe, or fallible helper before that work.
pub(super) const QUERY: usize = size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<&mut ReplayBudget>() + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>()
    + 2 * size_of::<&ReplayBudget>() // selected caller/method
    + 2 * size_of::<&Vec<ExprSkeletonPath>>() // capacity caller/method
    + 10 * size_of::<Option<usize>>() // five checked results and their matches
    + 28 * size_of::<usize>() // price, arithmetic self/RHS/results and held outputs
    + 5 * size_of::<bool>() // selected/negation, bound, equality, saved-backing test
    // On false selection the original consuming loop is acquired AFTER the
    // first borrowed item. This additional iterator/next frame is prepaid here;
    // it is unused when the mutable input frame returns the vector directly.
    + 2 * size_of::<std::vec::IntoIter<ExprSkeletonPath>>()
    + 2 * size_of::<Option<ExprSkeletonPath>>();

// The first borrowed next substitutes for the old first consuming next. The
// selected iterator performs one additional successful next; its two Option
// slots below pay that extra call. Remaining next/item roles correspond exactly
// to the old remaining consuming loop, not free repeated frame initialization.
// One actual iterator and ONE persistent empty scratch are initialized after
// admission. Each successful active iteration swaps that same empty owner back;
// no repeated default/path constructor is credited as a retained frame.
pub(super) const FRAME: usize = size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<&mut ReplayBudget>() + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>() + size_of::<usize>()
    + 2 * size_of::<std::slice::IterMut<'static, ExprSkeletonPath>>()
    + 2 * size_of::<&mut Vec<ExprSkeletonPath>>()
    + size_of::<&mut [ExprSkeletonPath]>()
    + 2 * size_of::<Option<&mut ExprSkeletonPath>>()
    + size_of::<&mut ExprSkeletonPath>()
    + 2 * size_of::<Observations>() // Default return and scratch receiver
    + 2 * size_of::<Vec<std::rc::Rc<super::SkeletonObservation>>>() // Vec::default return/field
    + size_of::<Option<std::rc::Rc<super::skeleton_work::ObservationDag>>>();
// TWO swaps per actual active path: each has two mutable arguments, each with
// caller initializer and callee receiving roles. Reserve the maximum count
// before any swap, including unused capacity for terminal/stopped input slots.
pub(super) const SWAP_CONTROLS: usize = 8 * size_of::<&mut Observations>();

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
            let owned_source = $work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            let continuation =
                lexical_skeleton::Continuation::sequenced(&slot.continuation, &suffix.continuation);
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
                    Some(expected) => {
                        if expected == capacity {
                            match capacity.checked_mul(std::mem::size_of::<ExprSkeletonPath>()) {
                                Some(bytes) => {
                                    match $minimum.checked_mul($crate::cleanup_plan::replay::sequence_vector::SWAP_CONTROLS) {
                                        Some(swap_bytes) => {
                                            match swap_bytes.checked_add($crate::cleanup_plan::replay::sequence_vector::FRAME) {
                                                Some(frame_bytes) => {
                                                    match frame_bytes.checked_add($crate::cleanup_plan::replay::sequence_vector::QUERY) {
                                                        Some(min_saved) => {
                                                            if bytes >= min_saved {
                                                                $crate::cleanup_plan::replay::sequence_vector::transfer!(
                                                                    $prefixes, $suffixes, $work, $share, $precharged, frame_bytes);
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
                                None => {}
                            }
                        }
                    }
                    None => {}
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
#[cfg(test)]
#[path = "sequence_vector/funded_tests.rs"]
pub(super) mod funded_tests;
