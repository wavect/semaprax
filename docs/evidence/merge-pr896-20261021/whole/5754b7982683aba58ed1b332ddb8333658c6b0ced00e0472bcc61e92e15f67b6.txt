//! Complete immediate pre-path paid owner, all old children remain independent.
use super::*;
pub(in crate::cleanup_plan::replay) fn validate_before_selected_path(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), expression_facts::errors::EngineError> {
    let permit = selected_context::child::Permit::acquire::<()>(function, budget)?;
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
        None => crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &dyn crate::cleanup::ReplayContext,
                &crate::hir::ResolvedFunction,
                &mut crate::cleanup_plan::replay::ReplayBudget,
                Result<
                    &'static str,
                    crate::cleanup_plan::replay::expression_facts::errors::EngineError,
                >
            ),
            4,
            "selected scalar schema call receiving",
            schema::selected_paid::selected(program, function, budget)
        )?,
    };
    #[cfg(not(test))]
    let selected_schema = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &dyn crate::cleanup::ReplayContext,
            &crate::hir::ResolvedFunction,
            &mut crate::cleanup_plan::replay::ReplayBudget,
            Result<
                &'static str,
                crate::cleanup_plan::replay::expression_facts::errors::EngineError,
            >
        ),
        4,
        "selected scalar schema call receiving",
        schema::selected_paid::selected(program, function, budget)
    )?;
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
    inventory_coverage::paid::validate_inventory_coverage(program, function, budget)?;
    status_construction::paid::validate_required_status_sources(program, function, budget)?;
    let storage = structural_maps::paid::slots(function, budget)?;
    let leaves = structural_maps::paid::leaves(function, budget)?;
    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &dyn expression_facts::Lookup,
            &mut ReplayBudget,
            Result<(), expression_facts::errors::EngineError>
        ),
        4,
        "selected structural driver call receiving",
        structural_control::selected::validate_status(function, facts, budget,)
    )?;
    structural_maps::paid::regions(function, &storage, budget)?;
    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &BTreeSet<StorageId>,
            &Leaves,
            &mut ReplayBudget,
            Result<(), expression_facts::errors::EngineError>
        ),
        5,
        "selected structural driver call receiving",
        structural_control::selected::validate_entry(function, &storage, &leaves, budget)
    )?;
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
    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &dyn crate::cleanup::ReplayContext,
            &ResolvedFunction,
            &dyn expression_facts::Lookup,
            &BTreeSet<StorageId>,
            &Leaves,
            &crate::cleanup_plan::ReplayRenewalBindings<'_>,
            &mut ReplayBudget,
            Result<(), expression_facts::errors::EngineError>
        ),
        8,
        "selected structural driver call receiving",
        structural_control::selected::validate_blocks_and_edges_paid(
            program, function, facts, &storage, &leaves, renewals, budget,
        )
    )?;
    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &dyn crate::cleanup::ReplayContext,
            &ResolvedFunction,
            &BTreeSet<StorageId>,
            &Leaves,
            &BTreeSet<ExpressionId>,
            BTreeSet<ExpressionId>,
            &mut ReplayBudget,
            Result<(), expression_facts::errors::EngineError>
        ),
        8,
        "selected structural driver call receiving",
        structural_control::selected::validate_exits(
            program,
            function,
            &storage,
            &leaves,
            &BTreeSet::new(),
            budget,
        )
    )?;
    structural_maps::paid_reference::validate(function, budget)?;
    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &mut ReplayBudget,
            Result<(), expression_facts::errors::EngineError>
        ),
        3,
        "selected structural driver call receiving",
        structural_control::selected::validate_reachable_acyclic_cfg(function, budget)
    )?;
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
