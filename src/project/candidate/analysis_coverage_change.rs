//! Exact base-to-candidate comparison of retained-source analysis boundaries.
//! This layer independently regenerates both owner reports and compares only
//! their closed evidence-status rows. It does not attach or infer evidence.

use std::sync::Arc;

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::project::{
    ProjectSemanticImage, IMAGE_ANALYSIS_COVERAGE_SCHEMA, MAX_IMAGE_ANALYSIS_COVERAGE_BYTES,
};

use super::{
    ProjectCandidate, MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_BYTES,
    PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_ANALYSIS_COVERAGE_CHANGE_SCHEMA: &str =
    "semaprax.project-candidate-analysis-coverage-change.v1";
pub const MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_CHANGE_BYTES: usize = 3 * 1024 * 1024;

const REPORT_DOMAIN: &[u8] = b"semaprax.project-candidate-analysis-coverage-change.v1\0";
const AREA_ORDER: [&str; 8] = [
    "declared_source_inputs",
    "declared_external_contracts",
    "deployment_configuration",
    "generated_file_provenance",
    "generated_artifacts",
    "external_api_behavior",
    "runtime_environment",
    "external_consumers",
];
const COMPARED_AREAS: [(&str, Option<&str>); 5] = [
    ("deployment_configuration", Some("deployment_configuration")),
    (
        "generated_file_provenance",
        Some("generated_file_provenance"),
    ),
    (
        "external_api_behavior",
        Some("external_api_and_deployed_runtime_contracts"),
    ),
    (
        "runtime_environment",
        Some("external_api_and_deployed_runtime_contracts"),
    ),
    ("external_consumers", None),
];

impl ProjectCandidate {
    /// Compare independently regenerated retained-source boundary inventories
    /// for this candidate's exact base and final revisions. No attachment,
    /// execution, consumer discovery, or compatibility assessment is implied.
    pub fn analysis_coverage_change(&self, expected_candidate: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;

        let base_image =
            ProjectSemanticImage::derive(Arc::clone(&self.base), self.base.project_revision())?;
        let base_bytes = base_image.analysis_coverage(base_image.image_digest())?;
        let final_bytes = self.analysis_coverage(expected_candidate)?;
        let base = parse_coverage(
            &base_bytes,
            IMAGE_ANALYSIS_COVERAGE_SCHEMA,
            MAX_IMAGE_ANALYSIS_COVERAGE_BYTES,
            "base analysis coverage",
        )?;
        let final_coverage = parse_coverage(
            &final_bytes,
            PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
            MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_BYTES,
            "candidate analysis coverage",
        )?;
        validate_bindings(self, &base_image, &base, &final_coverage)?;

        let base_areas = canonical_areas(&base)?;
        let final_areas = canonical_areas(&final_coverage)?;
        let mut changes = Vec::with_capacity(COMPARED_AREAS.len());
        let mut counts = Map::from_iter([
            ("unchanged".to_owned(), json!(0)),
            ("advanced".to_owned(), json!(0)),
            ("regressed".to_owned(), json!(0)),
            ("unknown".to_owned(), json!(0)),
        ]);
        for (name, blind_spot_domain) in COMPARED_AREAS {
            let base_area = unique_named(base_areas, "area", name, "base coverage area")?;
            let final_area = unique_named(final_areas, "area", name, "candidate coverage area")?;
            let change = classify(base_area, final_area)?;
            let current = counts[change]
                .as_u64()
                .expect("closed coverage change count");
            counts.insert(change.to_owned(), json!(current + 1));
            let (base_blind_spot, final_blind_spot) = match blind_spot_domain {
                Some(domain) => (
                    Some(unique_blind_spot(&base, domain)?.clone()),
                    Some(unique_blind_spot(&final_coverage, domain)?.clone()),
                ),
                None => (None, None),
            };
            changes.push(json!({
                "area":name,
                "change":change,
                "comparison_basis":"closed_evidence_status_and_exact_area_row",
                "base_status":base_area["status"],
                "final_status":final_area["status"],
                "base_area":base_area,
                "final_area":final_area,
                "blind_spot_domain":blind_spot_domain,
                "base_blind_spot":base_blind_spot,
                "final_blind_spot":final_blind_spot,
            }));
        }
        let overall = if counts["regressed"].as_u64() != Some(0) {
            "contains_regression"
        } else if counts["advanced"].as_u64() != Some(0) {
            "contains_advance"
        } else if counts["unknown"].as_u64() != Some(0) {
            "contains_unknown"
        } else {
            "unchanged"
        };
        let mut report = json!({
            "schema":PROJECT_CANDIDATE_ANALYSIS_COVERAGE_CHANGE_SCHEMA,
            "candidate_revision":self.candidate_digest(),
            "base_project_revision":self.base.project_revision(),
            "final_project_revision":self.revision.project_revision(),
            "base_workspace_revision":self.base.workspace_revision(),
            "final_workspace_revision":self.revision.workspace_revision(),
            "base_project_graph_digest":self.base.semantic_graph_digest(),
            "final_project_graph_digest":self.revision.semantic_graph_digest(),
            "base_image_revision":base_image.image_digest(),
            "base_coverage_sha256":sha256(base_bytes.as_bytes()),
            "final_coverage_sha256":sha256(final_bytes.as_bytes()),
            "base_coverage":base,
            "final_coverage":final_coverage,
            "area_changes":changes,
            "counts":counts,
            "overall_change":overall,
            "evidence_class":"independently_replayed_retained_source_analysis_boundary_comparison",
            "completeness":"not_assessed",
            "compatibility":"not_assessed",
            "source_authority":false,
            "approval_authority":false,
            "candidate_retained":false,
            "external_io":false,
            "execution":false,
            "runtime_observation":false,
            "environment_observation":false,
            "filesystem_observation":false,
            "network_observation":false,
            "publication_authority":false,
            "nonclaims":[
                "status_advance_is_only_a_stronger_attached_evidence_class_not_completeness",
                "unchanged_does_not_prove_no_environment_or_consumer_change",
                "unknown_is_not_regression_or_absence_evidence",
                "no_deployment_generated_file_external_provider_runtime_or_consumer_observation",
                "no_api_abi_behavioral_semantic_migration_or_runtime_compatibility_assessment",
                "no_percentage_score_ranking_approval_source_or_publication_authority"
            ]
        });
        let core = render(&report)?;
        report["report_revision"] = json!(domain_digest(REPORT_DOMAIN, core.as_bytes()));
        render(&report)
    }
}

fn parse_coverage(bytes: &str, schema: &str, maximum: usize, owner: &'static str) -> Result<Value> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(capacity("nested analysis coverage exceeds its byte bound"));
    }
    let value: Value = serde_json::from_str(bytes)
        .map_err(|_| invalid("nested analysis coverage is not compiler JSON"))?;
    if value.as_object().is_none() || value["schema"] != schema {
        return Err(invalid(match owner {
            "base analysis coverage" => "base analysis coverage schema is unexpected",
            _ => "candidate analysis coverage schema is unexpected",
        }));
    }
    Ok(value)
}

fn validate_bindings(
    candidate: &ProjectCandidate,
    base_image: &ProjectSemanticImage,
    base: &Value,
    final_coverage: &Value,
) -> Result<()> {
    if base["image_revision"] != base_image.image_digest()
        || base["project_revision"] != candidate.base.project_revision()
        || base["workspace_revision"] != candidate.base.workspace_revision()
        || base["project_graph_digest"] != candidate.base.semantic_graph_digest()
        || final_coverage["candidate_revision"] != candidate.candidate_digest()
        || final_coverage["base_project_revision"] != candidate.base.project_revision()
        || final_coverage["project_revision"] != candidate.revision.project_revision()
        || final_coverage["workspace_revision"] != candidate.revision.workspace_revision()
        || final_coverage["project_graph_digest"] != candidate.revision.semantic_graph_digest()
        || base["source_authority"] != false
        || base["external_io"] != false
        || base["execution"] != false
        || final_coverage["source_authority"] != false
        || final_coverage["external_io"] != false
        || final_coverage["execution"] != false
        || final_coverage["publication_authority"] != false
    {
        return Err(invalid("analysis coverage change bindings disagree"));
    }
    Ok(())
}

fn canonical_areas(coverage: &Value) -> Result<&[Value]> {
    let areas = coverage["areas"]
        .as_array()
        .filter(|areas| areas.len() == AREA_ORDER.len())
        .ok_or_else(|| invalid("analysis coverage area inventory is incomplete"))?;
    if areas
        .iter()
        .zip(AREA_ORDER)
        .any(|(row, expected)| row["area"] != expected)
    {
        return Err(invalid("analysis coverage area order disagrees"));
    }
    Ok(areas)
}

fn unique_named<'a>(
    rows: &'a [Value],
    field: &str,
    name: &str,
    owner: &'static str,
) -> Result<&'a Value> {
    let matching = rows
        .iter()
        .filter(|row| row[field].as_str() == Some(name))
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(invalid(match owner {
            "base coverage area" => "base coverage area is absent or duplicated",
            _ => "candidate coverage area is absent or duplicated",
        }));
    }
    Ok(matching[0])
}

fn unique_blind_spot<'a>(coverage: &'a Value, domain: &str) -> Result<&'a Value> {
    let rows = coverage["blind_spots"]
        .as_array()
        .ok_or_else(|| invalid("analysis coverage blind spots are absent"))?;
    let matching = rows
        .iter()
        .filter(|row| row["domain"].as_str() == Some(domain))
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(invalid(
            "analysis coverage blind spot is absent or duplicated",
        ));
    }
    Ok(matching[0])
}

fn classify(base: &Value, final_area: &Value) -> Result<&'static str> {
    if base == final_area {
        return Ok("unchanged");
    }
    let base_rank = status_rank(&base["status"])?;
    let final_rank = status_rank(&final_area["status"])?;
    Ok(match final_rank.cmp(&base_rank) {
        std::cmp::Ordering::Greater => "advanced",
        std::cmp::Ordering::Less => "regressed",
        std::cmp::Ordering::Equal => "unknown",
    })
}

fn status_rank(status: &Value) -> Result<u8> {
    match status.as_str() {
        Some("not_inspected") => Ok(0),
        Some("partial") => Ok(1),
        Some("known") => Ok(2),
        _ => Err(invalid("analysis coverage status is unsupported")),
    }
}

fn render(value: &Value) -> Result<String> {
    super::super::image::render(
        value.clone(),
        false,
        MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_CHANGE_BYTES,
    )
    .map_err(|_| capacity("analysis coverage change report exceeds its byte bound"))
}

fn sha256(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G492", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G493", message)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(status: &str, basis: &str) -> Value {
        json!({"area":"runtime_environment","status":status,"basis":basis,
            "limitations":[],"required_evidence":[]})
    }

    #[test]
    fn categorical_change_never_turns_same_status_drift_into_progress() {
        assert_eq!(
            classify(&area("not_inspected", "a"), &area("not_inspected", "a")).unwrap(),
            "unchanged"
        );
        assert_eq!(
            classify(&area("not_inspected", "a"), &area("partial", "b")).unwrap(),
            "advanced"
        );
        assert_eq!(
            classify(&area("partial", "a"), &area("not_inspected", "b")).unwrap(),
            "regressed"
        );
        assert_eq!(
            classify(&area("partial", "a"), &area("partial", "b")).unwrap(),
            "unknown"
        );
    }
}
