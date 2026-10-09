//! Exact owned and fieldless variant classifiers for interpreter admission.
use super::*;

/// Exact non-Copy sum profile admitted by Owned Byte Variant Algebra v1 plus
/// the bounded concrete authored generic extension. Backend selection consumes
/// the shared HIR classifier so it cannot reinterpret generic ownership.
pub(super) fn is_admitted_owned_byte_variant(
    declarations: &hir::DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    if is_admitted_fieldless_variant(declarations, ty) {
        return true;
    }
    if crate::iterator_ops::step_shape(declarations, ty) {
        return true;
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    let Some(item) = declarations.declaration(declaration) else {
        return false;
    };
    if item.kind != hir::DeclarationKind::Variant {
        return false;
    }
    if (item.identity_origin == hir::IdentityOrigin::CompilerOwned
        && hir::admitted_owned_byte_prelude_instance(declaration, arguments))
        || hir::is_admitted_concrete_owned_byte_variant(declarations, ty)
    {
        return true;
    }
    if !arguments.is_empty() {
        return false;
    }
    declarations
        .variant_cases(declaration)
        .is_some_and(|cases| {
            cases
                .iter()
                .flat_map(|case| &case.fields)
                .any(|field| field.ty == ResolvedType::Bytes)
                && cases.iter().flat_map(|case| &case.fields).all(|field| {
                    field.ty == ResolvedType::Bytes || is_admitted_resolved_scalar(&field.ty)
                })
        })
}

/// Shared direct owned-variant classifier for execution paths which can move
/// the selected case payload. Generic and prelude ownership remain Bytes-only;
/// the string branch is the separate direct monomorphic profile. The third
/// arm admits Copy Aggregate Variant Payload v1, whose every case field is
/// drop-free by construction, so the retained-call seam and other execution
/// paths gated on this classifier can transport it too.
pub(super) fn is_admitted_owned_variant(
    declarations: &hir::DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    hir::collection_outcome::runtime_admitted(declarations, ty)
        || is_admitted_owned_byte_variant(declarations, ty)
        || hir::is_admitted_owned_string_variant(declarations, ty)
        || is_admitted_copy_aggregate_variant(declarations, ty)
}

/// Local Copy-scalar variants, including concrete Option/Result instances.
/// Reuse the loop classifier's concrete payload substitution; full HIR replay
/// separately authenticates every constructor, field, pattern and ownership.
pub(super) fn is_admitted_copy_scalar_variant(
    declarations: &hir::DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    matches!(ty, ResolvedType::Nominal { .. })
        && crate::loop_calls::resolved_match_scrutinee_admitted(declarations, ty)
}

/// Copy Aggregate Variant Payload v1: every case field of a monomorphic
/// variant is a direct admitted Copy scalar or a further drop-free
/// Copy-closed nested record (`hir::is_admitted_copy_aggregate_variant_field`).
/// Such a variant owns no cleanup-plan leaf anywhere in its closure, so it is
/// re-derived here purely from the declaration index, mirroring the sibling
/// classifiers above rather than reusing cached source-level `TypeFacts`.
pub(super) fn is_admitted_copy_aggregate_variant(
    declarations: &hir::DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if !arguments.is_empty() {
        return false;
    }
    let Some(item) = declarations.declaration(declaration) else {
        return false;
    };
    if item.kind != hir::DeclarationKind::Variant {
        return false;
    }
    declarations
        .variant_cases(declaration)
        .is_some_and(|cases| {
            !cases.is_empty()
                && cases.iter().flat_map(|case| &case.fields).all(|field| {
                    hir::is_admitted_copy_aggregate_variant_field(declarations, &field.ty)
                })
        })
}

/// A monomorphic fieldless variant carries only a Copy case tag. Keep this
/// bounded value shape distinct from the owned byte-variant admission below.
pub(super) fn is_admitted_fieldless_variant(
    declarations: &hir::DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    arguments.is_empty()
        && declarations.declaration(declaration).is_some_and(|item| {
            item.kind == hir::DeclarationKind::Variant
                && item.identity_origin == hir::IdentityOrigin::Explicit
        })
        && declarations
            .variant_cases(declaration)
            .is_some_and(|cases| {
                !cases.is_empty() && cases.iter().all(|case| case.fields.is_empty())
            })
}

pub(super) fn is_option_u8(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Nominal { declaration, arguments }
            if declaration.as_str() == crate::prelude::OPTION_ID
                && arguments.as_slice() == [ResolvedType::U8]
    )
}

/// Text Toolkit v1: a match whose scrutinee is the direct `string_to_i64`
/// call and whose arms are the guard-free `Some { value }` / `None {}` cases.
pub(super) fn option_i64_match_is_admitted(
    scrutinee: &crate::hir::ResolvedExpr,
    arms: &[crate::hir::ResolvedMatchArm],
) -> bool {
    crate::string_ops::is_to_i64_call_hir(scrutinee)
        && arms.iter().all(|arm| {
            arm.guard.is_none() && option_pattern_is_admitted(&arm.pattern, &ResolvedType::I64)
        })
}

pub(super) fn option_u8_pattern_is_admitted(pattern: &crate::hir::ResolvedMatchPattern) -> bool {
    option_pattern_is_admitted(pattern, &ResolvedType::U8)
}

fn option_pattern_is_admitted(
    pattern: &crate::hir::ResolvedMatchPattern,
    payload: &ResolvedType,
) -> bool {
    let crate::hir::ResolvedMatchPattern::Variant {
        variant,
        case,
        fields,
    } = pattern
    else {
        return false;
    };
    if variant.as_str() != crate::prelude::OPTION_ID {
        return false;
    }
    (case.as_str() == crate::prelude::OPTION_NONE_ID && fields.is_empty())
        || (case.as_str() == crate::prelude::OPTION_SOME_ID
            && fields.len() == 1
            && fields[0].field.as_str() == crate::prelude::OPTION_SOME_VALUE_ID
            && fields[0].binding.ty == *payload)
}
