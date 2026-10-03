//! Complete law coverage joined only to independently derived Project evidence.
//! No proof URL, method record, report, or artifact hash creates evidence here.
use super::{evaluate, invalid, wire, LawPolicy, LawSelector, LawSet, ModelKind, Result};
use crate::assurance_manifest::{model_checking as mc, VerifiedProjectProof};
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "semaprax.strict-law-assurance.v1";

/// Method requirements form separate profiles, not a total evidence ordering.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum RequiredLawEvidence {
    CompilerStatic,
    /// Accept the entire pinned export trust profile explicitly. Narrower
    /// assumption policies fail closed rather than guessing which are unused.
    PinnedLeanSource {
        toolchain: String,
        accepted_assumptions: Vec<String>,
        accepted_axioms: Vec<String>,
    },
    ReferenceModel {
        model_digest: String,
        minimum_states: usize,
        minimum_depth: usize,
        minimum_transitions: usize,
    },
    /// Visible refusal until a solver-confirmed Project attachment is available.
    SmtSource,
    /// Artifact association is not a proof that lowering preserves semantics.
    VerifiedLowering,
}

/// Retained by the host independently of candidate manifests. Every protected
/// law requires exactly one requirement; a missing/added law cannot disappear.
#[derive(Clone, Debug)]
pub struct StrictLawPolicy {
    baseline: LawSet,
    inventory: LawPolicy,
    requirements: BTreeMap<String, RequiredLawEvidence>,
    digest: String,
}
impl StrictLawPolicy {
    pub fn new(
        baseline: LawSet,
        requirements: BTreeMap<String, RequiredLawEvidence>,
    ) -> Result<Self> {
        if requirements.len() > super::MAX_LAWS {
            return Err(super::capacity());
        }
        if requirements.len() != baseline.payload.laws.len()
            || baseline
                .payload
                .laws
                .iter()
                .any(|law| !requirements.contains_key(&law.definition.law_id))
        {
            return Err(invalid(
                "strict requirements must cover the exact independently selected law inventory",
            ));
        }
        let inventory = LawPolicy::strict(baseline.clone())?;
        let digest = wire::digest(
            b"semaprax.strict-law-policy.v1\0",
            &wire::canonical(&json!({"baseline":baseline.digest(),"requirements":requirements}))?,
        );
        Ok(Self {
            baseline,
            inventory,
            requirements,
            digest,
        })
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn base_revision(&self) -> &str {
        &self.baseline.payload.project_revision
    }
}

/// Read-only necessary evidence; no source, process, publication or build authority.
/// Opaque proofs are supplied by the already explicit trusted kernel boundary;
/// this API never resolves a proof_ref or silently starts an external tool.
pub fn derive(
    revision: &ProjectRevision,
    laws: &LawSet,
    policy: &StrictLawPolicy,
    proofs: &[VerifiedProjectProof],
) -> Result<String> {
    if proofs.len() > super::MAX_LAWS {
        return Err(super::capacity());
    }
    let inventory = evaluate::derive_report_with_proofs(revision, laws, &policy.inventory, proofs)?;
    let inventory_value = wire::report_value(&inventory)?;
    let mut rows = Vec::new();
    let law_rows = inventory_value["laws"]
        .as_array()
        .ok_or_else(|| invalid("derived law inventory is missing"))?;
    for row in law_rows {
        let id = row["law_id"]
            .as_str()
            .ok_or_else(|| invalid("derived law identity is missing"))?;
        let reason = match policy.requirements.get(id) {
            None => Some("law_requirement_missing"),
            Some(_) if row["status"] != "present" => Some("law_missing_unsupported_or_open"),
            Some(requirement) => check_requirement(laws, id, row, requirement),
        };
        rows.push(json!({"law_id":id,"obligation_id":row["obligation_id"],"semantic_digest":row["semantic_digest"],"satisfied":reason.is_none(),"failure":reason,"requirement":policy.requirements.get(id),"evidence":row["evidence"]}));
    }
    let satisfied = rows.iter().filter(|row| row["satisfied"] == true).count();
    wire::canonical(&json!({
        "schema":SCHEMA,"project_revision":revision.project_revision(),"program_root":laws.payload.program_root,
        "policy_digest":policy.digest,"baseline_law_digest":policy.baseline.digest(),"law_digest":laws.digest(),
        "inventory_report_digest":wire::digest(b"semaprax.strict-law-inventory.v1\0",&inventory),
        "accepted":!rows.is_empty() && satisfied == rows.len(),"counts":{"required":rows.len(),"satisfied":satisfied},
        "laws":rows,"source_authority":false,"execution_authority":false,"publication_authority":false,
        "nonclaims":["source_proof_is_not_proved_lowering","no_external_tool_invoked","opaque_kernel_evidence_requires_a_trusted_host_capability"]
    }))
}

/// Exact rederivation rejects altered/partial rows, forged success, stale source,
/// dependency, policy or proof identities. Submitted report bytes are never input
/// to evidence selection or to Project assurance derivation.
pub fn require(
    document: &str,
    revision: &ProjectRevision,
    laws: &LawSet,
    policy: &StrictLawPolicy,
    proofs: &[VerifiedProjectProof],
) -> Result<()> {
    if document.len() > super::MAX_BYTES {
        return Err(super::capacity());
    }
    let expected = derive(revision, laws, policy, proofs)?;
    if document != expected {
        return Err(super::drift(
            "strict law report differs from independent exact Project, law, policy or proof replay",
        ));
    }
    let report: Value = serde_json::from_str(&expected)
        .map_err(|_| invalid("invalid derived strict law report"))?;
    if report["accepted"] != true {
        return Err(vec![Diagnostic::io("SPX-LW130", "strict law coverage is missing, unsupported, inconclusive or fails its method/trust requirements")]);
    }
    Ok(())
}

fn check_requirement(
    laws: &LawSet,
    id: &str,
    row: &Value,
    requirement: &RequiredLawEvidence,
) -> Option<&'static str> {
    let Some(law) = laws
        .payload
        .laws
        .iter()
        .find(|law| law.definition.law_id == id)
    else {
        return Some("law_definition_missing");
    };
    let evidence = &row["evidence"];
    match requirement {
        RequiredLawEvidence::CompilerStatic => {
            let held_architecture = matches!(
                law.definition.selector,
                LawSelector::ForbidReaches { .. } | LawSelector::ProtocolRealizersBound { .. }
            ) && evidence["claims"][0]["status"] == "held";
            let compiler = evidence["methods"].as_array().is_some_and(|methods| {
                methods
                    .iter()
                    .any(|method| method["class"] == "compiler_proved")
            });
            (!held_architecture && !compiler).then_some("compiler_static_evidence_missing")
        }
        RequiredLawEvidence::PinnedLeanSource {
            toolchain,
            accepted_assumptions,
            accepted_axioms,
        } => {
            if toolchain != crate::proof_export::PINNED_TOOLCHAIN {
                return Some("kernel_toolchain_not_accepted");
            }
            if crate::proof_export::ASSUMPTIONS
                .iter()
                .any(|(id, _)| !accepted_assumptions.iter().any(|accepted| accepted == id))
                || crate::proof_export::kernel_report::STANDARD_AXIOMS
                    .iter()
                    .any(|id| !accepted_axioms.iter().any(|accepted| accepted == id))
            {
                return Some("kernel_translation_assumptions_or_axioms_not_accepted");
            }
            let confirmed = evidence["methods"].as_array().is_some_and(|methods| {
                methods.iter().any(|method| {
                    method["class"] == "theorem_proved"
                        && method["tool"] == crate::proof_export::KERNEL_IDENTITY
                        && method["tool_version"] == *toolchain
                })
            });
            (!confirmed).then_some("kernel_confirmed_exact_project_evidence_missing")
        }
        RequiredLawEvidence::ReferenceModel {
            model_digest,
            minimum_states,
            minimum_depth,
            minimum_transitions,
        } => {
            let LawSelector::ModelProperty { model, .. } = &law.definition.selector else {
                return Some("model_scope_does_not_match_law");
            };
            let (descriptor, bounds) = match model {
                ModelKind::Authorization => (
                    &mc::authorization_model::DESCRIPTOR,
                    mc::authorization_model::BOUNDS,
                ),
                ModelKind::Handle => (&mc::handle_model::DESCRIPTOR, mc::handle_model::BOUNDS),
            };
            if mc::model_digest(descriptor, bounds) != *model_digest
                || evidence["model_digest"] != *model_digest
            {
                return Some("reference_model_domain_or_identity_mismatch");
            }
            if bounds.max_states < *minimum_states
                || bounds.max_depth < *minimum_depth
                || bounds.max_transitions < *minimum_transitions
            {
                return Some("reference_model_bound_insufficient");
            }
            (evidence["verified"] != true || evidence["scope"] != "reference_model_only")
                .then_some("reference_model_not_verified")
        }
        RequiredLawEvidence::SmtSource => Some("solver_confirmed_project_attachment_unavailable"),
        RequiredLawEvidence::VerifiedLowering => Some("proved_lowering_evidence_unavailable"),
    }
}
