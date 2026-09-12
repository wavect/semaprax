//! `semaprax.smt-proof-certificate.v1`: a standalone, independently
//! re-checkable certificate binding one bounded SMT discharge attempt
//! (issue #184, [`super::smt_discharge`]) to the exact source bytes and
//! declaration it was produced from.
//!
//! See [`docs/SMT-PROOF-CERTIFICATE-V1.md`](../../docs/SMT-PROOF-CERTIFICATE-V1.md)
//! for the full specification. This module is proof data, not permission: it
//! never runs a target, discovers or runs project tests, writes source, or
//! removes a runtime guard, and a certificate it emits grants no execution,
//! publication, or signing authority.
//!
//! # Independence, not merely a wrapper
//!
//! A certificate that only the exporter that produced it can check is not
//! independently checked. Every field this module needs to make its own
//! claim checkable by a party that does not trust this exporter is embedded
//! verbatim, not merely referenced by digest:
//!
//! - The exact `QF_LIA` SMT-LIB2 script text ([`export_postcondition_certificate`]
//!   embeds [`super::smt_discharge::render_postcondition_script`]'s full
//!   output), so any third party can feed it to *any* QF_LIA-conformant
//!   solver — not only the one this exporter happened to run — and read the
//!   `unsat`/`sat` verdict for themselves.
//! - For a refuted (counterexample) certificate, the exact concrete model
//!   values the solver reported, so a third party can hand-evaluate the
//!   source declaration's `requires`/`ensures`/body against SEMAPRAX's
//!   documented checked-arithmetic semantics without running any code this
//!   crate ships, let alone trusting it.
//!
//! [`verify_certificate`] replays the certificate's own internal consistency
//! (digest, byte counts, closed vocabularies) without touching a filesystem.
//! [`verify_certificate_against_source`] additionally re-derives the SMT-LIB2
//! script from the current source bytes using the same deterministic
//! translation the exporter used, and — for a refuted certificate —
//! independently re-evaluates the recorded counterexample against
//! [`super::smt_discharge::replay_function`]'s checked-arithmetic evaluator,
//! which shares no code with the SMT-LIB2 translator. Neither function
//! trusts the certificate's own recorded verdict for what it can
//! independently recompute; only the "does an SMT solver actually say
//! `unsat` for this exact script" step is left to a real solver run
//! ([`verify_certificate_with_solver`], strictly opt-in).
//!
//! # The bug class this design specifically defends against
//!
//! Issue #184's own worst bug was giving `result` the same unconditional
//! range axiom a genuine parameter gets, when `result` is *defined* to equal
//! the body's term; asserting both made a real overflow query vacuously
//! `unsat` and a genuine defect was reported as proved. A certificate that
//! only recorded "the solver said unsat" would have no way to catch a
//! recurrence of that bug, or a hand-edited script exploiting the same
//! pattern: it would just repeat the false claim. Because this module
//! instead re-derives the *exact* script from source and requires byte
//! equality with the embedded script before accepting anything,
//! [`verify_certificate_against_source`] rejects any certificate whose
//! script was not genuinely produced by the real translator for the exact
//! source and ensures clause it claims — see `tests` for a regression that
//! constructs exactly this tampering and confirms it is rejected.
//!
//! # Scope and honest limitations
//!
//! - Only postcondition (`ensures`) discharge is certified; precondition
//!   consistency (`unsat` meaning "contradictory `requires`", not a proof of
//!   any obligation) is out of this tranche's scope.
//! - `crate::assurance_manifest::smt_discharge::DischargeOutcome::Refuted`
//!   never becomes a [`super::obligation::MethodRecord`] and this module
//!   changes nothing about that: a certificate is evidence about the
//!   discharge attempt itself (positive proof, or a validated concrete
//!   counterexample) — it is not merged into the Assurance Manifest's
//!   obligation lattice. Merging a certificate's `smt_proved` finding into
//!   that manifest is the candidate-assurance join (#129) other tranches
//!   own; see `docs/SMT-DISCHARGE-V1.md` "Integration status" for the two
//!   open gaps (`render.rs`'s unconditional `no_smt_solver_invoked`
//!   nonclaim, and `ExternalRecords`'s `SPX-Z101` collision with an
//!   already-derived obligation) this module deliberately routes around
//!   rather than papers over, by never attempting that merge at all.
//! - Exactly one compiled backend artifact target is bound: the Wasm core
//!   module the exact certified source/revision compiles to for its whole
//!   enclosing module (issue #186's "Artifact binding for one target"),
//!   recorded as `payload.artifact` (`target`, `bytes`, `sha256`). It is
//!   bound by domain-separated digest, the same way `source.sha256` binds
//!   source — not embedded verbatim, since (unlike the SMT-LIB2 script) it
//!   is a deterministic function of the exact same source bytes already
//!   bound, so embedding it would duplicate rather than add independent
//!   information. [`verify_certificate_against_source`] recompiles it from
//!   the bound source through this exact compiler and requires byte
//!   equality; [`verify_certificate_against_artifact`] instead checks a
//!   caller-supplied artifact's bytes against the recorded digest without
//!   touching source or this compiler at all. Neither step proves the
//!   backend lowering itself preserves the source theorem — see the
//!   `nonclaims` entries scoped to artifact binding.
//! - No native artifact is bound. Native codegen emits C11 *source text*
//!   that still needs an external, unpinned C toolchain this crate does not
//!   invoke (no ambient authority), so it cannot be a compiled binary
//!   artifact the way the Wasm core module already is.
//! - `verify_certificate_against_source`'s script re-derivation depends on
//!   this exact compiler's deterministic translator; a genuinely
//!   third-party-only check of a `proved` verdict still requires handing the
//!   embedded script text to an independent solver, which this module makes
//!   possible but does not itself require.

mod render;
mod verify;

#[cfg(test)]
mod tests;

pub use render::SCHEMA;
pub use verify::{
    verify_certificate, verify_certificate_against_artifact, verify_certificate_against_source,
    verify_certificate_with_solver,
};

use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::{graph, patch};

use super::smt_discharge::{
    parse_model, postcondition_obligation_id, render_postcondition_script, replay_function, run,
    solver_version, translate_function, Provisioning, ReplayOutcome, RunLimits, Verdict,
    ENV_Z3_PATH,
};

use render::CertificateBody;

fn no_certificate(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z105", message)
}

/// Compile `resolved` to its Wasm core module bytes and structurally
/// validate the result, exactly like [`crate::target_evidence`]'s own
/// `emit_validated_wasm` — this module deliberately does not reuse that
/// function itself (it is private to that module and tied to its
/// before/candidate pair), but applies the identical validation step so a
/// bound artifact is never one this compiler itself would reject.
///
/// Returns a plain `String` detail rather than a [`Diagnostic`] so both call
/// sites (a fresh export, and a source-bound replay) can wrap it in whichever
/// diagnostic code fits their own context.
fn compile_wasm_core_module(resolved: &ResolvedProgram) -> Result<Vec<u8>, String> {
    let bytes = crate::wasm::emit_resolved_module(resolved).map_err(|error| error.message)?;
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&bytes)
        .map_err(|error| {
            format!("compiler-emitted Wasm core module failed structural validation: {error}")
        })?;
    Ok(bytes)
}

fn describe_non_result_verdict(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Unknown => "solver returned unknown; nothing to certify".to_owned(),
        Verdict::Timeout => {
            "solver run exceeded the bounded timeout; nothing to certify".to_owned()
        }
        Verdict::CapacityExceeded => {
            "solver output exceeded the capped output budget; nothing to certify".to_owned()
        }
        Verdict::Crash { exit_code } => {
            format!("solver process crashed (exit={exit_code:?}); nothing to certify")
        }
        Verdict::Malformed { excerpt } => {
            format!("malformed solver output: {excerpt}; nothing to certify")
        }
        Verdict::NotProvisioned => format!("no solver provisioned; set {ENV_Z3_PATH}"),
        Verdict::Unsat | Verdict::Sat(_) => unreachable!("caller already matched this variant"),
    }
}

/// Attempt to discharge `declaration_id`'s `ensures` clause at
/// `ensures_index` against the source at `source_path`, and export a
/// standalone `semaprax.smt-proof-certificate.v1` certificate for the
/// result.
///
/// Read-only: like [`super::generate`], source bytes must remain unchanged
/// between the snapshot and the final check or this fails closed
/// (`SPX-Z104`-class drift, reusing [`crate::patch`]'s own mechanism).
///
/// Returns `Err` — never a certificate — when there is nothing to certify:
/// no solver provisioned, the declaration is outside the bounded subset,
/// the solver returned anything other than a definitive `unsat` or a
/// validated `sat` counterexample, or a `sat` model failed to parse or did
/// not validate under independent replay. A certificate makes a positive
/// or a validated-negative claim; an inconclusive attempt has nothing
/// certifiable to say.
///
/// This function spawns a solver process only when `provisioning` is
/// `Some`; with `None` it behaves exactly like the rest of this crate's
/// solver-backed tooling and never itself consults the environment.
pub fn export_postcondition_certificate(
    source_path: &Path,
    declaration_id: &str,
    ensures_index: usize,
    provisioning: Option<&Provisioning>,
    limits: &RunLimits,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = crate::parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = crate::verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.stable_id == declaration_id)
        .ok_or_else(|| {
            vec![no_certificate(format!(
                "no function with declaration id `{declaration_id}` in this source"
            ))]
        })?;

    // Checked before provisioning so a caller without a solver still gets a
    // useful, specific reason when the declaration itself is the problem,
    // rather than always being told only "no solver".
    let encoding = translate_function(function).map_err(|reason| {
        vec![no_certificate(format!(
            "declaration `{declaration_id}` is outside the bounded SMT-discharge subset ({}): {}",
            reason.code(),
            reason.detail()
        ))]
    })?;
    if ensures_index >= encoding.ensures.len() {
        return Err(vec![no_certificate(
            "ensures_index is out of range for this declaration".to_owned(),
        )]);
    }

    let Some(provisioning) = provisioning else {
        return Err(vec![no_certificate(format!(
            "no certificate without a real solver run; set {ENV_Z3_PATH}"
        ))]);
    };

    let timeout_ms = u64::try_from(limits.timeout.as_millis()).unwrap_or(u64::MAX);
    let script = render_postcondition_script(&encoding, ensures_index, timeout_ms);
    let verdict = run(provisioning, &script, limits);
    let solver_ver = solver_version(provisioning).unwrap_or_else(|| "unrecorded".to_owned());

    let body = match verdict {
        Verdict::Unsat => CertificateBody::Proved,
        Verdict::Sat(raw_model) => {
            let model = parse_model(&raw_model).map_err(|error| {
                vec![no_certificate(format!(
                    "sat but the model failed to parse: {error}"
                ))]
            })?;
            match replay_function(function, &model) {
                Err(error) => {
                    return Err(vec![no_certificate(format!(
                        "sat model failed to replay: {error}"
                    ))])
                }
                Ok(ReplayOutcome::Inconsistent { detail }) => {
                    return Err(vec![no_certificate(format!(
                        "sat model did not validate under independent checked-arithmetic \
                         replay: {detail}"
                    ))])
                }
                Ok(
                    outcome @ (ReplayOutcome::Trapped { .. }
                    | ReplayOutcome::EnsuresViolated { .. }),
                ) => CertificateBody::Refuted { model, outcome },
            }
        }
        other => return Err(vec![no_certificate(describe_non_result_verdict(&other))]),
    };

    let resolved = crate::hir::resolve(&program)?;
    let artifact_bytes = compile_wasm_core_module(&resolved).map_err(|detail| {
        vec![no_certificate(format!(
            "declaration `{declaration_id}`'s enclosing module does not compile to the bound \
             Wasm core module artifact target: {detail}"
        ))]
    })?;
    let artifact_sha256 = render::artifact_digest(&artifact_bytes);

    let obligation_id = postcondition_obligation_id(declaration_id, ensures_index);
    let source_sha256 = render::source_digest(snapshot.source());
    let path_text = source_path.display().to_string();

    let input = render::RenderInput {
        source_path_text: &path_text,
        revision: &revision,
        source_sha256: &source_sha256,
        declaration_id,
        obligation_id: &obligation_id,
        ensures_index,
        compiler_version: env!("CARGO_PKG_VERSION"),
        timeout_ms,
        max_output_bytes: limits.max_output_bytes,
        solver_identity: provisioning.identity,
        solver_version: &solver_ver,
        artifact_sha256: &artifact_sha256,
        artifact_bytes: artifact_bytes.len(),
        script: &script,
        body: &body,
    };
    let certificate = render::render(&input);

    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(certificate)
}
