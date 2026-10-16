//! Known-minimum path capacity at the original first output work frontier.
use super::{Diagnostic, ExprSkeletonPath};
use std::mem::size_of;

// New cold scalar/flag acquisition, before count/selection initialization.
// The caller keeps its original Diagnostic failure ABI on an initial refusal.
pub(super) const fn count_controls<const SEQUENCED: bool>() -> usize {
    size_of::<Result<(), Diagnostic>>() // metadata payment
        + 4 * size_of::<usize>() // count initializer/receiver, price argument, capacity return
        + 6 * size_of::<bool>() // held slot, false init, two comparisons, payer bool, final false
        + if SEQUENCED { size_of::<bool>() } else { 0 }
        + 2 * size_of::<&Vec<ExprSkeletonPath>>() // header borrow initializer and capacity receiver
        + size_of::<&mut super::ReplayBudget>()
        + size_of::<&super::ResolvedFunction>()
        + size_of::<&str>()
}
// Only the selected first output initializes these arithmetic/payment controls.
// Match removes the old checked-next-power Diagnostic-bearing Result entirely.
pub(super) const fn capacity_controls() -> usize {
    size_of::<Result<(), Diagnostic>>() // selected metadata payment
        + 2 * size_of::<Option<usize>>() // checked power result and match receiver
        + 4 * size_of::<usize>() // checked-power receiver copy, Some binding, capacity initializer/receiver
        + size_of::<usize>() // price argument
        + size_of::<&mut super::ReplayBudget>()
        + size_of::<&super::ResolvedFunction>()
        + size_of::<&str>()
}
macro_rules! count {
    ($work:ident, $sequenced:literal, $count:expr, $phase:expr) => {{
        $work.budget.reserve_bytes(
            $work.function,
            $crate::cleanup_plan::replay::path_array::count_controls::<$sequenced>(),
            $phase,
        )?;
        $count
    }};
}
pub(super) use count;
macro_rules! select {
    ($first:ident, $minimum:ident, $target:expr, $work:ident) => {
        let mut $first = false;
        if $minimum >= 8 {
            if ($target).capacity() == 0 {
                $first = $work.budget.pays_allocations();
            }
        }
    };
}
pub(super) use select;
macro_rules! push {
    ($work:ident, $first:ident, $minimum:ident, $target:expr, $path:expr, $phase:expr $(,)?) => {{
        if $first {
            // These locals replace the original push method's argument receivers.
            // Evaluate the original owned path expression before its work debit.
            let path = $path;
            let phase = $phase;
            let target: &mut Vec<$crate::cleanup_plan::replay::ExprSkeletonPath> = $target;
            $work.charge(1, phase)?;
            $crate::cleanup_plan::replay::note_skeleton_materialization();
            $work.budget.reserve_bytes(
                $work.function,
                $crate::cleanup_plan::replay::path_array::capacity_controls(),
                phase,
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
            // This is the original push's reserve_vec boundary, with its known
            // first capacity. Every nested Result/authentication remains real.
            $work
                .budget
                .reserve_vec($work.function, target, capacity, phase)?;
            target.push(path);
            $first = false;
        } else {
            $work.push_expr_path($target, $path, $phase)?;
        }
    }};
}
pub(super) use push;

// Initial count selection is reached only after the original first output's
// owned path, work debit and materialization. No first flag/capacity probe or
// finished-path helper argument/result exists before that frontier.
pub(super) const COMPLETED_COUNT_CONTROLS: usize = size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<Option<usize>>() // checked-add result and match
    + 6 * size_of::<usize>() // price, len return, add self/RHS, Some binding, count
    + 2 * size_of::<&std::vec::IntoIter<ExprSkeletonPath>>() // len caller/method
    + 2 * size_of::<&super::ReplayBudget>() // payer caller/method
    + 2 * size_of::<bool>() // count threshold and payer result
    + size_of::<&mut super::ReplayBudget>()
    + size_of::<&super::ResolvedFunction>()
    + size_of::<&str>();
const COMPLETED_OPTIONAL_CONTROLS: usize = COMPLETED_COUNT_CONTROLS + capacity_controls();
const COMPLETED_CAPACITY: usize =
    (COMPLETED_OPTIONAL_CONTROLS.div_ceil(size_of::<super::SkeletonPath>()) + 1)
        .next_power_of_two();
pub(super) const COMPLETED_MINIMUM: usize = if COMPLETED_CAPACITY / 2 + 1 > 8 {
    COMPLETED_CAPACITY / 2 + 1
} else {
    8
};
macro_rules! completed_first {
    ($work:ident, $remaining:ident, $target:ident, $path:expr, $phase:expr) => {{
        // These replace the original push_skeleton_path owned path/phase
        // parameters; neither is a new forwarding carrier.
        let path = $path;
        let phase = $phase;
        $work.charge(1, phase)?;
        $crate::cleanup_plan::replay::note_skeleton_materialization();
        $work.budget.reserve_bytes(
            $work.function,
            $crate::cleanup_plan::replay::path_array::COMPLETED_COUNT_CONTROLS,
            phase,
        )?;
        let count = match $remaining.len().checked_add(1) {
            Some(count) => count,
            None => {
                return Err($work.budget.budget_refusal(
                    $work.function,
                    "cleanup replay completed path count overflowed",
                    "",
                ))
            }
        };
        if count >= $crate::cleanup_plan::replay::path_array::COMPLETED_MINIMUM {
            if $work.budget.pays_allocations() {
                $work.budget.reserve_bytes(
                    $work.function,
                    $crate::cleanup_plan::replay::path_array::capacity_controls(),
                    phase,
                )?;
                let capacity = match count.checked_next_power_of_two() {
                    Some(capacity) => capacity,
                    None => {
                        return Err($work.budget.budget_refusal(
                            $work.function,
                            "cleanup replay minimum path capacity overflowed",
                            "",
                        ))
                    }
                };
                $work
                    .budget
                    .reserve_vec($work.function, &mut $target, capacity, phase)?;
            } else {
                $work
                    .budget
                    .reserve_vec($work.function, &mut $target, 1, phase)?;
            }
        } else {
            $work
                .budget
                .reserve_vec($work.function, &mut $target, 1, phase)?;
        }
        $target.push(path);
    }};
}
pub(super) use completed_first;

#[cfg(test)]
mod before;
#[cfg(test)]
mod funded_tests;
#[cfg(test)]
pub(super) mod main696;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod completed_tests;

pub(super) mod sequence_minimum;
