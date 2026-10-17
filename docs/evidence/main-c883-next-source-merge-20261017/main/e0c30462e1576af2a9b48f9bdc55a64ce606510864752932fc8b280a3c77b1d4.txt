//! A known next push can acquire its existing growth capacity at clone time.
//! This removes the intermediate clone backing; future capacity stays the same.
use super::*;

#[track_caller]
pub(super) fn for_push<T: Clone + ClonePayload>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    values: &Vec<T>,
    phase: &str,
) -> Result<Vec<T>, Diagnostic> {
    #[cfg(test)]
    if tests::original_selected() {
        return skeleton_clone(budget, function, values, phase);
    }
    if !budget.pays_allocations() {
        return skeleton_clone(budget, function, values, phase);
    }
    budget.charge_skeleton(function, 1, phase)?;
    // Keep the complete old clone census, including all nested owned backing
    // and its independently funded oracle work. Nothing is repriced or waived.
    budget.reserve_clone(function, values, phase)?;
    let capacity = values
        .len()
        .checked_add(1)
        .and_then(|required| physical_storage::replay_growth_capacity(values.len(), required))
        .ok_or_else(|| {
            budget.budget_refusal(function, "cleanup replay branch capacity overflowed", "")
        })?;
    budget.reserve_array::<T>(function, capacity - values.len(), phase)?;
    note_skeleton_materialization();
    let mut result = Vec::with_capacity(capacity);
    result.extend(values.iter().cloned());
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
mod tests;
