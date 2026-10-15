//! Borrowed call preorder; a refusal stops before subsequent call activity.
use super::*;

pub(crate) fn try_visit_resolved_calls<'e, E>(
    expression: &'e ResolvedExpr,
    visit: &mut impl FnMut(
        &'e DeclarationId,
        Option<&'e FunctionInstanceId>,
        &'e [ResolvedType],
    ) -> Result<(), E>,
) -> Result<(), E> {
    match &expression.kind {
        ResolvedExprKind::FunctionReference { target } => visit(target, None, &[])?,
        ResolvedExprKind::Call {
            callee,
            instance,
            type_arguments,
            ..
        } => visit(callee, instance.as_ref(), type_arguments)?,
        _ => {}
    }
    // Call inspection historically visits a closure's body before captures.
    // The structural cursor uses capture-first runtime order, so retain this
    // explicit branch while sharing its linear block/match enumeration.
    if let ResolvedExprKind::Closure { body, captures, .. } = &expression.kind {
        try_visit_resolved_calls(body, visit)?;
        for capture in captures {
            try_visit_resolved_calls(&capture.value, visit)?;
        }
    } else {
        for child in crate::hir::lexical_exit::ChildCursor::new(expression) {
            try_visit_resolved_calls(child, visit)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
