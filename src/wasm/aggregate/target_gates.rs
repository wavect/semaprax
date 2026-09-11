//! Target admission gates the aggregate lane applies before it emits anything.
//!
//! Both module profiles ask the same question first: does this program use a
//! semantic profile the WebAssembly lane does not implement? Answering it in
//! one place keeps the two emitters from drifting, and keeps each refusal a
//! stable diagnostic rather than a broken carrier.

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedProgram, ResolvedTypeDeclarationKind};

/// Refuse every semantic profile the aggregate lane cannot lower.
pub(super) fn reject_unsupported_profiles(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    if program
        .types
        .iter()
        .any(|item| matches!(item.kind, ResolvedTypeDeclarationKind::Resource { .. }))
    {
        return Err(super::resource_gate());
    }
    crate::hir::owned_record_collection::reject_for_target(
        program,
        crate::hir::owned_record_collection::WASM_TARGET_CODE,
        "WebAssembly",
    )
}
