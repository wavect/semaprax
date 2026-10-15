//! Row Loan v1 node shape in the live validation scope. Its parent call slot,
//! traversal region and callee proof are replayed by `hir::row_loan`.
use super::*;

impl HirValidator<'_> {
    /// Shared shape of the two scoped Vec nodes whose children are exactly a
    /// named Vec place and a usize index: the result type, its ownership and
    /// whether the children sit below a fused byte view.
    pub(super) fn validate_scoped_vec_node(
        &self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &dyn ScopeRead,
    ) -> Result<(ResolvedType, OwnershipMode, bool), Diagnostic> {
        if let ResolvedExprKind::VecFieldRead { origin, bytes, .. } = &expression.kind {
            if *origin == crate::hir::VecFieldReadOrigin::Traversal {
                if let Some(observer) = self.row_loan_observer {
                    observer.set(true);
                }
            }
            let ty = self.validate_vec_field_shape(function, expression, scope)?;
            let ownership = if matches!(ty, ResolvedType::Str | ResolvedType::SliceU8) {
                OwnershipMode::Borrow
            } else {
                OwnershipMode::Value
            };
            return Ok((ty, ownership, *bytes));
        }
        if let Some(observer) = self.row_loan_observer {
            observer.set(true);
        }
        let ty = self.validate_row_loan_shape(function, expression, scope)?;
        Ok((ty, OwnershipMode::Borrow, false))
    }

    /// The descriptor's whole-Vec owner and row index must both be available
    /// exact places of the live scope; the node borrows its exact element.
    fn validate_row_loan_shape(
        &self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &dyn ScopeRead,
    ) -> Result<ResolvedType, Diagnostic> {
        let ResolvedExprKind::RowLoan { loan, args } = &expression.kind else {
            return Err(hir_error("row loan requires its dedicated HIR node"));
        };
        if function.instance().is_some() || function.monomorphic_declaration().is_none() {
            return Err(hir_error(
                "row loans require monomorphic ordinary functions",
            ));
        }
        if loan.version != crate::hir::RowLoanVersion::V1
            || !loan.is_bound()
            || !crate::hir::row_loan::element_admitted(&self.program.declarations, &loan.element)
            || expression.ty != loan.element
            || expression.ownership != OwnershipMode::Borrow
        {
            return Err(hir_error(
                "row loan version, binding, element or borrowed result is invalid",
            ));
        }
        let [source, index] = args.as_slice() else {
            return Err(hir_error(
                "row loan requires exactly Vec and index children",
            ));
        };
        let (ResolvedExprKind::Place(owner), ResolvedExprKind::Place(row)) =
            (&source.kind, &index.kind)
        else {
            return Err(hir_error("row loan children must be exact places"));
        };
        let binding = scope
            .get(&owner.root)
            .ok_or_else(|| hir_error("row loan Vec owner is absent from its live scope"))?;
        let (source_ty, ownership) = self.resolve_place(owner, binding)?;
        let index_binding = scope
            .get(&row.root)
            .ok_or_else(|| hir_error("row loan index is absent from its live scope"))?;
        if !owner.projections.is_empty()
            || binding.mutable
            || !matches!(&source_ty, ResolvedType::Nominal { declaration, arguments }
                if declaration.as_str() == crate::prelude::VEC_ID
                    && arguments.as_slice() == std::slice::from_ref(&loan.element))
            || source.ty != source_ty
            || source.ownership != ownership
            || !matches!(ownership, OwnershipMode::Own | OwnershipMode::Borrow)
            || Self::place_availability(binding, &[]) != Availability::Available
            || !row.projections.is_empty()
            || index_binding.mutable
            || index_binding.ty != ResolvedType::Usize
            || index.ty != ResolvedType::Usize
            || index.ownership != OwnershipMode::Value
        {
            return Err(hir_error(
                "row loan Vec owner or index disagrees with its live immutable traversal",
            ));
        }
        if !crate::hir::row_loan::reserve_type_clone(&loan.element) {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        Ok(loan.element.clone())
    }
}

impl<'a> HirValidator<'a> {
    pub(super) fn with_row_loan_observer(mut self, observer: &'a std::cell::Cell<bool>) -> Self {
        self.row_loan_observer = Some(observer);
        self
    }
}

#[cfg(test)]
mod budget_tests;
