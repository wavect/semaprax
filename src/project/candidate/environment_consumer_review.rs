//! Environment-aware candidate review joined to one caller-attached package
//! consumer graph. The graph is already authenticated by its constructor; this
//! layer independently regenerates its reports and admits only an exact
//! candidate provider-source join. It discovers no ambient consumers.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::package_lock_v2::Coordinate;
use crate::package_semantic_graph::{
    PackageSemanticGraph, MAX_PACKAGE_SEMANTIC_REPORT_BYTES, PACKAGE_SEMANTIC_CONSUMERS_SCHEMA,
    PACKAGE_SEMANTIC_SUMMARY_SCHEMA,
};
use crate::project::MAX_PATH_BYTES;

use super::{
    wire, ProjectCandidate, MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_BYTES,
    MAX_PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_BYTES, PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_SCHEMA,
};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_SCHEMA: &str =
    "semaprax.project-candidate-environment-consumer-review.v1";
pub const PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_COVERAGE_SCHEMA: &str =
    "semaprax.project-candidate-environment-consumer-coverage.v1";
pub const MAX_PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_BYTES: usize =
    MAX_PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_BYTES
        + MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_BYTES
        + 2 * MAX_PACKAGE_SEMANTIC_REPORT_BYTES
        + 128 * 1024;

const REPORT_DOMAIN: &[u8] = b"semaprax.project-candidate-environment-consumer-review.v1\0";
const EXTERNAL_CONSUMERS_BASELINE: &str = r#"{"area":"external_consumers","basis":"retained_project_graph_has_no_external_consumer_inventory","limitations":["manifest_exports_do_not_enumerate_actual_clients","absence_of_graph_edges_is_not_absence_of_external_callers"],"required_evidence":["explicit_authenticated_consumer_inventory_and_compatibility_evidence"],"status":"not_inspected"}"#;

impl ProjectCandidate {
    /// Compose the complete candidate environment review with the summary and
    /// selected consumers of one caller-attached package graph. This marks the
    /// bounded external-consumer inventory partial; it does not discover
    /// installed consumers or assess compatibility.
    #[allow(clippy::too_many_arguments)]
    pub fn environment_consumer_review(
        &self,
        expected_candidate: &str,
        bundle_bytes: &[u8],
        expected_bundle_digest: &str,
        graph: &PackageSemanticGraph,
        provider: &Coordinate,
        provider_source_path: &str,
        target: &str,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        validate_selector(self, provider, provider_source_path)?;

        let environment_bytes = self.environment_aware_review(
            expected_candidate,
            bundle_bytes,
            expected_bundle_digest,
        )?;
        let graph_revision = graph.graph_digest();
        let summary_bytes = graph.summary(graph_revision)?;
        let consumers_bytes = graph.consumers(graph_revision, provider, target)?;
        let environment = parse_exact(
            &environment_bytes,
            PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_SCHEMA,
            MAX_PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_BYTES,
            "candidate environment review",
            true,
        )?;
        let summary = parse_exact(
            &summary_bytes,
            PACKAGE_SEMANTIC_SUMMARY_SCHEMA,
            MAX_PACKAGE_SEMANTIC_REPORT_BYTES,
            "package semantic summary",
            false,
        )?;
        let consumers = parse_exact(
            &consumers_bytes,
            PACKAGE_SEMANTIC_CONSUMERS_SCHEMA,
            MAX_PACKAGE_SEMANTIC_REPORT_BYTES,
            "package semantic consumers",
            false,
        )?;
        validate_environment(
            self,
            expected_candidate,
            expected_bundle_digest,
            &environment,
        )?;
        let provider_binding = validate_package_reports(
            self,
            &environment,
            graph_revision,
            provider,
            provider_source_path,
            target,
            &summary,
            &consumers,
        )?;

        let mut operational_coverage = environment["analysis_boundary_bundle"].clone();
        mark_external_consumers_partial(&mut operational_coverage)?;
        let coverage_object = operational_coverage
            .as_object_mut()
            .ok_or_else(|| invalid("candidate operational coverage is not an object"))?;
        coverage_object.insert(
            "schema".to_owned(),
            json!(PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_COVERAGE_SCHEMA),
        );
        coverage_object.insert(
            "evidence_class".to_owned(),
            json!("candidate_environment_boundary_with_attached_package_consumer_projection"),
        );
        append_nonclaims(
            &mut operational_coverage,
            &[
                "attached_package_graph_is_not_ambient_installed_or_deployed_consumer_discovery",
                "absence_from_the_attached_graph_is_not_absence_of_other_external_consumers",
                "imports_and_static_calls_are_not_runtime_use",
                "no_api_abi_behavioral_or_migration_compatibility_assessment",
                "no_package_filesystem_network_registry_or_dependency_acquisition_authority",
            ],
        )?;

        let import_count = array(&consumers, "imports")?.len();
        let call_count = array(&consumers, "calls")?.len();
        let mut report = json!({
            "schema":PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_SCHEMA,
            "candidate_revision":self.candidate_digest(),
            "base_project_revision":self.base.project_revision(),
            "candidate_project_revision":self.revision.project_revision(),
            "workspace_revision":self.revision.workspace_revision(),
            "project_graph_digest":self.revision.semantic_graph_digest(),
            "bundle_digest":expected_bundle_digest,
            "package_graph_revision":graph_revision,
            "provider":{"package":provider.package,"version":provider.version},
            "provider_source":provider_binding,
            "target":target,
            "environment_review_sha256":sha256(&environment_bytes),
            "package_summary_sha256":sha256(&summary_bytes),
            "package_consumers_sha256":sha256(&consumers_bytes),
            "environment_review":environment,
            "package_summary":summary,
            "package_consumers":consumers,
            "operational_coverage":operational_coverage,
            "advanced_areas":["external_consumers"],
            "counts":{"imports":import_count,"calls":call_count},
            "evidence_class":"exact_candidate_environment_review_and_authenticated_attached_package_consumer_source_join",
            "compatibility":"not_assessed",
            "nonclaims":[
                "complete_nested_reports_preserve_their_existing_nonclaims_and_false_grants",
                "attached_package_graph_is_not_ambient_installed_or_deployed_consumer_discovery",
                "absence_from_the_attached_graph_is_not_absence_of_other_external_consumers",
                "imports_do_not_prove_calls_and_static_calls_are_not_runtime_execution",
                "no_api_abi_behavioral_semantic_or_migration_compatibility_assessment",
                "no_source_approval_mutation_publication_package_acquisition_or_deployment_authority"
            ]
        });
        let report_object = report
            .as_object_mut()
            .expect("candidate environment consumer report is an object");
        for field in [
            "source_authority",
            "approval_authority",
            "publication_authority",
            "external_io",
            "filesystem_observation",
            "filesystem_authority",
            "environment_observation",
            "network_observation",
            "registry_observation",
            "installed_consumer_observation",
            "consumer_discovery_complete",
            "package_acquisition_authority",
            "execution",
            "runtime_observation",
            "runtime_authority",
            "provider_observation",
            "provider_authority",
            "generator_execution",
            "generator_authority",
            "conformance_evidence",
            "conformance_authority",
            "deployment_authority",
            "candidate_retained",
            "graph_retained",
        ] {
            report_object.insert(field.to_owned(), json!(false));
        }
        let core = wire::render(
            report.clone(),
            MAX_PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_BYTES,
        )?;
        report["report_revision"] = json!(wire::digest(REPORT_DOMAIN, core.as_bytes()));
        wire::render(
            report,
            MAX_PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_BYTES,
        )
        .map_err(|_| capacity("candidate environment consumer review exceeds its byte bound"))
    }
}

fn validate_selector(
    candidate: &ProjectCandidate,
    provider: &Coordinate,
    provider_source_path: &str,
) -> Result<()> {
    if candidate.revision.manifest().name() != provider.package
        || candidate.revision.manifest().package_version() != Some(provider.version.as_str())
    {
        return Err(binding(
            "candidate manifest name and version disagree with the provider coordinate",
        ));
    }
    if provider_source_path.is_empty()
        || provider_source_path.len() > MAX_PATH_BYTES
        || provider_source_path.contains('\0')
    {
        return Err(binding(
            "candidate provider source path is outside its exact logical-path bound",
        ));
    }
    Ok(())
}

fn validate_environment(
    candidate: &ProjectCandidate,
    expected_candidate: &str,
    expected_bundle_digest: &str,
    environment: &Value,
) -> Result<()> {
    if environment["candidate_revision"] != expected_candidate
        || environment["candidate_revision"] != candidate.candidate_digest()
        || environment["base_project_revision"] != candidate.base.project_revision()
        || environment["candidate_project_revision"] != candidate.revision.project_revision()
        || environment["workspace_revision"] != candidate.revision.workspace_revision()
        || environment["project_graph_digest"] != candidate.revision.semantic_graph_digest()
        || environment["bundle_digest"] != expected_bundle_digest
        || environment["semantic_compatibility"] != "not_assessed"
    {
        return Err(binding(
            "candidate environment review revisions or bundle binding disagree",
        ));
    }
    for field in [
        "source_authority",
        "approval_authority",
        "publication_authority",
        "external_io",
        "filesystem_observation",
        "filesystem_authority",
        "environment_observation",
        "generator_execution",
        "generator_authority",
        "network_observation",
        "provider_observation",
        "provider_authority",
        "runtime_observation",
        "runtime_authority",
        "conformance_evidence",
        "conformance_authority",
        "deployment_authority",
    ] {
        if environment[field] != false {
            return Err(binding(
                "candidate environment review claims unsupported authority or observation",
            ));
        }
    }
    if environment["nonclaims"].as_array().is_none()
        || environment["analysis_boundary_bundle"]
            .as_object()
            .is_none()
    {
        return Err(invalid(
            "candidate environment review is missing its complete nested facts",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_package_reports(
    candidate: &ProjectCandidate,
    environment: &Value,
    graph_revision: &str,
    provider: &Coordinate,
    provider_source_path: &str,
    target: &str,
    summary: &Value,
    consumers: &Value,
) -> Result<Value> {
    let candidate_sources = candidate
        .revision
        .sources()
        .iter()
        .filter(|source| source.path() == provider_source_path)
        .collect::<Vec<_>>();
    let candidate_source = if candidate_sources.len() == 1 {
        candidate_sources[0]
    } else {
        return Err(binding("candidate provider source is absent or duplicated"));
    };
    let coordinate = json!({"package":provider.package,"version":provider.version});
    let packages = summary["packages"]
        .as_array()
        .ok_or_else(|| invalid("package semantic summary package inventory is absent"))?;
    let provider_rows = packages
        .iter()
        .filter(|row| row["coordinate"] == coordinate)
        .collect::<Vec<_>>();
    let provider_fact = if provider_rows.len() == 1 {
        provider_rows[0]
    } else {
        return Err(binding(
            "package semantic summary provider inventory is absent or ambiguous",
        ));
    };
    let package_source_digest =
        crate::package_source_capsule::semantic_graph_source_digest(candidate_source.source());
    if summary["graph_revision"] != graph_revision
        || consumers["graph_revision"] != graph_revision
        || consumers["provider"] != coordinate
        || consumers["target"] != target
        || consumers["provider_source_revision"] != candidate_source.source_revision()
        || consumers["provider_source_digest"] != package_source_digest
        || consumers["project_association"] != "none"
        || provider_fact["source_revision"] != candidate_source.source_revision()
        || provider_fact["source_digest"] != package_source_digest
        || provider_fact["source_bytes"] != candidate_source.source().len()
    // `interface_source_revision` records the published interface, a separate
    // artifact from the provider source: a report subject must define `main`
    // and a workspace scalar provider may not, so the two revisions never
    // agree. The candidate source is already bound exactly by the revision,
    // digest and byte count above.
    {
        return Err(binding(
            "attached package graph disagrees with the exact candidate provider source or selector",
        ));
    }
    for report in [summary, consumers] {
        if report["source_authority"] != false
            || report["execution"] != false
            || report["publication_authority"] != false
            || report["nonclaims"].as_array().is_none()
        {
            return Err(binding(
                "attached package graph report claims authority or omits its nonclaims",
            ));
        }
    }
    let boundary_sources = environment["analysis_boundary_bundle"]["sources"]
        .as_array()
        .ok_or_else(|| invalid("candidate environment source inventory is absent"))?
        .iter()
        .filter(|source| source["path"] == provider_source_path)
        .collect::<Vec<_>>();
    if boundary_sources.len() != 1
        || boundary_sources[0]["source_revision"] != candidate_source.source_revision()
        || boundary_sources[0]["source_digest"] != candidate_source.source_digest()
    {
        return Err(binding(
            "candidate environment has no exact provider source join",
        ));
    }
    Ok(json!({
        "path":candidate_source.path(),
        "source_revision":candidate_source.source_revision(),
        "source_digest":candidate_source.source_digest(),
        "package_source_digest":package_source_digest,
        "source_bytes":candidate_source.source().len(),
        "association":"exact_candidate_source_and_attached_package_provider_fact"
    }))
}

fn mark_external_consumers_partial(coverage: &mut Value) -> Result<()> {
    let areas = coverage["areas"]
        .as_array_mut()
        .ok_or_else(|| invalid("candidate operational coverage area inventory is absent"))?;
    let mut matching = areas
        .iter_mut()
        .filter(|row| row["area"] == "external_consumers");
    let row = matching
        .next()
        .ok_or_else(|| binding("candidate external-consumer coverage area is absent"))?;
    if matching.next().is_some() {
        return Err(binding(
            "candidate external-consumer coverage area is duplicated",
        ));
    }
    let expected: Value = serde_json::from_str(EXTERNAL_CONSUMERS_BASELINE)
        .expect("closed external-consumer baseline is valid JSON");
    if *row != expected {
        return Err(binding(
            "candidate external-consumer coverage baseline has changed",
        ));
    }
    *row = json!({
        "area":"external_consumers",
        "status":"partial",
        "basis":"exact_candidate_provider_source_join_to_caller_attached_authenticated_package_graph",
        "limitations":[
            "attached_graph_is_not_ambient_installed_or_deployed_consumer_discovery",
            "absence_from_attached_graph_is_not_absence_of_other_external_consumers",
            "imports_and_static_calls_are_not_runtime_use",
            "not_api_abi_behavioral_or_migration_compatibility"
        ],
        "required_evidence":[
            "authorized_installed_consumer_inventory",
            "consumer_compatibility_and_runtime_conformance_evidence"
        ]
    });
    Ok(())
}

fn append_nonclaims(coverage: &mut Value, additions: &[&str]) -> Result<()> {
    let nonclaims = coverage["nonclaims"]
        .as_array_mut()
        .ok_or_else(|| invalid("candidate operational coverage nonclaims are absent"))?;
    for addition in additions {
        if nonclaims
            .iter()
            .any(|value| value.as_str() == Some(addition))
        {
            return Err(binding(
                "candidate operational coverage nonclaim is duplicated",
            ));
        }
        nonclaims.push(json!(addition));
    }
    Ok(())
}

fn parse_exact(
    bytes: &str,
    schema: &str,
    limit: usize,
    owner: &'static str,
    terminal_lf: bool,
) -> Result<Value> {
    if bytes.len() > limit {
        return Err(capacity(match owner {
            "candidate environment review" => {
                "candidate environment review exceeds its nested byte bound"
            }
            _ => "package semantic report exceeds its nested byte bound",
        }));
    }
    let value: Value = serde_json::from_str(bytes)
        .map_err(|_| invalid("nested environment consumer report is not compiler JSON"))?;
    if value.as_object().is_none() || value["schema"] != schema {
        return Err(invalid(
            "nested environment consumer report has an unexpected compiler schema",
        ));
    }
    let canonical = if terminal_lf {
        wire::render(value.clone(), limit)?
    } else {
        let mut canonical = value.clone();
        canonical.sort_all_objects();
        serde_json::to_string(&canonical)
            .map_err(|_| invalid("nested package semantic report cannot be rendered"))?
    };
    if canonical.as_bytes() != bytes.as_bytes() {
        return Err(invalid(
            "nested environment consumer report is not exact canonical JSON",
        ));
    }
    Ok(value)
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    value[key]
        .as_array()
        .ok_or_else(|| invalid("package semantic consumer inventory is absent"))
}

fn sha256(bytes: &str) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes.as_bytes()))
    )
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G472", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G473", message)]
}
fn binding(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G474", message)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coverage() -> Value {
        json!({
            "areas":[
                {"area":"source_semantics","status":"covered"},
                serde_json::from_str::<Value>(EXTERNAL_CONSUMERS_BASELINE).unwrap(),
                {"area":"runtime_behavior","status":"not_inspected"}
            ],
            "nonclaims":["existing_nonclaim"]
        })
    }

    #[test]
    fn attached_consumers_change_only_the_owned_area() {
        let mut value = coverage();
        let before = value.clone();
        mark_external_consumers_partial(&mut value).unwrap();
        assert_eq!(value["areas"][0], before["areas"][0]);
        assert_eq!(value["areas"][2], before["areas"][2]);
        assert_eq!(value["nonclaims"], before["nonclaims"]);
        assert_eq!(value["areas"][1]["area"], "external_consumers");
        assert_eq!(value["areas"][1]["status"], "partial");
    }

    #[test]
    fn changed_or_duplicated_external_consumer_baseline_fails_closed() {
        let mut changed = coverage();
        changed["areas"][1]["status"] = json!("covered");
        assert_eq!(
            mark_external_consumers_partial(&mut changed).unwrap_err()[0].code,
            "SPX-G474"
        );

        let mut duplicated = coverage();
        duplicated["areas"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::from_str::<Value>(EXTERNAL_CONSUMERS_BASELINE).unwrap());
        assert_eq!(
            mark_external_consumers_partial(&mut duplicated).unwrap_err()[0].code,
            "SPX-G474"
        );
    }
}
