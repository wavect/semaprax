//! Complete frozen eb02 iterative receiver; private test-only independent scopes.
#![allow(dead_code)]
use super::*;
mod resolve_box_call;
mod resolve_expr;
mod resolve_expr_frame;
mod resolve_for;
mod resolve_for_own;
mod resolve_pattern;
mod resolve_statement;
mod resolve_vec_call;
mod closure {
    use super::*;
    use crate::hir::closure::{borrowed, closure_id, paid, MAX_CAPTURES};
    mod mutable;
    mod once;
    mod resolve;
}
pub(super) struct Resolver<'r, 'p>(pub(super) &'r super::Resolver<'p>);
impl<'r, 'p> std::ops::Deref for Resolver<'r, 'p> {
    type Target = super::Resolver<'p>;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
impl Resolver<'_, '_> {
    pub(super) fn expression(
        &self,
        function: &FunctionExecutionId,
        expr: &crate::ast::Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        self.resolve_expr_iterative(function, expr, bindings, path)
    }
}
