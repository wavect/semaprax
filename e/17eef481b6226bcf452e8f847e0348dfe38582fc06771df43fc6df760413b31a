//! Ordinary late CFG validation: old semantic order and zero new work debits.
//! Selected fixed-two-successor validation remains in the separate paid owner.
use super::*;
use crate::cleanup_plan::replay::cfg_bounds;

pub(in crate::cleanup_plan::replay) fn validate(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    budget.reserve_array::<u8>(function, plan.blocks.len(), "ordinary CFG colors")?;
    let mut colors = vec![0u8; plan.blocks.len()];
    // Preserve the old vec![(entry,false)] exact first request. Later growth is
    // prepaid cumulatively while the previous backing remains charged.
    budget.reserve_array::<(BlockId, bool)>(function, 1, "ordinary CFG entry")?;
    let mut stack = vec![(plan.entry, false)];
    while let Some((block, expanded)) = stack.pop() {
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
        budget.reserve_vec(function, &mut stack, 1, "ordinary CFG finish")?;
        stack.push((block, true));
        // No branch-shape reclassification at this late boundary: earlier
        // structural authentication owns that verdict. Even hostile arities
        // retain the old reverse traversal when this helper is tested alone.
        let successors = cfg_bounds::targets(function, index)?;
        // The old owning constructor resolves every edge in authored order
        // before any reverse traversal color check.
        for ordinal in 0..successors.len() {
            successors.get(function, ordinal)?;
        }
        for ordinal in (0..successors.len()).rev() {
            let target = successors.get(function, ordinal)?;
            let color = colors
                .get(target)
                .ok_or_else(|| replay_error(function, "cleanup CFG references an unknown block"))?;
            if *color == 1 {
                return Err(replay_error(function, "cleanup CFG contains a cycle"));
            }
            if *color == 0 {
                budget.reserve_vec(function, &mut stack, 1, "ordinary CFG successor")?;
                stack.push((BlockId(target as u32), false));
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

pub(in crate::cleanup_plan::replay) fn validate_compact(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    budget.reserve_array::<u8>(function, plan.blocks.len(), "ordinary CFG colors")?;
    let mut colors = vec![0u8; plan.blocks.len()];
    budget.reserve_array::<BlockId>(function, 1, "ordinary CFG entry")?;
    let mut stack = vec![plan.entry];
    while let Some(block) = stack.pop() {
        let index = block.0 as usize;
        let color = colors
            .get_mut(index)
            .ok_or_else(|| replay_error(function, "cleanup CFG references an unknown block"))?;
        match *color {
            // A block puts its finish marker above every older pending entry
            // for itself. Later pushes reject gray targets, so only its own
            // finish marker can be popped while it is gray.
            1 => {
                *color = 2;
                continue;
            }
            2 => continue,
            _ => *color = 1,
        }
        budget.reserve_vec(function, &mut stack, 1, "ordinary CFG finish")?;
        stack.push(block);
        // No branch-shape reclassification at this late boundary: earlier
        // structural authentication owns that verdict. Even hostile arities
        // retain the old reverse traversal when this helper is tested alone.
        let successors = cfg_bounds::targets(function, index)?;
        // The old owning constructor resolves every edge in authored order
        // before any reverse traversal color check.
        for ordinal in 0..successors.len() {
            successors.get(function, ordinal)?;
        }
        for ordinal in (0..successors.len()).rev() {
            let target = successors.get(function, ordinal)?;
            let color = colors
                .get(target)
                .ok_or_else(|| replay_error(function, "cleanup CFG references an unknown block"))?;
            if *color == 1 {
                return Err(replay_error(function, "cleanup CFG contains a cycle"));
            }
            if *color == 0 {
                budget.reserve_vec(function, &mut stack, 1, "ordinary CFG successor")?;
                stack.push(BlockId(target as u32));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_cfg_stack_keeps_shared_targets_and_back_edge_refusals() {
        let source = r#"module cfg.stack; @id("main") fn main()->i64 {let x = if true {1} else {2}; if x == 0 {3} else {4}}"#;
        let program = crate::hir::resolve(&crate::check(source, "cfg-stack.spx").unwrap()).unwrap();
        let function = &program.functions[0];
        let mut cycle = function.clone();
        let branch = cycle
            .cleanup_plan
            .blocks
            .iter()
            .position(|block| matches!(block.terminator, CleanupTerminator::Branch(_)))
            .unwrap();
        let CleanupTerminator::Branch(edges) = &cycle.cleanup_plan.blocks[branch].terminator else {
            unreachable!()
        };
        cycle.cleanup_plan.edges[edges[0].0 as usize].to = BlockId(branch as u32);
        let mut invalid = function.clone();
        invalid.cleanup_plan.edges[edges[0].0 as usize].to = BlockId(u32::MAX);

        for input in [function, &cycle, &invalid] {
            let run = |old| {
                crate::bounded_output::with_limit_usage(usize::MAX, || {
                    let mut budget = ReplayBudget::new();
                    let result = if old {
                        validate(input, &mut budget)
                    } else {
                        validate_compact(input, &mut budget)
                    };
                    result.map_err(|error| {
                        (
                            error.code,
                            error.severity,
                            error.message,
                            error.path,
                            error.span,
                            error.help,
                        )
                    })
                })
            };
            let (expected, old_refused, old_paid) = run(true);
            let (actual, refused, paid) = run(false);
            assert_eq!(actual, expected);
            assert!(!refused && !old_refused);
            assert!(paid < old_paid);
        }
    }
}
