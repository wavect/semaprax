//! Exact old owner checks with prepaid static refusal data.
use super::super::selected_context::paid::refusal;
use super::{ReplayBudget, ResolvedFunction, expression_facts};
pub(in super::super) fn owner(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<crate::hir::FunctionExecutionId, expression_facts::errors::EngineError> {
    if !budget.selected() {
        return crate::hir::lexical_exit::owner(program, function).map_err(Into::into);
    }
    if !program.function_templates().is_empty() || !program.function_instances().is_empty() {
        return Err(refusal(
            function,
            budget,
            "selected lexical owner requires a module-local monomorphic invocation",
        ));
    }
    let mut found = false;
    for index in 0..program.function_count() {
        let current = program
            .function_at(index)
            .ok_or_else(|| refusal(function, budget, "selected member index is incomplete"))?;
        budget.charge(function, 1, "selected current function membership")?;
        if std::ptr::eq(current, function) {
            if found {
                return Err(refusal(
                    function,
                    budget,
                    "selected lexical function member is repeated",
                ));
            }
            found = true;
        }
    }
    if !found {
        return Err(refusal(
            function,
            budget,
            "selected lexical function is not held by its current context",
        ));
    }
    budget.reserve_clone(function, &function.id, "selected execution owner")?;
    let owner = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        lease.validate(program, function, budget)?;
        crate::hir::lexical_exit::validate_monomorphic_owner_with_work(
            function,
            &owner,
            &mut |units| budget.charge(function, units, "borrowed exact owner identity"),
        )?;
        return Ok(owner);
    }
    let permit = super::super::selected_context::child::Permit::profile::<()>(function, budget)?;
    permit.resolve(
        crate::hir::lexical_exit::validate_owner_inline(function, &owner)
            .map_err(super::super::selected_context::child::Deferred::Finite),
    )?;
    Ok(owner)
}
