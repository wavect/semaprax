//! Funded lower bound at the first active multi-suffix output work frontier.
use super::super::{Diagnostic, ExprSkeletonPath, ReplayBudget, ResolvedFunction};
use std::mem::size_of;

// The original path/phase/target locals, original charge/materialization and
// old capacity controls remain in the caller. Only additional probe state is
// charged here, before its first borrow, argument, arithmetic or return exists.
pub(in crate::cleanup_plan::replay) const RECEIVING: usize = size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>()
    + 2 * size_of::<&ReplayBudget>()
    + 2 * size_of::<&std::slice::Iter<'static, ExprSkeletonPath>>()
    + 8 * size_of::<Option<usize>>() // four checked returns and four pattern receivers
    // Two price roles, length return, six helper argument roles, helper return
    // and caller result, seven arithmetic inputs, four Some bindings, four
    // comparison operands, two return arms, and two shadow receiving roles.
    + 30 * size_of::<usize>()
    + 7 * size_of::<bool>(); // selected, comparisons, and branch receiving roles

// The original loop emits at least one output for every prefix. At its first
// active multi-suffix output, the still-held original iterator proves S - 1
// further outputs. No prefix scan, Candidate borrow, owner or Diagnostic exists.
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn capacity(
    prefixes: usize,
    remaining_suffixes: usize,
    old: usize,
) -> usize {
    let Some(count) = prefixes.checked_add(remaining_suffixes) else {
        return old;
    };
    if count > super::super::MAX_REPLAY_PATHS {
        return old;
    }
    let Some(capacity) = count.checked_next_power_of_two() else {
        return old;
    };
    let Some(removed) = capacity.checked_sub(old) else {
        return old;
    };
    let Some(bytes) = removed.checked_mul(size_of::<ExprSkeletonPath>()) else {
        return old;
    };
    if bytes >= RECEIVING {
        capacity
    } else {
        old
    }
}

#[cfg(test)]
macro_rules! push {
    ($work:ident, $first:ident, $minimum:ident, $pending:ident, $target:expr, $path:expr, $phase:expr $(,)?) => {{
        if $first {
            let path = $path;
            let phase = $phase;
            let target: &mut Vec<$crate::cleanup_plan::replay::ExprSkeletonPath> = $target;
            #[cfg(test)]
            $crate::cleanup_plan::replay::path_array::sequence_minimum::tests::record_first(&$work);
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
            // This is a real paid probe even when C == D. The existing first
            // flag already proves the paid, empty-target, P >= 8 route; reaching
            // this macro proves an active first prefix and at least two suffixes.
            $work.budget.reserve_bytes(
                $work.function,
                $crate::cleanup_plan::replay::path_array::sequence_minimum::RECEIVING,
                "sequenced minimum receiving frame",
            )?;
            let capacity = if $work.budget.selected() {
                capacity
            } else {
                $crate::cleanup_plan::replay::path_array::sequence_minimum::capacity(
                    $minimum,
                    $pending.len(),
                    capacity,
                )
            };
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
#[cfg(test)]
pub(in crate::cleanup_plan::replay) use push;

#[cfg(test)]
mod before;
#[cfg(test)]
pub(in crate::cleanup_plan::replay) mod tests;

#[cfg(test)]
mod immediate;
pub(in crate::cleanup_plan::replay) mod optimized;
#[cfg(test)]
pub(in crate::cleanup_plan::replay) mod optimized_tests;
