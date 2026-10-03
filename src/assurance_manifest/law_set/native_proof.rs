//! Opaque installed-tool evidence for an exact independently selected scalar law.
use super::{invalid, wire, LawSelector, LawSet, Result};
use crate::assurance_manifest::modular_law::cache::{self, ProofTaskCache, WorkMetrics};
use crate::assurance_manifest::{smt_discharge as smt, AssuranceClass, MethodRecord};
use crate::project::ProjectRevision;
use crate::proof_export::installed::{InstalledProofTool, ToolKind};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct VerifiedLawProof {
    law_id: String,
    law_digest: String,
    semantic_digest: String,
    project_revision: String,
    program_root: String,
    class: AssuranceClass,
    evidence: Value,
}

/// Generate only from a replayed typed law and a real installed-tool capability.
/// No callback, wire receipt, proof URL or method label constructs this token.
pub fn prove_scalar_law(
    revision: &ProjectRevision,
    laws: &LawSet,
    law_id: &str,
    tool: &InstalledProofTool,
) -> Result<VerifiedLawProof> {
    prove_scalar_law_with_cache(revision, laws, law_id, tool, None).map(|(proof, _)| proof)
}

/// Recheck the current relational law and all transitive declared law
/// dependencies before looking up a checked installed-Z3 task. A cache hit
/// skips only the registered solver process; the returned opaque proof is
/// always newly bound to this exact Project and LawSet. Lean uses the regular
/// fresh kernel route until a separately checked theorem recipe is admitted.
pub fn prove_scalar_law_z3_cached(
    project_root: &Path,
    revision: &ProjectRevision,
    laws: &LawSet,
    law_id: &str,
    tool: &InstalledProofTool,
    cache: &mut ProofTaskCache,
) -> Result<(VerifiedLawProof, WorkMetrics)> {
    if tool.kind() != ToolKind::Z3 {
        return Err(invalid("native relational cache requires installed Z3"));
    }
    prove_scalar_law_with_cache(revision, laws, law_id, tool, Some((project_root, cache)))
}

fn prove_scalar_law_with_cache(
    revision: &ProjectRevision,
    laws: &LawSet,
    law_id: &str,
    tool: &InstalledProofTool,
    mut cached: Option<(&Path, &mut ProofTaskCache)>,
) -> Result<(VerifiedLawProof, WorkMetrics)> {
    let laws = LawSet::replay(revision, &laws.payload.proof_profile, laws.to_json())?;
    let law = laws
        .payload
        .laws
        .iter()
        .find(|row| row.definition.law_id == law_id)
        .ok_or_else(|| invalid("native proof law is absent from the authenticated inventory"))?;
    if law.source_digest.is_none() {
        return Err(invalid("native proof law source is absent"));
    }
    let LawSelector::ScalarRelational {
        binders,
        proposition,
    } = &law.definition.selector
    else {
        return Err(invalid("native proof requires a scalar relational law"));
    };
    // Named assumptions are not smuggled into `requires`. The generated law
    // is universal over its admitted scalar ranges; open declared assumptions
    // still prevent complete coverage in the inventory evaluator.
    let parameters = binders
        .iter()
        .map(|binder| format!("{}: {}", binder.name, binder.scalar_type))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!("module semaprax.law.proof;\n@id(\"semaprax.law.proposition\")\nfn proposition({parameters}) -> i64\n ensures {proposition}\n{{ 0 }}\n@id(\"semaprax.law.main\") fn main() -> i64 {{ 0 }}\n");
    let program = crate::parse(&source, "<native-law-proof>").map_err(|error| vec![error])?;
    crate::hir::resolve(&program)?;
    let mut work = WorkMetrics::default();
    let (class, method, artifact) = match tool.kind() {
        ToolKind::Lean => {
            let certificate = crate::proof_export::source_certificate(
                &source,
                std::path::Path::new("<native-law-proof>"),
                "semaprax.law.proposition",
                0,
                tool,
            )?;
            crate::proof_export::verify_certificate(&certificate).map_err(|error| vec![error])?;
            let method = MethodRecord::new(
                AssuranceClass::TheoremProved,
                crate::proof_export::KERNEL_IDENTITY,
                crate::proof_export::PINNED_TOOLCHAIN,
            );
            (AssuranceClass::TheoremProved, method, certificate)
        }
        ToolKind::Z3 => {
            let encoding = smt::translate_function(&program.functions[0])
                .map_err(|_| invalid("native law is outside the admitted SMT scalar profile"))?;
            let rendered = smt::render_postcondition_script(&encoding, 0, tool.proof_timeout_ms());
            let script = rendered
                .strip_suffix("(get-model)\n")
                .ok_or_else(|| invalid("unexpected SMT translation response grammar"))?;
            if let Some((project_root, cache)) = cached.as_mut() {
                let index = super::dependency_index::derive(&laws)?;
                let logical = index
                    .logical_digest(law_id)
                    .ok_or_else(|| invalid("native proof law has no checked dependency closure"))?;
                work = cache::check_bound_task(
                    cache,
                    project_root,
                    tool,
                    "native-relational-z3",
                    law_id,
                    logical,
                    &smt::script_digest(script),
                    smt::BOUNDS_V1,
                    "none",
                    || tool.confirm_smt(script).map_err(|error| vec![error]),
                )?;
            } else {
                tool.confirm_smt(script).map_err(|error| vec![error])?;
                work.fresh = 1;
            }
            let method =
                MethodRecord::new(AssuranceClass::SmtProved, "z3", tool.expected_version());
            (AssuranceClass::SmtProved, method, script.to_owned())
        }
    };
    let evidence_digest = wire::digest(
        b"semaprax.native-law-proof.v1\0",
        &wire::canonical(&json!({
            "project_revision":revision.project_revision(),"law_digest":laws.digest(),
            "law_id":law_id,"semantic_digest":law.semantic_digest,"source":source,"artifact":artifact,
            "toolchain":tool.expected_version(),"host_profile":"trusted_local"
        }))?,
    );
    let mut method = method;
    method.proof_ref = Some(evidence_digest.clone());
    method.bounds = Some(match tool.kind() {
        ToolKind::Lean => format!(
            "{}; exact universally quantified typed scalar law; trusted source translation; not proved lowering",
            crate::proof_export::PROFILE_V1
        ),
        ToolKind::Z3 => smt::BOUNDS_V1.into(),
    });
    method.inputs = vec![laws.digest().into(), law.semantic_digest.clone()];
    let method: Value =
        serde_json::from_str(&crate::assurance_manifest::render::render_method(&method))
            .map_err(|_| invalid("native proof method rendering failed"))?;
    let proof = VerifiedLawProof {
        law_id: law_id.into(),
        law_digest: laws.digest().into(),
        semantic_digest: law.semantic_digest.clone(),
        project_revision: revision.project_revision().into(),
        program_root: laws.payload.program_root.clone(),
        class,
        evidence: json!({"schema":"semaprax.native-law-proof.v1","proof_digest":evidence_digest,
            "methods":[method],"scope":"universal_typed_scalar_law","proved_lowering":false,
            "publication_authority":false,"source_authority":false}),
    };
    Ok((proof, work))
}

pub(super) fn validate_all(
    revision: &ProjectRevision,
    laws: &LawSet,
    proofs: &[VerifiedLawProof],
) -> Result<()> {
    if proofs.len() > super::MAX_LAWS {
        return Err(super::capacity());
    }
    let mut seen = std::collections::BTreeSet::new();
    for proof in proofs {
        if !seen.insert(proof.law_id.as_str())
            || proof.project_revision != revision.project_revision()
            || proof.program_root != laws.payload.program_root
            || proof.law_digest != laws.digest()
            || laws.semantic_digest(&proof.law_id) != Some(proof.semantic_digest.as_str())
        {
            return Err(super::drift("native law proof is duplicate, stale or belongs to another exact Project/law inventory"));
        }
    }
    Ok(())
}

pub(super) fn evidence_for(
    law_id: &str,
    proofs: &[VerifiedLawProof],
) -> Option<(AssuranceClass, Value)> {
    proofs
        .iter()
        .find(|proof| proof.law_id == law_id)
        .map(|proof| (proof.class, proof.evidence.clone()))
}
