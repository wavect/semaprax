//! Physical moves selected only after the canonical plan authenticates transfer.
use crate::hir::ResolvedType;
pub(super) fn owned_move(ty: &ResolvedType, value: &str) -> String {
    if ty == &ResolvedType::OnceFunctionI64 {
        format!("spx_once_i64_move_v2(&{value})")
    } else if ty == &ResolvedType::OnceFunctionI64Pair {
        format!("spx_once_i64_pair_move_v3(&{value})")
    } else if matches!(ty, ResolvedType::OnceFunction) {
        format!("spx_once_move(&{value})")
    } else if matches!(ty, ResolvedType::Bytes) {
        format!("spx_bytes_move(&{value})")
    } else if matches!(ty, ResolvedType::String) || crate::map_ops::is_collection(ty) {
        value.to_owned()
    } else if crate::iterator_ops::is_iter(ty) {
        format!("spx_iter_move(spx_ctx, &{value})")
    } else if crate::cleanup::is_owned_bounded_box_type(ty) {
        format!("spx_box_move(spx_ctx, &{value})")
    } else {
        format!("spx_vec_move(spx_ctx, &{value})")
    }
}
