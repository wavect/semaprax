//! Exact source review composed with the three-declaration analysis boundary.
//! Both nested reports are independently regenerated from one immutable
//! candidate. Their presence supplies evidence, never ambient authority.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::{
    wire, ProjectCandidate, MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_BYTES,
    MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_BYTES,
    MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES,
    PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_SCHEMA,
    PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA,
};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_SCHEMA: &str =
    "semaprax.project-candidate-environment-aware-review.v1";
pub const MAX_PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_BYTES: usize =
    MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES
        + MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_BYTES
        + 64 * 1024;

const REPORT_DOMAIN: &[u8] = b"semaprax.project-candidate-environment-aware-review.v1\0";

impl ProjectCandidate {
    /// Independently regenerate the complete source review and canonical
    /// three-declaration analysis boundary for one exact candidate, then bind
    /// both reports and their revisions without approving source or claiming
    /// semantic, runtime, provider, environment, or deployment compatibility.
    pub fn environment_aware_review(
        &self,
        expected_candidate: &str,
        bundle_bytes: &[u8],
        expected_bundle_digest: &str,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if bundle_bytes.is_empty()
            || bundle_bytes.len() > MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_BYTES
        {
            return Err(capacity(
                "candidate environment review bundle is empty or exceeds its transport-safe bound",
            ));
        }
        let source_bytes = self.source_review(expected_candidate)?;
        let boundary_bytes = self.analysis_boundary_bundle(
            expected_candidate,
            bundle_bytes,
            expected_bundle_digest,
        )?;
        let source_review = parse_exact(
            &source_bytes,
            PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA,
            MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES,
            "candidate source review",
        )?;
        let boundary = parse_exact(
            &boundary_bytes,
            PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_SCHEMA,
            MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_BYTES,
            "candidate analysis-boundary bundle",
        )?;
        validate_bindings(
            self,
            expected_candidate,
            expected_bundle_digest,
            &source_review,
            &boundary,
        )?;

        let source_review_sha256 = sha256(&source_bytes);
        let analysis_boundary_bundle_sha256 = sha256(&boundary_bytes);
        let mut report = json!({
            "schema":PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_SCHEMA,
            "candidate_revision":self.candidate_digest(),
            "base_project_revision":self.base.project_revision(),
            "candidate_project_revision":self.revision.project_revision(),
            "workspace_revision":self.revision.workspace_revision(),
            "project_graph_digest":self.revision.semantic_graph_digest(),
            "bundle_digest":expected_bundle_digest,
            "source_review_report_revision":source_review["report_revision"],
            "source_review_sha256":source_review_sha256,
            "analysis_boundary_bundle_sha256":analysis_boundary_bundle_sha256,
            "source_review":source_review,
            "analysis_boundary_bundle":boundary,
            "evidence_class":"independently_regenerated_exact_candidate_source_and_declared_environment_boundary_review",
            "source_authority":false,
            "approval_authority":false,
            "publication_authority":false,
            "external_io":false,
            "filesystem_observation":false,
            "filesystem_authority":false,
            "environment_observation":false,
            "generator_execution":false,
            "generator_authority":false,
            "network_observation":false,
            "provider_observation":false,
            "provider_authority":false,
            "runtime_observation":false,
            "runtime_authority":false,
            "conformance_evidence":false,
            "conformance_authority":false,
            "semantic_compatibility":"not_assessed",
            "deployment_authority":false,
            "nonclaims":[
                "source_diff_and_declared_boundaries_are_not_source_approval_or_publication_permission",
                "canonical_declarations_are_not_current_filesystem_environment_generator_or_provider_observation",
                "no_runtime_external_consumer_deployment_or_conformance_evidence",
                "no_semantic_behavioral_api_abi_or_migration_compatibility_claim",
                "nested_reports_carry_no_authority_beyond_their_existing_false_grants"
            ]
        });
        let core = wire::render(
            report.clone(),
            MAX_PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_BYTES,
        )?;
        report["report_revision"] = json!(wire::digest(REPORT_DOMAIN, core.as_bytes()));
        wire::render(report, MAX_PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_BYTES)
            .map_err(|_| capacity("candidate environment review exceeds its byte bound"))
    }
}

fn parse_exact(bytes: &str, schema: &str, limit: usize, owner: &'static str) -> Result<Value> {
    if bytes.len() > limit {
        return Err(capacity(match owner {
            "candidate source review" => "candidate source review exceeds its nested byte bound",
            _ => "candidate analysis-boundary bundle exceeds its nested byte bound",
        }));
    }
    let value: Value = serde_json::from_str(bytes)
        .map_err(|_| invalid("nested candidate environment review report is not compiler JSON"))?;
    if value.as_object().is_none() || value["schema"] != schema {
        return Err(invalid(
            "nested candidate environment review report has an unexpected compiler schema",
        ));
    }
    if wire::render(value.clone(), limit)?.as_bytes() != bytes.as_bytes() {
        return Err(invalid(
            "nested candidate environment review report is not exact canonical JSON",
        ));
    }
    Ok(value)
}

fn validate_bindings(
    candidate: &ProjectCandidate,
    expected_candidate: &str,
    expected_bundle_digest: &str,
    source_review: &Value,
    boundary: &Value,
) -> Result<()> {
    if source_review["candidate_revision"] != expected_candidate
        || source_review["candidate_revision"] != candidate.candidate_digest()
        || source_review["base_project_revision"] != candidate.base.project_revision()
        || source_review["candidate_project_revision"] != candidate.revision.project_revision()
        || source_review["source_authority"] != false
        || source_review["report_revision"].as_str().is_none()
        || boundary["candidate_revision"] != candidate.candidate_digest()
        || boundary["base_project_revision"] != candidate.base.project_revision()
        || boundary["project_revision"] != candidate.revision.project_revision()
        || boundary["workspace_revision"] != candidate.revision.workspace_revision()
        || boundary["project_graph_digest"] != candidate.revision.semantic_graph_digest()
        || boundary["analysis_boundary_bundle"]["digest"] != expected_bundle_digest
        || boundary["source_authority"] != false
        || boundary["external_io"] != false
        || boundary["execution"] != false
        || boundary["candidate_retained"] != false
        || boundary["publication_authority"] != false
    {
        return Err(binding(
            "candidate environment review report revisions or authority facts disagree",
        ));
    }
    validate_source_inventory(candidate, &boundary["sources"])?;
    validate_changed_sources(candidate, &source_review["files"])
}

fn validate_source_inventory(candidate: &ProjectCandidate, value: &Value) -> Result<()> {
    let rows = value
        .as_array()
        .ok_or_else(|| invalid("candidate environment review source inventory is absent"))?;
    if rows.len() != candidate.revision.sources().len() {
        return Err(binding(
            "candidate environment review source inventory is incomplete",
        ));
    }
    let mut indexed = BTreeMap::new();
    for row in rows {
        let path = row["path"]
            .as_str()
            .ok_or_else(|| invalid("candidate environment review source path is absent"))?;
        if indexed.insert(path, row).is_some() {
            return Err(binding(
                "candidate environment review source path is duplicated",
            ));
        }
    }
    for source in candidate.revision.sources() {
        let row = indexed
            .get(source.path())
            .ok_or_else(|| binding("candidate environment review source join is absent"))?;
        if row["source_revision"] != source.source_revision()
            || row["source_digest"] != source.source_digest()
        {
            return Err(binding(
                "candidate environment review source identity disagrees",
            ));
        }
    }
    Ok(())
}

fn validate_changed_sources(candidate: &ProjectCandidate, value: &Value) -> Result<()> {
    let rows = value.as_array().ok_or_else(|| {
        invalid("candidate environment review changed-source inventory is absent")
    })?;
    let expected = candidate
        .base
        .sources()
        .iter()
        .zip(candidate.revision.sources())
        .filter(|(before, after)| before.source() != after.source())
        .map(|(before, _)| before.path())
        .collect::<BTreeSet<_>>();
    if rows.len() != expected.len() {
        return Err(binding(
            "candidate environment review changed-source inventory is incomplete",
        ));
    }
    let mut seen = BTreeSet::new();
    for row in rows {
        let path = row["path"]
            .as_str()
            .ok_or_else(|| invalid("candidate environment review changed-source path is absent"))?;
        let base = candidate
            .base
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .ok_or_else(|| binding("candidate environment review base source join is absent"))?;
        let after = candidate
            .revision
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .ok_or_else(|| {
                binding("candidate environment review candidate source join is absent")
            })?;
        if !seen.insert(path)
            || !expected.contains(path)
            || row["base_digest"] != base.source_digest()
            || row["candidate_digest"] != after.source_digest()
            || row["base_source"] != base.source()
            || row["candidate_source"] != after.source()
        {
            return Err(binding(
                "candidate environment review changed-source identity or text disagrees",
            ));
        }
    }
    Ok(())
}

fn sha256(bytes: &str) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes.as_bytes()))
    )
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G452", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G453", message)]
}
fn binding(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G454", message)]
}
