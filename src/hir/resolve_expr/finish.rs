//! Finish resolver result stacks and already-resolved class-prefix projections.
use super::*;
impl Resolver<'_> {
    pub(super) fn finish_upcast(
        &self,
        function: &FunctionExecutionId,
        path: String,
        holder: DeclarationId,
        span: crate::ast::Span,
        source: ResolvedExpr,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let declared = ResolvedType::Nominal {
            declaration: holder,
            arguments: Vec::new(),
        };
        Ok(ResolvedExpr {
            id: ExpressionId::new(function, &path),
            ty: declared.clone(),
            ownership: self.expression_ownership(&declared, OwnershipMode::Own, span)?,
            kind: ResolvedExprKind::Upcast {
                source: Box::new(source),
            },
            span,
        })
    }
}

impl Resolver<'_> {
    pub(super) fn finish_expression_results(
        &self,
        mut results: Vec<ResolvedExpr>,
        span: crate::ast::Span,
    ) -> Result<ResolvedExpr, Diagnostic> {
        if results.len() != 1 {
            return Err(self.error(
                "SPX-H006",
                "iterative expression resolver finished with an invalid result stack",
                span,
            ));
        }
        results.pop().ok_or_else(|| {
            self.error(
                "SPX-H006",
                "iterative expression resolver lost its root result",
                span,
            )
        })
    }
}
