//! Caller-funded direct clone and bounded current-HIR continuation query.
use super::*;

struct Bound {
    required: usize,
    forked: bool,
}
struct Query<'a> {
    plan: &'a crate::cleanup_plan::CleanupPlan,
    next: BlockId,
    remaining: usize,
    pushes: usize,
    forked: bool,
    block: Option<&'a crate::cleanup_plan::CleanupBlock>,
    edge: Option<&'a crate::cleanup_plan::CleanupEdge>,
    exit: Option<&'a crate::cleanup_plan::ExitTarget>,
}
// Full new query input actual/formal, result initializer/receiver, state and
// checked-expression receivers. The earlier inline payment owns this bank;
// neither this function nor a future skipped backing payment funds entry.
pub(in crate::cleanup_plan::replay) const QUERY_CONTROLS: usize = size_of::<Query<'static>>()
    + 2 * size_of::<&crate::cleanup_plan::CleanupPlan>()
    + 2 * size_of::<BlockId>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<Option<Bound>>()
    + size_of::<Bound>()
    + 8 * size_of::<Option<usize>>()
    + 8 * size_of::<usize>()
    + 2 * size_of::<Option<&crate::cleanup_plan::CleanupBlock>>()
    + 2 * size_of::<Option<&crate::cleanup_plan::CleanupEdge>>()
    + 2 * size_of::<Option<&crate::cleanup_plan::ExitTarget>>()
    + 2 * size_of::<&Vec<crate::cleanup_plan::CleanupBlock>>()
    + 2 * size_of::<&[crate::cleanup_plan::CleanupBlock]>()
    + 2 * size_of::<&Vec<crate::cleanup_plan::CleanupEdge>>()
    + 2 * size_of::<&[crate::cleanup_plan::CleanupEdge]>()
    + 2 * size_of::<&Vec<crate::cleanup_plan::ExitTarget>>()
    + 2 * size_of::<&[crate::cleanup_plan::ExitTarget]>()
    + 2 * size_of::<&Vec<crate::cleanup_plan::CleanupTransition>>()
    + 2 * size_of::<&Vec<EdgeId>>()
    + 2 * size_of::<&[EdgeId]>()
    + 2 * size_of::<Option<&EdgeId>>()
    + 4 * size_of::<&EdgeId>()
    + 2 * size_of::<&crate::cleanup_plan::CleanupBlock>()
    + 2 * size_of::<&crate::cleanup_plan::CleanupEdge>()
    + 2 * size_of::<&crate::cleanup_plan::ExitTarget>()
    + size_of::<&BlockId>()
    + size_of::<&crate::cleanup_plan::ExitTargetId>()
    + 8 * size_of::<bool>();

fn bound(plan: &crate::cleanup_plan::CleanupPlan, target: BlockId, prefix: usize) -> Option<Bound> {
    let mut query = Query {
        plan,
        next: target,
        remaining: prefix,
        pushes: 0,
        forked: false,
        block: None,
        edge: None,
        exit: None,
    };
    while query.remaining != 0 {
        query.remaining -= 1;
        query.block = query.plan.blocks.get(query.next.0 as usize);
        let block = query.block?;
        query.pushes = query.pushes.checked_add(block.transitions.len())?;
        let edge = match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                query.pushes = query.pushes.checked_add(1)?;
                edge
            }
            CleanupTerminator::Branch(edges) => {
                if query.forked || edges.len() < 2 {
                    return None;
                }
                query.pushes = query.pushes.checked_add(1)?;
                query.forked = true;
                edges.last()?
            }
            CleanupTerminator::Exit(exit) => {
                query.exit = query.plan.exits.get(exit.0 as usize);
                match &query.exit?.continuation {
                    ExitContinuation::CommitResult { .. }
                    | ExitContinuation::ReturnUnit
                    | ExitContinuation::ReturnFailure { .. } => {
                        return Some(Bound {
                            required: prefix.checked_add(1)?.checked_add(query.pushes)?,
                            forked: query.forked,
                        });
                    }
                    // The shipping enumerator follows these exact edges.
                    // A lexical continuation emits at most one marker, whereas
                    // Continue emits none. Neither is a terminal publication.
                    ExitContinuation::FunctionReturn { edge, .. }
                    | ExitContinuation::LoopBreak { edge, .. } => {
                        query.pushes = query.pushes.checked_add(1)?;
                        edge
                    }
                    ExitContinuation::Continue(edge) => edge,
                    ExitContinuation::LoopLatch { .. } => return None,
                }
            }
        };
        query.edge = query.plan.edges.get(edge.0 as usize);
        query.next = query.edge?.to;
    }
    None
}

pub(in crate::cleanup_plan::replay) fn probe(
    input: TakeCandidate<'_>,
) -> production_capacity::Decision {
    #[cfg(test)]
    if super::tests::original_selected() {
        return production_capacity::Decision {
            take: false,
            copy_capacity: None,
        };
    }
    if input.budget.payer != AllocationPayer::ActiveBuilder || input.budget.selected() {
        return production_capacity::Decision {
            take: false,
            copy_capacity: None,
        };
    }
    let bound = bound(input.plan, input.target, input.values.len());
    let growth = input.values.len().checked_add(1).and_then(|required| {
        physical_storage::replay_growth_capacity(input.values.len(), required)
    });
    let (Some(bound), Some(growth)) = (bound, growth) else {
        return production_capacity::Decision {
            take: false,
            copy_capacity: None,
        };
    };
    production_capacity::Decision {
        take: !bound.forked
            && input.remaining_edges == 0
            && input.edge_count >= 2
            && input.values.capacity() > input.values.len()
            && input.values.capacity() <= growth
            && bound.required <= input.values.capacity()
            && growth
                .checked_mul(size_of::<std::rc::Rc<SkeletonObservation>>())
                .is_some_and(|bytes| bytes >= TAKE_BYTES),
        copy_capacity: growth
            .checked_sub(bound.required)
            .filter(|spare| *spare >= production_capacity::COPY_SLOTS)
            .map(|_| bound.required),
    }
}

// The direct generic call is real: all argument/formal receivers and the Vec
// Result initializer/receiver exist. Fund them at the earlier caller payment.
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn copy_controls<T: Clone, V>() -> usize {
    QUERY_CONTROLS
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&Vec<T>>()
        + 2 * size_of::<V>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<&crate::cleanup_plan::CleanupPlan>()
        + 2 * size_of::<BlockId>()
        + 2 * size_of::<Result<Vec<T>, Diagnostic>>()
        + 2 * size_of::<Option<usize>>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<&AllocationPayer>()
        + size_of::<&ReplayBudget>()
        + 2 * size_of::<&Vec<T>>()
        + 4 * size_of::<bool>()
}

// Whole literal old inline census plus the direct bank. This replaces the
// prior zero-argument fee expression without an additional runtime fee call,
// argument, arithmetic helper Result or unpriced receiving usize.
pub(in crate::cleanup_plan::replay) fn inline_controls<T: Clone, V>() -> usize {
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
        + (QUERY_CONTROLS
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&Vec<T>>()
        + 2 * size_of::<V>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<&crate::cleanup_plan::CleanupPlan>()
        + 2 * size_of::<BlockId>()
        + 2 * size_of::<Result<Vec<T>, Diagnostic>>()
        + 2 * size_of::<Option<usize>>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<&AllocationPayer>()
        + size_of::<&ReplayBudget>()
        + 2 * size_of::<&Vec<T>>()
        + 4 * size_of::<bool>())
}

#[track_caller]
pub(in crate::cleanup_plan::replay) fn copy_prepaid<
    T: Clone + ClonePayload,
    V: physical_clone::IntoReplay<T>,
>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    values: &Vec<T>,
    observation: V,
    phase: &str,
    plan: &crate::cleanup_plan::CleanupPlan,
    target: BlockId,
) -> Result<Vec<T>, Diagnostic> {
    let original = values
        .len()
        .checked_add(1)
        .and_then(|required| physical_storage::replay_growth_capacity(values.len(), required));
    let capacity = if budget.payer == AllocationPayer::ActiveBuilder && !budget.selected() {
        #[cfg(test)]
        let enabled = !super::tests::original_selected();
        #[cfg(not(test))]
        let enabled = true;
        if enabled {
            bound(plan, target, values.len()).map(|bound| bound.required)
        } else {
            None
        }
    } else {
        None
    };
    budget.charge_skeleton(function, 1, phase)?;
    budget.reserve_clone(function, values, phase)?;
    note_skeleton_materialization();
    // The immediate push work must refuse before every extra capacity debit,
    // exactly as in the complete predecessor producer.
    budget.charge_skeleton(function, 1, "plan branch observation push")?;
    note_skeleton_materialization();
    let capacity = match (capacity, original) {
        (Some(capacity), Some(original)) if capacity <= original => capacity,
        (_, Some(original)) => original,
        _ => {
            return Err(budget.budget_refusal(
                function,
                "cleanup replay branch capacity overflowed",
                "",
            ));
        }
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
    #[cfg(test)]
    if Some(capacity) != original {
        note_plan_copy();
    }
    Ok(result)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod before;
