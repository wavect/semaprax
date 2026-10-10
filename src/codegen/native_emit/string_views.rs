//! Reachability and C runtime fragments for rooted owned-String views.

use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedProgram};

pub(super) fn program_has_owned_string_byte_view(program: &ResolvedProgram) -> bool {
    if program
        .declarations
        .byte_slice_provenances()
        .any(|(_, provenance)| provenance.root_kind == crate::hir::ByteSliceRootKind::OwnedString)
    {
        return true;
    }
    // An inline fused view has no local Slice binding and hence no binding
    // provenance entry. Its authenticated field path still needs the Str ABI.
    let mut pending = Vec::new();
    for function in super::string_runtime_functions(program, true) {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if is_projected_string_byte_view(program, expression) {
            return true;
        }
        pending.extend(super::resolved_expr_children(expression));
    }
    false
}

fn is_projected_string_byte_view(program: &ResolvedProgram, expression: &ResolvedExpr) -> bool {
    matches!(&expression.kind,
        ResolvedExprKind::BorrowPlace { operation, place }
            if operation.as_str() == crate::byte_ops::STR_AS_BYTES_ID
                && expression.ty == crate::hir::ResolvedType::SliceU8
                && crate::hir::projected_string_view::path_admitted(
                    &program.declarations, &place.projections))
}

pub(super) fn program_uses_string_as_str(
    program: &ResolvedProgram,
    include_instances: bool,
) -> bool {
    let mut pending = Vec::new();
    for function in super::string_runtime_functions(program, include_instances) {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if is_projected_string_byte_view(program, expression)
            || matches!(&expression.kind,
            ResolvedExprKind::BorrowPlace { operation, .. }
                if operation.as_str() == crate::byte_ops::STRING_AS_STR_ID
                    || operation.as_str() == crate::byte_ops::STR_AS_BYTES_ID
                        && expression.ty == crate::hir::ResolvedType::SliceU8
                        && program.declarations.byte_slice_provenances().any(
                            |(_, provenance)| {
                                provenance.producer.as_ref() == Some(&expression.id)
                                    && provenance.root_kind
                                        == crate::hir::ByteSliceRootKind::OwnedString
                            }
                        ))
        {
            return true;
        }
        pending.extend(super::resolved_expr_children(expression));
    }
    false
}

pub(super) const TERMINATED_RUNTIME_C: &str = r#"static __attribute__((unused)) spx_str_v1 spx_string_as_str(const char *value) {
    spx_str_v1 view = { .data = (const uint8_t *)value, .len = (uint64_t)strlen(value) };
    spx_str_require_valid(view);
    return view;
}
"#;

pub(super) const LENGTH_DELIMITED_RUNTIME_C: &str = r#"static __attribute__((unused)) spx_str_v1 spx_string_as_str(const char *value) {
    spx_str_v1 view = { .data = (const uint8_t *)value, .len = spx_string_length_v10(value) };
    spx_str_require_valid(view);
    return view;
}
"#;
