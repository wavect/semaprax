//! Named expression resolution, including source-only row refusal. A row in an
//! argument slot becomes an unbound Row Loan v1 descriptor that only an
//! ordinary call can bind; every other row use is refused here.
use super::path_arena::ExprPath;
use super::*;
impl Resolver<'_, '_> {
    pub(super) fn named_value(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: ExprPath<'_>,
        name: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        if let Some(loan) = self.row_argument(function, expr, bindings, path) {
            return loan;
        }
        if let Some(reference) = self.function_reference(function, expr, bindings, path)? {
            return Ok(reference);
        }
        let binding = bindings.get(name).ok_or_else(|| {
            self.error("SPX-H002", format!("unresolved value `{name}`"), expr.span)
        })?;
        Ok(ResolvedExpr {
            id: path.expression_id(function),
            ty: binding.ty.clone(),
            ownership: binding.ownership,
            kind: ResolvedExprKind::Place(Place {
                root: binding.id.clone(),
                projections: Vec::new(),
            }),
            span: expr.span,
        })
    }
}
