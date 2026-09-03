//! Reachability and C runtime fragments for rooted owned-String views.

use crate::hir::{ResolvedExprKind, ResolvedProgram};

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
        if matches!(&expression.kind,
            ResolvedExprKind::BorrowPlace { operation, .. }
                if operation.as_str() == crate::byte_ops::STRING_AS_STR_ID)
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
