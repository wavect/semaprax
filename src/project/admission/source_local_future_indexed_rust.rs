//! Closed admission for the RI-13 source-local Future/native-Rust join.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::super::ProjectManifest;

pub(super) fn prepare(
    program: &ResolvedProgram,
    manifest: &ProjectManifest,
) -> Result<crate::resumable_effects::source_signature::SourceEffectSignature, Diagnostic> {
    let dependencies = manifest.rust_dependencies();
    let exact_dependencies = manifest.dependencies().is_empty()
        && manifest.dependency_sources().is_empty()
        && dependencies.len() == 2
        && dependencies[0].name() == "regex"
        && dependencies[0].version() == "=1.13.1"
        && dependencies[0].features().is_empty()
        && dependencies[1].name() == "url"
        && dependencies[1].version() == "=2.5.8"
        && dependencies[1].features().is_empty();
    let exact_exports = manifest
        .web_exports()
        .iter()
        .map(String::as_str)
        .eq(["regex.run", "url.run"]);
    if !exact_dependencies || !exact_exports {
        return Err(Diagnostic::io(
            "SPX-H006",
            "source-local-future-indexed-rust.v1 requires exact regex/url dependencies and exports",
        ));
    }
    super::source_local_future::prepare(program, manifest)
}
