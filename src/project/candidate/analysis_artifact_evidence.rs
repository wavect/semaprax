//! Candidate analysis boundaries with one freshly replayed pathless artifact
//! delta. The attachment grants no materialization or execution authority.

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;
use crate::project::{ImageArtifactKind, IMAGE_ARTIFACT_PROJECTION_SCHEMA};

use super::{
    ProjectCandidate, PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
    PROJECT_CANDIDATE_ARTIFACT_DELTA_SCHEMA,
};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_SCHEMA: &str =
    "semaprax.project-candidate-analysis-artifact-evidence.v1";
pub const MAX_PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_BYTES: usize = 10 * 1024 * 1024;

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

impl ProjectCandidate {
    /// Attach an independently rebuilt selected pathless carrier delta to this
    /// candidate's blind-spot report. No serialized artifact report is trusted.
    pub fn analysis_artifact_evidence(
        &self,
        expected_candidate: &str,
        kind: ImageArtifactKind,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let mut coverage = parse(
            &self.analysis_coverage(expected_candidate)?,
            PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
            "candidate analysis coverage has an unexpected compiler schema",
        )?;
        let artifact_delta = parse(
            &self.artifact_delta(expected_candidate, kind)?,
            PROJECT_CANDIDATE_ARTIFACT_DELTA_SCHEMA,
            "candidate artifact delta has an unexpected compiler schema",
        )?;
        validate_bindings(self, kind, &coverage, &artifact_delta)?;

        let object = coverage
            .as_object_mut()
            .ok_or_else(|| invalid("candidate analysis coverage is not an object"))?;
        if object.get("evidence_class")
            != Some(&json!("retained_source_analysis_boundary_inventory"))
        {
            return Err(invalid(
                "candidate analysis coverage evidence class is unexpected",
            ));
        }
        object.insert(
            "schema".into(),
            json!(PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_SCHEMA),
        );
        object.insert(
            "evidence_class".into(),
            json!("retained_source_and_verified_pathless_candidate_artifact_evidence"),
        );
        let areas = object
            .get_mut("areas")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("candidate analysis coverage areas are absent"))?;
        if areas.len() != AREA_ORDER.len()
            || areas
                .iter()
                .zip(AREA_ORDER)
                .any(|(row, name)| row["area"].as_str() != Some(name))
        {
            return Err(invalid(
                "candidate analysis coverage area inventory is not canonical",
            ));
        }
        let generated = &mut areas[4];
        if *generated
            != json!({
                "area": "generated_artifacts",
                "status": "not_inspected",
                "basis": "this_query_does_not_generate_or_replay_target_artifacts",
                "limitations": [
                    "existing_projection_apis_require_separate_invocation",
                    "deployed_artifact_identity_and_consumers_are_unknown"
                ],
                "required_evidence": [
                    "source_bound_artifact_projection_and_independent_deployment_binding"
                ]
            })
        {
            return Err(invalid(
                "candidate analysis coverage generated-artifact boundary is unexpected",
            ));
        }
        *generated = json!({
            "area": "generated_artifacts",
            "status": "partial",
            "basis": "independently_replayed_selected_pathless_candidate_artifact",
            "limitations": [
                "only_the_selected_web_npm_openapi_or_c_carrier_was_inspected",
                "artifact_report_omits_encoded_file_bodies_but_binds_verified_envelope_and_file_hashes",
                "no_filesystem_materialization_installation_deployment_or_runtime_execution",
                "outside_projection_is_not_platform_absence",
                "zero_selected_carrier_files_is_not_absence_of_other_artifact_kinds_or_deployed_artifacts"
            ],
            "required_evidence": [
                "authorized_materialization_and_deployment_binding_for_the_selected_artifact",
                "runtime_and_external_consumer_conformance_for_the_selected_artifact"
            ]
        });
        object.insert("artifact_delta".into(), artifact_delta);
        super::super::image::render(
            coverage,
            false,
            MAX_PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_BYTES,
        )
        .map_err(|_| capacity("candidate analysis artifact evidence exceeds its byte bound"))
    }
}

fn parse(bytes: &str, schema: &str, message: &'static str) -> Result<Value> {
    let value: Value = serde_json::from_str(bytes)
        .map_err(|_| invalid("nested candidate artifact evidence is not compiler JSON"))?;
    if value.as_object().is_none() || value["schema"] != schema {
        return Err(invalid(message));
    }
    Ok(value)
}

fn validate_bindings(
    candidate: &ProjectCandidate,
    kind: ImageArtifactKind,
    coverage: &Value,
    delta: &Value,
) -> Result<()> {
    let base_projection = &delta["base"];
    let projection = &delta["candidate"];
    if coverage["candidate_revision"] != candidate.candidate_digest()
        || coverage["base_project_revision"] != candidate.base.project_revision()
        || coverage["project_revision"] != candidate.revision.project_revision()
        || coverage["workspace_revision"] != candidate.revision.workspace_revision()
        || coverage["project_graph_digest"] != candidate.revision.semantic_graph_digest()
        || coverage["source_authority"] != false
        || coverage["external_io"] != false
        || coverage["execution"] != false
        || coverage["candidate_retained"] != false
        || coverage["publication_authority"] != false
        || delta["candidate_digest"] != candidate.candidate_digest()
        || delta["base_project_revision"] != candidate.base.project_revision()
        || delta["project_revision"] != candidate.revision.project_revision()
        || delta["kind"] != kind.name()
        || delta["source_authority"] != false
        || delta["artifact_materialization"] != false
        || delta["target_execution"] != false
        || delta["evidence_class"]
            != "exact_candidate_replay_and_independently_replayed_pathless_carrier_delta"
        || base_projection["schema"] != IMAGE_ARTIFACT_PROJECTION_SCHEMA
        || base_projection["project_revision"] != candidate.base.project_revision()
        || base_projection["project_graph_digest"] != candidate.base.semantic_graph_digest()
        || base_projection["kind"] != kind.name()
        || base_projection["evidence_class"] != "independently_replayed_pathless_compiler_artifacts"
        || base_projection["source_authority"] != false
        || base_projection["artifact_materialization"] != false
        || base_projection["target_execution"] != false
        || projection["schema"] != IMAGE_ARTIFACT_PROJECTION_SCHEMA
        || projection["image_revision"] != coverage["image_revision"]
        || projection["project_revision"] != coverage["project_revision"]
        || projection["project_graph_digest"] != coverage["project_graph_digest"]
        || projection["kind"] != kind.name()
        || projection["evidence_class"] != "independently_replayed_pathless_compiler_artifacts"
        || projection["source_authority"] != false
        || projection["artifact_materialization"] != false
        || projection["target_execution"] != false
    {
        return Err(invalid(
            "candidate coverage and artifact evidence bindings disagree",
        ));
    }
    validate_sources(&coverage["sources"], &projection["sources"])?;
    validate_revision_sources(candidate.base.sources(), &base_projection["sources"])
}

fn validate_sources(coverage: &Value, projection: &Value) -> Result<()> {
    let coverage = coverage
        .as_array()
        .ok_or_else(|| invalid("candidate analysis coverage source inventory is absent"))?;
    let projection = projection
        .as_array()
        .ok_or_else(|| invalid("candidate artifact source inventory is absent"))?;
    if coverage.len() != projection.len() {
        return Err(invalid(
            "candidate coverage and artifact source inventories disagree",
        ));
    }
    for source in coverage {
        let path = source["path"]
            .as_str()
            .ok_or_else(|| invalid("candidate coverage source path is absent"))?;
        let matching = projection
            .iter()
            .filter(|row| row["path"].as_str() == Some(path))
            .collect::<Vec<_>>();
        if matching.len() != 1
            || matching[0]["source_revision"] != source["source_revision"]
            || matching[0]["source_digest"] != source["source_digest"]
        {
            return Err(invalid(
                "candidate artifact evidence has no exact coverage source join",
            ));
        }
    }
    Ok(())
}

fn validate_revision_sources(
    expected: &[crate::project::ProjectSource],
    projection: &Value,
) -> Result<()> {
    let projection = projection
        .as_array()
        .ok_or_else(|| invalid("base artifact source inventory is absent"))?;
    if expected.len() != projection.len() {
        return Err(invalid("base artifact source inventory is incomplete"));
    }
    for source in expected {
        let matching = projection
            .iter()
            .filter(|row| row["path"].as_str() == Some(source.path()))
            .collect::<Vec<_>>();
        if matching.len() != 1
            || matching[0]["source_revision"] != source.source_revision()
            || matching[0]["source_digest"] != source.source_digest()
        {
            return Err(invalid(
                "base artifact evidence has no exact retained source join",
            ));
        }
    }
    Ok(())
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G352", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G353", message)]
}
