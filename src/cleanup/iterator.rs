//! Exact conditional iterator ownership; authored variants retain prior admission.
use crate::hir::{DeclarationId, ResolvedType};
pub(crate) fn variant_leaf_lifecycle(
    container: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    ty: &ResolvedType,
) -> Option<&'static str> {
    if *ty == ResolvedType::Bytes {
        return Some(super::BYTES_DROP_LIFECYCLE_ID);
    }
    (crate::iterator_ops::is_step(container)
        && case.as_str() == crate::iterator_ops::YIELD_ID
        && field.as_str() == crate::iterator_ops::REST_ID
        && crate::iterator_ops::is_iter(ty)
        && crate::iterator_ops::element(container) == crate::iterator_ops::element(ty))
    .then_some(super::ITER_DROP_LIFECYCLE_ID)
}
