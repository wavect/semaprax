//! Renewal site identity is derived from HIR, independently of attached transitions.
use crate::hir::{ExpressionId, ResolvedBinding, ResolvedFunction};
pub(crate) fn binding<'a>(
    function: &'a ResolvedFunction,
    at: &ExpressionId,
) -> Option<&'a ResolvedBinding> {
    crate::string_ops::replacement::binding(function, at)
        .or_else(|| crate::hir::vec_loop_renewal::binding(function, at))
        .or_else(|| crate::hir::iterator_loop::renewal_binding(function, at))
}
