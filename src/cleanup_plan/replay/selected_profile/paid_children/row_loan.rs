//! Complete selected row_loan finite-error twins; old bodies remain independent.
use super::collections;
use super::{Deferred, Error, Permit};
use crate::cleanup::ReplayContext;
use crate::cleanup_plan::renewal_profile::{
    child_refusal::Refusal, profile_refusal::ProfileRefusal,
};

use crate::cleanup_plan::replay::ReplayBudget;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction};
pub(super) fn validate_inner(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<(), Deferred> {
    let ResolvedExprKind::RowLoan { loan, args } = &expression.kind else {
        return Ok(());
    };
    collections::prepay_inner(
        program,
        function,
        collections::type_key_bytes_inner(&loan.element)?,
        budget,
    )?;
    if crate::cleanup::selected_inline::selected_flat_record_fields_inline(program, &loan.element)?
        .is_none()
    {
        return Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedRowLoanElementIsNotACurrentFlatRecord,
        )));
    }
    budget.child_charge(function, 8, "selected row loan facts")?;
    let [owner, _] = args.as_slice() else {
        return Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedRowLoanHasInvalidArity,
        )));
    };
    if !crate::cleanup_plan::row_loan::exact_places(args)
        || crate::cleanup::selected_inline::selected_vector_element_inline(program, &owner.ty)?
            != Some(&loan.element)
        || expression.ty != loan.element
        || expression.ownership != OwnershipMode::Borrow
    {
        return Err(Deferred::Finite(Refusal::Profile(
            ProfileRefusal::SelectedRowLoanCarrierIndexOrBorrowedResultDisagrees,
        )));
    }
    Ok(())
}
pub(in crate::cleanup_plan::replay) fn validate(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Explicit legacy/non-owner entry; this result is never certified prepaid.
    if !budget.selected() || budget.physical.is_none() {
        return super::super::row_loan::validate(program, function, expression, budget)
            .map_err(Error::Owned);
    }

    let permit = Permit::profile::<()>(function, budget)?;
    let outcome = validate_inner(program, function, expression, budget);
    permit.resolve(outcome)
}
