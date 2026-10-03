//! Exact Project Phase-A admission for the interpreter-only Rust Future route.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::resumable_effects::source_local_future::admitted_source_future_signature;
use crate::resumable_effects::source_signature::SourceEffectSignature;

use super::super::ProjectManifest;

pub(super) fn prepare(
    program: &ResolvedProgram,
    manifest: &ProjectManifest,
) -> Result<SourceEffectSignature, Diagnostic> {
    let [function_id] = manifest.rust_async_exports() else {
        return Err(Diagnostic::io(
            "SPX-H006",
            "source-local-future.v1 requires exactly one selected Rust async export",
        ));
    };
    if !manifest.web_exports().is_empty() {
        return Err(Diagnostic::io(
            "SPX-H006",
            "source-local-future.v1 has no Web exports",
        ));
    }
    admitted_source_future_signature(program, function_id)
}
