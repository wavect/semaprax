//! Compare the full expected prefix without constructing owned ancestry results.
use super::*;

// One existing authority-query frame is replaced, without a wider payer copy.
const _: () = assert!(
    std::mem::size_of::<&AllocationPayer>() + std::mem::size_of::<bool>()
        <= std::mem::size_of::<&ReplayBudget>() + std::mem::size_of::<bool>()
);
// The selected call replaces expected_exit_regions at the same lexical site.
// Its actual borrowed arguments are identical; its returned/received bool
// result and comparison fit the old result/received Vec header and comparison.
// This is a header counterpart only: no removed Vec allocation funds a role.
const _: () = assert!(
    2 * std::mem::size_of::<Result<bool, Diagnostic>>()
        + std::mem::size_of::<bool>()
        + std::mem::size_of::<&ResolvedFunction>()
        + std::mem::size_of::<&ExitTarget>()
        + std::mem::size_of::<&mut ReplayBudget>()
        <= 2 * std::mem::size_of::<Result<Vec<CleanupRegionId>, Diagnostic>>()
            + std::mem::size_of::<Vec<CleanupRegionId>>()
            + std::mem::size_of::<bool>()
            + std::mem::size_of::<&ResolvedFunction>()
            + std::mem::size_of::<&ExitTarget>()
            + std::mem::size_of::<&mut ReplayBudget>()
);
// Actual new latch predicate slots and the reserve/charge preflight call and
// receiving families. These invocation-owned slots are reused across exits;
// the two predicate operations themselves are charged before every evaluation.
pub(in crate::cleanup_plan::replay::expression_facts::receiving) const EXIT_SELECTOR_BYTES: usize =
    2 * std::mem::size_of::<bool>()
        + 4 * std::mem::size_of::<Result<(), Diagnostic>>()
        + 4 * std::mem::size_of::<&mut ReplayBudget>()
        + 4 * std::mem::size_of::<&ResolvedFunction>()
        + 4 * std::mem::size_of::<usize>()
        + 4 * std::mem::size_of::<&str>();

struct Matching<'hir> {
    source: Option<CleanupRegionId>,
    target: Option<CleanupRegionId>,
    cursor: Option<CleanupRegionId>,
    node: Option<&'hir crate::cleanup_plan::CleanupRegion>,
    block: Option<&'hir crate::cleanup_plan::CleanupBlock>,
    edge: Option<&'hir crate::cleanup_plan::CleanupEdge>,
    next: Option<usize>,
    source_len: usize,
    target_len: usize,
    cut: usize,
    equal: bool,
}

// The ordinary callers select this only for non-latch exits. All old paid and
// unobserved constructors, owning-continue validation and loop latches remain.
pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn exit_regions_match(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    const PHASE: &str = "cleanup exit region comparison";
    budget.reserve_bytes(function, 0, PHASE)?;
    budget.charge(function, 0, PHASE)?;
    budget.reserve_bytes(
        function,
        std::mem::size_of::<Matching<'_>>()
            + 2 * std::mem::size_of::<Result<bool, Diagnostic>>()
            + 2 * std::mem::size_of::<&mut ReplayBudget>()
            + std::mem::size_of::<&ResolvedFunction>()
            + std::mem::size_of::<&ExitTarget>()
            + std::mem::size_of::<&crate::cleanup_plan::CleanupPlan>()
            + std::mem::size_of::<Option<&crate::cleanup_plan::CleanupBlock>>()
            + 2 * std::mem::size_of::<&crate::cleanup_plan::CleanupBlock>()
            + std::mem::size_of::<Option<&crate::cleanup_plan::CleanupEdge>>()
            + std::mem::size_of::<&crate::cleanup_plan::CleanupEdge>()
            + 3 * std::mem::size_of::<Option<CleanupRegionId>>()
            + 4 * std::mem::size_of::<CleanupRegionId>()
            + 2 * std::mem::size_of::<&crate::cleanup_plan::CleanupRegion>()
            + std::mem::size_of::<Option<&crate::cleanup_plan::CleanupRegion>>()
            + 2 * std::mem::size_of::<usize>()
            + 2 * std::mem::size_of::<bool>(),
        PHASE,
    )?;
    budget.charge(function, 11, PHASE)?;
    budget.charge(function, 1, PHASE)?;
    let plan = &function.cleanup_plan;
    let mut state = Matching {
        source: None,
        target: None,
        cursor: None,
        node: None,
        block: None,
        edge: None,
        next: None,
        source_len: 0,
        target_len: 0,
        cut: 0,
        equal: true,
    };
    state.block = plan.blocks.get(exit.from.0 as usize);
    let Some(block) = state.block else {
        return Err(replay_error(
            function,
            "cleanup exit has an unknown source block",
        ));
    };
    state.source = Some(block.region);
    state.cursor = state.source;
    // Record equality without reporting it until all old validation completes.
    while let Some(region) = state.cursor {
        budget.charge(function, 3, PHASE)?;
        state.node = plan.regions.get(region.0 as usize);
        let Some(node) = state.node else {
            return Err(replay_error(
                function,
                "cleanup region chain references an unknown region",
            ));
        };
        if state.equal && state.source_len < exit.leaves_regions.len() {
            budget.charge(function, 1, PHASE)?;
            state.equal = exit.leaves_regions[state.source_len] == region;
        }
        state.next = state.source_len.checked_add(1);
        let Some(next) = state.next else {
            return Err(budget.budget_refusal(function, "region chain length overflow", ""));
        };
        state.source_len = next;
        state.cursor = node.parent;
    }
    state.cut = state.source_len;
    match &exit.continuation {
        ExitContinuation::Continue(edge)
        | ExitContinuation::LoopBreak { edge, .. }
        | ExitContinuation::FunctionReturn { edge, .. } => {
            state.edge = plan.edges.get(edge.0 as usize);
            let Some(edge) = state.edge else {
                return Err(replay_error(
                    function,
                    "cleanup continuation has an unknown target",
                ));
            };
            state.block = plan.blocks.get(edge.to.0 as usize);
            let Some(block) = state.block else {
                return Err(replay_error(
                    function,
                    "cleanup continuation has an unknown target",
                ));
            };
            state.target = Some(block.region);
            state.cursor = state.target;
            budget.charge(function, 5, PHASE)?;
            while let Some(region) = state.cursor {
                budget.charge(function, 3, PHASE)?;
                state.node = plan.regions.get(region.0 as usize);
                let Some(node) = state.node else {
                    return Err(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    ));
                };
                state.next = state.target_len.checked_add(1);
                let Some(next) = state.next else {
                    return Err(budget.budget_refusal(
                        function,
                        "region chain length overflow",
                        "",
                    ));
                };
                state.target_len = next;
                state.cursor = node.parent;
            }
            // Both immutable parent chains have now completed. Align their
            // lengths, then find exactly the first source member of the target
            // ancestry, preserving the old leaf-to-root prefix without sorting.
            state.cut = 0;
            while state.source_len > state.target_len {
                budget.charge(function, 3, PHASE)?;
                let Some(region) = state.source else {
                    return Err(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    ));
                };
                state.node = plan.regions.get(region.0 as usize);
                let Some(node) = state.node else {
                    return Err(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    ));
                };
                state.source = node.parent;
                state.source_len -= 1;
                state.cut += 1;
            }
            while state.target_len > state.source_len {
                budget.charge(function, 3, PHASE)?;
                let Some(region) = state.target else {
                    return Err(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    ));
                };
                state.node = plan.regions.get(region.0 as usize);
                let Some(node) = state.node else {
                    return Err(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    ));
                };
                state.target = node.parent;
                state.target_len -= 1;
            }
            while state.source != state.target {
                budget.charge(function, 5, PHASE)?;
                let Some(region) = state.source else {
                    return Err(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    ));
                };
                state.node = plan.regions.get(region.0 as usize);
                let Some(node) = state.node else {
                    return Err(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    ));
                };
                state.source = node.parent;
                let Some(region) = state.target else {
                    return Err(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    ));
                };
                state.node = plan.regions.get(region.0 as usize);
                let Some(node) = state.node else {
                    return Err(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    ));
                };
                state.target = node.parent;
                state.cut += 1;
            }
        }
        ExitContinuation::CommitResult { .. }
        | ExitContinuation::ReturnFailure { .. }
        | ExitContinuation::ReturnUnit => {}
        ExitContinuation::LoopLatch { .. } => {
            return Err(replay_error(
                function,
                "loop latch requires the owning region receiver",
            ));
        }
    }
    Ok(state.equal && exit.leaves_regions.len() == state.cut)
}

#[cfg(test)]
mod tests;

pub(in crate::cleanup_plan::replay::expression_facts::receiving) mod compact;
#[cfg(test)]
mod compact_tests;
