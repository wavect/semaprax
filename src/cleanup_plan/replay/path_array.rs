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

#[cfg(test)]
mod before;
#[cfg(test)]
mod funded_tests;
#[cfg(test)]
pub(super) mod main696;
#[cfg(test)]
mod tests;
