//! Ordinary paid lexical driver; verification order remains independent.
use super::*;
pub(super) fn validate_core<'hir, E: structural_control::call_receiving::Policy>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    build_facts: impl FnOnce(
        &'hir ResolvedFunction,
        &mut ReplayBudget,
    ) -> Result<expression_facts::Facts<'hir>, E>,
) -> Result<(), E> {
    let plan = &function.cleanup_plan;
    selected_profile::validate(program, function, budget)?;
    let owner = selected_owner::owner(program, function, budget)?;
    crate::hir::lexical_exit::validate_function(program, function, &owner)?;
    // The lexical resource plan was deferred before enumerating path refusal.
    // Bound structure now, but construct no plan automaton until every exact
    // lexical/certificate/ownership check below has authenticated its meaning.
    let structure_units = plan_structure_units(plan);
    if structure_units > MAX_REPLAY_WORK_UNITS {
        return Err(replay_error(
            function,
            "cleanup replay structure exceeds the global work budget",
        )
        .into());
    }
    budget.charge(function, structure_units, "structural validation")?;
    let expression_facts = build_facts(function, budget)?;
    let (storage, leaves) = {
        ordinary_inventory::validate_inventory_coverage(program, function, budget)?;
        validate_required_status_sources(program, function)?;
        let storage = validate_slots_paid_core::<E>(function, budget)?;
        let leaves = collect_leaves_paid(function, budget)?;
        expression_facts::receiving::validate_status_sources_core::<E>(
            function,
            &expression_facts,
            budget,
        )?;
        expression_facts::receiving::validate_regions(function, &storage, budget)?;
        expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;
        expression_facts::receiving::validate_blocks_and_edges(
            program,
            function,
            &expression_facts,
            &storage,
            &leaves,
            helpers,
            budget,
        )?;
        (storage, leaves)
    };

    let facts = lexical_hir::collect(function, budget)?;
    let return_operands =
        expression_facts::receiving::ReturnOperands::new::<E>(function, &facts, budget)?;
    return_operands.validate_exits(program, function, &facts, &storage, &leaves, budget)?;
    let auth =
        return_operands.authenticate_certificates(program, function, &owner, &facts, budget)?;
    // Borrowed identity backing and its original owner stay live through auth,
    // CFG/path replay and the final factored comparison, as the old inventory did.
    let _return_operands = return_operands;
    structural_maps::validate_reference_coverage(function, budget)?;
    structural_control::validate_ordinary_cfg(function, budget)?;
    // Lexical functions always replay every path: no summary shortcut applies.
    validate_path_states::<E>(
        program,
        function,
        &storage,
        &leaves,
        budget,
        Some(&auth),
        helpers,
    )?;
    factored::strategy::validate_authenticated(
        program,
        function,
        budget,
        helpers,
        expression_facts.len(),
    )
    .map_err(E::from)
}
