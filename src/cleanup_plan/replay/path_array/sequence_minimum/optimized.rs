//! Smaller paid no-growth header at the unchanged lexical first-output frontier.
use super::super::super::{Diagnostic, ExprSkeletonPath, ReplayBudget, ResolvedFunction};
use std::mem::size_of;

// Complete new payment ABI plus selected/iterator references. Scalar roles:
// two price roles, length return/binding, checked-sub operands, Some binding,
// comparison operands, two no-query return arms and the capacity shadow.
// Boolean roles: selected return/condition, comparison/Some arm, None arm,
// match initializer/receiver and branch condition. No helper arguments exist.
pub(in crate::cleanup_plan::replay) const HEADER: usize = size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>()
    + 2 * size_of::<&ReplayBudget>()
    + 2 * size_of::<&std::slice::Iter<'static, ExprSkeletonPath>>()
    + 2 * size_of::<Option<usize>>()
    + 12 * size_of::<usize>()
    + 8 * size_of::<bool>();
pub(in crate::cleanup_plan::replay) const TOTAL: usize = HEADER + super::RECEIVING;

pub(in crate::cleanup_plan::replay) fn capacity(
    prefixes: usize,
    remaining_suffixes: usize,
    old: usize,
) -> usize {
    let Some(count) = prefixes.checked_add(remaining_suffixes) else {
        return old;
    };
    if count > super::super::super::MAX_REPLAY_PATHS {
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
    if bytes >= TOTAL {
        capacity
    } else {
        old
    }
}

macro_rules! push {
    ($work:ident, $first:ident, $minimum:ident, $pending:ident, $target:expr, $path:expr, $phase:expr $(,)?) => {{
        if $first {
            let path = $path;
            let phase = $phase;
            let target: &mut Vec<$crate::cleanup_plan::replay::ExprSkeletonPath> = $target;
            #[cfg(test)]
            $crate::cleanup_plan::replay::path_array::sequence_minimum::optimized_tests::record_first(&$work);
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
            // New scalar header is paid before selected, iterator length,
            // checked subtraction, boolean, or any new return carrier exists.
            $work.budget.reserve_bytes(
                $work.function,
                $crate::cleanup_plan::replay::path_array::sequence_minimum::optimized::HEADER,
                "sequenced minimum receiving frame",
            )?;
            let capacity = if $work.budget.selected() {
                capacity
            } else {
                let remaining = $pending.len();
                let can_grow = match capacity.checked_sub($minimum) {
                    Some(room) => remaining > room,
                    None => true,
                };
                if can_grow {
                    // Conservatively retain the complete old query envelope.
                    // The capacity predicate covers BOTH actually paid frames.
                    $work.budget.reserve_bytes(
                        $work.function,
                        $crate::cleanup_plan::replay::path_array::sequence_minimum::RECEIVING,
                        "sequenced minimum receiving frame",
                    )?;
                    $crate::cleanup_plan::replay::path_array::sequence_minimum::optimized::capacity(
                        $minimum,
                        remaining,
                        capacity,
                    )
                } else {
                    capacity
                }
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
pub(in crate::cleanup_plan::replay) use push;
