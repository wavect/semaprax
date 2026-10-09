//! Loop Calls v1: the user-function signatures a `while` or `for` body may
//! call.
//!
//! The source verifier, its recursive oracle, HIR resolution, and HIR
//! validation share these predicates so the four admission layers cannot
//! disagree. A loop body is its own per-iteration cleanup region, so a call
//! that consumes a body-local `string`, or returns a new `string`, settles
//! like any other owned call in that region: its staged arguments transfer
//! at the call's commit boundary and an unconsumed result is released when
//! the iteration ends. Exact compiler-owned Copy-scalar Vec parameters may be
//! borrowed without transferring their owner. Declared read-only input effects
//! are allowed. A consumed outer binding still changes ownership liveness
//! inside the loop and keeps its existing diagnostic.

use crate::ast::{ParamMode, Program, Type, TypeDeclarationKind};
use crate::hir::{OwnershipMode, ResolvedType};
use crate::source_verify::is_scalar_source_type;

/// One source parameter a loop-body call admits: a Copy scalar or flat Copy variant, a borrowed
/// byte slice, named `str`, or exact Copy-scalar Vec, or an owned `string` the call consumes.
pub(crate) fn ast_param_admitted(program: &Program, mode: ParamMode, ty: &Type) -> bool {
    match mode {
        ParamMode::Value => {
            ast_copy_variant(program, ty)
                || crate::source_verify::declared_type::copy_record_collection::source_admitted(
                    program, ty,
                )
                || is_scalar_source_type(ty)
                || *ty == Type::String
        }
        ParamMode::Own => *ty == Type::String || crate::map_ops::ast_collection(ty),
        ParamMode::Borrow => {
            matches!(ty, Type::SliceU8 | Type::Str)
                || crate::map_ops::ast_collection(ty)
                || crate::vec_ops::ast_copy_vec(ty)
                || matches!(ty, Type::Named { name, arguments } if name == "Vec" && matches!(arguments.as_slice(), [element] if crate::source_verify::declared_type::copy_record_collection::source_admitted(program, element)))
        }
        ParamMode::Shared => false,
    }
}

/// The view subset reusable by whole-record owner renewal. Admission here
/// establishes only the carrier; ordinary ownership and loan replay must still
/// authenticate its named origin and reject overlap with the renewed owner.
pub(crate) fn ast_renewal_view(ty: &Type) -> bool {
    matches!(ty, Type::SliceU8 | Type::Str)
}

pub(crate) fn resolved_renewal_view(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::SliceU8 | ResolvedType::Str)
}

/// User loop calls may repeat only these already-declared read authorities.
/// The normal effect checker still requires the caller and module to grant
/// each effect; this predicate does not confer authority.
pub(crate) fn effects_admitted(effects: &[String]) -> bool {
    effects.iter().all(|effect| {
        matches!(
            effect.as_str(),
            crate::command_io_ops::ARGS_READ_EFFECT
                | crate::filesystem_ops::READ_EFFECT
                | crate::environment_ops::EFFECT
        )
    })
}

/// One source result a loop-body call admits: a Copy scalar, flat Copy variant or new `string`.
pub(crate) fn ast_result_admitted(program: &Program, ty: &Type) -> bool {
    is_scalar_source_type(ty)
        || *ty == Type::String
        || ast_copy_variant(program, ty)
        || crate::source_verify::declared_type::copy_record_collection::source_admitted(program, ty)
        || crate::map_ops::ast_collection(ty)
}

/// The resolved twin of [`ast_param_admitted`].
pub(crate) fn resolved_param_admitted(
    declarations: &crate::hir::DeclarationIndex,
    ownership: OwnershipMode,
    ty: &ResolvedType,
) -> bool {
    match ownership {
        OwnershipMode::Value => {
            crate::hir::is_scalar_resolved_type(ty)
                || *ty == ResolvedType::String
                || resolved_match_scrutinee_admitted(declarations, ty)
                || crate::hir::copy_record_collection::admitted(declarations, ty)
        }
        OwnershipMode::Own => *ty == ResolvedType::String || crate::map_ops::is_collection(ty),
        OwnershipMode::Borrow => {
            matches!(ty, ResolvedType::SliceU8 | ResolvedType::Str)
                || crate::map_ops::is_collection(ty)
                || crate::vec_ops::resolved_copy_vec(ty)
                || crate::hir::copy_record_collection::is_vec(declarations, ty)
        }
        OwnershipMode::Shared => false,
    }
}

/// The resolved twin of [`ast_result_admitted`].
pub(crate) fn resolved_result_admitted(
    declarations: &crate::hir::DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    crate::hir::is_scalar_resolved_type(ty)
        || *ty == ResolvedType::String
        || resolved_match_scrutinee_admitted(declarations, ty)
        || crate::hir::copy_record_collection::admitted(declarations, ty)
        || crate::map_ops::is_collection(ty)
}

/// Owned String Loops v2: a `match` in a loop body is cleanup-inert when its
/// scrutinee is a Copy scalar or a variant whose every payload is a Copy
/// scalar (payload-free cases, `Option<i64>`, `Result<i64, u8>`, ...). Its
/// arms may still yield a `string`, which joins like any branch result.
pub(crate) fn resolved_match_scrutinee_admitted(
    declarations: &crate::hir::DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    if crate::hir::is_scalar_resolved_type(ty) {
        return true;
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    let Some(cases) = declarations.variant_cases(declaration) else {
        return false;
    };
    cases.iter().all(|case| {
        case.fields.iter().all(|field| match &field.ty {
            ResolvedType::TypeParameter { index, .. } => arguments
                .get(*index as usize)
                .is_some_and(crate::hir::is_scalar_resolved_type),
            other => crate::hir::is_scalar_resolved_type(other),
        })
    })
}

/// The `SPX-T252` refusal for a loop-body `match` over any other scrutinee.
pub(crate) fn match_scrutinee_refusal(ty: &str) -> String {
    format!(
        "a match in a loop body needs a Copy scalar or a variant with only Copy scalar payloads; this one matches `{ty}`, so match it before the loop"
    )
}

/// Exact source twin of the flat concrete Copy-payload variant classifier.
/// Only direct scalar fields or direct concrete scalar type arguments qualify.
pub(crate) fn ast_copy_variant(program: &Program, ty: &Type) -> bool {
    let Type::Named { name, arguments } = ty else {
        return false;
    };
    let Some(declaration) = program
        .types
        .iter()
        .chain(crate::prelude::declarations_for_program(program))
        .find(|d| d.name == *name)
    else {
        return false;
    };
    let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
        return false;
    };
    if arguments.len() != declaration.type_parameters.len() {
        return false;
    }
    cases.iter().all(|case| {
        case.fields.iter().all(|field| {
            if is_scalar_source_type(&field.ty) {
                return true;
            }
            let Type::Named {
                name,
                arguments: nested,
            } = &field.ty
            else {
                return false;
            };
            nested.is_empty()
                && declaration
                    .type_parameters
                    .iter()
                    .position(|parameter| parameter.name == *name)
                    .and_then(|index| arguments.get(index))
                    .is_some_and(is_scalar_source_type)
        })
    })
}
