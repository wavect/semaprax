//! Borrowed CFG topology and funded ordinary preflight scratch. No semantic
//! work event or quota changes; original saturating propagation order remains.
use super::*;

#[derive(Clone, Copy)]
pub(in crate::cleanup_plan::replay) enum Targets<'a> {
    Edges(&'a [EdgeId]),
    One(BlockId),
    Terminal,
}
impl Targets<'_> {
    pub(in crate::cleanup_plan::replay) fn len(self) -> usize {
        match self {
            Self::Edges(edges) => edges.len(),
            Self::One(_) => 1,
            Self::Terminal => 0,
        }
    }
    pub(in crate::cleanup_plan::replay) fn get(
        self,
        function: &ResolvedFunction,
        index: usize,
    ) -> Result<usize, Diagnostic> {
        match self {
            Self::Edges(edges) => function
                .cleanup_plan
                .edges
                .get(edges[index].0 as usize)
                .map(|edge| edge.to.0 as usize)
                .ok_or_else(|| invalid(function)),
            Self::One(block) => Ok(block.0 as usize),
            Self::Terminal => unreachable!("terminal has no successors"),
        }
    }
}
fn invalid(function: &ResolvedFunction) -> Diagnostic {
    replay_error(
        function,
        "cleanup replay preflight references an unknown id",
    )
}
pub(in crate::cleanup_plan::replay) fn targets(
    function: &ResolvedFunction,
    index: usize,
) -> Result<Targets<'_>, Diagnostic> {
    let plan = &function.cleanup_plan;
    match &plan.blocks[index].terminator {
        CleanupTerminator::Goto(edge) => Ok(Targets::Edges(std::slice::from_ref(edge))),
        CleanupTerminator::Branch(edges) => Ok(Targets::Edges(edges)),
        CleanupTerminator::Exit(exit) => match &plan
            .exits
            .get(exit.0 as usize)
            .ok_or_else(|| invalid(function))?
            .continuation
        {
            ExitContinuation::Continue(edge)
            | ExitContinuation::LoopBreak { edge, .. }
            | ExitContinuation::FunctionReturn { edge, .. } => {
                Ok(Targets::Edges(std::slice::from_ref(edge)))
            }
            ExitContinuation::LoopLatch { .. } => {
                Ok(lexical_cert::latch_resume(plan, *exit).map_or(Targets::Terminal, Targets::One))
            }
            ExitContinuation::CommitResult { .. }
            | ExitContinuation::ReturnUnit
            | ExitContinuation::ReturnFailure { .. } => Ok(Targets::Terminal),
        },
    }
}

pub(super) fn bounds(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<CfgReplayBounds, Diagnostic> {
    let plan = &function.cleanup_plan;
    let entry = plan
        .blocks
        .get(plan.entry.0 as usize)
        .map(|_| plan.entry.0 as usize)
        .ok_or_else(|| invalid(function))?;
    // Preserve old per-block lookup-before-range order: an unknown edge later
    // in one branch precedes an earlier edge's out-of-range destination.
    for index in 0..plan.blocks.len() {
        let successors = targets(function, index)?;
        for ordinal in 0..successors.len() {
            successors.get(function, ordinal)?;
        }
        for ordinal in 0..successors.len() {
            if successors.get(function, ordinal)? >= plan.blocks.len() {
                return Err(invalid(function));
            }
        }
    }
    budget.reserve_array::<bool>(function, plan.blocks.len(), "CFG preflight reachable")?;
    let mut reachable = vec![false; plan.blocks.len()];
    budget.reserve_array::<usize>(function, 1, "CFG preflight discovery entry")?;
    let mut discover = vec![entry];
    while let Some(index) = discover.pop() {
        if std::mem::replace(&mut reachable[index], true) {
            continue;
        }
        let successors = targets(function, index)?;
        budget.reserve_vec(
            function,
            &mut discover,
            successors.len(),
            "CFG preflight discovery successors",
        )?;
        // Preserve the original exact-size Vec::extend constructor when no
        // payer is attached; individual pushes would change its growth layout.
        discover.extend((0..successors.len()).map(|ordinal| {
            successors
                .get(function, ordinal)
                .expect("immutable prevalidated CFG target")
        }));
    }
    budget.reserve_array::<usize>(function, plan.blocks.len(), "CFG preflight indegree")?;
    let mut indegree = vec![0usize; plan.blocks.len()];
    for (index, reachable) in reachable.iter().enumerate() {
        if *reachable {
            let successors = targets(function, index)?;
            for ordinal in 0..successors.len() {
                let successor = successors.get(function, ordinal)?;
                indegree[successor] = indegree[successor].saturating_add(1);
            }
        }
    }
    budget.reserve_array::<usize>(function, 1, "CFG preflight queue entry")?;
    let mut pending = VecDeque::from([entry]);
    budget.reserve_array::<usize>(function, plan.blocks.len(), "CFG preflight incoming")?;
    let mut incoming = vec![0usize; plan.blocks.len()];
    incoming[entry] = 1;
    let mut total = 0usize;
    let mut skeleton_work = 1usize;
    let mut terminal_paths = 0usize;
    let mut visited = 0usize;
    let ceiling = MAX_REPLAY_WORK_UNITS.saturating_add(1);
    while let Some(index) = pending.pop_front() {
        visited = visited.saturating_add(1);
        let paths = incoming[index];
        let block = &plan.blocks[index];
        let local = block.transitions.len().saturating_add(1);
        total = total
            .saturating_add(local.saturating_mul(paths))
            .min(ceiling);
        skeleton_work = skeleton_work
            .saturating_add(
                skeleton_bound::plan_skeleton_block_weight(plan, block).saturating_mul(paths),
            )
            .min(ceiling);
        let successors = targets(function, index)?;
        if matches!(successors, Targets::Terminal) {
            terminal_paths = terminal_paths.saturating_add(paths);
        }
        for ordinal in 0..successors.len() {
            let successor = successors.get(function, ordinal)?;
            incoming[successor] = incoming[successor].saturating_add(paths).min(ceiling);
            indegree[successor] = indegree[successor].saturating_sub(1);
            if indegree[successor] == 0 {
                budget.reserve_deque(function, &mut pending, 1, "CFG preflight ready queue")?;
                pending.push_back(successor);
            }
        }
        if total >= ceiling {
            return Ok(CfgReplayBounds {
                terminal_paths: MAX_REPLAY_PATHS.saturating_add(1),
                work: ceiling,
                skeleton_work: ceiling,
            });
        }
    }
    if visited != reachable.iter().filter(|reachable| **reachable).count() {
        return Err(replay_error(function, "cleanup CFG contains a cycle"));
    }
    Ok(CfgReplayBounds {
        terminal_paths,
        work: total,
        skeleton_work,
    })
}

/// Preserve local payer provenance until the legacy semantic-only fallback is
/// selected. A carrier/capacity refusal need not poison the enclosing ledger.
pub(super) fn enumerating_result(
    result: Result<CfgReplayBounds, Diagnostic>,
    budget: &ReplayBudget,
) -> Result<Option<CfgReplayBounds>, Diagnostic> {
    match result {
        Ok(cfg) if cfg.terminal_paths <= MAX_REPLAY_PATHS => Ok(Some(cfg)),
        Err(error) if budget.refusal_spent || crate::bounded_output::active_refused() => Err(error),
        Ok(_) | Err(_) => Ok(None),
    }
}

#[cfg(test)]
#[path = "cfg_bounds/tests.rs"]
mod tests;
