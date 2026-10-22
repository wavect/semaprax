//! Complete immediate preparation receiver; only its test-private name differs.
use super::*;

impl Resolver<'_, '_> {
    pub(in crate::hir) fn prepare_sort_by_original(
        &self,
        function: &FunctionExecutionId,
        expression: &Expr,
        types: &[crate::ast::Type],
        args: &[Expr],
    ) -> Result<(ResolvedType, DeclarationId), Diagnostic> {
        // Field uniqueness shares this literal-metadata route but keeps its own
        // distinct versioned descriptor, borrowed operand and Bool result.
        let unique = matches!(&expression.kind, ExprKind::Call { name, .. }
            if name == crate::vec_ops::field_unique::NAME);
        let update = matches!(&expression.kind, ExprKind::Call { name, .. } if name == crate::vec_ops::field_update::NAME);
        let fail = || {
            if update {
                return self.error(
                    "SPX-H006",
                    crate::vec_ops::field_update::SHAPE,
                    expression.span,
                );
            }
            if unique {
                self.error(
                    "SPX-H006",
                    crate::vec_ops::field_unique::SHAPE,
                    expression.span,
                )
            } else {
                self.error("SPX-H006","vec_sort_by requires a monomorphic explicit record and one finite literal key list",expression.span)
            }
        };
        if function.instance().is_some() {
            return Err(fail());
        }
        let [ty] = types else {
            return Err(fail());
        };
        let literal = if update {
            let [_, _, _, literal] = args else {
                return Err(fail());
            };
            literal
        } else {
            let [_, literal] = args else {
                return Err(fail());
            };
            literal
        };
        let ExprKind::String(keys) = &literal.kind else {
            return Err(fail());
        };
        let element = self.resolve_type(ty, expression.span)?;
        let descriptor = if update {
            crate::vec_ops::field_update::resolve(&self.declarations, &element, keys)
                .and_then(|d| d.identity())
        } else if unique {
            crate::vec_ops::field_unique::resolve(&self.declarations, &element, keys)
                .and_then(|d| d.identity())
        } else {
            crate::vec_ops::sort_keys::resolve(&self.declarations, &element, keys)
                .and_then(|d| d.identity())
        }
        .ok_or_else(fail)?;
        Ok((element, DeclarationId::new(descriptor)))
    }
}
