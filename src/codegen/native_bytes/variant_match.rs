//! Follow only canonical continuations from an already selected aggregate join.
use super::*;
impl NativeBytesPlan {
    pub(in crate::codegen) fn apply_variant_match_destructure_at(
        &self,
        expression: &ExpressionId,
        case: &DeclarationId,
    ) -> Result<String, Diagnostic> {
        // Match-keyed TransferVariant rows are post-arm continuations. Only
        // selected scrutinee-field transfers belong before the arm body.
        self.apply_variant_case_at_inner(expression, case, false)
    }

    pub(in crate::codegen) fn apply_variant_join_continuation(
        &self,
        expression: &ExpressionId,
        carrier: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let mut source = CleanupPlace {
            storage: StorageId::Temporary(expression.clone()),
            projections: Vec::new(),
        };
        let mut output = String::new();
        for transition in self.transitions.get(expression).into_iter().flatten() {
            if let CleanupTransition::TransferVariant {
                source: candidate,
                destination,
                variant,
                ..
            } = transition
            {
                if *candidate != source {
                    continue;
                }
                if *variant != layout.variant {
                    return Err(error("variant join continuation identity differs"));
                }
                output.push_str(&self.emit_variant_transfer(
                    candidate,
                    destination,
                    carrier,
                    layout,
                )?);
                source = destination.clone();
            }
        }
        Ok(output)
    }
}

impl NativeBytesPlan {
    pub(in crate::codegen) fn apply_variant_if_branch(
        &self,
        parent: &ExpressionId,
        branch: &crate::hir::ResolvedExpr,
        carrier: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let source = match &branch.kind {
            crate::hir::ResolvedExprKind::Place(place) => {
                let mut projections = Vec::new();
                for projection in &place.projections {
                    let crate::hir::PlaceProjection::Field(field) = projection else {
                        return Err(error("owning variant If place has unsupported projection"));
                    };
                    projections.push(field.clone());
                }
                CleanupPlace {
                    storage: StorageId::Value(place.root.clone()),
                    projections,
                }
            }
            _ => CleanupPlace {
                storage: StorageId::Temporary(branch.id.clone()),
                projections: Vec::new(),
            },
        };
        let destination = CleanupPlace {
            storage: StorageId::Temporary(parent.clone()),
            projections: Vec::new(),
        };
        let count = self.transitions.get(parent).into_iter().flatten().filter(|transition| matches!(transition,
            CleanupTransition::TransferVariant { source: actual_source, destination: actual_destination, variant, .. }
            if *actual_source == source && *actual_destination == destination && *variant == layout.variant)).count();
        if count != 1 {
            return Err(error(
                "owning variant If has no unique canonical branch transfer",
            ));
        }
        self.emit_variant_transfer(&source, &destination, carrier, layout)
    }
}
