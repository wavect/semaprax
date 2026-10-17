//! Fused history clone and its known immediate push, with paid new controls.
use super::*;
use std::mem::size_of;

// The producer is physically in the enumerator. Its outer Vec Result and
// owned observation argument no longer exist; all nested Result controls stay.
// One budget/function reference pair pays the caller's reservation boundary.
// The payload-reference term remains for the direct clone census argument.
pub(super) fn inline_controls<T: Clone>() -> usize {
    size_of::<Result<(), Diagnostic>>() // caller payment result
        + 4 * size_of::<Result<(), Diagnostic>>() // clone/push work, clone census, conversion
        + 2 * size_of::<Result<(), Diagnostic>>() // backing wrapper and nested byte payment
        + size_of::<Result<usize, Diagnostic>>() // checked array size conversion
        + size_of::<Option<usize>>()
        + 2 * size_of::<Vec<T>>() // initializer and the enumerator's named branch
        + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>()
        + size_of::<&Vec<T>>() // direct clone census argument
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<&mut Vec<T>>() + size_of::<&[T]>()
        + 2 * size_of::<std::slice::Iter<'_, T>>()
        + 2 * size_of::<std::iter::Cloned<std::slice::Iter<'_, T>>>()
}

// Paid by the enumerator before evaluating the new helper's arguments. The
// original clone/push controls are not credited against these actual new sites.
#[cfg(test)]
pub(super) fn controls<T: Clone, V>() -> usize {
    size_of::<Result<Vec<T>, Diagnostic>>() // fused receiving result
        + size_of::<Result<(), Diagnostic>>() // caller payment result
        + 4 * size_of::<Result<(), Diagnostic>>() // clone/push work, clone census, conversion
        + 2 * size_of::<Result<(), Diagnostic>>() // backing wrapper and nested byte payment
        + size_of::<Result<usize, Diagnostic>>() // reserve_array checked byte-size conversion
        + size_of::<Option<usize>>() // reserve_array checked multiplication
        + 2 * size_of::<Vec<T>>() // initializer and named local receiver
        + 2 * size_of::<&mut ReplayBudget>() + 2 * size_of::<&ResolvedFunction>()
        + size_of::<&Vec<T>>() + size_of::<V>() // new argument receivers
        + 2 * size_of::<&str>() // fixed phase receivers
        + 2 * size_of::<usize>() // controls() receiving value and capacity local
        + 2 * size_of::<bool>() // empty and arithmetic overflow branch receivers
        + 2 * size_of::<&mut Vec<T>>() // extend and push method receivers
        + size_of::<&[T]>() // iter slice receiver
        + 2 * size_of::<std::slice::Iter<'_, T>>() // iterator initializer and cloned receiver
        + 2 * size_of::<std::iter::Cloned<std::slice::Iter<'_, T>>>() // initializer and extend receiver
}

#[cfg(test)]
#[track_caller]
pub(super) fn with_observation<T: Clone + ClonePayload, V: physical_clone::IntoReplay<T>>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    values: &Vec<T>,
    observation: V,
) -> Result<Vec<T>, Diagnostic> {
    if !budget.pays_allocations() {
        let mut branch = skeleton_clone(
            budget,
            function,
            values,
            "cleanup-plan skeleton branch clone",
        )?;
        skeleton_push(
            budget,
            function,
            &mut branch,
            observation,
            "plan branch observation push",
        )?;
        return Ok(branch);
    }
    budget.charge_skeleton(function, 1, "cleanup-plan skeleton branch clone")?;
    budget.reserve_clone(function, values, "cleanup-plan skeleton branch clone")?;
    note_skeleton_materialization();
    // The original push work refusal must win before any extra capacity debit.
    budget.charge_skeleton(function, 1, "plan branch observation push")?;
    note_skeleton_materialization();
    let capacity = if values.is_empty() {
        1
    } else {
        if values.len() > usize::MAX / 2 {
            return Err(budget.budget_refusal(
                function,
                "cleanup replay branch capacity overflowed",
                "",
            ));
        }
        values.len() * 2
    };
    budget.reserve_array::<T>(
        function,
        capacity - values.len(),
        "plan branch observation push",
    )?;
    budget.reserve_bytes(
        function,
        physical_clone::conversion_bytes::<T, _>(&observation),
        "plan branch observation push",
    )?;
    let mut result = Vec::with_capacity(capacity);
    result.extend(values.iter().cloned());
    result.push(observation.into());
    Ok(result)
}

/// A pure probe for transferring the final branch's already funded prefix.
pub(super) struct TakeCandidate<'a> {
    pub(super) values: &'a Vec<std::rc::Rc<SkeletonObservation>>,
    pub(super) remaining_edges: usize,
    pub(super) edge_count: usize,
    pub(super) budget: &'a ReplayBudget,
    pub(super) plan: &'a crate::cleanup_plan::CleanupPlan,
    pub(super) target: BlockId,
}
struct TakeBounds<'a> {
    required: Option<usize>,
    capacity: Option<usize>,
    block: Option<&'a crate::cleanup_plan::CleanupBlock>,
    exit: Option<&'a crate::cleanup_plan::ExitTarget>,
    next: BlockId,
    spare: usize,
    remaining: usize,
}
// Complete caller/callee headers, checked-bound state and temporary results,
// growth operands, borrowed block/exit/edge receivers and both bool results.
// No existing Vec/Result owner is credited against these new layouts.
pub(super) const TAKE_BYTES: usize = 2 * std::mem::size_of::<TakeCandidate<'static>>()
    + std::mem::size_of::<TakeBounds<'static>>()
    + 4 * std::mem::size_of::<Option<usize>>()
    + 4 * std::mem::size_of::<usize>()
    + 4 * std::mem::size_of::<&crate::cleanup_plan::CleanupBlock>()
    + 2 * std::mem::size_of::<bool>();

// The real enumerator pays BEFORE TakeCandidate/length/policy receivers.
// TAKE_BYTES retains the original Candidate/Bounds and checked arithmetic
// layout. No later clone or skipped backing is credited to this payment.
pub(super) const TAKE_CALL_CONTROLS: usize = TAKE_BYTES
    + 2 * size_of::<Result<(), Diagnostic>>() // receiving reservation and nested payment
    + 2 * size_of::<&mut ReplayBudget>() + 2 * size_of::<&ResolvedFunction>() // reserve actual/formal
    + 2 * size_of::<&str>() + 2 * size_of::<usize>() // phase and byte request actual/formal
    + size_of::<&std::slice::Iter<'static, EdgeId>>() // remaining-edge len receiver
    + size_of::<&Vec<EdgeId>>() // total edge len receiver
    + 2 * size_of::<&ReplayBudget>() // selected() actual/formal receiver
    + 4 * size_of::<&AllocationPayer>() // two PartialEq actuals and two formals
    + 16 * size_of::<&Vec<std::rc::Rc<SkeletonObservation>>>() // eight len/capacity actual/formal pairs
    + 2 * size_of::<&Vec<crate::cleanup_plan::CleanupBlock>>() // block Vec dereference actual/formal
    + 2 * size_of::<&[crate::cleanup_plan::CleanupBlock]>() // get actual/formal
    + 2 * size_of::<&Vec<crate::cleanup_plan::CleanupTransition>>() // transition len actual/formal
    + 2 * size_of::<&Vec<crate::cleanup_plan::CleanupEdge>>() // Vec dereference actual/formal
    + 2 * size_of::<&[crate::cleanup_plan::CleanupEdge]>() // get actual/formal
    + 2 * size_of::<&Vec<crate::cleanup_plan::ExitTarget>>() // Vec dereference actual/formal
    + 2 * size_of::<&[crate::cleanup_plan::ExitTarget]>() // get actual/formal
    + size_of::<Option<&crate::cleanup_plan::CleanupEdge>>() // get result
    + size_of::<&crate::cleanup_plan::CleanupEdge>() // successful edge binding
    + size_of::<Option<&crate::cleanup_plan::ExitTarget>>() // copied exit match
    + size_of::<&crate::cleanup_plan::ExitTarget>() // successful exit binding
    + size_of::<&BlockId>() + size_of::<&crate::cleanup_plan::ExitTargetId>() // terminator borrowed patterns
    + 5 * size_of::<usize>() // transition length, three indexes, multiplication input
    + 14 * size_of::<bool>() // lazy eligibility5+aggregate, capacity5, loop, matches, caller condition
    + size_of::<&mut Vec<std::rc::Rc<SkeletonObservation>>>(); // take receiver

#[allow(dead_code, reason = "complete independently retained pre-fusion TAKE predicate")]
pub(super) fn can_take(input: TakeCandidate<'_>) -> bool {
    #[cfg(test)]
    if tests::original_selected() {
        return false;
    }
    if input.budget.payer != AllocationPayer::ActiveBuilder
        || input.budget.selected()
        || input.remaining_edges != 0
        || input.edge_count < 2
        || input.values.capacity() <= input.values.len()
    {
        return false;
    }
    let mut bounds = TakeBounds {
        required: input.values.len().checked_add(1),
        capacity: None,
        block: None,
        exit: None,
        next: input.target,
        spare: input.values.capacity() - input.values.len() - 1,
        remaining: input.values.len(),
    };
    let Some(required) = bounds.required else {
        return false;
    };
    bounds.capacity = physical_storage::replay_growth_capacity(input.values.len(), required);
    let Some(capacity) = bounds.capacity else {
        return false;
    };
    if input.values.capacity() > capacity
        || !capacity
            .checked_mul(std::mem::size_of::<std::rc::Rc<SkeletonObservation>>())
            .is_some_and(|bytes| bytes >= TAKE_BYTES)
    {
        return false;
    }
    // The earlier branch has already cloned this entire prefix. Bound this
    // read-only shape probe by that existing prefix length; exhaustion merely
    // keeps the original clone route, with no new admission limit or quota.
    while bounds.remaining != 0 {
        bounds.remaining -= 1;
        bounds.block = input.plan.blocks.get(bounds.next.0 as usize);
        let Some(block) = bounds.block else {
            return false;
        };
        // Every transition emits at most one observation. Counting even the
        // ignored cleanup transitions conservatively bounds all future growth.
        bounds.required = bounds.spare.checked_sub(block.transitions.len());
        let Some(spare) = bounds.required else {
            return false;
        };
        bounds.spare = spare;
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                bounds.required = bounds.spare.checked_sub(1);
                let Some(spare) = bounds.required else {
                    return false;
                };
                bounds.spare = spare;
                let Some(edge) = input.plan.edges.get(edge.0 as usize) else {
                    return false;
                };
                bounds.next = edge.to;
            }
            CleanupTerminator::Exit(exit) => {
                bounds.exit = input.plan.exits.get(exit.0 as usize);
                let Some(exit) = bounds.exit else {
                    return false;
                };
                return matches!(
                    exit.continuation,
                    ExitContinuation::CommitResult { .. }
                        | ExitContinuation::ReturnUnit
                        | ExitContinuation::ReturnFailure { .. }
                );
            }
            CleanupTerminator::Branch(_) => return false,
        }
    }
    false
}

#[cfg(test)]
pub(super) fn note_take() {
    tests::note_take();
}
#[cfg(test)]
pub(super) fn note_generic_take() {
    tests::note_generic_take();
}

#[cfg(test)]
pub(in crate::cleanup_plan::replay) mod before;
#[cfg(test)]
pub(in crate::cleanup_plan::replay) use before::for_push;
#[cfg(test)]
use before::{main_replay_input, plan_skeleton_paths};
#[cfg(test)]
mod funded_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod inline_tests;

#[cfg(test)]
mod before_inline;

pub(in crate::cleanup_plan::replay) mod production_capacity;
