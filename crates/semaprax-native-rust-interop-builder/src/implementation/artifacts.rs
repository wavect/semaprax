// Pure deterministic artifact projection and structurally independent replay.
// This module has no filesystem, process, platform, settlement, or publication authority.
//
// The projection is split by artifact: `descriptor` and `descriptor_replay`
// own the JSON descriptor, `header`, `c_expression`, and `c_artifact` own the
// generated C, `rust_artifact` owns the generated Rust, and `generated_replay`
// and `c_replay` own the independent replays. The bounded entry points that
// the private lane calls stay here.
use super::*;

mod c_artifact;
mod c_expression;
mod c_replay;
mod descriptor;
mod descriptor_replay;
mod generated_replay;
mod header;
mod rust_artifact;

pub(super) use c_artifact::*;
pub(super) use c_expression::*;
pub(super) use c_replay::*;
pub(super) use descriptor::*;
pub(super) use descriptor_replay::*;
pub(super) use generated_replay::*;
pub(super) use header::*;
pub(super) use rust_artifact::*;

pub(super) fn render_descriptor(
    spec: &Spec,
    hir_digest: &str,
    status_domains: &[String],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<String, Diagnostic> {
    render_descriptor_with_limit(
        spec,
        hir_digest,
        status_domains,
        exports,
        imports,
        MAX_DESCRIPTOR_BYTES,
    )
}

pub(super) fn generate_c(
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<String, Diagnostic> {
    render_exact_artifact("max_generated_c_bytes", MAX_GENERATED_C_BYTES, |sink| {
        generate_c_into(sink, spec, closure, exports, imports)
    })
}

pub(super) fn generate_rust_artifacts(
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(String, String), Diagnostic> {
    generate_rust_artifacts_with_limit(spec, exports, imports, MAX_GENERATED_RUST_BYTES)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_generated_exact(
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
    header: &str,
    c: &str,
    rust: &str,
    ffi: &str,
) -> Result<(), Diagnostic> {
    if !replay_header_exact(header, exports, imports) {
        return Err(b111());
    }
    if !replay_safe_rust_exact(rust, spec, exports, imports)
        || !replay_private_ffi_exact(ffi, spec, exports, imports)
        || !replay_c_exact(c, spec, closure, exports, imports)?
    {
        return Err(b111());
    }
    replay_generated(header, c, rust, ffi)
}
