use super::super::{dominates, obligation_id, AssuranceClass, ObligationKind};
use super::{drift, invalid, wire, ContractKind, LawRow, LawSelector, LawSet, ModelKind, Result};
use crate::architecture_claims::{ArchitectureClaim, ArchitectureClaimSet};
use crate::project::ProjectRevision;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Independently selected policy; never decoded from a candidate report.
/// Disabled callers keep using the original Project assurance route.
#[derive(Clone, Debug)]
pub struct LawPolicy {
    baseline: LawSet,
    allow_empty: bool,
}
impl LawPolicy {
    pub fn strict(baseline: LawSet) -> Result<Self> {
        if baseline.payload.laws.is_empty() {
            return Err(empty());
        }
        Ok(Self {
            baseline,
            allow_empty: false,
        })
    }
    /// Deliberate empty profiles can only protect an empty baseline.
    pub fn deliberate_empty(baseline: LawSet) -> Result<Self> {
        if !baseline.payload.laws.is_empty() {
            return Err(empty());
        }
        Ok(Self {
            baseline,
            allow_empty: true,
        })
    }
}
fn empty() -> Vec<crate::diagnostic::Diagnostic> {
    vec![crate::diagnostic::Diagnostic::io("SPX-LW105", "strict law policy requires a nonempty protected inventory; deliberate empty policy requires an empty baseline")]
}

/// Derive all expected rows before counts. No candidate-supplied successes are accepted.
pub fn derive_report(
    revision: &ProjectRevision,
    candidate: &LawSet,
    policy: &LawPolicy,
) -> Result<String> {
    derive_report_with_proofs(revision, candidate, policy, &[])
}

pub(super) fn derive_report_with_proofs(
    revision: &ProjectRevision,
    candidate: &LawSet,
    policy: &LawPolicy,
    proofs: &[super::super::VerifiedProjectProof],
) -> Result<String> {
    let candidate = LawSet::replay(
        revision,
        &policy.baseline.payload.proof_profile,
        candidate.to_json(),
    )?;
    let mut expected: BTreeMap<&str, &LawRow> = policy
        .baseline
        .payload
        .laws
        .iter()
        .map(|row| (row.definition.law_id.as_str(), row))
        .collect();
    let current: BTreeMap<&str, &LawRow> = candidate
        .payload
        .laws
        .iter()
        .map(|row| (row.definition.law_id.as_str(), row))
        .collect();
    for (id, row) in &current {
        if let Some(protected) = expected.get(id) {
            if protected.semantic_digest != row.semantic_digest {
                return Err(drift(
                    "stable law identity retargeted or proposition/evidence policy changed",
                ));
            }
        }
        expected.insert(id, row);
    }
    if expected.is_empty() && !policy.allow_empty {
        return Err(empty());
    }
    let base_report =
        super::super::project::derive_with_verified_proofs(revision, &Default::default(), proofs)?;
    let base: Value = serde_json::from_str(&base_report)
        .map_err(|_| invalid("Project assurance derivation failed"))?;
    let obligations = base["payload"]["obligations"]
        .as_array()
        .ok_or_else(|| invalid("Project assurance inventory missing"))?;
    let mut rows = BTreeMap::new();
    for (id, row) in &expected {
        let mut result = if !current.contains_key(id) {
            fact("missing", "law_definition_missing", None, None)
        } else if row.source_digest.is_none() {
            fact("missing", "law_source_module_missing", None, None)
        } else {
            evaluate(revision, row, obligations)?
        };
        if result["status"] == "present" && !row.definition.assumption_ids.is_empty() {
            result["status"] = json!("awaiting_evidence");
            result["reason"] = json!("declared_assumptions_remain_open");
        }
        result["law_id"] = json!(id);
        result["semantic_digest"] = json!(row.semantic_digest);
        result["definition"] = json!(row.definition);
        result["provenance"] = json!({"module_id": row.module_id, "module_digest": row.module_digest, "source_path": row.source_path, "source_digest": row.source_digest});
        rows.insert((*id).to_owned(), result);
    }
    // Both sets are acyclic independently, and unchanged protected definitions
    // prevent union retargeting. Still explicitly settle dependency coverage.
    let mut settled = BTreeSet::new();
    while settled.len() < rows.len() {
        let ready: Vec<_> = expected
            .iter()
            .filter(|(id, row)| {
                !settled.contains(**id)
                    && row
                        .definition
                        .requires_laws
                        .iter()
                        .all(|dep| settled.contains(dep.as_str()))
            })
            .map(|(id, _)| (*id).to_owned())
            .collect();
        if ready.is_empty() {
            return Err(invalid("cyclic or dangling expected law dependency"));
        }
        for id in ready {
            if rows[&id]["status"] == "present"
                && expected[id.as_str()]
                    .definition
                    .requires_laws
                    .iter()
                    .any(|dep| rows[dep]["status"] != "present")
            {
                rows.get_mut(&id).unwrap()["status"] = json!("awaiting_evidence");
                rows.get_mut(&id).unwrap()["reason"] = json!("required_law_not_covered");
            }
            settled.insert(id);
        }
    }
    let count = |status: &str| rows.values().filter(|row| row["status"] == status).count();
    let covered = count("present");
    let payload = json!({
        "report_schema": "semaprax.law-set-report.v1",
        "project_revision": candidate.payload.project_revision,
        "program_root": candidate.payload.program_root,
        "proof_profile": candidate.payload.proof_profile,
        "protected_baseline_digest": policy.baseline.digest,
        "candidate_inventory_digest": candidate.digest,
        "deliberate_empty": policy.allow_empty,
        "counts": {"required": rows.len(), "covered": covered, "missing": count("missing"), "unsupported": count("unsupported"), "open": count("awaiting_evidence")},
        "accepted": covered == rows.len(),
        "laws": rows.values().collect::<Vec<_>>(),
        "dependency_inventory": {"protected_modules": policy.baseline.payload.modules, "candidate_modules": candidate.payload.modules, "project_assurance_digest": base["payload_digest"]},
        "nonclaims": ["inventory_is_not_proof", "model_properties_describe_reference_models_only", "no_runtime_execution_or_publication_authority"]
    });
    Ok(wire::encode(&payload)?.0)
}

/// Independent exact replay; missing/forged rows and empty all-passed documents fail.
pub fn verify_report(
    document: &str,
    revision: &ProjectRevision,
    candidate: &LawSet,
    policy: &LawPolicy,
) -> Result<()> {
    wire::report_value(document)?;
    if derive_report(revision, candidate, policy)? != document {
        return Err(drift(
            "law report differs from protected expected inventory and independent evidence",
        ));
    }
    Ok(())
}

/// Policy gate for callers that require coverage rather than an inventory report.
pub fn require_satisfied(
    document: &str,
    revision: &ProjectRevision,
    candidate: &LawSet,
    policy: &LawPolicy,
) -> Result<()> {
    verify_report(document, revision, candidate, policy)?;
    if wire::report_value(document)?["accepted"] != true {
        return Err(vec![crate::diagnostic::Diagnostic::io(
            "SPX-LW106",
            "required laws are missing, unsupported or awaiting evidence",
        )]);
    }
    Ok(())
}

fn fact(status: &str, reason: &str, obligation: Option<String>, evidence: Option<Value>) -> Value {
    json!({"status": status, "reason": reason, "obligation_id": obligation, "evidence": evidence})
}
fn evidence_fact(row: &LawRow, id: String, class: AssuranceClass, evidence: Value) -> Value {
    if dominates(class, row.definition.evidence.class()) {
        fact(
            "present",
            "required_evidence_available",
            Some(id),
            Some(evidence),
        )
    } else {
        fact(
            "awaiting_evidence",
            "required_evidence_unavailable",
            Some(id),
            Some(evidence),
        )
    }
}
fn evaluate(revision: &ProjectRevision, row: &LawRow, obligations: &[Value]) -> Result<Value> {
    match &row.definition.selector {
        LawSelector::ScalarRelational { .. } => Ok(fact(
            "awaiting_evidence",
            "scalar_relational_proposition_has_no_verified_proof_attachment",
            Some(format!("relational:{}", row.definition.law_id)),
            None,
        )),
        LawSelector::Contract {
            declaration_id,
            clause,
            proposition,
        } => {
            let mut matches = Vec::new();
            let mut owners = 0;
            for source in revision.sources() {
                if source.source_graph_schema() == "semaprax.native-law.v1" {
                    continue;
                }
                let mut program =
                    crate::parse(source.source(), source.path()).map_err(|error| vec![error])?;
                for ty in &mut program.types {
                    if let crate::ast::TypeDeclarationKind::Class { methods, .. } = &mut ty.kind {
                        program.functions.append(methods);
                    }
                }
                for function in &program.functions {
                    if function.stable_id != *declaration_id {
                        continue;
                    }
                    owners += 1;
                    let (clauses, kind, prefix) = match clause {
                        ContractKind::Precondition => {
                            (&function.requires, ObligationKind::Precondition, "require")
                        }
                        ContractKind::Postcondition => {
                            (&function.ensures, ObligationKind::Postcondition, "ensure")
                        }
                    };
                    for (index, expression) in clauses.iter().enumerate() {
                        if crate::format::expr(expression, 0) == *proposition {
                            matches.push(obligation_id(
                                kind,
                                declaration_id,
                                &format!("{prefix}:{index}"),
                            ));
                        }
                    }
                }
            }
            if owners > 1 || matches.len() > 1 {
                return Err(invalid("ambiguous contract law subject or proposition"));
            }
            let Some(id) = matches.pop() else {
                return Ok(fact(
                    "missing",
                    "contract_subject_or_proposition_missing",
                    None,
                    None,
                ));
            };
            let Some(obligation) = obligations.iter().find(|obligation| obligation["id"] == id)
            else {
                return Ok(fact(
                    "unsupported",
                    "contract_outside_admitted_assurance_view",
                    Some(id),
                    None,
                ));
            };
            let class =
                AssuranceClass::from_token(obligation["classification"].as_str().unwrap_or(""))
                    .ok_or_else(|| invalid("unknown derived assurance class"))?;
            Ok(evidence_fact(row, id, class, obligation.clone()))
        }
        LawSelector::ForbidReaches { claim_id, from, to } => architecture(
            revision,
            row,
            ArchitectureClaim::forbid_reaches(claim_id, from, to)?,
            from,
            &format!("architecture:forbid_reaches:{claim_id}"),
        ),
        LawSelector::ProtocolRealizersBound {
            claim_id,
            protocol_id,
        } => architecture(
            revision,
            row,
            ArchitectureClaim::protocol_realizers_bound(claim_id, protocol_id)?,
            protocol_id,
            &format!("architecture:protocol_realizers_bound:{claim_id}"),
        ),
        LawSelector::ModelProperty { model, property } => {
            use super::super::model_checking::{
                self as mc, authorization_model as a, handle_model as h,
            };
            let (descriptor, bounds, verified) = match model {
                ModelKind::Authorization => (
                    &a::DESCRIPTOR,
                    a::BOUNDS,
                    matches!(
                        mc::check_safety(&a::Correct, a::BOUNDS).outcome,
                        mc::SafetyOutcome::Verified
                    ),
                ),
                ModelKind::Handle => (
                    &h::DESCRIPTOR,
                    h::BOUNDS,
                    matches!(
                        mc::check_safety(&h::Correct, h::BOUNDS).outcome,
                        mc::SafetyOutcome::Verified
                    ),
                ),
            };
            let id = obligation_id(
                ObligationKind::ArchitectureLaw,
                descriptor.name,
                &format!("model-property:{property}"),
            );
            let evidence = json!({"scope": "reference_model_only", "model_digest": mc::model_digest(descriptor, bounds), "property": property, "verified": verified});
            if verified {
                Ok(evidence_fact(
                    row,
                    id,
                    AssuranceClass::ModelChecked,
                    evidence,
                ))
            } else {
                Ok(fact(
                    "awaiting_evidence",
                    "model_property_not_proved",
                    Some(id),
                    Some(evidence),
                ))
            }
        }
    }
}
fn architecture(
    revision: &ProjectRevision,
    row: &LawRow,
    claim: ArchitectureClaim,
    subject: &str,
    locator: &str,
) -> Result<Value> {
    let id = obligation_id(ObligationKind::ArchitectureLaw, subject, locator);
    let result = ArchitectureClaimSet::new(vec![claim])?.evaluate(revision);
    let result = match result {
        Ok(result) => result,
        Err(errors) if errors.iter().any(|error| error.code == "SPX-AC602") => return Err(errors),
        Err(_) => {
            return Ok(fact(
                "missing",
                "architecture_subject_missing",
                Some(id),
                None,
            ))
        }
    };
    let value: Value = serde_json::from_str(result.to_json())
        .map_err(|_| invalid("invalid architecture result"))?;
    let status = value["claims"][0]["status"].as_str().unwrap_or("");
    match status {
        "held" => Ok(evidence_fact(
            row,
            id,
            AssuranceClass::CompilerProved,
            value,
        )),
        "unevaluable" => Ok(fact(
            "unsupported",
            "architecture_claim_unevaluable",
            Some(id),
            Some(value),
        )),
        "violated" => Ok(fact(
            "awaiting_evidence",
            "architecture_claim_violated",
            Some(id),
            Some(value),
        )),
        _ => Err(invalid("unknown architecture result status")),
    }
}
