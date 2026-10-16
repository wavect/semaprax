//! Complete immediate836 driver, independent of new certificate/precision route.
use super::*;
pub(in crate::cleanup_plan::replay) mod records;
mod paid_nodes;
pub(in crate::cleanup_plan::replay) fn validate_paid(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), expression_facts::errors::EngineError> {
    let permit = selected_context::child::Permit::profile::<()>(function, budget)?;
    if !records::paid::signature(program, function, budget)? {
        return Err(permit.refuse(
            crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Scalar(
                crate::cleanup_plan::renewal_profile::scalar_refusal::ScalarRefusal::Signature,
            ),
        ));
    }
    #[cfg(test)]
    let borrowed = program.borrowed_helper_lease(function);
    #[cfg(test)]
    let has_borrowed = borrowed.is_some();
    #[cfg(not(test))]
    let has_borrowed = false;
    if !has_borrowed {
        paid_nodes::validate(program, function, budget)?;
    }
    selected_profile::paid::validate(program, function, budget)?;
    selected_owner::paid::owner(program, function, budget)?;
    if !has_borrowed
        && (function.loan_plan.schema != crate::loan_plan::LOAN_PLAN_SCHEMA_V1
            || !function.loan_plan.loans.is_empty()
            || !function.loan_plan.endpoints.is_empty()
            || !function.loan_plan.edges.is_empty())
    {
        return Err(permit.refuse(
            crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Scalar(
                crate::cleanup_plan::renewal_profile::scalar_refusal::ScalarRefusal::Loan,
            ),
        ));
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
        return Err(permit.refuse(
            crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Scalar(
                crate::cleanup_plan::renewal_profile::scalar_refusal::ScalarRefusal::Schema,
            ),
        ));
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
    status_construction::paid::validate_required_status_sources(program, function, budget)?;
    let storage = structural_maps::paid::slots(function, budget)?;
    let leaves = structural_maps::paid::leaves(function, budget)?;
    status_validation::validate_core::<expression_facts::errors::EngineError>(
        function, facts, budget,
    )?;
    structural_maps::paid::regions(function, &storage, budget)?;
    structural_control::validate_entry(function, &storage, &leaves, budget)?;
    let renewals = match program.primitive_renewals(function) {
        Some(value) => value,
        None => {
            return Err(permit.refuse(
                crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Scalar(
                    crate::cleanup_plan::renewal_profile::scalar_refusal::ScalarRefusal::Lease,
                ),
            ))
        }
    };
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
        Some(false) => Err(permit.refuse(
            crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Scalar(
                crate::cleanup_plan::renewal_profile::scalar_refusal::ScalarRefusal::Trace,
            ),
        )),
        None => Err(permit.refuse(
            crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Scalar(
                crate::cleanup_plan::renewal_profile::scalar_refusal::ScalarRefusal::Multiplicity,
            ),
        )),
    }
}
