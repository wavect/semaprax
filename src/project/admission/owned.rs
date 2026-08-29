//! Descriptor-first Project-v8/v10 admission.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::super::{ProjectManifest, PublicApiDescriptor, PublicApiSubject};

pub(super) fn prepare(
    program: &ResolvedProgram,
    manifest: &ProjectManifest,
    subject: PublicApiSubject<'_>,
) -> Result<PublicApiDescriptor, Diagnostic> {
    let derived =
        super::super::derive_public_api_descriptor(program, manifest.web_exports(), subject)?;
    let replayed = super::super::replay_public_api_descriptor(
        program,
        manifest.web_exports(),
        subject,
        &derived.canonical_bytes(),
        &derived.digest(),
    )?;
    crate::wasm::emit_resolved_module_with_owned_data_exports(program, &replayed)?;
    Ok(replayed)
}
