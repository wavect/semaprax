//! The source marker reuses one ordinary Block expression/result identity.
use crate::ast::Expr;
use crate::diagnostic::Diagnostic;
use crate::hir::borrowed_root_oracle::resolve_expr_frame::Frame;
use crate::hir::{
    Binding, FunctionExecutionId, ResolvedExpr, ResolvedExprKind, ResolvedType,
};
use std::{collections::BTreeMap, rc::Rc};
use super::super::Resolver;

pub(super) fn schedule<'a>(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    expression: &'a Expr,
    bindings: Rc<BTreeMap<String, Binding>>,
    path: super::path_arena::PathId,
    frames: &mut Vec<Frame<'a>>,
) -> Result<(), Diagnostic> {
    let expansion =
        crate::ast::vec_constructor::inspect(expression).map_err(crate::hir::hir_error)?;
    let element =
        resolver.resolve_call_type_argument(function, expansion.element, expression.span)?;
    frames.push(Frame::FinishVecConstructor {
        element,
        count: expansion.count,
    });
    frames.push(Frame::Enter {
        expr: expansion.value,
        bindings,
        path,
    });
    Ok(())
}
pub(in crate::hir) fn finish(
    results: &mut Vec<ResolvedExpr>,
    element: ResolvedType,
    count: u32,
) -> Result<(), Diagnostic> {
    let mut expression = results.pop().expect("constructor block resolved");
    let ResolvedExprKind::Block { statements, tail } = expression.kind else {
        return Err(crate::hir::hir_error(
            "Vec constructor did not lower to its block",
        ));
    };
    expression.kind = ResolvedExprKind::VecConstructor {
        element,
        count,
        statements,
        tail,
    };
    crate::hir::vec_constructor::authenticate(&expression)?;
    results.push(expression);
    Ok(())
}

#[cfg(test)]
impl Resolver<'_, '_> {
    pub(in crate::hir) fn resolve_constructor_reference(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
        nearest: Option<&crate::hir::ControlTarget>,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let expansion =
            crate::ast::vec_constructor::inspect(expr).map_err(crate::hir::hir_error)?;
        let element = self.resolve_call_type_argument(function, expansion.element, expr.span)?;
        let resolved = self.resolve_expr_recursive_reference_inner(
            function,
            expansion.value,
            bindings,
            path,
            nearest,
        )?;
        let mut results = vec![resolved];
        finish(&mut results, element, expansion.count)?;
        Ok(results.pop().expect("reference constructor result"))
    }
}
