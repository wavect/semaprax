//! Complete exits receiver borrowing only the exact authenticated status directory.
use super::super::remaining::{selected, validate_finalizer_actions};
use super::*;

const BORROW_BYTES: usize = 2 * std::mem::size_of::<&StatusSources<'static>>()
    + 2 * std::mem::size_of::<&Option<Directory<&'static StatusSourceId>>>()
    + std::mem::size_of::<Option<&Directory<&'static StatusSourceId>>>()
    + std::mem::size_of::<&Directory<&'static StatusSourceId>>()
    + std::mem::size_of::<bool>();
// Two token and two as_ref argument copies, return/pattern assignment, branch.
const BORROW_UNITS: usize = 7;

pub(in crate::cleanup_plan::replay) fn validate_exits_retained(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    return_operands: &BTreeSet<ExpressionId>,
    budget: &mut ReplayBudget,
    validated: &StatusSources<'_>,
) -> Result<(), Diagnostic> {
    if !std::ptr::eq(function, validated.function) || !std::ptr::eq(budget, validated.budget) {
        return Err(replay_error(
            function,
            "exit status directory belongs to another function or budget",
        ));
    }
    if !validated.ledger.current() {
        return Err(budget.budget_refusal(
            function,
            "retained exit status invocation owner changed",
            "",
        ));
    }
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
    // Preserve the complete original constructor's zero/current/sticky checks.
    budget.reserve_bytes(function, 0, "cleanup exit status directory")?;
    budget.charge(function, 0, "cleanup exit status directory")?;
    // New caller/receiving argument overlap, optional borrow return and actual
    // receiving directory reference are prepaid in full before borrowing ids.
    budget.reserve_bytes(
        function,
        BORROW_BYTES,
        "retained exit status borrow controls",
    )?;
    budget.charge(
        function,
        BORROW_UNITS,
        "retained exit status borrow bookkeeping",
    )?;
    let Some(statuses) = validated.ids.as_ref() else {
        // Tree facts never lend a directory; execute the complete old receiver.
        return super::super::remaining::validate_exits(
            program,
            function,
            storage,
            leaves,
            return_operands,
            budget,
        );
    };
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
        if budget.payer == AllocationPayer::ActiveBuilder
            && !matches!(exit.continuation, ExitContinuation::LoopLatch { .. })
        {
            if !helpers::exit_regions_match(function, exit, budget)? {
                return Err(replay_error(
                    function,
                    "cleanup exit does not leave the exact source-to-target region chain",
                ));
            }
        } else {
            let expected_regions = helpers::expected_exit_regions(function, exit, budget)?;
            if exit.leaves_regions != expected_regions {
                return Err(replay_error(
                    function,
                    "cleanup exit does not leave the exact source-to-target region chain",
                ));
            }
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
                super::require_status_paid(function, statuses, source, budget)?;
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

#[cfg(test)]
mod tests;
