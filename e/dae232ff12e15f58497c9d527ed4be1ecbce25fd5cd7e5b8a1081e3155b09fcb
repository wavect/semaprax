//! Independently saved full original owning branch-sensitive CFG census.
use super::super::super::*;

/// Bound actual CFG traversal work without multiplying every structure item by
/// every terminal path. The cleanup graph is authenticated as acyclic later;
/// this independent saturating propagation is deliberately cycle-safe and
/// fails closed if a forged graph keeps increasing multiplicity.
pub(super) fn bounds(function: &ResolvedFunction) -> Result<CfgReplayBounds, Diagnostic> {
    let plan = &function.cleanup_plan;
    let invalid = || {
        replay_error(
            function,
            "cleanup replay preflight references an unknown id",
        )
    };
    let entry = plan
        .blocks
        .get(plan.entry.0 as usize)
        .map(|_| plan.entry.0 as usize)
        .ok_or_else(invalid)?;
    let mut successors = vec![Vec::<usize>::new(); plan.blocks.len()];
    let mut terminal = vec![false; plan.blocks.len()];
    for (index, block) in plan.blocks.iter().enumerate() {
        let targets = match &block.terminator {
            CleanupTerminator::Goto(edge) => plan
                .edges
                .get(edge.0 as usize)
                .map(|edge| vec![edge.to.0 as usize])
                .ok_or_else(invalid)?,
            CleanupTerminator::Branch(edges) => edges
                .iter()
                .map(|edge| {
                    plan.edges
                        .get(edge.0 as usize)
                        .map(|edge| edge.to.0 as usize)
                        .ok_or_else(invalid)
                })
                .collect::<Result<Vec<_>, _>>()?,
            CleanupTerminator::Exit(exit) => {
                match &plan
                    .exits
                    .get(exit.0 as usize)
                    .ok_or_else(invalid)?
                    .continuation
                {
                    ExitContinuation::Continue(edge)
                    | ExitContinuation::LoopBreak { edge, .. }
                    | ExitContinuation::FunctionReturn { edge, .. } => plan
                        .edges
                        .get(edge.0 as usize)
                        .map(|edge| vec![edge.to.0 as usize])
                        .ok_or_else(invalid)?,
                    ExitContinuation::LoopLatch { .. } => {
                        match lexical_cert::latch_resume(plan, *exit) {
                            Some(entry) => vec![entry.0 as usize],
                            None => {
                                terminal[index] = true;
                                Vec::new()
                            }
                        }
                    }
                    ExitContinuation::CommitResult { .. }
                    | ExitContinuation::ReturnUnit
                    | ExitContinuation::ReturnFailure { .. } => {
                        terminal[index] = true;
                        Vec::new()
                    }
                }
            }
        };
        if targets.iter().any(|target| *target >= plan.blocks.len()) {
            return Err(invalid());
        }
        successors[index] = targets;
    }

    let mut reachable = vec![false; plan.blocks.len()];
    let mut discover = vec![entry];
    while let Some(index) = discover.pop() {
        if std::mem::replace(&mut reachable[index], true) {
            continue;
        }
        discover.extend(successors[index].iter().copied());
    }
    let mut indegree = vec![0_usize; plan.blocks.len()];
    for (index, targets) in successors.iter().enumerate() {
        if reachable[index] {
            for target in targets {
                indegree[*target] = indegree[*target].saturating_add(1);
            }
        }
    }
    let mut pending = VecDeque::from([entry]);
    let mut incoming = vec![0_usize; plan.blocks.len()];
    incoming[entry] = 1;
    let mut total = 0_usize;
    let mut skeleton_work = 1_usize; // Initial cleanup-plan queue push.
    let mut terminal_paths = 0_usize;
    let mut visited = 0_usize;
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
        if terminal[index] {
            terminal_paths = terminal_paths.saturating_add(paths);
        }
        for successor in &successors[index] {
            incoming[*successor] = incoming[*successor].saturating_add(paths).min(ceiling);
            indegree[*successor] = indegree[*successor].saturating_sub(1);
            if indegree[*successor] == 0 {
                pending.push_back(*successor);
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
