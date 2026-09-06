//! Exact compiler-owned `Vec<T>` call lowering shared by the production
//! iterative resolver and its test-only recursive reference.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::ast::{Expr, Span, Type};
use crate::diagnostic::Diagnostic;

use super::expr_nodes::{ResolvedExpr, ResolvedExprKind};
use super::ids::{DeclarationId, ExpressionId, FunctionExecutionId};
use super::nodes::{is_scalar_resolved_type, OwnershipMode, ResolvedBinding, ResolvedType};
use super::resolve_expr_frame::Frame;
use super::{Binding, Resolver};

pub(super) fn validate_whole_assignment(
    resolver: &Resolver<'_>,
    target: &ResolvedBinding,
    value: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    if value.ty != target.ty {
        return Err(resolver.error(
            "SPX-U102",
            format!(
                "assigned value type `{}` does not exactly match binding type `{}`",
                value.ty.identity_key(),
                target.ty.identity_key()
            ),
            value.span,
        ));
    }
    if (value.ownership != OwnershipMode::Value || !is_scalar_resolved_type(&value.ty))
        && !crate::vec_ops::is_same_owner_push_hir(value, &target.id)
    {
        return Err(resolver.error(
            "SPX-U105",
            "explicit mutation v1 supports only scalar Copy values",
            value.span,
        ));
    }
    Ok(())
}

pub(super) fn schedule<'expr>(
    resolver: &Resolver<'_>,
    frames: &mut Vec<Frame<'expr>>,
    type_arguments: &[Type],
    args: &'expr [Expr],
    bindings: Rc<BTreeMap<String, Binding>>,
    site: (String, Span),
    op: crate::vec_ops::VecOp,
) -> Result<(), Diagnostic> {
    let (path, span) = site;
    if type_arguments.len() != 1 || args.len() != op.arity() {
        return Err(resolver.error(
            "SPX-H006",
            format!("invalid vector operation `{}` call shape", op.name()),
            span,
        ));
    }
    let element = resolver.resolve_type(&type_arguments[0], span)?;
    if !crate::vec_ops::resolved_element_is_admitted(&element) {
        return Err(resolver.error(
            "SPX-H006",
            format!(
                "vector operation `{}` has an inadmissible element type",
                op.name()
            ),
            span,
        ));
    }
    frames.push(Frame::FinishVecOp {
        span,
        path: path.clone(),
        op,
        element,
        argument_count: args.len(),
    });
    frames.push(Frame::ChildNext {
        children: args,
        index: 0,
        bindings,
        path,
        segment: "arg",
    });
    Ok(())
}

pub(super) fn finish(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    path: &str,
    span: Span,
    op: crate::vec_ops::VecOp,
    element: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    for (index, argument) in args.iter().enumerate() {
        if !op.accepts_resolved(index, &argument.ty, &element) {
            return Err(resolver.error(
                "SPX-H006",
                format!(
                    "vector operation `{}` argument {index} has the wrong type",
                    op.name()
                ),
                argument.span,
            ));
        }
    }
    let ty = op.resolved_return_type(&element);
    let ownership = resolver.expression_ownership(
        &ty,
        match op {
            crate::vec_ops::VecOp::WithCapacity | crate::vec_ops::VecOp::Push => OwnershipMode::Own,
            _ => OwnershipMode::Value,
        },
        span,
    )?;
    Ok(ResolvedExpr {
        id: ExpressionId::new(function, path),
        ty,
        ownership,
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(op.id()),
            type_arguments: vec![element],
            instance: None,
            args,
        },
        span,
    })
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_reference(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    id: ExpressionId,
    name: &str,
    type_arguments: &[Type],
    args: &[Expr],
    bindings: &BTreeMap<String, Binding>,
    path: &str,
    span: Span,
    op: crate::vec_ops::VecOp,
) -> Result<ResolvedExpr, Diagnostic> {
    if type_arguments.len() != 1 || args.len() != op.arity() {
        return Err(resolver.error(
            "SPX-H006",
            format!("invalid vector operation `{name}` call shape"),
            span,
        ));
    }
    let element = resolver.resolve_type(&type_arguments[0], span)?;
    if !crate::vec_ops::resolved_element_is_admitted(&element) {
        return Err(resolver.error(
            "SPX-H006",
            format!("vector operation `{name}` has an inadmissible element type"),
            span,
        ));
    }
    let args = args
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            resolver.resolve_expr_recursive_reference(
                function,
                argument,
                bindings,
                &format!("{path}.arg.{index}"),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut resolved = finish(resolver, function, path, span, op, element, args)?;
    resolved.id = id;
    Ok(resolved)
}
