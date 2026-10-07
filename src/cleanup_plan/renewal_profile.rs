//! Renewal site identity is derived from HIR, independently of attached transitions.
use crate::hir::{ExpressionId, ResolvedBinding, ResolvedFunction, ResolvedProgram};
pub(crate) fn binding<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    at: &ExpressionId,
) -> Option<&'a ResolvedBinding> {
    crate::hir::vec_loop_renewal::binding(function, at)
        .or_else(|| crate::hir::iterator_loop::renewal_binding(program, function, at))
}
