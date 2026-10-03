//! Real installed proofs for exact retained Project postconditions.
use super::installed::{InstalledProofTool, ToolKind};
use crate::assurance_manifest::modular_law::cache::{self, ProofTaskCache, WorkMetrics};
use crate::assurance_manifest::{
    smt_discharge as smt, AssuranceClass, MethodRecord, VerifiedProjectProof,
};
use crate::{diagnostic::Diagnostic, project::ProjectRevision};
use std::path::Path;

/// The returned evidence cannot be retargeted, serialized into authority or
/// attached to another source/Project. No application entry point is executed.
pub fn prove_postcondition(
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
) -> Result<VerifiedProjectProof, Vec<Diagnostic>> {
    prove_postcondition_with_cache(revision, source_path, declaration, index, tool, None)
        .map(|(proof, _)| proof)
}

/// The exact selected Project and checked source are rederived on every call.
/// A fresh satisfiable-domain model is still obtained and replayed because it
/// is part of the newly source-bound proof receipt. Only the complete Z3
/// postcondition query may reuse an authenticated checked-success task.
pub fn prove_postcondition_z3_cached(
    project_root: &Path,
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
    cache: &mut ProofTaskCache,
) -> Result<(VerifiedProjectProof, WorkMetrics), Vec<Diagnostic>> {
    if tool.kind() != ToolKind::Z3 {
        return Err(error("source postcondition cache requires installed Z3"));
    }
    prove_postcondition_with_cache(
        revision,
        source_path,
        declaration,
        index,
        tool,
        Some((project_root, cache)),
    )
}

/// Rebuild the current Lean export, bounded precondition witness, Wasm
/// artifact, source-bound certificate and ProgramRoot attachment. Only the
/// pinned kernel's checked theorem/axiom result may be reused.
pub fn prove_postcondition_lean_cached(
    project_root: &Path,
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
    cache: &mut ProofTaskCache,
) -> Result<(VerifiedProjectProof, WorkMetrics), Vec<Diagnostic>> {
    if tool.kind() != ToolKind::Lean {
        return Err(error("source postcondition cache requires installed Lean"));
    }
    prove_postcondition_with_cache(
        revision,
        source_path,
        declaration,
        index,
        tool,
        Some((project_root, cache)),
    )
}

fn prove_postcondition_with_cache(
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
    mut cached: Option<(&Path, &mut ProofTaskCache)>,
) -> Result<(VerifiedProjectProof, WorkMetrics), Vec<Diagnostic>> {
    let selected = [
        revision.entry_program(),
        revision.public_api_program(),
        revision.test_program(),
    ]
    .iter()
    .any(|program| {
        program
            .functions
            .iter()
            .any(|function| function.id.as_str() == declaration)
    });
    if !selected {
        return Err(error(
            "proof declaration is outside the exact selected Project HIR",
        ));
    }
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == source_path)
        .ok_or_else(|| error("proof source is absent from exact retained Project"))?;
    if source.source_graph_schema() == "semaprax.native-law.v1" {
        return Err(error(
            "native relational law attachment is not a source postcondition",
        ));
    }
    let program = crate::parse(source.source(), source_path).map_err(|error| vec![error])?;
    crate::hir::resolve(&program)?;
    if tool.kind() == ToolKind::Lean {
        let (certificate, work) = if let Some((project_root, cache)) = cached.as_mut() {
            super::source_certificate_cached(
                project_root,
                source.source(),
                Path::new(source_path),
                declaration,
                index,
                "direct-project-lean",
                &format!("{declaration}:{index}"),
                "",
                tool,
                cache,
            )?
        } else {
            (
                super::source_certificate(
                    source.source(),
                    Path::new(source_path),
                    declaration,
                    index,
                    tool,
                )?,
                WorkMetrics::default(),
            )
        };
        let binding = super::bind_certificate_to_program_root(&certificate, revision, source_path)
            .map_err(|error| vec![error])?;
        let proof = super::assurance_method_attachment(&certificate, &binding, revision, tool)
            .map_err(|error| vec![error])?;
        if let Some((_, cache)) = cached.as_mut() {
            cache.record_event(
                revision,
                "direct-project-lean",
                &format!("{declaration}:{index}"),
                work,
            );
        }
        return Ok((proof, work));
    }
    let function = program
        .functions
        .iter()
        .find(|function| function.stable_id == declaration)
        .ok_or_else(|| error("exact selected declaration is absent"))?;
    let encoding = smt::translate_function(function)
        .map_err(|_| error("declaration is outside the admitted SMT source profile"))?;
    if index >= encoding.ensures.len() {
        return Err(error("exact selected postcondition is absent"));
    }
    let domain_script = smt::render_domain_witness_script(&encoding, tool.proof_timeout_ms());
    let domain_model = tool
        .smt_domain_model(&domain_script)
        .map_err(|error| vec![error])?;
    smt::validate_domain_witness(function, &domain_model)
        .map_err(|reason| error(&format!("domain witness failed checked replay: {reason}")))?;
    let domain_witness = domain_model
        .iter()
        .map(|(name, value)| {
            let value = match value {
                smt::ModelValue::Int(raw) => format!("int:{raw}"),
                smt::ModelValue::Bool(raw) => format!("bool:{raw}"),
            };
            (name.clone(), value)
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    // Same existing checked-arithmetic translation. Model retrieval is a
    // separate command and unnecessary for strict success-only proof checking.
    let rendered = smt::render_postcondition_script(&encoding, index, tool.proof_timeout_ms());
    let script = rendered
        .strip_suffix("(get-model)\n")
        .ok_or_else(|| error("unexpected SMT translator response grammar"))?;
    let work = if let Some((project_root, cache)) = cached.as_mut() {
        // The admitted direct scalar subset has no calls. Complete current
        // domain and proof scripts therefore describe every implementation,
        // contract, overflow, range, and branch dependency of this query.
        let logical = cache::logical_subject_digest(&[
            "direct-project-postcondition-z3",
            declaration,
            &domain_script,
            script,
        ]);
        cache::check_bound_task(
            cache,
            project_root,
            tool,
            "direct-project-z3",
            &format!("{declaration}:{index}"),
            &logical,
            &smt::script_digest(script),
            smt::BOUNDS_V1,
            "none",
            || tool.confirm_smt(script).map_err(|error| vec![error]),
        )?
    } else {
        tool.confirm_smt(script).map_err(|error| vec![error])?;
        WorkMetrics {
            fresh: 1,
            ..WorkMetrics::default()
        }
    };
    let version = tool.expected_version();
    let root = revision.program_root()?;
    let obligation = smt::postcondition_obligation_id(declaration, index);
    let receipt = serde_json::json!({
        "schema":"semaprax.installed-smt-project-proof.v1", "project_revision":revision.project_revision(),
        "program_root":root.program_root(), "source_path":source.path(),
        "source_revision":source.source_revision(), "source_digest":source.source_digest(),
        "declaration_id":declaration, "obligation_id":obligation,
        "script":script, "domain_script":domain_script, "domain_witness":domain_witness,
        "toolchain":version, "profile":smt::BOUNDS_V1,
        "host_profile":"trusted_local", "proved_lowering":false
    });
    let digest = super::certificate::domain_digest(
        b"semaprax.installed-smt-project-proof.v1\0",
        receipt.to_string().as_bytes(),
    );
    let mut method = MethodRecord::new(AssuranceClass::SmtProved, "z3", version);
    method.proof_ref = Some(digest.clone());
    method.bounds = Some(smt::BOUNDS_V1.into());
    method.inputs = vec![root.program_root().into(), source.source_digest().into()];
    method.detail = Some("Exact retained source checked by explicitly authorized installed Z3; trusted translation and local host, no proved lowering or execution authority".into());
    let proof = VerifiedProjectProof::kernel_confirmed(
        obligation,
        declaration.into(),
        method,
        revision.project_revision().into(),
        root.program_root().into(),
        source.path().into(),
        source.source_revision().into(),
        source.source_digest().into(),
        digest,
    );
    if let Some((_, cache)) = cached.as_mut() {
        cache.record_event(
            revision,
            "direct-project-z3",
            &format!("{declaration}:{index}"),
            work,
        );
    }
    Ok((proof, work))
}

fn error(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-LW140", message)]
}

/// Installed LAW-06 postcondition proof for an exact selected Project source.
/// The result joins ordinary Project and LAW-04 strict assurance as one opaque
/// `VerifiedProjectProof`; it carries no execution or publication authority.
fn prove_modular_postcondition_with<F>(
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
    prove: F,
) -> Result<VerifiedProjectProof, Vec<Diagnostic>>
where
    F: FnOnce() -> Result<
        crate::assurance_manifest::modular_law::summary::ModularProof,
        crate::assurance_manifest::modular_law::summary::ModularFailure,
    >,
{
    use crate::assurance_manifest::modular_law::certificate;
    use sha2::{Digest as _, Sha256};
    if tool.kind() != ToolKind::Z3 {
        return Err(error("modular scalar postconditions require installed Z3"));
    }
    let source = revision
        .sources()
        .iter()
        .find(|row| row.path() == source_path)
        .ok_or_else(|| error("modular proof source is absent from exact retained Project"))?;
    if source.source_graph_schema() == "semaprax.native-law.v1" {
        return Err(error("native relational law is not a source postcondition"));
    }
    let program =
        crate::parse(source.source(), source_path).map_err(|diagnostic| vec![diagnostic])?;
    let function = program
        .functions
        .iter()
        .find(|function| function.stable_id == declaration)
        .ok_or_else(|| error("modular declaration is absent from exact selected source"))?;
    if index >= function.ensures.len() {
        return Err(error("modular postcondition index is absent"));
    }
    let proof = prove()
        .map_err(|reason| error(&format!("modular proof was not established: {reason:?}")))?;
    if index >= proof.caller_postcondition_scripts.len() {
        return Err(error("modular postcondition was not checked"));
    }
    let certificate = certificate::encode_installed(&proof, tool)
        .map_err(|reason| error(&format!("modular proof transcript unavailable: {reason:?}")))?;
    let mut hash = Sha256::new();
    hash.update(b"semaprax.installed-modular-project-proof.v1\0");
    hash.update((certificate.len() as u64).to_le_bytes());
    hash.update(certificate.as_bytes());
    let proof_ref = format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()));
    let root = revision.program_root()?;
    let mut method = MethodRecord::new(AssuranceClass::SmtProved, "z3", tool.expected_version());
    method.proof_ref = Some(proof_ref.clone());
    method.bounds = Some(crate::assurance_manifest::modular_law::BOUNDS_V1.into());
    method.inputs = vec![root.program_root().into(), source.source_digest().into()];
    method
        .inputs
        .extend(proof.plan.summaries.iter().map(|row| row.digest.clone()));
    method.detail = Some("Exact retained Project call dependencies checked through registered installed Z3; no proved lowering or publication authority".into());
    Ok(VerifiedProjectProof::kernel_confirmed(
        smt::postcondition_obligation_id(declaration, index),
        declaration.into(),
        method,
        revision.project_revision().into(),
        root.program_root().into(),
        source.path().into(),
        source.source_revision().into(),
        source.source_digest().into(),
        proof_ref,
    ))
}

/// Prove every current modular query with registered installed Z3.
pub fn prove_modular_postcondition(
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
) -> Result<VerifiedProjectProof, Vec<Diagnostic>> {
    prove_modular_postcondition_with(revision, source_path, declaration, index, tool, || {
        crate::assurance_manifest::modular_law::installed::prove_straight_line_installed(
            revision,
            declaration,
            tool,
        )
    })
}

/// Rebuild exact current-Project evidence from checked logical-query reuse.
/// Only the compiler-owned cache's private installed-Z3 successes may be reused;
/// the old source-bound proof or certificate is never rewritten.
pub fn prove_modular_postcondition_cached(
    project_root: &std::path::Path,
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
    cache: &mut crate::assurance_manifest::modular_law::cache::ProofTaskCache,
) -> Result<
    (
        VerifiedProjectProof,
        crate::assurance_manifest::modular_law::cache::WorkMetrics,
    ),
    Vec<Diagnostic>,
> {
    let mut work = None;
    let proof = prove_modular_postcondition_with(
        revision,
        source_path,
        declaration,
        index,
        tool,
        || {
            let cached = crate::assurance_manifest::modular_law::cache::prove_straight_line_installed_cached(
            project_root, revision, declaration, tool, cache,
        )?;
            work = Some(cached.work);
            Ok(cached.proof)
        },
    )?;
    Ok((
        proof,
        work.expect("successful cache proof records work metrics"),
    ))
}

/// LAW-07 installed finite-aggregate proof. Returns an inert, versioned
/// source-bound transcript plus an opaque attachment for this exact Project.
/// The transcript alone cannot be passed as a proved method.
pub fn prove_structured_postcondition(
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
) -> Result<(String, VerifiedProjectProof), Vec<Diagnostic>> {
    crate::assurance_manifest::structured_law::installed::prove_installed_project(
        revision,
        source_path,
        declaration,
        index,
        tool,
    )
}

/// Recheck an inert LAW-07 transcript against the exact retained Project and
/// rerun the installed backend before issuing a new opaque attachment.
pub fn replay_structured_postcondition(
    certificate: &str,
    revision: &ProjectRevision,
    tool: &InstalledProofTool,
) -> Result<VerifiedProjectProof, Vec<Diagnostic>> {
    crate::assurance_manifest::structured_law::installed::replay_installed_project(
        certificate,
        revision,
        tool,
    )
}
