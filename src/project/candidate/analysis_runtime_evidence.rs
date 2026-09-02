//! Candidate analysis boundaries with one freshly executed, policy-bounded
//! reference-interpreter test report. No serialized evidence is trusted.

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;

use super::{
    CandidateTestPolicy, ProjectCandidate, PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
    PROJECT_CANDIDATE_TEST_REPORT_SCHEMA,
};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_SCHEMA: &str =
    "semaprax.project-candidate-analysis-runtime-evidence.v1";
pub const MAX_PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_BYTES: usize = 4 * 1024 * 1024;

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
    /// Attach one freshly executed reference-interpreter attempt to this exact
    /// candidate's blind-spot report. The attempt does not establish target,
    /// deployment, external-provider, consumer, or dynamic-coverage evidence.
    pub fn analysis_runtime_evidence(
        &self,
        expected_candidate: &str,
        policy: &CandidateTestPolicy,
    ) -> Result<String> {
        // Authenticate before deriving either owner report. Both nested owners
        // independently repeat the candidate check, and execution independently
        // replays the complete candidate before evaluating its declared test root.
        self.require_candidate(expected_candidate)?;
        let mut coverage = parse(
            &self.analysis_coverage(expected_candidate)?,
            PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
            "candidate analysis coverage has an unexpected compiler schema",
        )?;
        let test = self.execute_tests(expected_candidate, policy)?;
        let report_digest = test.report_digest().to_owned();
        let test_report = parse(
            test.to_json(),
            PROJECT_CANDIDATE_TEST_REPORT_SCHEMA,
            "candidate test report has an unexpected compiler schema",
        )?;
        validate_bindings(self, policy, &coverage, &test_report, test.passed())?;

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
            json!(PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_SCHEMA),
        );
        object.insert(
            "evidence_class".into(),
            json!("retained_source_and_bounded_reference_interpreter_evidence"),
        );
        object.insert("execution".into(), json!(true));
        object.insert("reference_interpreter_execution".into(), json!(true));
        object.insert("target_execution".into(), json!(false));

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
        let runtime = unique_area(areas, "runtime_environment")?;
        if *runtime
            != json!({
                "area": "runtime_environment",
                "status": "not_inspected",
                "basis": "no_runtime_or_host_environment_observation",
                "limitations": [
                    "runtime_paths_test_coverage_liveness_and_environment_drift_are_not_measured"
                ],
                "required_evidence": [
                    "authorized_execution_and_environment_evidence_bound_to_this_revision"
                ]
            })
        {
            return Err(invalid(
                "candidate analysis coverage runtime boundary is unexpected",
            ));
        }
        *runtime = json!({
            "area": "runtime_environment",
            "status": "partial",
            "basis": "exact_candidate_replay_and_bounded_reference_interpreter_test_closure_attempt",
            "limitations": [
                "reference_interpreter_only_not_native_wasm_generated_or_deployed_runtime",
                "one_manifest_declared_test_closure_is_not_dynamic_path_coverage",
                "no_trace_liveness_environment_configuration_or_drift_observation",
                "pass_is_not_full_quality_compatibility_external_api_or_deployment_proof",
                "nonpassing_outcome_is_one_bounded_attempt_not_complete_failure_classification"
            ],
            "required_evidence": [
                "native_and_wasm_runtime_conformance_bound_to_this_candidate",
                "authenticated_deployment_environment_and_external_provider_evidence",
                "dynamic_coverage_and_full_quality_profile_evidence"
            ]
        });

        let nonclaims = object
            .get_mut("nonclaims")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("candidate analysis coverage nonclaims are absent"))?;
        for nonclaim in [
            "reference_interpreter_only_not_native_wasm_generated_or_deployed_runtime",
            "no_dynamic_path_coverage_trace_liveness_or_environment_drift_observation",
            "one_declared_test_closure_is_not_full_quality_or_external_contract_proof",
            "no_current_filesystem_deployment_external_provider_or_consumer_authentication",
            "no_source_publication_target_process_network_or_external_io_authority",
        ] {
            if nonclaims
                .iter()
                .any(|value| value.as_str() == Some(nonclaim))
            {
                return Err(invalid(
                    "candidate analysis runtime nonclaim inventory is duplicated",
                ));
            }
            nonclaims.push(json!(nonclaim));
        }
        object.insert("candidate_test_report_digest".into(), json!(report_digest));
        object.insert("candidate_test_report".into(), test_report);

        super::super::image::render(
            coverage,
            false,
            MAX_PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_BYTES,
        )
        .map_err(|_| capacity("candidate analysis runtime evidence exceeds its byte bound"))
    }
}

fn parse(bytes: &str, schema: &str, message: &'static str) -> Result<Value> {
    let value: Value = serde_json::from_str(bytes)
        .map_err(|_| invalid("nested candidate runtime evidence is not compiler JSON"))?;
    if value.as_object().is_none() || value["schema"] != schema {
        return Err(invalid(message));
    }
    Ok(value)
}

fn validate_bindings(
    candidate: &ProjectCandidate,
    policy: &CandidateTestPolicy,
    coverage: &Value,
    report: &Value,
    passed: bool,
) -> Result<()> {
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
        || report["candidate_digest"] != candidate.candidate_digest()
        || report["base_project_revision"] != candidate.base.project_revision()
        || report["project_revision"] != candidate.revision.project_revision()
        || report["workspace_revision"] != candidate.revision.workspace_revision()
        || report["candidate_replay"] != "exact_source_and_evidence_replay_before_execution"
        || report["execution_scope"] != "complete_manifest_declared_test_closure"
        || report["compiler"]["compatibility"] != "semaprax.candidate-tests.interpreter.v1"
        || report["compiler"]["binary_identity_claimed"] != false
        || report["options"]["max_steps"] != policy.max_steps()
        || report["options"]["max_execution_bytes"] != policy.max_execution_bytes()
        || report["options"]["max_report_bytes"] != policy.max_report_bytes()
        || report["options"]["trace"]["mode"] != "disabled"
        || report["options"]["trace"]["max_events"] != 0
        || report["options"]["trace"]["max_bytes"] != 0
        || report["passed"] != passed
        || report["execution"].as_object().is_none()
        || report["nonclaims"]
            != json!([
                "reference_interpreter_only",
                "no_native_or_wasm_execution",
                "no_full_quality_gate_success",
                "no_dynamic_coverage",
                "no_source_publication_authority",
                "no_trace_produced"
            ])
    {
        return Err(invalid(
            "candidate coverage and reference-interpreter evidence bindings disagree",
        ));
    }
    validate_source_bindings(coverage, report)?;
    Ok(())
}

fn validate_source_bindings(coverage: &Value, report: &Value) -> Result<()> {
    let sources = coverage["sources"]
        .as_array()
        .ok_or_else(|| invalid("candidate analysis coverage source inventory is absent"))?;
    let executed = report["source_inventory"]["candidate"]
        .as_array()
        .ok_or_else(|| invalid("candidate test source inventory is absent"))?;
    if sources.len() != executed.len() {
        return Err(invalid(
            "candidate coverage and test source inventories disagree",
        ));
    }
    for source in sources {
        let path = source["path"]
            .as_str()
            .ok_or_else(|| invalid("candidate coverage source path is absent"))?;
        let matching = executed
            .iter()
            .filter(|entry| entry["path"].as_str() == Some(path))
            .collect::<Vec<_>>();
        if matching.len() != 1
            || matching[0]["source_revision"] != source["source_revision"]
            || matching[0]["source_digest"] != source["source_digest"]
        {
            return Err(invalid(
                "candidate coverage has no exact test source inventory join",
            ));
        }
    }
    let origin = &report["test_origin"];
    let origin_path = origin["path"]
        .as_str()
        .ok_or_else(|| invalid("candidate test origin path is absent"))?;
    let matching = sources
        .iter()
        .filter(|source| source["path"].as_str() == Some(origin_path))
        .collect::<Vec<_>>();
    if matching.len() != 1
        || matching[0]["source_revision"] != origin["source_revision"]
        || matching[0]["source_digest"] != origin["source_digest"]
        || origin["module"].as_str().is_none()
        || origin["stable_id"].as_str().is_none()
    {
        return Err(invalid(
            "candidate test origin has no exact coverage source join",
        ));
    }
    Ok(())
}

fn unique_area<'a>(areas: &'a mut [Value], name: &str) -> Result<&'a mut Value> {
    let mut found = None;
    for (index, row) in areas.iter().enumerate() {
        if row["area"] == name && found.replace(index).is_some() {
            return Err(invalid("candidate analysis coverage area is duplicated"));
        }
    }
    found
        .map(|index| &mut areas[index])
        .ok_or_else(|| invalid("candidate analysis coverage area is absent"))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G361", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G362", message)]
}
