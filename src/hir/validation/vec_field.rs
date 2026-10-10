//! Independent live-root and result replay for nonallocating element field reads.
use super::*;
impl HirValidator<'_> {
    pub(super) fn validate_vec_field_shape(
        &self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &BTreeMap<ValueId, ValidationBinding>,
    ) -> Result<ResolvedType, Diagnostic> {
        let ResolvedExprKind::VecFieldRead {
            element,
            field,
            bytes,
            args,
        } = &expression.kind
        else {
            return Err(hir_error(
                "scoped vector read requires its dedicated HIR node",
            ));
        };
        if function.instance().is_some() || function.monomorphic_declaration().is_none() {
            return Err(hir_error(
                "scoped vector reads require monomorphic ordinary functions",
            ));
        }
        let [source, index] = args.as_slice() else {
            return Err(hir_error(
                "scoped vector read requires exactly vector and index children",
            ));
        };
        let ResolvedExprKind::Place(place) = &source.kind else {
            return Err(hir_error(
                "scoped vector read requires a named carrier path",
            ));
        };
        let binding = scope
            .get(&place.root)
            .ok_or_else(|| hir_error("scoped vector source is absent from its live scope"))?;
        let (source_ty, ownership) = self.resolve_place(place, binding)?;
        if source_ty != crate::vec_ops::resolved_vec(element.clone())
            || source.ty != source_ty
            || source.ownership != ownership
            || !matches!(ownership, OwnershipMode::Own | OwnershipMode::Borrow)
            || Self::place_availability(binding, &place.projections) != Availability::Available
            || index.ty != ResolvedType::Usize
            || index.ownership != OwnershipMode::Value
        {
            return Err(hir_error(
                "scoped vector read source, ownership or index disagrees with its live carrier",
            ));
        }
        if !place.projections.is_empty()
            && !crate::hir::owned_collection_record::projected_field(
                &self.program.declarations,
                place,
                &source_ty,
            )
        {
            return Err(hir_error(
                "scoped vector read has an unauthenticated projected carrier",
            ));
        }
        super::super::vec_field::field(&self.program.declarations,element,field)
            .and_then(|selected|selected.result_type(*bytes))
            .ok_or_else(|| hir_error("scoped vector field identity, declaration order, type or fused view is invalid"))
    }

    pub(super) fn validate_literal_format_arguments(
        &self,
        function: &FunctionExecutionId,
        template: &str,
        args: &[ResolvedExpr],
    ) -> Result<(), Diagnostic> {
        let pieces =
            crate::literal_format::scan(template).map_err(|reason| hir_error(reason.message()))?;
        if crate::literal_format::field_count(&pieces) != args.len() {
            return Err(hir_error(
                "literal format field count does not match arguments",
            ));
        }
        if args
            .iter()
            .any(|arg| !crate::literal_format::accepts_hir_type(&arg.ty))
        {
            return Err(hir_error("literal format has an unsupported argument type"));
        }
        if function.instance().is_some()
            || function
                .monomorphic_declaration()
                .is_none_or(|id| self.program.declarations.declaration(id).is_none())
        {
            return Err(hir_error(
                "literal format is not admitted in generic or closure bodies",
            ));
        }
        Ok(())
    }
}
