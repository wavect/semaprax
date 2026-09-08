//! Physical moves selected only after the canonical plan authenticates transfer.
use crate::hir::ResolvedType;
pub(super) fn owned_move(ty: &ResolvedType, value: &str) -> String {
    if matches!(ty, ResolvedType::Bytes) {
        format!("spx_bytes_move(&{value})")
    } else if crate::iterator_ops::is_iter(ty) {
        format!("spx_iter_move(spx_ctx, &{value})")
    } else if crate::cleanup::is_owned_bounded_box_type(ty) {
        format!("spx_box_move(spx_ctx, &{value})")
    } else {
        format!("spx_vec_move(spx_ctx, &{value})")
    }
}
