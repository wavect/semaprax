//! Candidate analysis boundaries with one explicit, independently replayed
//! package-consumer evidence attachment. No serialized evidence is trusted.

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;

use super::{
    CandidatePackageConsumerReplayInput, ProjectCandidate,
    PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA, PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_SCHEMA,
};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_SCHEMA: &str =
    "semaprax.project-candidate-analysis-evidence.v1";
pub const MAX_PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_BYTES: usize = 3 * 1024 * 1024;

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
    /// Attach one freshly verified explicit package-consumer corpus to the
    /// blind-spot report for this exact candidate. This does not discover
    /// ambient consumers or prove compatibility, execution, or completeness.
    pub fn analysis_evidence(
        &self,
        expected_candidate: &str,
        package: &CandidatePackageConsumerReplayInput<'_>,
    ) -> Result<String> {
        // Authenticate before deriving either report or processing caller-owned
        // package inputs. Both nested queries independently repeat this check.
        self.require_candidate(expected_candidate)?;
        let mut coverage = parse(
            &self.analysis_coverage(expected_candidate)?,
            PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
            "candidate analysis coverage",
        )?;
        let replay = parse(
            &self.package_consumer_replay(expected_candidate, package)?,
            PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_SCHEMA,
            "candidate package consumer replay",
        )?;
        validate_bindings(self, &coverage, &replay)?;

        let object = coverage
            .as_object_mut()
            .ok_or_else(|| invalid("candidate analysis coverage is not an object"))?;
        object.insert(
            "schema".into(),
            json!(PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_SCHEMA),
        );
        if object.get("evidence_class")
            != Some(&json!("retained_source_analysis_boundary_inventory"))
        {
            return Err(invalid(
                "candidate analysis coverage evidence class is unexpected",
            ));
        }
        object.insert(
            "evidence_class".into(),
            json!("retained_source_and_explicit_package_consumer_evidence"),
        );
        let areas = object
            .get_mut("areas")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("candidate analysis coverage areas are absent"))?;
        if areas.len() != 8 {
            return Err(invalid(
                "candidate analysis coverage area inventory is incomplete",
            ));
        }
        if areas
            .iter()
            .zip(AREA_ORDER)
            .any(|(row, name)| row["area"].as_str() != Some(name))
        {
            return Err(invalid(
                "candidate analysis coverage area inventory is not canonical",
            ));
        }
        let external = unique_area(areas, "external_consumers")?;
        if *external
            != json!({
                "area": "external_consumers",
                "status": "not_inspected",
                "basis": "retained_project_graph_has_no_external_consumer_inventory",
                "limitations": [
                    "manifest_exports_do_not_enumerate_actual_clients",
                    "absence_of_graph_edges_is_not_absence_of_external_callers"
                ],
                "required_evidence": [
                    "explicit_authenticated_consumer_inventory_and_compatibility_evidence"
                ]
            })
        {
            return Err(invalid(
                "candidate analysis coverage external-consumer boundary is unexpected",
            ));
        }
        *external = json!({
            "area": "external_consumers",
            "status": "partial",
            "basis": "explicit_authenticated_candidate_provider_package_consumer_source_replay",
            "limitations": [
                "absence_from_this_replay_is_not_absence_of_other_external_consumers",
                "not_api_abi_or_behavioral_compatibility",
                "imports_and_static_calls_are_not_runtime_execution"
            ],
            "required_evidence": [
                "authorized_installed_consumer_inventory",
                "consumer_compatibility_and_runtime_conformance_evidence"
            ]
        });
        object.insert("package_consumer_replay".into(), replay);
        super::super::image::render(
            coverage,
            false,
            MAX_PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_BYTES,
        )
        .map_err(|_| capacity("candidate analysis evidence report exceeds its byte bound"))
    }
}

fn parse(bytes: &str, schema: &str, owner: &'static str) -> Result<Value> {
    let value: Value = serde_json::from_str(bytes)
        .map_err(|_| invalid("nested candidate analysis evidence is not compiler JSON"))?;
    if value.as_object().is_none() || value["schema"] != schema {
        return Err(invalid(match owner {
            "candidate analysis coverage" => {
                "candidate analysis coverage has an unexpected compiler schema"
            }
            _ => "candidate package consumer replay has an unexpected compiler schema",
        }));
    }
    Ok(value)
}

fn validate_bindings(candidate: &ProjectCandidate, coverage: &Value, replay: &Value) -> Result<()> {
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
        || replay["candidate_revision"] != candidate.candidate_digest()
        || replay["base_project_revision"] != candidate.base.project_revision()
        || replay["candidate_project_revision"] != candidate.revision.project_revision()
        || replay["candidate_workspace_revision"] != candidate.revision.workspace_revision()
        || replay["candidate_project_graph_digest"] != candidate.revision.semantic_graph_digest()
        || replay["project_association"] != "candidate_provider_source_projection_only"
        || replay["source_authority"] != false
        || replay["execution"] != false
        || replay["publication_authority"] != false
        || replay["candidate_retained"] != false
        || replay["graph_retained"] != false
    {
        return Err(invalid(
            "candidate coverage and package evidence bindings disagree",
        ));
    }
    let provider = replay["provider_source"]
        .as_object()
        .ok_or_else(|| invalid("candidate package provider source binding is absent"))?;
    let path = provider
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("candidate package provider source path is absent"))?;
    let sources = coverage["sources"]
        .as_array()
        .ok_or_else(|| invalid("candidate analysis coverage source inventory is absent"))?;
    let matching = sources
        .iter()
        .filter(|source| source["path"].as_str() == Some(path))
        .collect::<Vec<_>>();
    if matching.len() != 1
        || matching[0]["source_revision"] != provider["candidate_source_revision"]
        || matching[0]["source_digest"] != provider["candidate_source_digest"]
    {
        return Err(invalid(
            "candidate package evidence has no exact coverage source join",
        ));
    }
    Ok(())
}

fn unique_area<'a>(areas: &'a mut [Value], name: &str) -> Result<&'a mut Value> {
    let mut found = None;
    for (index, row) in areas.iter().enumerate() {
        if row["area"] == name {
            if found.replace(index).is_some() {
                return Err(invalid("candidate analysis coverage area is duplicated"));
            }
        }
    }
    found
        .map(|index| &mut areas[index])
        .ok_or_else(|| invalid("candidate analysis coverage area is absent"))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G338", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G339", message)]
}
