//! Checked synthetic signatures for consuming iterator calls.
use crate::diagnostic::Diagnostic;
use crate::hir::{ExpressionId, ResolvedParam, ResolvedType};
pub(super) fn resolved_params(
    op: crate::iterator_ops::IteratorOp,
    has_instance: bool,
    argument_count: usize,
    type_arguments: &[ResolvedType],
    expression: &ExpressionId,
) -> Result<Vec<ResolvedParam>, Diagnostic> {
    if has_instance
        || argument_count != 1
        || !matches!(type_arguments, [element] if crate::iterator_ops::resolved_element_is_admitted(element))
    {
        return Err(super::plan_error(format!(
            "cleanup iterator call `{expression}` has inconsistent shape"
        )));
    }
    Ok(crate::iterator_ops::resolved_params(op, &type_arguments[0]))
}
