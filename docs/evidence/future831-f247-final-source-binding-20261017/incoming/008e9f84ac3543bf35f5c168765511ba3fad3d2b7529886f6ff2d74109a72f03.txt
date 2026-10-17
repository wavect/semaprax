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

/// The existing clone, using a smaller backing only when its entire continuation
/// through the next fork and that fork's final terminal path will fit.
#[track_caller]
pub(super) fn for_plan_push<T: Clone + ClonePayload>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    values: &Vec<T>,
    phase: &str,
    plan: &crate::cleanup_plan::CleanupPlan,
    target: BlockId,
) -> Result<Vec<T>, Diagnostic> {
    #[cfg(test)]
    if tests::original_selected() {
        return for_push(budget, function, values, phase);
    }
    if budget.payer != AllocationPayer::ActiveBuilder || budget.selected() {
        return for_push(budget, function, values, phase);
    }
    let capacity = plan_capacity(plan, target, values.len());
    let original = values
        .len()
        .checked_add(1)
        .and_then(|required| physical_storage::replay_growth_capacity(values.len(), required));
    let (Some(capacity), Some(original)) = (capacity, original) else {
        return for_push(budget, function, values, phase);
    };
    if !original
        .checked_sub(capacity)
        .and_then(|spare| spare.checked_mul(std::mem::size_of::<T>()))
        .is_some_and(|saved| saved >= PLAN_PROBE_BYTES)
    {
        return for_push(budget, function, values, phase);
    }
    // Retain the complete clone census and its independently funded work.
    // The extra probe state is paid in addition to every real backing byte.
    budget.charge_skeleton(function, 1, phase)?;
    budget.reserve_array::<u8>(function, PLAN_PROBE_BYTES, phase)?;
    budget.reserve_clone(function, values, phase)?;
    budget.reserve_array::<T>(function, capacity - values.len(), phase)?;
    note_skeleton_materialization();
    let mut result = Vec::with_capacity(capacity);
    result.extend(values.iter().cloned());
    #[cfg(test)]
    tests::note_plan_copy();
    Ok(result)
}

struct PlanCapacityProbe<'a> {
    plan: &'a crate::cleanup_plan::CleanupPlan,
    next: BlockId,
    remaining: usize,
    pushes: usize,
    final_path: bool,
    block: Option<&'a crate::cleanup_plan::CleanupBlock>,
    edge: Option<&'a crate::cleanup_plan::CleanupEdge>,
    exit: Option<&'a crate::cleanup_plan::ExitTarget>,
}
// Complete probe state, caller/callee input headers and result slots, checked
// arithmetic results, clone-capacity operands, borrowed receivers and booleans.
pub(super) const PLAN_PROBE_BYTES: usize = std::mem::size_of::<PlanCapacityProbe<'static>>()
    + 2 * std::mem::size_of::<(&crate::cleanup_plan::CleanupPlan, BlockId, usize)>()
    + 2 * std::mem::size_of::<Option<usize>>()
    + 4 * std::mem::size_of::<Option<usize>>()
    + 4 * std::mem::size_of::<usize>()
    + 3 * std::mem::size_of::<&crate::cleanup_plan::CleanupBlock>()
    + 2 * std::mem::size_of::<bool>();

pub(super) fn plan_capacity(
    plan: &crate::cleanup_plan::CleanupPlan,
    target: BlockId,
    prefix: usize,
) -> Option<usize> {
    let mut probe = PlanCapacityProbe {
        plan,
        next: target,
        remaining: prefix,
        pushes: 0,
        final_path: false,
        block: None,
        edge: None,
        exit: None,
    };
    // This bounded, read-only shape test can only select the old clone route
    // on exhaustion. It introduces no program admission limit or work account.
    while probe.remaining != 0 {
        probe.remaining -= 1;
        probe.block = probe.plan.blocks.get(probe.next.0 as usize);
        let block = probe.block?;
        // Count ignored transitions too: every transition emits at most one
        // top-level observation in the unchanged plan enumerator.
        probe.pushes = probe.pushes.checked_add(block.transitions.len())?;
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                probe.pushes = probe.pushes.checked_add(1)?;
                probe.edge = probe.plan.edges.get(edge.0 as usize);
                probe.next = probe.edge?.to;
            }
            CleanupTerminator::Branch(edges) => {
                if probe.final_path || edges.len() < 2 {
                    return None;
                }
                // Earlier forks clone the source unchanged. Reserve its final
                // branch observation and complete terminal suffix, so the old
                // final-prefix transfer still fits without growing this backing.
                probe.pushes = probe.pushes.checked_add(1)?;
                probe.edge = probe.plan.edges.get(edges.last()?.0 as usize);
                probe.next = probe.edge?.to;
                probe.final_path = true;
            }
            CleanupTerminator::Exit(exit) => {
                probe.exit = probe.plan.exits.get(exit.0 as usize);
                return match probe.exit?.continuation {
                    ExitContinuation::CommitResult { .. }
                    | ExitContinuation::ReturnUnit
                    | ExitContinuation::ReturnFailure { .. } => {
                        prefix.checked_add(1)?.checked_add(probe.pushes)
                    }
                    _ => None,
                };
            }
        }
    }
    None
}

#[cfg(test)]
pub(super) fn note_take() {
    tests::note_take();
}

#[cfg(test)]
mod tests;
