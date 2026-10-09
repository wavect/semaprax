//! Independent recursive record projection reference resolution.
use super::*;

impl Resolver<'_> {
    pub(super) fn resolve_projection_reference(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        base: &Expr,
        field: &str,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<(ResolvedExprKind, ResolvedType, OwnershipMode), Diagnostic> {
        let base = self.resolve_expr_recursive_reference(
            function,
            base,
            bindings,
            &format!("{path}.base"),
        )?;
        let ResolvedType::Nominal {
            declaration: record,
            arguments,
        } = &base.ty
        else {
            return Err(self.error(
                "SPX-H001",
                format!("cannot resolve field `{field}` on a non-record value"),
                expr.span,
            ));
        };
        if self
            .declarations
            .declaration(record)
            .is_none_or(|item| item.kind != DeclarationKind::Record)
        {
            return Err(self.error(
                "SPX-H001",
                format!("cannot resolve field `{field}` on a non-record value"),
                expr.span,
            ));
        }
        let instance_arguments = arguments.clone();
        let field_id = self
            .declarations
            .field_id(record, field)
            .cloned()
            .ok_or_else(|| {
                self.error(
                    "SPX-H001",
                    format!("unresolved field `{field}` on record `{record}`"),
                    expr.span,
                )
            })?;
        let field_ty = self
            .declarations
            .record_fields(record)
            .and_then(|fields| fields.iter().find(|item| item.id == field_id))
            .map(|field| field.ty.clone())
            .ok_or_else(|| {
                self.error(
                    "SPX-H001",
                    format!("field `{field_id}` has no resolved type"),
                    expr.span,
                )
            })?;
        let field_ty = substitute_type(&field_ty, record, &instance_arguments)?;
        let ownership = self.expression_ownership(&field_ty, base.ownership, expr.span)?;
        let kind = match &base.kind {
            ResolvedExprKind::Place(place) => {
                let mut place = place.clone();
                place
                    .projections
                    .push(PlaceProjection::Field(field_id.clone()));
                ResolvedExprKind::Place(place)
            }
            _ => ResolvedExprKind::Project {
                base: Box::new(base),
                field: field_id,
            },
        };
        Ok((kind, field_ty, ownership))
    }
}
