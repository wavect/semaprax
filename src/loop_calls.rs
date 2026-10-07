//! Loop Calls v1: the user-function signatures a `while` or `for` body may
//! call.
//!
//! The source verifier, its recursive oracle, HIR resolution, and HIR
//! validation share these predicates so the four admission layers cannot
//! disagree. A loop body is its own per-iteration cleanup region, so a call
//! that consumes a body-local `string`, or returns a new `string`, settles
//! like any other owned call in that region: its staged arguments transfer
//! at the call's commit boundary and an unconsumed result is released when
//! the iteration ends. A consumed outer binding still changes ownership
//! liveness inside the loop and keeps its existing diagnostic.

use crate::ast::{ParamMode, Type};
use crate::hir::{OwnershipMode, ResolvedType};
use crate::source_verify::is_scalar_source_type;

/// One source parameter a loop-body call admits: a Copy scalar, a borrowed
/// byte slice, or an owned `string` the call consumes.
pub(crate) fn ast_param_admitted(mode: ParamMode, ty: &Type) -> bool {
    match mode {
        ParamMode::Value => is_scalar_source_type(ty) || *ty == Type::String,
        ParamMode::Own => *ty == Type::String,
        ParamMode::Borrow => *ty == Type::SliceU8,
        ParamMode::Shared => false,
    }
}

/// One source result a loop-body call admits: a Copy scalar or a new `string`.
pub(crate) fn ast_result_admitted(ty: &Type) -> bool {
    is_scalar_source_type(ty) || *ty == Type::String
}

/// The resolved twin of [`ast_param_admitted`].
pub(crate) fn resolved_param_admitted(ownership: OwnershipMode, ty: &ResolvedType) -> bool {
    match ownership {
        OwnershipMode::Value => crate::hir::is_scalar_resolved_type(ty) || *ty == ResolvedType::String,
        OwnershipMode::Own => *ty == ResolvedType::String,
        OwnershipMode::Borrow => *ty == ResolvedType::SliceU8,
        OwnershipMode::Shared => false,
    }
}

/// The resolved twin of [`ast_result_admitted`].
pub(crate) fn resolved_result_admitted(ty: &ResolvedType) -> bool {
    crate::hir::is_scalar_resolved_type(ty) || *ty == ResolvedType::String
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
