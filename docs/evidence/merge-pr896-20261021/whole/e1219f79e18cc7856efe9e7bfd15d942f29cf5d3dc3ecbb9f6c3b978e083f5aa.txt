//! One funded production TAKE/copy shape traversal; no direct-clone authority.
use super::*;
#[cfg(test)]
mod before;
#[cfg(test)]
mod main_c883;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) struct Decision {
    pub(in crate::cleanup_plan::replay) take: bool,
    pub(in crate::cleanup_plan::replay) copy_capacity: Option<usize>,
}
#[cfg(test)]
impl Decision {
    const NONE: Self = Self {
        take: false,
        copy_capacity: None,
    };
}

// Added to the earlier same-invocation branch-inline payment, before clone work
// and the unchanged TAKE payment. No new reserve ABI or old-price credit exists.
pub(in crate::cleanup_plan::replay) const ADDITIONAL_CONTROLS: usize = 2 * size_of::<Decision>() // actual returned value and caller-retained receiver
    + 4 * size_of::<Option<usize>>() // terminal Some/receiver and caller field/match
    + 4 * size_of::<usize>() // terminal operands/value and selected-copy binding
    + 3 * size_of::<&Vec<EdgeId>>() // Branch pattern, as_slice actual/formal
    + 2 * size_of::<&[EdgeId]>() // as_slice return and slice-pattern receiver
    + 4 * size_of::<&EdgeId>() // Goto/Branch bindings and common selected-edge receive
    + 10 * size_of::<bool>(); // fork/cache, branch/exit/fit/copy predicates and receives
                              // Same zero-argument ABI and complete literal old pricing expression; no
                              // forwarding call/Result is introduced. The earlier reserve retains this bank
                              // on the same budget/function/invocation until the actual probe is entered.
pub(in crate::cleanup_plan::replay) fn production_inline_controls<T: Clone>() -> usize {
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
        + ADDITIONAL_CONTROLS
        + direct::PROBE_CONTROLS
}

// Retain the original added-production-price threshold. The current query
// bank is separately paid before entry; net application gains remain pending.
pub(super) const COPY_SLOTS: usize =
    ADDITIONAL_CONTROLS / size_of::<std::rc::Rc<SkeletonObservation>>() + 1;

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn probe(input: TakeCandidate<'_>) -> Decision {
    #[cfg(test)]
    if super::tests::original_selected() {
        return Decision::NONE;
    }
    if input.budget.payer != AllocationPayer::ActiveBuilder || input.budget.selected() {
        return Decision::NONE;
    }
    let mut bounds = TakeBounds {
        required: input.values.len().checked_add(1),
        capacity: None,
        block: None,
        exit: None,
        next: input.target,
        spare: 0,
        remaining: input.values.len(),
    };
    let Some(required) = bounds.required else {
        return Decision::NONE;
    };
    bounds.capacity = physical_storage::replay_growth_capacity(input.values.len(), required);
    let Some(capacity) = bounds.capacity else {
        return Decision::NONE;
    };
    // growth_capacity guarantees capacity >= required. The same spare owner
    // now counts down from the clone growth bound, rather than the held Vec.
    bounds.spare = capacity - required;
    let take_eligible = input.remaining_edges == 0
        && input.edge_count >= 2
        && input.values.capacity() > input.values.len()
        && input.values.capacity() <= capacity
        && capacity
            .checked_mul(size_of::<std::rc::Rc<SkeletonObservation>>())
            .is_some_and(|bytes| bytes >= TAKE_BYTES);
    let mut forked = false;
    while bounds.remaining != 0 {
        bounds.remaining -= 1;
        bounds.block = input.plan.blocks.get(bounds.next.0 as usize);
        let Some(block) = bounds.block else {
            return Decision::NONE;
        };
        bounds.required = bounds.spare.checked_sub(block.transitions.len());
        let Some(spare) = bounds.required else {
            return Decision::NONE;
        };
        bounds.spare = spare;
        // Both Goto and the one possible final Branch select one borrowed ID.
        // There is exactly one actual edge Vec/slice/get receiver below.
        let selected_edge = match &block.terminator {
            CleanupTerminator::Goto(edge) => edge,
            CleanupTerminator::Branch(edges) => {
                if forked {
                    return Decision::NONE;
                }
                let [_, .., last] = edges.as_slice() else {
                    return Decision::NONE;
                };
                forked = true;
                last
            }
            CleanupTerminator::Exit(exit) => {
                bounds.exit = input.plan.exits.get(exit.0 as usize);
                let Some(exit) = bounds.exit else {
                    return Decision::NONE;
                };
                if !matches!(
                    exit.continuation,
                    ExitContinuation::CommitResult { .. }
                        | ExitContinuation::ReturnUnit
                        | ExitContinuation::ReturnFailure { .. }
                ) {
                    return Decision::NONE;
                }
                // spare only decreased by checked_sub, so this cannot wrap.
                bounds.required = Some(capacity - bounds.spare);
                let Some(required) = bounds.required else {
                    return Decision::NONE;
                };
                return Decision {
                    take: take_eligible && !forked && required <= input.values.capacity(),
                    copy_capacity: if bounds.spare >= COPY_SLOTS {
                        Some(required)
                    } else {
                        None
                    },
                };
            }
        };
        bounds.required = bounds.spare.checked_sub(1);
        let Some(spare) = bounds.required else {
            return Decision::NONE;
        };
        bounds.spare = spare;
        let Some(edge) = input.plan.edges.get(selected_edge.0 as usize) else {
            return Decision::NONE;
        };
        bounds.next = edge.to;
    }
    Decision::NONE
}

// All arguments are lexical identifiers, not new runtime forwarding owners.
macro_rules! produce {
    ($budget:ident, $function:ident, $observations:ident, $observation:ident,
     $plan:ident, $edge:ident, $remaining_edges:ident, $edges:ident,
     $branch:ident, $queue:ident) => {{
        let decision = $crate::cleanup_plan::replay::branch_clone::direct::probe(
            $crate::cleanup_plan::replay::branch_clone::TakeCandidate {
                values: &$observations,
                remaining_edges: $remaining_edges.len(),
                edge_count: $edges.len(),
                budget: $budget,
                plan: $plan,
                target: $edge.to,
            },
        );
        if decision.take {
            note_skeleton_materialization();
            $budget.charge_skeleton($function, 1, "plan branch observation push")?;
            note_skeleton_materialization();
            $budget.reserve_bytes($function, 0, "plan branch observation push")?;
            $budget.reserve_bytes(
                $function,
                physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(
                    &$observation,
                ),
                "plan branch observation push",
            )?;
            #[cfg(test)]
            branch_clone::note_take();
            #[cfg(test)]
            branch_clone::note_generic_take();
            $branch = std::mem::take(&mut $observations);
            $branch.push($observation.into());
            skeleton_queue_push(
                $budget,
                $function,
                &mut $queue,
                ($edge.to, $branch),
                "plan branch state push",
            )?;
            continue;
        }
        $budget.reserve_clone(
            $function,
            &$observations,
            "cleanup-plan skeleton branch clone",
        )?;
        note_skeleton_materialization();
        // Original push work precedes every extra backing debit.
        $budget.charge_skeleton($function, 1, "plan branch observation push")?;
        note_skeleton_materialization();
        let capacity = if let Some(capacity) = decision.copy_capacity {
            capacity
        } else if $observations.is_empty() {
            1
        } else {
            if $observations.len() > usize::MAX / 2 {
                return Err($budget.budget_refusal(
                    $function,
                    "cleanup replay branch capacity overflowed",
                    "",
                ));
            }
            $observations.len() * 2
        };
        $budget.reserve_array::<std::rc::Rc<SkeletonObservation>>(
            $function,
            capacity - $observations.len(),
            "plan branch observation push",
        )?;
        $budget.reserve_bytes(
            $function,
            physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(&$observation),
            "plan branch observation push",
        )?;
        $branch = Vec::with_capacity(capacity);
        $branch.extend($observations.iter().cloned());
        $branch.push($observation.into());
        // Count only the successfully filled bounded-copy Vec.
        // TAKE, shape probes and original fallback do not count.
        #[cfg(test)]
        if decision.copy_capacity.is_some() {
            branch_clone::note_plan_copy();
        }
        skeleton_queue_push(
            $budget,
            $function,
            &mut $queue,
            ($edge.to, $branch),
            "plan branch state push",
        )?;
        continue;
    }};
}
pub(in crate::cleanup_plan::replay) use produce;
