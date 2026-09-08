use super::*;
use crate::iterator_ops::IteratorOp;
#[allow(clippy::too_many_arguments)]
pub(super) fn schedule<'expr>(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    frames: &mut Vec<Frame<'expr>>,
    type_arguments: &[Type],
    args: &'expr [Expr],
    bindings: Rc<BTreeMap<String, Binding>>,
    path: String,
    span: Span,
    op: IteratorOp,
) -> Result<(), Diagnostic> {
    if type_arguments.len() != 1 || args.len() != 1 {
        return Err(resolver.error(
            "SPX-T290",
            "iterator operations require one concrete scalar type argument and one owned argument",
            span,
        ));
    }
    let element = resolver.resolve_type(&type_arguments[0], span)?;
    if !crate::iterator_ops::resolved_element_is_admitted(&element) {
        return Err(resolver.error(
            "SPX-T290",
            "iterator elements must be concrete Copy scalars",
            span,
        ));
    }
    let _ = function;
    frames.push(Frame::FinishOwnedGenericOp {
        span,
        path: path.clone(),
        op: OwnedGenericCallSite::Iterator(op),
        element,
        argument_count: 1,
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
    function: &FunctionExecutionId,
    path: &str,
    span: Span,
    op: IteratorOp,
    element: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    if !matches!(args.as_slice(),[value] if value.ty==op.resolved_param_type(&element)&&value.ownership==OwnershipMode::Own)
    {
        return Err(Diagnostic::io(
            "SPX-H006",
            "iterator operation requires its exact owned carrier",
        ));
    }
    Ok(ResolvedExpr {
        id: ExpressionId::new(function, path),
        ownership: OwnershipMode::Own,
        ty: op.resolved_return_type(&element),
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
pub(super) fn reference(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    call: ReferenceCall<'_>,
    op: IteratorOp,
) -> Result<ResolvedExpr, Diagnostic> {
    if call.type_arguments.len() != 1 || call.args.len() != 1 {
        return Err(Diagnostic::io("SPX-H006", "invalid iterator call shape"));
    }
    let element = resolver.resolve_type(&call.type_arguments[0], call.span)?;
    if !crate::iterator_ops::resolved_element_is_admitted(&element) {
        return Err(Diagnostic::io("SPX-H006", "invalid iterator element"));
    }
    let argument = resolver.resolve_expr_recursive_reference(
        function,
        &call.args[0],
        call.bindings,
        &format!("{}.arg0", call.path),
    )?;
    finish(function, call.path, call.span, op, element, vec![argument])
}
