//! Target admission gates the aggregate lane applies before it emits anything.
//!
//! Both module profiles ask the same question first: does this program use a
//! semantic profile the WebAssembly lane does not implement? Answering it in
//! one place keeps the two emitters from drifting, and keeps each refusal a
//! stable diagnostic rather than a broken carrier.

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedProgram, ResolvedTypeDeclarationKind};

/// Refuse every semantic profile the aggregate lane cannot lower.
///
/// The SPX-AI-019 owned-record collection element used to be refused here
/// (`SPX-W125`). It is not any more: this lane lowers it per element through
/// the owned-payload host boundary (SPX-AI-020), so it has nothing left to
/// refuse and every admitted element now executes rather than agreeing by
/// refusal. An element outside the admitted shape is still a front-end
/// diagnostic, and `vec_record_payload` re-derives the same admission at the
/// emission boundary.
pub(super) fn reject_unsupported_profiles(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    if program
        .types
        .iter()
        .any(|item| matches!(item.kind, ResolvedTypeDeclarationKind::Resource { .. }))
    {
        return Err(super::resource_gate());
    }
    Ok(())
}
