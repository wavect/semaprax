//! Complete pre-adoption receiving function, independently compiled for controls.
use super::*;
use super::super::original_materializer as schema;
pub(super) fn validate_original<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    if helpers.is_some_and(|index| {
        !program
            .full_program()
            .is_some_and(|full| index.matches_program(full))
    }) {
        return Err(replay_error(
            function,
            "cleanup replay helper index belongs to a different typed program",
        ));
    }
    // Reconstruct the ordinary schema first, preserving all its current-source
    // probes/error order. A private source-held projection adds allocation
    // branches; neither its attached v20 string nor an ordinary context selects it.
    let ordinary_schema = schema::selected_schema_with_index(program, function, helpers)?;
    let expected_schema = if allocation_paths::current(program, function).is_some() {
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20
    } else {
        ordinary_schema
    };
    if plan.schema != expected_schema {
        return Err(replay_error(
            function,
            format_args!(
                "uses cleanup-plan schema `{}` instead of HIR-derived `{expected_schema}`",
                plan.schema
            ),
        ));
    }

    if lexical_cert::requires_lexical(function) {
        return lexical_replay::validate_with_index(program, function, budget, helpers);
    }
    if plan.lexical_control.is_some() {
        return Err(replay_error(
            function,
            "function without lexical exits carries a lexical-control certificate",
        ));
    }
    validate_replay_size_budget(function)?;
    budget.charge(
        function,
        plan_structure_units(plan),
        "structural validation",
    )?;

    let expression_facts = expression_facts::with_budget(function, budget)?;
    ordinary_inventory::validate_inventory_coverage(program, function, budget)?;
    validate_required_status_sources(program, function)?;
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    expression_facts::receiving::validate_status_sources(function, &expression_facts, budget)?;
    validate_regions(function, &storage)?;
    validate_entry(function, &storage, &leaves)?;
    expression_facts::receiving::validate_blocks_and_edges(
        program,
        function,
        &expression_facts,
        &storage,
        &leaves,
        helpers,
        budget,
    )?;
    validate_exits(program, function, &storage, &leaves, &BTreeSet::new())?;
    structural_maps::validate_reference_coverage(function, budget)?;
    structural_control::validate_ordinary_cfg(function, budget)?;
    if !cleanup_inert_path_product_can_be_summarized(function)?
        && !status_only_paths_can_be_summarized(function)
        && !cleanup_inert_large_decisions_can_be_summarized(function)
    {
        validate_path_states::<Diagnostic>(
            program, function, &storage, &leaves, budget, None, helpers,
        )?;
        validate_typed_control_skeleton_with_index(
            program,
            function,
            budget,
            helpers,
            continuations,
        )?;
    }
    Ok(())
}
