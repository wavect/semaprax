//! Differential lexical target resolution; no production resolver frames are reused.
use super::*;
pub(super) fn exit(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    expr: &Expr,
    bindings: &BTreeMap<String, Binding>,
    path: &str,
    nearest: Option<&crate::hir::ControlTarget>,
) -> Result<(ResolvedExprKind, ResolvedType, OwnershipMode), Diagnostic> {
    let ExprKind::LexicalExit { kind, value } = &expr.kind else {
        unreachable!()
    };
    let target = match kind {
        crate::ast::LexicalExitKind::Return => {
            crate::hir::ControlTarget::Function(function.clone())
        }
        _ => nearest.cloned().ok_or_else(|| {
            resolver.error(
                "SPX-T313",
                "lexical loop exit has no enclosing loop",
                expr.span,
            )
        })?,
    };
    let value = value
        .as_ref()
        .map(|operand| {
            resolver
                .resolve_expr_recursive_reference_inner(
                    function,
                    operand,
                    bindings,
                    &format!("{path}.value"),
                    None,
                )
                .map(Box::new)
        })
        .transpose()?;
    Ok((
        ResolvedExprKind::LexicalExit {
            kind: *kind,
            target,
            value,
        },
        ResolvedType::Never,
        OwnershipMode::Value,
    ))
}
pub(super) fn while_loop(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    condition: &Expr,
    body: &Expr,
    scope: &BTreeMap<String, Binding>,
    path: &str,
    span: crate::ast::Span,
    nearest: Option<&crate::hir::ControlTarget>,
) -> Result<ResolvedStatement, Diagnostic> {
    resolver.reject_while_disallowed_scoped(condition, Some(function))?;
    resolver.reject_while_disallowed_in_scope(body, Some(function), Some(scope))?;
    let resolved_condition = resolver.resolve_expr_recursive_reference_inner(
        function,
        condition,
        scope,
        &format!("{path}.condition"),
        nearest,
    )?;
    if resolved_condition.ty != ResolvedType::Bool {
        return Err(resolver.error("SPX-T251", "`while` condition must be bool", condition.span));
    }
    let target = crate::hir::ControlTarget::Loop {
        owner: function.clone(),
        site: resolved_condition.id.clone(),
    };
    let resolved_body = resolver.resolve_expr_recursive_reference_inner(
        function,
        body,
        scope,
        &format!("{path}.body"),
        Some(&target),
    )?;
    let span = condition.span.merge(span);
    if crate::hir::lexical_exit::contains(&resolved_body) {
        Ok(ResolvedStatement::LexicalWhile {
            condition: Box::new(resolved_condition),
            body: Box::new(resolved_body),
            latch: crate::hir::LoopLatchKind::Condition,
            span,
        })
    } else {
        Ok(ResolvedStatement::While {
            condition: Box::new(resolved_condition),
            body: Box::new(resolved_body),
            span,
        })
    }
}

pub(super) fn if_expression(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    expr: &Expr,
    bindings: &BTreeMap<String, Binding>,
    path: &str,
    nearest: Option<&crate::hir::ControlTarget>,
) -> Result<(ResolvedExprKind, ResolvedType, OwnershipMode), Diagnostic> {
    let ExprKind::If {
        condition,
        then_branch,
        else_branch,
    } = &expr.kind
    else {
        unreachable!()
    };

    let condition = resolver.resolve_expr_recursive_reference_inner(
        function,
        condition,
        bindings,
        &format!("{path}.condition"),
        nearest,
    )?;
    let then_branch = resolver.resolve_expr_recursive_reference_inner(
        function,
        then_branch,
        bindings,
        &format!("{path}.then"),
        nearest,
    )?;
    let else_branch = resolver.resolve_expr_recursive_reference_inner(
        function,
        else_branch,
        bindings,
        &format!("{path}.else"),
        nearest,
    )?;
    let normal = if then_branch.ty == ResolvedType::Never {
        &else_branch
    } else {
        &then_branch
    };
    let ty = normal.ty.clone();
    let ownership = normal.ownership;
    Ok((
        ResolvedExprKind::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        },
        ty,
        ownership,
    ))
}

impl Resolver<'_> {
    #[cfg(test)]
    #[allow(dead_code)]
    pub(in crate::hir) fn resolve_expr_recursive_reference(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        self.resolve_expr_recursive_reference_inner(function, expr, bindings, path, None)
    }
}
