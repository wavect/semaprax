//! Complete immediate c7 paid driver, not dispatched through new status/scalar helpers.
use super::*;
pub(in crate::cleanup_plan::replay) fn validate_paid(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), expression_facts::errors::EngineError> {
    if !records::signature(program, function, budget)? {
        return Err(replay_error(
            function,
            "selected helper signature is outside Copy scalars",
        )
        .into());
    }
    #[cfg(test)]
    let borrowed = program.borrowed_helper_lease(function);
    #[cfg(test)]
    let has_borrowed = borrowed.is_some();
    #[cfg(not(test))]
    let has_borrowed = false;
    if !has_borrowed {
        scalar_nodes(program, function, budget)?;
    }
    selected_profile::paid::validate(program, function, budget)?;
    selected_owner::paid::owner(program, function, budget)?;
    if !has_borrowed
        && (function.loan_plan.schema != crate::loan_plan::LOAN_PLAN_SCHEMA_V1
            || !function.loan_plan.loans.is_empty()
            || !function.loan_plan.endpoints.is_empty()
            || !function.loan_plan.edges.is_empty())
    {
        return Err(replay_error(
            function,
            "pure scalar helper carries an unexpected loan proof",
        )
        .into());
    }
    #[cfg(test)]
    let selected_schema = match &borrowed {
        Some(lease) => lease.schema(program, function, budget)?,
        None => schema::selected_schema_with_index(program, function, None)?,
    };
    #[cfg(not(test))]
    let selected_schema = schema::selected_schema_with_index(program, function, None)?;
    let plan = &function.cleanup_plan;
    if !budget.selected() || plan.lexical_control.is_some() || plan.schema != selected_schema {
        return Err(replay_error(
            function,
            "selected scalar helper requires its exact nonlexical schema",
        )
        .into());
    }
    budget.charge(
        function,
        plan_structure_units(plan),
        "selected scalar helper structure",
    )?;
    let ordinary_facts;
    #[cfg(test)]
    let borrowed_facts;
    let facts: &dyn expression_facts::Lookup;
    #[cfg(test)]
    if has_borrowed {
        borrowed_facts = expression_facts::BorrowedFacts::build(function, budget)?;
        facts = &borrowed_facts;
    } else {
        ordinary_facts = expression_facts::paid_arena(function, budget)?;
        facts = &ordinary_facts;
    }
    #[cfg(not(test))]
    {
        ordinary_facts = expression_facts::paid_arena(function, budget)?;
        facts = &ordinary_facts;
    }
    inventory_coverage::validate_inventory_coverage(program, function, budget)?;
    status_construction::validate_required_status_sources(program, function, budget)?;
    let storage = structural_maps::paid::slots(
        function, budget,
    )?;
    let leaves = structural_maps::paid::leaves(function, budget)?;
    status_validation::validate_core::<expression_facts::errors::EngineError>(
        function, facts, budget,
    )?;
    structural_maps::paid::regions(function, &storage, budget)?;
    structural_control::validate_entry(function, &storage, &leaves, budget)?;
    let renewals = program
        .primitive_renewals(function)
        .ok_or_else(|| replay_error(function, "selected scalar helper lacks its function lease"))?;
    structural_control::validate_blocks_and_edges_paid::<expression_facts::errors::EngineError>(
        program, function, facts, &storage, &leaves, renewals, budget,
    )?;
    structural_control::validate_exits(
        program,
        function,
        &storage,
        &leaves,
        &BTreeSet::new(),
        budget,
    )?;
    structural_maps::paid_reference::validate(function, budget)?;
    structural_control::validate_reachable_acyclic_cfg(function, budget)?;
    validate_path_states::<expression_facts::errors::EngineError>(
        program, function, &storage, &leaves, budget, None, None,
    )?;
    match factored::compare_with_index(program, function, budget, None)? {
        Some(true) => Ok(()),
        Some(false) => Err(replay_error(
            function,
            "selected scalar helper control trace disagrees with typed HIR",
        )
        .into()),
        None => Err(replay_error(
            function,
            "selected scalar helper trace multiplicity overflowed",
        )
        .into()),
    }
}
