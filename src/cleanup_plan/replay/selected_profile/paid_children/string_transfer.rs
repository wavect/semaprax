//! Complete selected string_transfer finite-error twins; old bodies remain independent.
use crate::cleanup_plan::renewal_profile::{
    child_refusal::Refusal, profile_refusal::ProfileRefusal,
};

use super::{Deferred, Error, Permit};

use crate::cleanup_plan::replay::ReplayBudget;
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedFunction};
pub(super) fn operand_inner<'a>(
    function: &ResolvedFunction,
    expression: &'a ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Option<&'a ResolvedExpr>, Deferred> {
    let ResolvedExprKind::Call { callee, .. } = &expression.kind else {
        return Ok(None);
    };
    budget.child_charge(
        function,
        callee.as_str().len().min(crate::string_ops::take::ID.len()) + 1,
        "selected String transfer operation identity",
    )?;
    if callee.as_str() != crate::string_ops::take::ID {
        return Ok(None);
    }
    budget.child_charge(
        function,
        crate::string_ops::take::ID.len() + 2,
        "selected String transfer exact operand shape and repeated identity",
    )?;
    crate::string_ops::take::operand(expression)
        .map(Some)
        .ok_or_else(|| {
            Deferred::Finite(Refusal::Profile(
                ProfileRefusal::SelectedStringTransferRequiresOneNamedOwnedString,
            ))
        })
}
pub(in crate::cleanup_plan::replay) fn operand<'a>(
    function: &ResolvedFunction,
    expression: &'a ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Option<&'a ResolvedExpr>, Error> {
    // Explicit legacy/non-owner entry; this result is never certified prepaid.
    if !budget.selected() || budget.physical.is_none() {
        return super::super::string_transfer::operand(function, expression, budget)
            .map_err(Error::Owned);
    }

    let permit = Permit::profile::<Option<&'a ResolvedExpr>>(function, budget)?;
    let outcome = operand_inner(function, expression, budget);
    permit.resolve(outcome)
}
