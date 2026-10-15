//! Complete ordinary paid exit checks, with authenticated paid membership.
use super::super::remaining::selected;
use super::super::remaining::validate_finalizer_actions;
use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_exits(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    return_operands: &ReturnOperands<'_, '_>,
    facts: &lexical_hir::LexicalFacts<'_>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
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
        let expected_regions = helpers::expected_exit_regions(function, exit, budget)?;
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
            ExitContinuation::CommitResult { source } => validate_result_source(
                program,
                function,
                source,
                storage,
                leaves,
                return_operands,
                facts,
                budget,
            )?,
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
                validate_result_source(
                    program,
                    function,
                    source,
                    storage,
                    leaves,
                    return_operands,
                    facts,
                    budget,
                )?
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

#[allow(clippy::too_many_arguments)]
fn validate_result_source(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    source: &CleanupResultSource,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    return_operands: &ReturnOperands<'_, '_>,
    facts: &lexical_hir::LexicalFacts<'_>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    match source {
        CleanupResultSource::Scalar { expression } => {
            if type_needs_drop(program, function, &function.return_type)? {
                return Err(replay_error(
                    function,
                    "droppable function result uses a scalar result commit",
                ));
            }
            if expression != &function.body.id
                && !return_operands.contains(function, facts, expression, budget)?
            {
                return Err(replay_error(
                    function,
                    "scalar result commit does not publish the function body",
                ));
            }
        }
        CleanupResultSource::Owned { storage: result } => {
            if !matches!(
                function.return_type,
                ResolvedType::Nominal { .. }
                    | ResolvedType::Bytes
                    | ResolvedType::String
                    | ResolvedType::StringMap
                    | ResolvedType::OnceFunction
                    | ResolvedType::OnceFunctionI64
                    | ResolvedType::OnceFunctionI64Pair
            ) || !type_needs_drop(program, function, &function.return_type)?
                || result.storage != StorageId::ProvisionalResult
                || !result.projections.is_empty()
            {
                return Err(replay_error(
                    function,
                    "owned result commit must publish the whole droppable provisional result",
                ));
            }
            validate_place(function, result, storage, leaves)?;
            let provisional_leaves = leaves
                .values()
                .filter(|leaf| leaf.place.storage == StorageId::ProvisionalResult)
                .count();
            if provisional_leaves == 0 {
                return Err(replay_error(
                    function,
                    "owned result commit has no provisional-result leaves",
                ));
            }
        }
    }
    Ok(())
}
