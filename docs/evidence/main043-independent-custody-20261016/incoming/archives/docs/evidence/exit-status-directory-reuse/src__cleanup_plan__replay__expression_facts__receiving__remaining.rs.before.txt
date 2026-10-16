//! Complete exits/finalizers: paid retained directories, unchanged place/chain helpers.
use super::*;
mod selected;

pub(in crate::cleanup_plan::replay) fn validate_exits(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    return_operands: &BTreeSet<ExpressionId>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return crate::cleanup_plan::replay::validate_exits(
            program,
            function,
            storage,
            leaves,
            return_operands,
        );
    }
    let plan = &function.cleanup_plan;
    let mut statuses = Directory::new(function, budget, "cleanup exit status directory")?;
    for source in &plan.status_sources {
        statuses.insert(function, &source.id, budget)?;
    }
    let mut continuation_edges =
        Directory::new(function, budget, "cleanup exit continuation directory")?;
    for (index, exit) in plan.exits.iter().enumerate() {
        if plan.lexical_control.is_none()
            && matches!(
                exit.continuation,
                ExitContinuation::LoopLatch { .. }
                    | ExitContinuation::LoopBreak { .. }
                    | ExitContinuation::FunctionReturn { .. }
            )
        {
            return Err(replay_error(
                function,
                "lexical exit continuation has no lexical-control certificate",
            ));
        }
        if exit.id.0 != u32_index(function, index, "cleanup exit")?
            || !block_exists(function, exit.from)
        {
            return Err(replay_error(
                function,
                "cleanup exit has a non-contiguous ID or unknown source block",
            ));
        }
        let expected_regions = expected_exit_regions(function, exit)?;
        if exit.leaves_regions != expected_regions {
            return Err(replay_error(
                function,
                "cleanup exit does not leave the exact source-to-target region chain",
            ));
        }
        let mut prior: Option<super::CleanupRegionId> = None;
        for region in &exit.leaves_regions {
            let Some(item) = plan.regions.get(region.0 as usize) else {
                return Err(replay_error(
                    function,
                    "cleanup exit references unknown region",
                ));
            };
            if let Some(child) = prior {
                if plan.regions[child.0 as usize].parent != Some(*region) {
                    return Err(replay_error(
                        function,
                        "cleanup exit regions are not a parent chain",
                    ));
                }
            }
            prior = Some(item.id);
        }
        validate_finalizer_actions(function, &exit.finalize_in_order, storage, leaves, budget)?;
        match &exit.continuation {
            ExitContinuation::Continue(edge) => {
                super::validate_owned_edge_paid(
                    function,
                    exit.from,
                    *edge,
                    &mut continuation_edges,
                    budget,
                )?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(replay_error(
                        function,
                        "cleanup continuation edge is conditional",
                    ));
                }
            }
            ExitContinuation::CommitResult { source } => {
                validate_result_source(program, function, source, storage, leaves, return_operands)?
            }
            ExitContinuation::FunctionReturn { edge, source, .. } => {
                super::validate_owned_edge_paid(
                    function,
                    exit.from,
                    *edge,
                    &mut continuation_edges,
                    budget,
                )?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(replay_error(
                        function,
                        "function-return continuation edge is conditional",
                    ));
                }
                validate_result_source(program, function, source, storage, leaves, return_operands)?
            }
            ExitContinuation::LoopBreak { edge, .. } => {
                super::validate_owned_edge_paid(
                    function,
                    exit.from,
                    *edge,
                    &mut continuation_edges,
                    budget,
                )?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(replay_error(
                        function,
                        "loop-break continuation edge is conditional",
                    ));
                }
            }
            ExitContinuation::LoopLatch { .. } => {}
            ExitContinuation::ReturnFailure { source } => {
                super::require_status_paid(function, &statuses, source, budget)?;
                if !selected::unique_status(&plan.blocks[exit.from.0 as usize].transitions, source)
                {
                    return Err(replay_error(
                        function,
                        "failure exit does not return its uniquely selected status",
                    ));
                }
            }
            ExitContinuation::ReturnUnit => {
                return Err(replay_error(
                    function,
                    "ReturnUnit is invalid for current source-function return types",
                ));
            }
        }
    }
    for region in &plan.regions {
        if plan.lexical_control.is_some() && region.normal_scope_end == UNRESOLVED_NORMAL_END {
            // Authenticated as abrupt-only by the lexical certificate replay.
            continue;
        }
        let exit = &plan.exits[region.normal_scope_end.0 as usize];
        if !exit.leaves_regions.contains(&region.id) {
            return Err(replay_error(
                function,
                "region normal-scope exit does not leave that region",
            ));
        }
    }
    Ok(())
}

fn validate_finalizer_actions(
    function: &ResolvedFunction,
    actions: &[crate::cleanup_plan::FinalizeAction],
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let mut finalized = Directory::new(function, budget, "cleanup finalizer guard directory")?;
    for action in actions {
        let under = validate_place(function, &action.source, storage, leaves)?;
        if under.len() != 1
            || under[0] != action.guard_flag
            || leaves[&action.guard_flag].place != action.source
        {
            return Err(replay_error(
                function,
                "finalizer guard does not name its exact cleanup leaf",
            ));
        }
        let leaf = leaves
            .get(&action.guard_flag)
            .expect("validated cleanup flag");
        if leaf.lifecycle != action.lifecycle_id
            || !finalized.insert(function, action.guard_flag, budget)?
        {
            return Err(replay_error(
                function,
                "finalizer lifecycle or guard is invalid or repeated",
            ));
        }
        if let Some(condition) = &action.active_case {
            if condition.storage != action.source.storage
                || action.source.projections.first() != Some(&condition.case)
                || function
                    .cleanup_plan
                    .slots
                    .iter()
                    .find(|slot| slot.storage == condition.storage)
                    .is_none_or(|slot| {
                        !matches!(
                            &slot.ty,
                            ResolvedType::Nominal { declaration, .. }
                                if declaration == &condition.variant
                        )
                    })
            {
                return Err(replay_error(
                    function,
                    "conditional finalizer does not bind its exact variant case path",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
