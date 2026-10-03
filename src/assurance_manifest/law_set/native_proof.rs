//! Opaque installed-tool evidence for an exact independently selected scalar law.
use super::{invalid, wire, LawSelector, LawSet, Result};
use crate::assurance_manifest::modular_law::cache::{self, ProofTaskCache, WorkMetrics};
use crate::assurance_manifest::{smt_discharge as smt, AssuranceClass, MethodRecord};
use crate::project::ProjectRevision;
use crate::proof_export::installed::{InstalledProofTool, ToolKind};
use crate::proof_export::list_induction::{self, Certificate as ListCertificate, ProofModule};
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

fn list_subject(
    revision: &ProjectRevision,
    laws: &LawSet,
    law_id: &str,
    tool: &InstalledProofTool,
) -> Result<(LawSet, super::LawRow, crate::ast::Program)> {
    if tool.kind() != ToolKind::Lean {
        return Err(invalid("list induction requires installed pinned Lean"));
    }
    let laws = LawSet::replay(revision, &laws.payload.proof_profile, laws.to_json())?;
    let row = laws
        .payload
        .laws
        .iter()
        .find(|row| row.definition.law_id == law_id)
        .ok_or_else(|| invalid("list induction law is absent from selected inventory"))?
        .clone();
    let LawSelector::ListInduction {
        declaration_id,
        theorem,
    } = &row.definition.selector
    else {
        return Err(invalid("selected law is outside list induction profile"));
    };
    if row.source_digest.is_none()
        || list_induction::declaration_for_theorem(theorem) != Some(declaration_id.as_str())
    {
        return Err(invalid(
            "list induction source or fixed theorem association is absent",
        ));
    }
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
            .any(|function| function.id.as_str() == declaration_id)
    });
    if !selected {
        return Err(invalid(
            "list induction declaration is outside selected Project HIR",
        ));
    }
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == row.source_path)
        .ok_or_else(|| invalid("list induction source is absent from retained Project"))?;
    if source.source_graph_schema() == "semaprax.native-law.v1" {
        return Err(invalid("list induction needs checked function source"));
    }
    let program = crate::check(source.source(), source.path())?;
    Ok((laws, row, program))
}

fn attached_list_proof(
    revision: &ProjectRevision,
    laws: &LawSet,
    row: &super::LawRow,
    certificate: &ListCertificate,
    document: &str,
) -> Result<VerifiedLawProof> {
    let LawSelector::ListInduction { theorem, .. } = &row.definition.selector else {
        return Err(invalid("list law selector changed during attachment"));
    };
    let name = format!("SemapraxLaw08.{theorem}");
    if !certificate
        .theorem_law_ids
        .iter()
        .any(|(found, declaration)| {
            found == &name && list_induction::declaration_for_theorem(theorem) == Some(declaration)
        })
    {
        return Err(invalid(
            "certified theorem is not associated with selected source",
        ));
    }
    let evidence_digest = wire::digest(
        b"semaprax.list-induction-law-proof.v1\0",
        &wire::canonical(&json!({
            "project_revision":revision.project_revision(),
            "program_root":laws.payload.program_root,
            "law_digest":laws.digest(),
            "law_id":row.definition.law_id,
            "semantic_digest":row.semantic_digest,
            "source_path":row.source_path,
            "source_digest":row.source_digest,
            "certificate":document,
        }))?,
    );
    let mut method = MethodRecord::new(
        AssuranceClass::TheoremProved,
        crate::proof_export::KERNEL_IDENTITY,
        crate::proof_export::PINNED_TOOLCHAIN,
    );
    method.proof_ref = Some(evidence_digest.clone());
    method.bounds = Some(list_induction::PROFILE.into());
    method.inputs = vec![
        laws.digest().into(),
        row.semantic_digest.clone(),
        certificate.source_sha256.clone(),
        certificate.proof_module_sha256.clone(),
    ];
    let method: Value =
        serde_json::from_str(&crate::assurance_manifest::render::render_method(&method))
            .map_err(|_| invalid("list induction method rendering failed"))?;
    Ok(VerifiedLawProof {
        law_id: row.definition.law_id.clone(),
        law_digest: laws.digest().into(),
        semantic_digest: row.semantic_digest.clone(),
        project_revision: revision.project_revision().into(),
        program_root: laws.payload.program_root.clone(),
        class: AssuranceClass::TheoremProved,
        evidence: json!({
            "schema":"semaprax.list-induction-law-proof.v1",
            "proof_digest":evidence_digest,
            "proof_module_sha256":certificate.proof_module_sha256,
            "source_sha256":certificate.source_sha256,
            "theorem":name,
            "coverage":certificate.coverage,
            "axioms":certificate.axioms,
            "methods":[method],
            "proved_lowering":false,
            "publication_authority":false,
            "source_authority":false,
        }),
    })
}

/// Independently replay a selected Project/LawSet, then run the real pinned
/// Lean kernel over the caller's separate current proof module. The returned
/// opaque law proof can be consumed by strict Project coverage.
pub fn prove_list_induction_law(
    revision: &ProjectRevision,
    laws: &LawSet,
    law_id: &str,
    current_proofs: &ProofModule,
    tool: &InstalledProofTool,
) -> Result<(String, VerifiedLawProof)> {
    let (laws, row, program) = list_subject(revision, laws, law_id, tool)?;
    let certificate =
        list_induction::prove(&program, current_proofs, tool).map_err(|error| vec![error])?;
    let document = serde_json::to_string(&certificate)
        .map_err(|_| invalid("list induction certificate serialization failed"))?;
    let proof = attached_list_proof(revision, &laws, &row, &certificate, &document)?;
    Ok((document, proof))
}

/// A certificate cannot mint law evidence by itself. Replay reconstructs the
/// exact current Project subject and separately held proof module, then reruns
/// the installed Lean kernel before returning a fresh opaque proof.
pub fn replay_list_induction_law(
    document: &str,
    revision: &ProjectRevision,
    laws: &LawSet,
    law_id: &str,
    current_proofs: &ProofModule,
    tool: &InstalledProofTool,
) -> Result<VerifiedLawProof> {
    if document.len() > 262_144 {
        return Err(invalid("list induction certificate exceeds byte bound"));
    }
    let certificate: ListCertificate = serde_json::from_str(document)
        .map_err(|_| invalid("list induction certificate schema is invalid"))?;
    if serde_json::to_string(&certificate).ok().as_deref() != Some(document) {
        return Err(invalid("list induction certificate is noncanonical"));
    }
    let (laws, row, program) = list_subject(revision, laws, law_id, tool)?;
    list_induction::verify_against_module(&program, current_proofs, &certificate, tool)
        .map_err(|error| vec![error])?;
    attached_list_proof(revision, &laws, &row, &certificate, document)
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
/// always newly bound to this exact Project and LawSet.
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

/// The same checked relational-law dependency closure can reuse the pinned
/// Lean kernel's exact theorem/axiom result. The certificate and opaque law
/// proof are reconstructed from the current law and Project on every call.
pub fn prove_scalar_law_lean_cached(
    project_root: &Path,
    revision: &ProjectRevision,
    laws: &LawSet,
    law_id: &str,
    tool: &InstalledProofTool,
    cache: &mut ProofTaskCache,
) -> Result<(VerifiedLawProof, WorkMetrics)> {
    if tool.kind() != ToolKind::Lean {
        return Err(invalid("native relational cache requires installed Lean"));
    }
    prove_scalar_law_with_cache(revision, laws, law_id, tool, Some((project_root, cache)))
}

/// Revalidate a selected native relational law closure in deterministic
/// prerequisite order. Complete subject/profile admission is checked before
/// any installed process or cache mutation. Each successful proof is newly
/// associated with the current Project and LawSet; a failed later law cannot
/// promote itself or an incomplete dependent as checked.
pub fn prove_scalar_law_batch_cached(
    project_root: &Path,
    revision: &ProjectRevision,
    laws: &LawSet,
    selected: &[String],
    tool: &InstalledProofTool,
    cache: &mut ProofTaskCache,
) -> Result<Vec<(String, VerifiedLawProof, WorkMetrics)>> {
    let laws = LawSet::replay(revision, &laws.payload.proof_profile, laws.to_json())?;
    let order = super::dependency_index::derive(&laws)?.ordered_closure(selected)?;
    for id in &order {
        let row = laws
            .payload
            .laws
            .iter()
            .find(|row| row.definition.law_id == *id)
            .ok_or_else(|| invalid("selected native proof law is absent"))?;
        if row.source_digest.is_none() {
            return Err(invalid("selected native proof law source is absent"));
        }
        if !matches!(
            row.definition.selector,
            LawSelector::ScalarRelational { .. }
        ) {
            return Err(invalid(
                "selected native proof dependency is outside the scalar relational profile",
            ));
        }
    }
    let mut checked = Vec::with_capacity(order.len());
    for id in order {
        let (proof, work) =
            prove_scalar_law_with_cache(revision, &laws, &id, tool, Some((project_root, cache)))?;
        checked.push((id, proof, work));
    }
    Ok(checked)
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
            let certificate = if let Some((project_root, cache)) = cached.as_mut() {
                let index = super::dependency_index::derive(&laws)?;
                let logical = index
                    .logical_digest(law_id)
                    .ok_or_else(|| invalid("native proof law has no checked dependency closure"))?;
                let (certificate, checked_work) = crate::proof_export::source_certificate_cached(
                    project_root,
                    &source,
                    Path::new("<native-law-proof>"),
                    "semaprax.law.proposition",
                    0,
                    "native-relational-lean",
                    law_id,
                    logical,
                    tool,
                    cache,
                )?;
                work = checked_work;
                certificate
            } else {
                crate::proof_export::source_certificate(
                    &source,
                    Path::new("<native-law-proof>"),
                    "semaprax.law.proposition",
                    0,
                    tool,
                )?
            };
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
    if let Some((_, cache)) = cached.as_mut() {
        let role = match tool.kind() {
            ToolKind::Lean => "native-relational-lean",
            ToolKind::Z3 => "native-relational-z3",
        };
        cache.record_event(revision, role, law_id, work);
    }
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

/// A deliberately closed optimization law: checked i64 addition by zero
/// preserves the exact operand and cannot overflow. A native proof token is
/// required even though the implementation also recognizes this one safe
/// primitive shape. No report text or law name can substitute for that token.
pub fn require_checked_i64_add_zero_identity(
    revision: &ProjectRevision,
    laws: &LawSet,
    law_id: &str,
    proof: &VerifiedLawProof,
) -> Result<()> {
    let replayed = LawSet::replay(revision, &laws.payload.proof_profile, laws.to_json())?;
    validate_all(revision, &replayed, std::slice::from_ref(proof))?;
    let row = replayed
        .payload
        .laws
        .iter()
        .find(|row| row.definition.law_id == law_id)
        .ok_or_else(|| invalid("optimization identity law is absent"))?;
    if proof.law_id != law_id
        || !row.definition.assumption_ids.is_empty()
        || !row.definition.requires_laws.is_empty()
        || !matches!(
            &row.definition.selector,
            LawSelector::ScalarRelational { binders, proposition }
                if binders.len() == 1
                    && binders[0].scalar_type == "i64"
                    && proposition
                        == &format!("{} + 0 == {}", binders[0].name, binders[0].name)
        )
    {
        return Err(invalid(
            "optimization needs the exact assumption-free checked i64 x + 0 == x law",
        ));
    }
    Ok(())
}
