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
