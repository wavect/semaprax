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

pub(in crate::cleanup_plan::replay) use collections::type_key_bytes;

pub(in crate::cleanup_plan::replay) mod byte_iterator {
    //! Closed preflight for the compiler-lowered Bytes iterator protocol.
    //! Latch and cleanup replay remain independent of these shape checks.
    use crate::hir::{
        OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedMatchArm,
        ResolvedMatchMode, ResolvedMatchPattern, ResolvedType,
    };
    use crate::iterator_ops::{self, IteratorOp};

    use super::super::super::selected_context::paid::refusal;
    use super::{Error, ReplayBudget};

    fn nominal(ty: &ResolvedType, id: &str) -> bool {
        matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == id && arguments.as_slice() == [ResolvedType::Bytes])
    }

    pub(in super::super) fn local_type(ty: &ResolvedType) -> bool {
        nominal(ty, iterator_ops::ITER_ID) || nominal(ty, iterator_ops::STEP_ID)
    }

    pub(in super::super) fn step(ty: &ResolvedType) -> bool {
        nominal(ty, iterator_ops::STEP_ID)
    }

    pub(in super::super) fn done(expression: &ResolvedExpr) -> bool {
        matches!(&expression.kind, ResolvedExprKind::ConstructVariant { variant, case, fields }
        if variant.as_str() == iterator_ops::STEP_ID && case.as_str() == iterator_ops::DONE_ID
            && fields.is_empty() && step(&expression.ty) && expression.ownership == OwnershipMode::Own)
    }

    pub(in crate::cleanup_plan::replay) fn call(
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        op: IteratorOp,
        budget: &mut ReplayBudget,
    ) -> Result<(), Error> {
        type Controls<'a> = (
            &'a ResolvedFunction,
            &'a ResolvedExpr,
            &'a mut ReplayBudget,
            IteratorOp,
            Option<&'a ResolvedExpr>,
            (&'static str, &'static str),
            Result<(), Error>,
        );
        budget.reserve_array::<Controls<'_>>(function, 2, "selected Bytes iterator call")?;
        budget.charge(function, 8, "selected Bytes iterator call")?;
        let (input, output) = match op {
            IteratorOp::VecIntoIter => (crate::prelude::VEC_ID, iterator_ops::ITER_ID),
            IteratorOp::Next => (iterator_ops::ITER_ID, iterator_ops::STEP_ID),
        };
        if !matches!(&expression.kind, ResolvedExprKind::Call { type_arguments, instance: None, args, .. }
        if type_arguments.as_slice() == [ResolvedType::Bytes]
            && matches!(args.as_slice(), [arg] if nominal(&arg.ty, input) && arg.ownership == OwnershipMode::Own))
            || !nominal(&expression.ty, output)
            || expression.ownership != OwnershipMode::Own
        {
            return Err(refusal(
                function,
                budget,
                "selected Bytes iterator call shape disagrees",
            ));
        }
        Ok(())
    }

    pub(in super::super) fn arms(
        function: &ResolvedFunction,
        mode: ResolvedMatchMode,
        arms: &[ResolvedMatchArm],
        budget: &mut ReplayBudget,
    ) -> Result<(), Error> {
        type Controls<'a> = (
            &'a ResolvedFunction,
            ResolvedMatchMode,
            &'a [ResolvedMatchArm],
            &'a mut ReplayBudget,
            OwnershipMode,
            [&'a ResolvedMatchArm; 2],
            [&'a ResolvedMatchPattern; 2],
            Result<(), Error>,
        );
        budget.reserve_array::<Controls<'_>>(function, 2, "selected Bytes iterator match")?;
        budget.charge(function, 12, "selected Bytes iterator match")?;
        let ownership = match mode {
            ResolvedMatchMode::Own => OwnershipMode::Own,
            ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
            ResolvedMatchMode::Value => {
                return Err(refusal(
                    function,
                    budget,
                    "selected Bytes iterator match mode disagrees",
                ));
            }
        };
        if !matches!(arms, [done, yielded]
        if done.guard.is_none() && yielded.guard.is_none()
            && matches!(&done.pattern, ResolvedMatchPattern::Variant { variant, case, fields }
                if variant.as_str() == iterator_ops::STEP_ID && case.as_str() == iterator_ops::DONE_ID && fields.is_empty())
            && matches!(&yielded.pattern, ResolvedMatchPattern::Variant { variant, case, fields }
                if variant.as_str() == iterator_ops::STEP_ID && case.as_str() == iterator_ops::YIELD_ID
                    && matches!(fields.as_slice(), [item, rest]
                        if item.field.as_str() == iterator_ops::ITEM_ID
                            && item.binding.ty == ResolvedType::Bytes && item.binding.ownership == ownership
                            && rest.field.as_str() == iterator_ops::REST_ID
                            && nominal(&rest.binding.ty, iterator_ops::ITER_ID) && rest.binding.ownership == ownership)))
        {
            return Err(refusal(
                function,
                budget,
                "selected Bytes iterator cases disagree",
            ));
        }
        Ok(())
    }
}
