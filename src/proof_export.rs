//! `semaprax.lean-obligation-export.v1`: deterministic export of a small
//! pure SEMAPRAX subset to Lean 4, honest coverage accounting for
//! everything outside it, a fail-closed parser for a pinned Lean kernel's
//! verdict, and `semaprax.lean-proof-certificate.v1` — a certificate
//! binding an accepted result to exact source bytes, semantic revision,
//! compiler identity, and one compiled artifact.
//!
//! One tranche of issue #186. Read the "What this is not" section before
//! quoting anything from here.
//!
//! # Where this sits relative to what already exists
//!
//! This module is deliberately *not* a second proof pipeline. It reuses:
//!
//! - the bounded-subset vocabulary
//!   ([`crate::assurance_manifest::smt_discharge::UnsupportedReason`],
//!   `Sort`, `NumericMode` and its checked-range facts), so a Lean coverage
//!   report and an SMT one speak one language;
//! - [`crate::assurance_manifest::smt_discharge::postcondition_obligation_id`]
//!   for postcondition obligation identity, so an obligation has the same
//!   id whichever method addresses it;
//! - [`crate::assurance_manifest::proof_certificate::ExternalKernelCapability`],
//!   the seam that module introduced *for this issue* ("a future
//!   obligation-export translator targeting a real proof-assistant kernel
//!   … can implement the trait and reuse this module's binding ordering
//!   guarantee"), rather than inventing a second one;
//! - the pinned Lean toolchain of `proofs/kernel0-lean` (issue #188), rather
//!   than introducing a second Lean pin.
//!
//! # What this is not
//!
//! - **No Lean toolchain is invoked from this crate.** Running Lean is
//!   expressed as the [`LeanKernel`] capability, which the caller supplies;
//!   no implementation of it ships here, so nothing in this module acquires
//!   ambient process authority, and nothing here has ever executed a Lean
//!   kernel of its own accord.
//! - **A certificate is proof data, not authority.** It grants no
//!   execution, publication, signing, or merge permission. A separately
//!   replayed ProgramRoot association may turn its selected postcondition
//!   into one exact Assurance Manifest method attachment, but that remains
//!   evidence data rather than a permission or candidate-acceptance grant.
//! - **No ProgramRoot binding.** A managed-workspace `ProgramRoot`
//!   ([`crate::project::ProgramRoot`]) is derived from a
//!   `SemanticWorkspaceRevision`; this export binds a single source file's
//!   semantic revision ([`graph::revision`]) instead. That narrower fact is
//!   recorded in every v1 certificate's `nonclaims`; it is deliberately not
//!   rewritten. [`program_root`] adds an additive, independently replayed
//!   association for callers that hold a retained Project revision.
//! - **No claim about lowering.** Binding artifact bytes says the
//!   certificate's source compiles to those exact bytes through this exact
//!   compiler. It does not say the backend preserves the proved theorem.
//!
//! # The failure mode this module is built around
//!
//! Issue #186 names it directly: silently skipping a construct and then
//! reporting the enclosing obligation as proved. Three structural defenses:
//!
//! 1. A declaration is admitted *wholesale* or refused with one closed
//!    reason ([`profile::Excluded`]); there is no partial translation.
//! 2. Every declaration in the module lands in exactly one of two lists —
//!    exported, or unsupported-with-a-reason — and both lists are embedded
//!    in the generated Lean header, the coverage report, and every
//!    certificate.
//! 3. A postcondition is only ever certified when every *other* obligation
//!    of the same declaration (each arithmetic node's checked-range
//!    obligation) was confirmed by the same kernel run;
//!    [`verify::verify_certificate`] re-checks that from the certificate
//!    alone.

pub mod certificate;
pub mod installed;
pub mod installed_project;
pub mod kernel_report;
pub mod lean;
pub mod profile;
pub mod program_root;
pub mod verify;

#[cfg(test)]
mod tests;

use std::path::Path;

pub use certificate::{render_coverage, CERTIFICATE_SCHEMA, COVERAGE_SCHEMA};
pub use kernel_report::{KernelVerdict, Rejection, KERNEL_IDENTITY, PINNED_TOOLCHAIN};
pub use lean::{export_module, ModuleExport, ASSUMPTIONS, EXPORT_SCHEMA, NAMESPACE};
pub use profile::{Excluded, PROFILE_V1};
pub use program_root::{
    assurance_method_attachment, bind_certificate_to_program_root,
    verify_certificate_against_program_root, verify_certificate_against_project_source,
    verify_certificate_with_kernel_against_program_root, verify_program_root_binding,
    CheckedProgramRootBinding, PROGRAM_ROOT_BINDING_SCHEMA,
};
pub use verify::{
    verify_certificate, verify_certificate_against_artifact, verify_certificate_against_source,
    verify_certificate_with_capability, verify_certificate_with_kernel, CheckedCertificate,
};

pub(crate) use crate::assurance_manifest::smt_discharge::postcondition_obligation_id;

use crate::diagnostic::Diagnostic;
use crate::graph;
use crate::hir::ResolvedProgram;
use crate::patch;

fn no_export(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z110", message)
}

/// One run of an external Lean toolchain, as reported by whoever ran it.
pub struct KernelRun {
    /// The toolchain the caller actually ran, e.g. `leanprover/lean4:v4.34.0`.
    /// Checked against [`PINNED_TOOLCHAIN`]; a mismatch is refused.
    pub toolchain: String,
    /// The verbatim combined build output, including every `#print axioms`
    /// line. Parsed by [`kernel_report::parse`]; nothing else about the run
    /// is trusted.
    pub output: String,
}

/// Running an external Lean kernel, as an explicit capability.
///
/// The opt-in [`installed::InstalledProofTool`] implementation uses explicit
/// held executables and the existing bounded process provider. Merely exporting
/// proof data gains no process authority. Embedders may supply another trusted
/// capability; recorded fixture output is not physical kernel evidence.
pub trait LeanKernel {
    /// Check `lean_source` with a pinned Lean toolchain and return its raw
    /// output verbatim.
    fn check(&self, lean_source: &str) -> Result<KernelRun, Diagnostic>;
}

/// Compile `resolved` to Wasm core module bytes and structurally validate
/// them, so a bound artifact is never one this compiler itself would
/// reject. Mirrors the SMT certificate module's identical step.
fn compile_wasm_core_module(resolved: &ResolvedProgram) -> Result<Vec<u8>, String> {
    let bytes = crate::wasm::emit_resolved_module(resolved).map_err(|error| error.message)?;
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&bytes)
        .map_err(|error| {
            format!("compiler-emitted Wasm core module failed structural validation: {error}")
        })?;
    Ok(bytes)
}

/// Read-only whole-module export: the generated Lean document plus the
/// coverage report naming every declaration this profile refused.
///
/// Spawns no process and writes nothing. Source bytes must be unchanged
/// between the snapshot and the final check, reusing [`patch`]'s own
/// fail-closed drift mechanism.
pub fn export_source(source_path: &Path) -> Result<(ModuleExport, String), Vec<Diagnostic>> {
    let canonical = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical)?;
    let program = crate::parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = crate::verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);
    let export = export_module(&program, &revision);
    let coverage = render_coverage(&export);
    patch::validate_source_unchanged(&canonical, source_path, &snapshot, &revision)?;
    Ok((export, coverage))
}

/// Export one declaration's `ensures` obligation, have `kernel` check the
/// generated Lean document, and — only if the kernel accepted every
/// obligation of that declaration with no admitted hole and no non-standard
/// axiom — emit a certificate.
///
/// Returns `Err`, never a certificate, when there is nothing to certify:
/// the declaration is outside the profile, the index is out of range, the
/// kernel reported a different toolchain, or the verdict was anything other
/// than a clean acceptance. `sorry`, an admitted axiom, a build error and a
/// timeout are all refusals, not weaker forms of success.
pub fn export_obligation_certificate(
    source_path: &Path,
    declaration_id: &str,
    ensures_index: usize,
    kernel: &dyn LeanKernel,
) -> Result<String, Vec<Diagnostic>> {
    let canonical = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical)?;
    let program = crate::parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let rendered = source_certificate(
        snapshot.source(),
        source_path,
        declaration_id,
        ensures_index,
        kernel,
    )?;
    patch::validate_source_unchanged(
        &canonical,
        source_path,
        &snapshot,
        &graph::revision(&program),
    )?;
    Ok(rendered)
}

pub(super) fn source_certificate(
    source: &str,
    source_path: &Path,
    declaration_id: &str,
    ensures_index: usize,
    kernel: &dyn LeanKernel,
) -> Result<String, Vec<Diagnostic>> {
    let program = crate::parse(source, source_path).map_err(|error| vec![error])?;
    let diagnostics = crate::verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);
    let export = export_module(&program, &revision);

    if let Some((_, _, reason)) = export
        .unsupported
        .iter()
        .find(|(id, _, _)| id == declaration_id)
    {
        return Err(vec![no_export(format!(
            "declaration `{declaration_id}` is outside {PROFILE_V1} ({}): {}",
            reason.code(),
            reason.detail()
        ))]);
    }
    let function = export
        .exported
        .iter()
        .find(|item| item.declaration_id == declaration_id)
        .ok_or_else(|| {
            vec![no_export(format!(
                "no declaration with id `{declaration_id}` in this source"
            ))]
        })?;
    let obligation = function
        .obligations
        .iter()
        .find(|item| item.ensures_index == Some(ensures_index))
        .ok_or_else(|| {
            vec![no_export(format!(
                "ensures index {ensures_index} is out of range for `{declaration_id}`"
            ))]
        })?;
    let theorem_name = format!("{NAMESPACE}.{}", obligation.theorem_name);

    let run = kernel
        .check(&export.lean_source)
        .map_err(|error| vec![error])?;
    let expected = export.theorem_names();
    let axioms = match kernel_report::parse(&expected, &run.toolchain, &run.output) {
        KernelVerdict::Checked { axioms } => axioms,
        KernelVerdict::Rejected(rejection) => {
            return Err(vec![no_export(format!(
                "the pinned Lean kernel did not accept this export ({}): {}",
                rejection.code(),
                rejection.detail()
            ))])
        }
    };

    let resolved = crate::hir::resolve(&program)?;
    let artifact = compile_wasm_core_module(&resolved).map_err(|detail| {
        vec![no_export(format!(
            "declaration `{declaration_id}`'s enclosing module does not compile to the bound \
             artifact target: {detail}"
        ))]
    })?;

    let path_text = source_path.display().to_string();
    let source_sha256 = certificate::source_digest(source);
    let artifact_sha256 = certificate::artifact_digest(&artifact);
    let rendered = certificate::render_certificate(&certificate::CertificateInput {
        source_path_text: &path_text,
        source_sha256: &source_sha256,
        export: &export,
        declaration_id,
        ensures_index,
        obligation_id: &obligation.obligation_id,
        theorem_name: &theorem_name,
        compiler_version: env!("CARGO_PKG_VERSION"),
        toolchain: &run.toolchain,
        axioms: &axioms,
        artifact_sha256: &artifact_sha256,
        artifact_bytes: artifact.len(),
    });

    Ok(rendered)
}
