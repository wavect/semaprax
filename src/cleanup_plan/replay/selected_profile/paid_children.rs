//! Actual paid Context helpers; no implicit Diagnostic-to-paid conversion.
pub(super) use super::super::selected_context::child::{Deferred, Permit};
pub(super) type Error = super::super::expression_facts::errors::EngineError;
pub(super) mod collections;
pub(super) mod flat_records;
pub(super) mod row_loan;
pub(super) mod string_transfer;
use crate::cleanup_plan::replay::ReplayBudget;
use crate::hir::{LoopLatchKind, ResolvedExpr, ResolvedFunction};
pub(in crate::cleanup_plan::replay) use collections::{call_facts, field, prepay};
pub(in crate::cleanup_plan::replay) use string_transfer::operand;
pub(super) fn literal(
    function: &ResolvedFunction,
    value: &str,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    if !budget.selected() || budget.physical.is_none() {
        return crate::hir::static_text::validate_literal(value).map_err(Error::Owned);
    }
    let permit = Permit::profile::<()>(function, budget)?;
    let result = crate::hir::static_text::validate_literal_inline(value).map_err(Deferred::Finite);
    permit.resolve(result)
}
pub(super) fn latch(
    function: &ResolvedFunction,
    condition: &ResolvedExpr,
    body: &ResolvedExpr,
    latch: &LoopLatchKind,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    if !budget.selected() || budget.physical.is_none() {
        return crate::hir::lexical_exit::validate_latch(condition, body, latch)
            .map_err(Error::Owned);
    }
    let permit = Permit::profile::<()>(function, budget)?;
    let result = crate::hir::lexical_exit::validate_latch_inline(condition, body, latch)
        .map_err(Deferred::Finite);
    permit.resolve(result)
}

#[cfg(test)]
mod tests;
