//! Complete immediate pre-control paid driver, with every old child route.
use super::*;
pub(in crate::cleanup_plan::replay) fn validate_before_selected_control(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), expression_facts::errors::EngineError> {
    if !budget.selected() && budget.pays_allocations() {
        return ordinary::validate_core(
            program,
            function,
            budget,
            None,
            expression_facts::paid_arena,
        );
    }
    let plan = &function.cleanup_plan;
    selected_profile::paid::validate(program, function, budget)?;
    let owner = selected_owner::paid::owner(program, function, budget)?;
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
    let expression_facts = expression_facts::paid_arena(function, budget)?;
    let selected = budget.selected();
    let (storage, leaves) = if selected {
        inventory_coverage::paid::validate_inventory_coverage(program, function, budget)?;
        status_construction::paid::validate_required_status_sources(program, function, budget)?;
        let storage = structural_maps::paid::slots(function, budget)?;
        let leaves = structural_maps::paid::leaves(function, budget)?;
        status_validation::validate_core::<expression_facts::errors::EngineError>(
            function,
            &expression_facts,
            budget,
        )?;
        structural_maps::paid::regions(function, &storage, budget)?;
        structural_control::validate_entry(function, &storage, &leaves, budget)?;
        let renewals = program.primitive_renewals(function).ok_or_else(|| {
            replay_error(
                function,
                "selected replay lacks its independently rebuilt renewal lease",
            )
        })?;
        structural_control::validate_blocks_and_edges_paid::<expression_facts::errors::EngineError>(
            program,
            function,
            &expression_facts,
            &storage,
            &leaves,
            renewals,
            budget,
        )?;
        (storage, leaves)
    } else {
        ordinary_inventory::validate_inventory_coverage(program, function, budget)?;
        validate_required_status_sources(program, function)?;
        let storage =
            validate_slots_paid_core::<expression_facts::errors::EngineError>(function, budget)?;
        let leaves = collect_leaves_paid(function, budget)?;
        expression_facts::receiving::validate_status_sources_core::<
            expression_facts::errors::EngineError,
        >(function, &expression_facts, budget)?;
        expression_facts::receiving::validate_regions(function, &storage, budget)?;
        expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;
        expression_facts::receiving::validate_blocks_and_edges(
            program,
            function,
            &expression_facts,
            &storage,
            &leaves,
            None,
            budget,
        )?;
        (storage, leaves)
    };

    let facts = lexical_hir::collect(function, budget)?;
    let return_operands = if selected {
        let mut operands = BTreeSet::new();
        for operand in facts
            .exits
            .iter()
            .filter(|exit| exit.kind == LexicalExitKind::Return)
            .filter_map(|exit| exit.value)
        {
            if !operands.contains(&operand.id) {
                budget.reserve_btree_node::<ExpressionId, ()>(
                    function,
                    "return operand identity index",
                )?;
            }
            operands.insert(operand.id.clone());
        }
        structural_control::validate_exits(
            program, function, &storage, &leaves, &operands, budget,
        )?;
        operands
    } else {
        let operands = facts
            .exits
            .iter()
            .filter(|exit| exit.kind == LexicalExitKind::Return)
            .filter_map(|exit| exit.value.map(|operand| operand.id.clone()))
            .collect::<BTreeSet<_>>();
        expression_facts::receiving::validate_exits(
            program, function, &storage, &leaves, &operands, budget,
        )?;
        operands
    };
    // Keep the independently validated operand inventory live through certificate authentication.
    let _return_operands = return_operands;
    let auth = lexical_cert::paid::authenticate(program, function, &owner, &facts, budget)?;
    if selected {
        structural_maps::paid_reference::validate(function, budget)?;
        structural_control::validate_reachable_acyclic_cfg(function, budget)?;
    } else {
        structural_maps::validate_reference_coverage(function, budget)?;
        structural_control::validate_ordinary_cfg(function, budget)?;
    }
    // Lexical functions always replay every path: no summary shortcut applies.
    validate_path_states::<expression_facts::errors::EngineError>(
        program,
        function,
        &storage,
        &leaves,
        budget,
        Some(&auth),
        None,
    )?;
    factored::strategy::validate_authenticated(
        program,
        function,
        budget,
        None,
        expression_facts.len(),
    )
    .map_err(expression_facts::errors::EngineError::from)
}
