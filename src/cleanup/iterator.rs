//! Exact conditional iterator and selected native Result ownership.
use crate::hir::{DeclarationId, ResolvedType};

pub(crate) fn variant_record_field(
    program: &crate::hir::ResolvedProgram,
    container: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    ty: &ResolvedType,
) -> bool {
    crate::iterator_ops::step_shape(&program.declarations, container)
        && case.as_str() == crate::iterator_ops::YIELD_ID
        && field.as_str() == crate::iterator_ops::ITEM_ID
        && crate::iterator_ops::element(container) == Some(ty)
        && crate::hir::owned_record_collection::is_admitted_owned_record_collection_element(
            &program.declarations,
            ty,
        )
}

pub(crate) fn variant_leaf_lifecycle<'a>(
    program: &'a crate::hir::ResolvedProgram,
    container: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    ty: &ResolvedType,
) -> Option<&'a str> {
    if let Some(lifecycle) = primitive_leaf_lifecycle(ty) {
        return Some(lifecycle);
    }
    if crate::hir::admitted_ri06_regex_result(program, container)
        && case.as_str() == crate::prelude::RESULT_OK_ID
        && field.as_str() == crate::prelude::RESULT_OK_VALUE_ID
    {
        let ResolvedType::Nominal { arguments, .. } = container else {
            return None;
        };
        if arguments.first() != Some(ty) {
            return None;
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return None;
        };
        if !arguments.is_empty() {
            return None;
        }
        return program.types.iter().find_map(|item| match &item.kind {
            crate::hir::ResolvedTypeDeclarationKind::Resource { drop }
                if &item.id == declaration =>
            {
                Some(drop.id.as_str())
            }
            _ => None,
        });
    }
    (crate::iterator_ops::is_step(container)
        && case.as_str() == crate::iterator_ops::YIELD_ID
        && field.as_str() == crate::iterator_ops::REST_ID
        && crate::iterator_ops::is_iter(ty)
        && crate::iterator_ops::element(container) == crate::iterator_ops::element(ty))
    .then_some(super::ITER_DROP_LIFECYCLE_ID)
}

/// Compiler-owned primitive cleanup identities, independently derived from type.
pub(crate) fn primitive_leaf_lifecycle(ty: &ResolvedType) -> Option<&'static str> {
    match ty {
        ResolvedType::OnceFunction => Some(crate::hir::closure::once::DROP_ID),
        ResolvedType::OnceFunctionI64 => Some(crate::hir::closure::once::MIXED_DROP_ID),
        ResolvedType::OnceFunctionI64Pair => Some(crate::hir::closure::once::PAIR_DROP_ID),
        ResolvedType::Bytes => Some(super::BYTES_DROP_LIFECYCLE_ID),
        ResolvedType::String => Some(super::STRING_DROP_LIFECYCLE_ID),
        ResolvedType::StringMap => Some(crate::string_ops::MAP_DROP_LIFECYCLE_ID),
        ty if crate::map_ops::is_typed_collection(ty) => Some(crate::map_ops::DROP_ID),
        ty if crate::stdin_stream_ops::is_reader(ty) => Some(crate::stdin_stream_ops::DROP_ID),
        _ => None,
    }
}
