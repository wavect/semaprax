//! Exact C member paths for authenticated nested record cleanup leaves.

use crate::cleanup_plan::CleanupTransition;
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ExpressionId};

pub(super) fn c_field_path(path: &[DeclarationId]) -> Result<String, Diagnostic> {
    if path.is_empty() {
        return Err(super::error(
            "nested owned Bytes leaf has an empty field path",
        ));
    }
    Ok(path
        .iter()
        .map(crate::codegen::native_emit::c_field_symbol)
        .collect::<Vec<_>>()
        .join("."))
}

pub(super) fn authenticate_transfers_at(
    plan: &super::NativeBytesPlan,
    at: &ExpressionId,
) -> Result<String, Diagnostic> {
    let mut output = String::new();
    for transition in plan.transitions.get(at).into_iter().flatten() {
        let CleanupTransition::Transfer {
            source,
            destination,
            ..
        } = transition
        else {
            continue;
        };
        for (source, destination) in plan.transfer_pairs(source, destination)? {
            output.push_str(&format!(
                "if (!{} || {}) spx_runtime_invariant_failure(\"record match transfer preflight liveness {} to {}\");\n",
                source.flag, destination.flag, source.value, destination.value,
            ));
        }
    }
    Ok(output)
}
