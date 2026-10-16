//! Independent recursive literal-format reference resolution.
use super::*;

impl Resolver<'_> {
    pub(super) fn resolve_literal_format_reference(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
        type_arguments: &[crate::ast::Type],
        args: &[Expr],
        id: ExpressionId,
    ) -> Result<ResolvedExpr, Diagnostic> {
        if !type_arguments.is_empty()
            || function.instance().is_some()
            || function
                .monomorphic_declaration()
                .is_none_or(|id| self.declarations.declaration(id).is_none())
        {
            return Err(self.error(
                "SPX-H006",
                "string_format is admitted only in monomorphic ordinary function bodies",
                expr.span,
            ));
        }
        let Some(Expr {
            kind: ExprKind::String(template),
            ..
        }) = args.first()
        else {
            return Err(self.error(
                "SPX-H006",
                "string_format requires a compile-time string literal",
                expr.span,
            ));
        };
        let pieces = crate::literal_format::scan(template)
            .map_err(|reason| self.error("SPX-H006", reason.message(), expr.span))?;
        if args.len() - 1 != crate::literal_format::field_count(&pieces) {
            return Err(self.error(
                "SPX-H006",
                "string_format literal field count does not match arguments",
                expr.span,
            ));
        }
        let resolved = args
            .iter()
            .enumerate()
            .skip(1)
            .map(|(index, argument)| {
                self.resolve_expr_recursive_reference(
                    function,
                    argument,
                    bindings,
                    &format!("{path}.arg.{index}"),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if resolved.iter().any(|arg| {
            !crate::literal_format::accepts_hir_type(&arg.ty)
                || (arg.ty == ResolvedType::String && arg.ownership != OwnershipMode::Own)
        }) {
            return Err(self.error(
                "SPX-H006",
                "string_format values must be i64, u8, usize, bool, or own string",
                expr.span,
            ));
        }
        Ok(ResolvedExpr {
            id,
            ty: ResolvedType::String,
            ownership: self.expression_ownership(
                &ResolvedType::String,
                OwnershipMode::Own,
                expr.span,
            )?,
            kind: ResolvedExprKind::LiteralFormat {
                template: template.clone(),
                args: resolved,
            },
            span: expr.span,
        })
    }
}
