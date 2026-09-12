//! Which resolved types the native lane lowers as plan-owned carriers.

use crate::hir::{ResolvedProgram, ResolvedType};

/// The compiler-owned bounded `Vec<T>` carriers the native lane lowers.
///
/// `crate::cleanup::is_owned_bounded_vec_type` answers the target-neutral
/// question from the resolved type alone. The SPX-AI-019 owned-record element
/// profile additionally needs `DeclarationIndex` facts to re-derive its
/// element admission, so it stays a separate question and every native site
/// that maps a carrier onto a machine representation asks this one instead.
/// Widening the shared predicate would silently change the Wasm lane, which
/// still refuses this profile.
pub(in crate::codegen) fn is_native_owned_vec_type(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> bool {
    crate::cleanup::is_owned_bounded_vec_type(ty)
        || crate::hir::owned_record_collection::is_owned_record_vec_type(&program.declarations, ty)
}

/// `true` when the canonical cleanup plan owns `ty` directly, as one leaf,
/// rather than through projected record or variant fields.
pub(super) fn is_direct_plan_owned(program: &ResolvedProgram, ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Bytes)
        || is_native_owned_vec_type(program, ty)
        || crate::cleanup::is_owned_bounded_box_type(ty)
        || crate::iterator_ops::is_iter(ty)
}
