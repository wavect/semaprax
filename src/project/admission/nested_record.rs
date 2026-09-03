//! Descriptor-first, authority-free Project-v11 admission.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::super::{NestedOwnedRecordApiDescriptor, ProjectManifest, PublicApiSubject};

pub(super) fn prepare(
    program: &ResolvedProgram,
    manifest: &ProjectManifest,
    subject: PublicApiSubject<'_>,
) -> Result<NestedOwnedRecordApiDescriptor, Diagnostic> {
    let derived = super::super::derive_nested_owned_record_api_descriptor(
        program,
        manifest.web_exports(),
        subject,
    )?;
    let replayed = super::super::replay_nested_owned_record_api_descriptor(
        program,
        manifest.web_exports(),
        subject,
        &derived.canonical_bytes(),
        &derived.digest(),
    )?;
    crate::wasm::emit_resolved_module_with_nested_owned_record_exports(program, &replayed)?;
    Ok(replayed)
}
