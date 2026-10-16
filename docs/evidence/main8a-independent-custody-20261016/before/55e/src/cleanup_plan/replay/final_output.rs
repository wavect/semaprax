//! Fixed receiving controls and lexical observation selection for final-output moves.
use super::{
    Diagnostic, EdgeId, ExprSkeletonPath, Observations, ReplayBudget, ResolvedFunction,
    SkeletonObservation, SkeletonWork,
};
use std::mem::size_of;

// Initial payment precedes all new len/capacity/index receiving values. Some
// paid index slots remain unused when the exact two-edge/spare shape is absent.
pub(super) const PLAN_SELECTION_CONTROLS: usize = size_of::<Result<(), Diagnostic>>()
    + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>() + size_of::<&str>()
    + 6 * size_of::<usize>() // price, edge len, observation len/capacity, two indices
    + 2 * size_of::<bool>() // two-edge and spare predicates
    + 3 * size_of::<&Vec<EdgeId>>() // len and two index receivers
    + 2 * size_of::<&Vec<std::rc::Rc<SkeletonObservation>>>(); // len/capacity

// These are real direct payment/work/auth/conversion receivers, not a helper
// return. No checked growth or replacement array exists in the proven spare slot.
pub(super) const PLAN_MOVE_CONTROLS: usize = 5 * size_of::<Result<(), Diagnostic>>()
    + size_of::<Vec<std::rc::Rc<SkeletonObservation>>>() // moved named receiver
    + 5 * size_of::<&mut ReplayBudget>() + 5 * size_of::<&ResolvedFunction>()
    + 5 * size_of::<&str>()
    + 2 * size_of::<usize>() // price and conversion byte count
    + size_of::<&SkeletonObservation>() // conversion census argument
    + size_of::<&mut Vec<std::rc::Rc<SkeletonObservation>>>(); // actual push receiver

pub(super) const HIR_SELECTION_CONTROLS: usize = size_of::<Result<(), Diagnostic>>()
    + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>() + size_of::<&str>()
    + 2 * size_of::<usize>() // price and length
    + 2 * size_of::<bool>() // predicate initializer/receiving slot
    + size_of::<&std::borrow::Cow<'_, [ExprSkeletonPath]>>()
    + size_of::<&[ExprSkeletonPath]>();

pub(super) const HIR_MOVE_CONTROLS: usize = 2 * size_of::<Result<(), Diagnostic>>()
    + size_of::<Observations>() // existing named result receives a move, no aggregate clone
    + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>() + 2 * size_of::<usize>() // price and direct zero index
    + size_of::<&mut SkeletonWork<'_, '_>>() // retained logical work call
    + size_of::<&std::borrow::Cow<'_, [ExprSkeletonPath]>>() // direct index receiver
    + 2 * size_of::<&[ExprSkeletonPath]>() // dereference and index receivers
    + size_of::<&ExprSkeletonPath>(); // actual suffix binding

macro_rules! observation {
    ($edge:ident, $budget:ident, $function:ident) => {{
        match &$edge.condition {
            EdgeCondition::BooleanResult(expression, value) => SkeletonObservation::Boolean {
                expression: skeleton_clone(
                    $budget,
                    $function,
                    expression,
                    "plan Boolean expression clone",
                )?,
                value: *value,
            },
            EdgeCondition::VariantCase {
                scrutinee,
                case,
                matches,
            } => SkeletonObservation::VariantCase {
                scrutinee: skeleton_clone(
                    $budget,
                    $function,
                    scrutinee,
                    "plan variant scrutinee clone",
                )?,
                case: skeleton_clone($budget, $function, case, "plan variant case clone")?,
                matches: *matches,
            },
            EdgeCondition::ArmSelected {
                scrutinee,
                arm,
                selected,
            } => SkeletonObservation::ArmSelected {
                scrutinee: skeleton_clone(
                    $budget,
                    $function,
                    scrutinee,
                    "plan scalar-match scrutinee clone",
                )?,
                arm: *arm,
                selected: *selected,
            },
            EdgeCondition::StatusZero(source) => SkeletonObservation::Status {
                source: skeleton_clone(
                    $budget,
                    $function,
                    source,
                    "plan zero status-source clone",
                )?,
                success: true,
            },
            EdgeCondition::StatusNonzero(source) => SkeletonObservation::Status {
                source: skeleton_clone(
                    $budget,
                    $function,
                    source,
                    "plan nonzero status-source clone",
                )?,
                success: false,
            },
            EdgeCondition::Always => {
                return Err(replay_error(
                    $function,
                    "branch skeleton contains an unconditional edge",
                ));
            }
        }
    }};
}
pub(super) use observation;

#[cfg(test)]
pub(super) mod before;
#[cfg(test)]
mod tests;

// Allocation-free actual-event observation, never a production allowance.
#[cfg(test)]
std::thread_local! {
    static MOVES: std::cell::Cell<(usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0)) };
}
#[cfg(test)]
pub(super) fn record_move(len: usize, capacity: usize) {
    // The unchanged first sibling has already checked this exact old capacity.
    let removed = if len == 0 {
        1
    } else {
        len.checked_mul(2).unwrap()
    };
    let removed = removed
        .checked_mul(size_of::<std::rc::Rc<SkeletonObservation>>())
        .unwrap();
    MOVES.with(|slot| {
        let (count, bytes, held) = slot.get();
        slot.set((
            count.checked_add(1).unwrap(),
            bytes.checked_add(removed).unwrap(),
            held.checked_add(capacity).unwrap(),
        ));
    });
}
#[cfg(test)]
fn take_moves() -> (usize, usize, usize) {
    MOVES.with(|slot| slot.replace((0, 0, 0)))
}

#[cfg(test)]
#[path = "final_output/before_compaction.rs"]
mod before_compaction;

#[cfg(test)]
#[path = "final_output/before_transfer_current.rs"]
mod before_transfer_current;
#[cfg(test)]
#[path = "final_output/current_observation_tests.rs"]
mod current_observation_tests;
