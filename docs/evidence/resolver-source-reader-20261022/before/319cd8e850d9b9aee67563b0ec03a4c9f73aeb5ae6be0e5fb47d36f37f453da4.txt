use super::*;
use crate::list_ops::ListOp;

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule<'expr>(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    frames: &mut Vec<Frame<'expr>>,
    type_arguments: &[Type],
    args: &'expr [Expr],
    bindings: Rc<BTreeMap<String, Binding>>,
    path: PathId,
    span: Span,
    op: ListOp,
) -> Result<(), Diagnostic> {
    if !type_arguments.is_empty() || args.len() != op.argument_count() {
        return Err(resolver.error(
            "SPX-T291",
            "immutable list operation requires its exact fixed arity and no type arguments",
            span,
        ));
    }
    let _ = function;
    frames.push(Frame::FinishOwnedGenericOp {
        span,
        path,
        op: OwnedGenericCallSite::List(op),
        element: ResolvedType::I64,
        argument_count: args.len(),
    });
    frames.push(Frame::ChildNext {
        children: args,
        index: 0,
        bindings,
        path,
        segment: Seg::Arg,
    });
    Ok(())
}

pub(super) fn finish<'p>(
    function: &FunctionExecutionId,
    path: impl Into<ExprPath<'p>>,
    span: Span,
    op: ListOp,
    element: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    if element != ResolvedType::I64
        || args.len() != op.argument_count()
        || args
            .iter()
            .zip(op.resolved_params())
            .any(|(actual, expected)| {
                actual.ty != expected.ty || actual.ownership != expected.ownership
            })
    {
        return Err(Diagnostic::io(
            "SPX-H006",
            "immutable list operation requires its exact checked arguments",
        ));
    }
    Ok(ResolvedExpr {
        id: path.into().expression_id(function),
        ownership: OwnershipMode::Value,
        ty: op.resolved_return_type(),
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(op.id()),
            type_arguments: Vec::new(),
            instance: None,
            args,
        },
        span,
    })
}

#[cfg(test)]
pub(super) fn reference(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    call: ReferenceCall<'_>,
    op: ListOp,
) -> Result<ResolvedExpr, Diagnostic> {
    if !call.type_arguments.is_empty() || call.args.len() != op.argument_count() {
        return Err(Diagnostic::io(
            "SPX-H006",
            "invalid immutable list operation shape",
        ));
    }
    let mut args = Vec::with_capacity(call.args.len());
    for (index, arg) in call.args.iter().enumerate() {
        args.push(resolver.resolve_expr_recursive_reference(
            function,
            arg,
            call.bindings,
            &format!("{}.arg{index}", call.path),
        )?);
    }
    finish(function, call.path, call.span, op, ResolvedType::I64, args)
}
