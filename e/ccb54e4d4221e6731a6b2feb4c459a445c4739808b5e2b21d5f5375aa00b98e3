use crate::cleanup_plan::replay::*;
pub(in crate::cleanup_plan::replay) fn validate_structure_with_budget_storage<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::ReplayStructure,
    );
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
    let ordinary_schema =
        schema::paid_summary::selected_with_budget(program, function, helpers, budget)?;
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
    let status_cases =
        expression_facts::receiving::StatusCases::validate(program, function, budget)?;
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    let statuses = expression_facts::receiving::validate_status_sources_certified(
        function,
        &expression_facts,
        budget,
        &status_cases,
    )?;
    expression_facts::receiving::validate_regions(function, &storage, budget)?;
    expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;
    expression_facts::receiving::validate_blocks_and_edges_retained(
        program,
        function,
        &expression_facts,
        &storage,
        &leaves,
        helpers,
        budget,
        &statuses,
    )?;
    expression_facts::receiving::validate_exits_retained(
        program,
        function,
        &storage,
        &leaves,
        &BTreeSet::new(),
        budget,
        &statuses,
    )?;
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
