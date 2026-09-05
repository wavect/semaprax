use super::*;

/// A decision-only CFG with no cleanup state has no path-dependent state to
/// replay. Its edge expressions, reachability, regions, exits and result source
/// are already checked structurally by the caller, so enumerating every
/// combination of lazy boolean outcomes would add no cleanup evidence.
pub(super) fn cleanup_plan_requires_path_replay(function: &ResolvedFunction) -> bool {
    let plan = &function.cleanup_plan;
    !plan.slots.is_empty()
        || !plan.entry_state.live_owned_parameters.is_empty()
        || !plan.entry_state.conditional_owned_parameters.is_empty()
        || plan
            .blocks
            .iter()
            .any(|block| !block.transitions.is_empty())
        || plan
            .exits
            .iter()
            .any(|exit| !exit.finalize_in_order.is_empty())
}

pub(super) fn cleanup_inert_path_product_can_be_summarized(
    function: &ResolvedFunction,
) -> Result<bool, Diagnostic> {
    if cleanup_plan_requires_path_replay(function) {
        return Ok(false);
    }
    let cfg_paths = branch_sensitive_cfg_bounds(function)?.terminal_paths;
    let semantic_paths = hir_terminal_path_bound(function)?;
    Ok(cfg_paths > MAX_REPLAY_PATHS && semantic_paths > MAX_REPLAY_PATHS)
}

pub(super) const STATUS_ONLY_PATH_SUMMARY_THRESHOLD: usize = 512;
const CLEANUP_INERT_EDGE_SUMMARY_THRESHOLD: usize = 1_024;

pub(super) fn cleanup_inert_large_decisions_can_be_summarized(function: &ResolvedFunction) -> bool {
    !cleanup_plan_requires_path_replay(function)
        && function.cleanup_plan.edges.len() > CLEANUP_INERT_EDGE_SUMMARY_THRESHOLD
}

/// Long scalar sequences can have one checked-status failure path per
/// statement. Every retained suffix repeats its complete successful prefix,
/// even though the plan carries no ownership state. Above the bounded exact
/// replay window, the independent status-source, edge, exit, reachability and
/// sticky-selection validators are the canonical summary for this shape.
pub(super) fn status_only_paths_can_be_summarized(function: &ResolvedFunction) -> bool {
    let plan = &function.cleanup_plan;
    plan.status_sources.len() > STATUS_ONLY_PATH_SUMMARY_THRESHOLD
        && plan.slots.is_empty()
        && plan.entry_state.live_owned_parameters.is_empty()
        && plan.entry_state.conditional_owned_parameters.is_empty()
        && plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .all(|transition| matches!(transition, CleanupTransition::SelectFailure { .. }))
        && plan
            .exits
            .iter()
            .all(|exit| exit.finalize_in_order.is_empty())
}

pub(super) fn plan_structure_units(plan: &crate::cleanup_plan::CleanupPlan) -> usize {
    let mut units = plan
        .slots
        .len()
        .saturating_add(plan.status_sources.len())
        .saturating_add(plan.blocks.len())
        .saturating_add(plan.edges.len())
        .saturating_add(plan.regions.len())
        .saturating_add(plan.exits.len());
    for block in &plan.blocks {
        units = units.saturating_add(block.transitions.len());
    }
    for exit in &plan.exits {
        units = units
            .saturating_add(exit.leaves_regions.len())
            .saturating_add(exit.finalize_in_order.len());
    }
    units
}

pub(super) fn validate_replay_size_budget(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let structure_units = plan_structure_units(&function.cleanup_plan);
    if structure_units > MAX_REPLAY_WORK_UNITS {
        return Err(replay_error(
            function,
            "cleanup replay structure exceeds the global work budget",
        ));
    }
    if status_only_paths_can_be_summarized(function)
        || cleanup_inert_large_decisions_can_be_summarized(function)
    {
        return Ok(());
    }
    let cfg = branch_sensitive_cfg_bounds(function)?;
    let semantic_paths = hir_terminal_path_bound(function)?;
    if !cleanup_plan_requires_path_replay(function)
        && cfg.terminal_paths > MAX_REPLAY_PATHS
        && semantic_paths > MAX_REPLAY_PATHS
    {
        return Ok(());
    }
    if cfg.terminal_paths > MAX_REPLAY_PATHS {
        return Err(replay_error(
            function,
            "cleanup replay path bound exceeds the global path budget",
        ));
    }
    if semantic_paths > MAX_REPLAY_PATHS {
        return Err(replay_error(
            function,
            "cleanup replay semantic path bound exceeds the global path budget",
        ));
    }
    let expression_units = expression_facts(function)?.len();
    if cfg.work.saturating_add(expression_units) > MAX_REPLAY_WORK_UNITS {
        return Err(replay_error(
            function,
            "cleanup replay combined path/work bound exceeds the global budget",
        ));
    }
    Ok(())
}
