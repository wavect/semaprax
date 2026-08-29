//! Descriptor-first Project-v9 flat owned-record admission.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::super::{FlatOwnedRecordApiDescriptor, ProjectManifest, PublicApiSubject};

pub(super) fn prepare(
    program: &ResolvedProgram,
    manifest: &ProjectManifest,
    subject: PublicApiSubject<'_>,
) -> Result<FlatOwnedRecordApiDescriptor, Diagnostic> {
    let derived = super::super::derive_flat_owned_record_api_descriptor(
        program,
        manifest.web_exports(),
        subject,
    )?;
    let replayed = super::super::replay_flat_owned_record_api_descriptor(
        program,
        manifest.web_exports(),
        subject,
        &derived.canonical_bytes(),
        &derived.digest(),
    )?;
    crate::wasm::emit_resolved_module_with_flat_owned_record_exports(program, &replayed)?;
    Ok(replayed)
}
