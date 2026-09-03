//! Exact C member paths for authenticated nested record cleanup leaves.

use crate::diagnostic::Diagnostic;
use crate::hir::DeclarationId;

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
