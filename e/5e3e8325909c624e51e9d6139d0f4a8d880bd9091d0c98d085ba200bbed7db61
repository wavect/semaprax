//! Complete exit-region traversal with lossless narrow H006 transport.
use super::*;
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn exit_regions_match(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    budget: &mut ReplayBudget,
) -> Result<bool, ByteFailure> {
    const PHASE: &str = "cleanup exit region comparison";
    budget.reserve_compact_bytes(function, 0, PHASE)?;
    budget.charge_compact(function, 0, PHASE)?;
    budget.reserve_compact_bytes(
        function,
        std::mem::size_of::<Matching<'_>>()
            + 2 * std::mem::size_of::<Result<bool, ByteFailure>>()
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
    budget.charge_compact(function, 11, PHASE)?;
    budget.charge_compact(function, 1, PHASE)?;
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
        return Err(ByteFailure::from_budget(replay_error(
            function,
            "cleanup exit has an unknown source block",
        )));
    };
    state.source = Some(block.region);
    state.cursor = state.source;
    // Record equality without reporting it until all old validation completes.
    while let Some(region) = state.cursor {
        budget.charge_compact(function, 3, PHASE)?;
        state.node = plan.regions.get(region.0 as usize);
        let Some(node) = state.node else {
            return Err(ByteFailure::from_budget(replay_error(
                function,
                "cleanup region chain references an unknown region",
            )));
        };
        if state.equal && state.source_len < exit.leaves_regions.len() {
            budget.charge_compact(function, 1, PHASE)?;
            state.equal = exit.leaves_regions[state.source_len] == region;
        }
        state.next = state.source_len.checked_add(1);
        let Some(next) = state.next else {
            return Err(budget.compact_refusal(function, "region chain length overflow", ""));
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
                return Err(ByteFailure::from_budget(replay_error(
                    function,
                    "cleanup continuation has an unknown target",
                )));
            };
            state.block = plan.blocks.get(edge.to.0 as usize);
            let Some(block) = state.block else {
                return Err(ByteFailure::from_budget(replay_error(
                    function,
                    "cleanup continuation has an unknown target",
                )));
            };
            state.target = Some(block.region);
            state.cursor = state.target;
            budget.charge_compact(function, 5, PHASE)?;
            while let Some(region) = state.cursor {
                budget.charge_compact(function, 3, PHASE)?;
                state.node = plan.regions.get(region.0 as usize);
                let Some(node) = state.node else {
                    return Err(ByteFailure::from_budget(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    )));
                };
                state.next = state.target_len.checked_add(1);
                let Some(next) = state.next else {
                    return Err(budget.compact_refusal(
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
                budget.charge_compact(function, 3, PHASE)?;
                let Some(region) = state.source else {
                    return Err(ByteFailure::from_budget(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    )));
                };
                state.node = plan.regions.get(region.0 as usize);
                let Some(node) = state.node else {
                    return Err(ByteFailure::from_budget(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    )));
                };
                state.source = node.parent;
                state.source_len -= 1;
                state.cut += 1;
            }
            while state.target_len > state.source_len {
                budget.charge_compact(function, 3, PHASE)?;
                let Some(region) = state.target else {
                    return Err(ByteFailure::from_budget(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    )));
                };
                state.node = plan.regions.get(region.0 as usize);
                let Some(node) = state.node else {
                    return Err(ByteFailure::from_budget(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    )));
                };
                state.target = node.parent;
                state.target_len -= 1;
            }
            while state.source != state.target {
                budget.charge_compact(function, 5, PHASE)?;
                let Some(region) = state.source else {
                    return Err(ByteFailure::from_budget(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    )));
                };
                state.node = plan.regions.get(region.0 as usize);
                let Some(node) = state.node else {
                    return Err(ByteFailure::from_budget(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    )));
                };
                state.source = node.parent;
                let Some(region) = state.target else {
                    return Err(ByteFailure::from_budget(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    )));
                };
                state.node = plan.regions.get(region.0 as usize);
                let Some(node) = state.node else {
                    return Err(ByteFailure::from_budget(replay_error(
                        function,
                        "cleanup region chain references an unknown region",
                    )));
                };
                state.target = node.parent;
                state.cut += 1;
            }
        }
        ExitContinuation::CommitResult { .. }
        | ExitContinuation::ReturnFailure { .. }
        | ExitContinuation::ReturnUnit => {}
        ExitContinuation::LoopLatch { .. } => {
            return Err(ByteFailure::from_budget(replay_error(
                function,
                "loop latch requires the owning region receiver",
            )));
        }
    }
    Ok(state.equal && exit.leaves_regions.len() == state.cut)
}
