//! Lexical one-successor output-vector transfer after the first original work.
use super::*;
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
        $crate::cleanup_plan::replay::physical_budget::compact_try!($work
            .budget
            .reserve_compact_bytes(
                $work.function,
                $frame_bytes,
                "single-successor vector receiving frame",
            ));
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
                $crate::cleanup_plan::replay::final_output::compact::hir_move!($work);
                $crate::cleanup_plan::replay::final_output::compact::hir_work!($work);
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
                $crate::cleanup_plan::replay::final_output::compact::hir_move!($work);
                #[cfg(test)]
                $crate::cleanup_plan::replay::sequence_vector::funded_tests::record_first($work);
                $crate::cleanup_plan::replay::final_output::compact::hir_work!($work);
                $crate::cleanup_plan::replay::note_skeleton_materialization();
            }
        }
        $precharged = true;
        $crate::cleanup_plan::replay::physical_budget::compact_try!($work.budget.reserve_compact_bytes(
            $work.function,
            $crate::cleanup_plan::replay::final_output::compact::VECTOR_QUERY,
            "single-successor vector selection",
        ));
        if !$work.budget.selected() {
            if $minimum <= MAX_REPLAY_PATHS {
                let capacity = $prefixes.capacity();
                match $minimum.checked_next_power_of_two() {
                    Some(expected) if expected == capacity => {
                            match capacity.checked_mul(std::mem::size_of::<ExprSkeletonPath>()) {
                                Some(bytes) => {
                                    match $minimum.checked_mul($crate::cleanup_plan::replay::sequence_vector::SWAP_CONTROLS) {
                                        Some(swap_bytes) => {
                                            match swap_bytes.checked_add($crate::cleanup_plan::replay::final_output::compact::VECTOR_FRAME) {
                                                Some(frame_bytes) => {
                                                    match frame_bytes.checked_add($crate::cleanup_plan::replay::final_output::compact::VECTOR_QUERY) {
                                                        Some(min_saved) if bytes >= min_saved => {
                                                            $crate::cleanup_plan::replay::sequence_vector::transfer!(
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
#[cfg(test)]
#[path = "sequence_vector/funded_tests.rs"]
pub(super) mod funded_tests;

// This caller already owns prefixes while its caller retains the full suffix
// vector. The slice is an actual borrow, never a cheaper claimed owned path.
//
// Observable work is payer-route independent: this route debits exactly the
// ordinary route's total work for the same prefixes/suffixes.
// d70e4af22 shifted rows with `Vec::remove`/`insert` and charged that
// quadratic relocation on the ActiveBuilder route only, so the same program
// could be admitted or refused depending on the payer. The receiver is now a
// `VecDeque` over the same allocation: every row moves a bounded constant
// number of times, so there is no quadratic relocation left to price, and
// selection depends only on the payer and the shape, never on remaining work.
macro_rules! inplace_dispatch {
    ($prefixes:ident, $suffixes:ident, $work:ident) => {{
        if matches!($work.budget.payer, AllocationPayer::ActiveBuilder)
            && !$work.budget.merge_paths
            && $prefixes.len() >= 8
            && $prefixes.len() <= 256
            && $suffixes.len() > 1
            && $suffixes.len() <= 8
            && $crate::cleanup_plan::replay::sequence_vector::inplace_selectable()
        {
            $work.budget.reserve_bytes(
                $work.function,
                $crate::cleanup_plan::replay::sequence_vector::INPLACE_QUERY,
                "inplace path eligibility controls",
            )?;
            $work.budget.reserve_bytes(
                $work.function,
                $crate::cleanup_plan::replay::sequence_vector::INPLACE_BODY,
                "inplace complete path receiving controls",
            )?;
            return $crate::cleanup_plan::replay::sequence_vector::inplace_sequence(
                $prefixes, $suffixes, $work,
            );
        }
    }};
}
pub(super) use inplace_dispatch;

#[cfg(test)]
thread_local! {
    static ORDINARY_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Test-only selector: run `operation` with the inplace representation
/// disabled, so a gate can price the ordinary route on an ActiveBuilder.
#[cfg(test)]
pub(super) fn with_ordinary_route<T>(operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ORDINARY_ONLY.with(|only| only.set(self.0));
        }
    }
    let _restore = Restore(ORDINARY_ONLY.with(|only| only.replace(true)));
    operation()
}

pub(super) fn inplace_selectable() -> bool {
    #[cfg(test)]
    if ORDINARY_ONLY.with(std::cell::Cell::get) {
        return false;
    }
    true
}
// Actual new scan/receiver carriers, including full original paths/results.
// These are storage fees, never a new path, work or allocation allowance.
pub(super) const INPLACE_QUERY: usize = 2 * std::mem::size_of::<Result<(), Diagnostic>>()
    + std::mem::size_of::<std::iter::Enumerate<std::slice::Iter<'static, ExprSkeletonPath>>>()
    + 4 * std::mem::size_of::<usize>()
    + 4 * std::mem::size_of::<bool>();
pub(super) const INPLACE_BODY: usize = 2 * std::mem::size_of::<(
    Vec<ExprSkeletonPath>,
    &[ExprSkeletonPath],
    &mut SkeletonWork<'static, 'static>,
)>() + 2 * std::mem::size_of::<
    Result<Vec<ExprSkeletonPath>, Diagnostic>,
>() + 3 * std::mem::size_of::<ExprSkeletonPath>()
    + 2 * std::mem::size_of::<Observations>()
    + std::mem::size_of::<std::iter::Enumerate<std::slice::Iter<'static, ExprSkeletonPath>>>()
    + 8 * std::mem::size_of::<usize>()
    + 2 * std::mem::size_of::<&mut SkeletonWork<'static, 'static>>()
    + 2 * std::mem::size_of::<Result<(), Diagnostic>>()
    + std::mem::size_of::<VecDeque<ExprSkeletonPath>>();

pub(super) fn inplace_sequence(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    // `From<Vec>` reuses the allocation without moving rows. The deque holds
    // the unprocessed prefixes at its front and finished outputs at its back,
    // so each row leaves the front once and each output enters the back once.
    let mut rows = VecDeque::from(prefixes);
    let pending = rows.len();
    for _ in 0..pending {
        // Each pass pops one of the `pending` original rows, which stay ahead
        // of every output, so the front is always an unprocessed prefix.
        let Some(mut prefix) = rows.pop_front() else {
            break;
        };
        if prefix.failed || prefix.residual {
            work.charge(1, "short-circuited skeleton path")?;
            note_skeleton_materialization();
            work.budget
                .reserve_bytes(work.function, 0, "short-circuited skeleton path")?;
            // The front slot just vacated keeps this push within capacity.
            rows.push_back(prefix);
        } else {
            for (offset, suffix) in suffixes.iter().enumerate() {
                let observations = if offset == 0 {
                    work.charge(1, "skeleton prefix clone")?;
                    skeleton_work::enumerated_prefix::share!(work, prefix);
                    skeleton_work::prefix_fusion::after_work!(
                        work,
                        prefix.observations,
                        suffix.observations
                    )
                } else {
                    let observations = if offset + 1 == suffixes.len() {
                        work.charge(1, "skeleton prefix clone")?;
                        note_skeleton_materialization();
                        std::mem::take(&mut prefix.observations)
                    } else {
                        work.clone_observations(&prefix.observations, "skeleton prefix clone")?
                    };
                    work.extend_owned_observations(
                        observations,
                        &suffix.observations,
                        "skeleton suffix clone",
                    )?
                };
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
                work.budget.reserve_deque(
                    work.function,
                    &mut rows,
                    1,
                    "sequenced skeleton path",
                )?;
                rows.push_back(path);
            }
            drop(prefix);
        }
    }
    // Rotates the ring into place within the same allocation: linear in the
    // already-paid output rows, with no reallocation and no reordering.
    Ok(Vec::from(rows))
}
