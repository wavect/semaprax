use super::*;

fn successor(
    function: &ResolvedFunction,
    block: BlockId,
    index: usize,
) -> Result<Option<BlockId>, Diagnostic> {
    let plan = &function.cleanup_plan;
    let item = plan
        .blocks
        .get(block.0 as usize)
        .ok_or_else(|| replay_error(function, "cleanup CFG references an unknown block"))?;
    let edge = match &item.terminator {
        CleanupTerminator::Goto(edge) => (index == 0).then_some(*edge),
        CleanupTerminator::Branch(edges) => {
            if edges.len() != 2 || edges[0] == edges[1] {
                return Err(replay_error(
                    function,
                    "cleanup branch must own exactly two distinct edges",
                ));
            }
            edges.get(index).copied()
        }
        CleanupTerminator::Exit(exit) => {
            let target = plan
                .exits
                .get(exit.0 as usize)
                .ok_or_else(|| replay_error(function, "cleanup CFG references an unknown exit"))?;
            match &target.continuation {
                ExitContinuation::Continue(edge)
                | ExitContinuation::LoopBreak { edge, .. }
                | ExitContinuation::FunctionReturn { edge, .. } => (index == 0).then_some(*edge),
                ExitContinuation::LoopLatch { .. } => {
                    return Ok(if index == 0 {
                        lexical_cert::latch_resume(plan, *exit)
                    } else {
                        None
                    })
                }
                ExitContinuation::CommitResult { .. }
                | ExitContinuation::ReturnFailure { .. }
                | ExitContinuation::ReturnUnit => None,
            }
        }
    };
    edge.map(|id| {
        plan.edges
            .get(id.0 as usize)
            .map(|edge| edge.to)
            .ok_or_else(|| replay_error(function, "cleanup CFG references an unknown edge"))
    })
    .transpose()
}

pub(in crate::cleanup_plan::replay) fn validate_reachable_acyclic_cfg(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    budget.reserve_array::<u8>(function, plan.blocks.len(), "structural CFG colors")?;
    let mut colors = vec![0_u8; plan.blocks.len()];
    let mut stack = Vec::new();
    budget.reserve_vec(function, &mut stack, 1, "structural CFG entry")?;
    stack.push((plan.entry, false));
    while let Some((block, expanded)) = stack.pop() {
        budget.charge(function, 1, "structural CFG traversal")?;
        let index = block.0 as usize;
        let color = colors
            .get_mut(index)
            .ok_or_else(|| replay_error(function, "cleanup CFG references an unknown block"))?;
        if expanded {
            *color = 2;
            continue;
        }
        match *color {
            1 => return Err(replay_error(function, "cleanup CFG contains a cycle")),
            2 => continue,
            _ => *color = 1,
        }
        budget.reserve_vec(function, &mut stack, 1, "structural CFG finish")?;
        stack.push((block, true));
        // The authenticated terminator has at most two successors. Read it
        // directly in the same reverse order as the legacy owned scratch Vec.
        for next in (0..2).rev() {
            let Some(target) = successor(function, block, next)? else {
                continue;
            };
            let color = colors
                .get(target.0 as usize)
                .ok_or_else(|| replay_error(function, "cleanup CFG references an unknown block"))?;
            if *color == 1 {
                return Err(replay_error(function, "cleanup CFG contains a cycle"));
            }
            if *color == 0 {
                budget.reserve_vec(function, &mut stack, 1, "structural CFG successor")?;
                stack.push((target, false));
            }
        }
    }
    if colors.contains(&0) {
        return Err(replay_error(
            function,
            "cleanup CFG contains an unreachable block",
        ));
    }
    Ok(())
}

mod ordinary;
pub(in crate::cleanup_plan::replay) use ordinary::validate as validate_ordinary;
