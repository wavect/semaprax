//! Span-aware HIR validation for borrowed byte arguments and views.

use super::*;

pub(super) fn hir_diagnostic_at_span(
    mut diagnostic: Diagnostic,
    span: crate::ast::Span,
) -> Diagnostic {
    if diagnostic.code == "SPX-H006"
        && diagnostic.span.is_none()
        && span.start < span.end
        && span.line != 0
        && span.column != 0
    {
        diagnostic.span = Some(span);
    }
    diagnostic
}

pub(super) fn hir_error_at_span(span: crate::ast::Span, message: impl Into<String>) -> Diagnostic {
    hir_diagnostic_at_span(hir_error(message), span)
}

impl<'a> HirValidator<'a> {
    pub(super) fn validate_byte_view_place(
        &self,
        operation: crate::byte_ops::ByteOp,
        place: &Place,
        span: crate::ast::Span,
        scope: &BTreeMap<ValueId, ValidationBinding>,
    ) -> Result<(), Diagnostic> {
        let binding = scope
            .get(&place.root)
            .ok_or_else(|| hir_error_at_span(span, "borrowed view root is out of scope"))?;
        if Self::place_availability(binding, &place.projections) != Availability::Available {
            return Err(hir_error_at_span(
                span,
                "borrowed view place is moved or conditionally moved",
            ));
        }
        let (place_ty, place_ownership) = self
            .resolve_place(place, binding)
            .map_err(|diagnostic| hir_diagnostic_at_span(diagnostic, span))?;
        if place.projections.is_empty() {
            if !operation.accepts_resolved(0, &place_ty) {
                return Err(hir_error_at_span(
                    span,
                    "borrowed view root has the wrong storage type",
                ));
            }
            return Ok(());
        }
        if operation == crate::byte_ops::ByteOp::StringAsStr {
            return Err(hir_error_at_span(
                span,
                "owned String view requires one unprojected named storage root",
            ));
        }
        if operation != crate::byte_ops::ByteOp::BytesAsSlice
            || place.projections.is_empty()
            || place
                .projections
                .iter()
                .any(|projection| !matches!(projection, PlaceProjection::Field(_)))
            || binding.ownership != OwnershipMode::Own
            || !super::type_reachability::is_admitted_nested_owned_byte_record(
                &self.program.declarations,
                &binding.ty,
            )
            || place_ty != ResolvedType::Bytes
            || place_ownership != OwnershipMode::Own
        {
            return Err(hir_error_at_span(
                span,
                "projected byte view is outside the exact nested owned-Bytes field profile",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_borrowed_bytes_call_argument(
        &self,
        call: &ResolvedExpr,
        argument: &ResolvedExpr,
        parameter: &ResolvedParam,
        parameter_index: usize,
        scope: &BTreeMap<ValueId, ValidationBinding>,
    ) -> Result<(), Diagnostic> {
        if parameter.ty != ResolvedType::Bytes || parameter.ownership != OwnershipMode::Borrow {
            return Ok(());
        }
        let ResolvedExprKind::Call {
            type_arguments,
            instance,
            ..
        } = &call.kind
        else {
            return Err(hir_error_at_span(
                call.span,
                "borrowed Bytes argument is not attached to an exact call",
            ));
        };
        if instance.is_some() || !type_arguments.is_empty() {
            return Err(hir_error_at_span(
                call.span,
                "borrowed Bytes calls must be monomorphic source-defined calls",
            ));
        }
        let ResolvedExprKind::Place(place) = &argument.kind else {
            return Err(hir_error_at_span(
                argument.span,
                "borrowed Bytes call argument is not an exact storage place",
            ));
        };
        let binding = scope.get(&place.root).ok_or_else(|| {
            hir_error_at_span(argument.span, "borrowed Bytes call root is out of scope")
        })?;
        if Self::place_availability(binding, &place.projections) != Availability::Available {
            return Err(hir_error_at_span(
                argument.span,
                "borrowed Bytes call place is moved or conditionally moved",
            ));
        }
        let (place_ty, place_ownership) = self
            .resolve_place(place, binding)
            .map_err(|diagnostic| hir_diagnostic_at_span(diagnostic, argument.span))?;
        if place_ty != ResolvedType::Bytes || argument.ty != ResolvedType::Bytes {
            return Err(hir_error_at_span(
                argument.span,
                "borrowed Bytes call place has the wrong storage type",
            ));
        }
        let admitted = if place.projections.is_empty() {
            matches!(place_ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        } else {
            !place.projections.is_empty()
                && place
                    .projections
                    .iter()
                    .all(|projection| matches!(projection, PlaceProjection::Field(_)))
                && binding.ownership == OwnershipMode::Own
                && place_ownership == OwnershipMode::Own
                && super::type_reachability::is_admitted_nested_owned_byte_record(
                    &self.program.declarations,
                    &binding.ty,
                )
        };
        if !admitted {
            return Err(hir_error_at_span(
                argument.span,
                "borrowed Bytes call is outside the exact named or nested owned-field profile",
            ));
        }
        if binding.ownership == OwnershipMode::Own {
            let argument = u16::try_from(parameter_index).map_err(|_| {
                hir_error_at_span(call.span, "borrowed Bytes argument index overflows")
            })?;
            if !self
                .canonical_loan_ids
                .contains_key(&(call.id.clone(), LoanCause::BorrowedCall { argument }))
            {
                return Err(hir_error_at_span(
                    call.span,
                    "borrowed Bytes call lacks its canonical shared-loan identity",
                ));
            }
        }
        Ok(())
    }
}
