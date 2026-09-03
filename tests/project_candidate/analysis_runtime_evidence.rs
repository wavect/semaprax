//! Candidate runtime-boundary evidence regressions, authored and deliberately unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, CandidateTestPolicy, ProjectCandidate, ProjectRevision,
    SemanticChange, MAX_PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_BYTES,
    PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_SCHEMA,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-analysis-runtime-evidence-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v1"
name = "candidate-analysis-runtime-evidence"
entry = "runtime.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["runtime.add"]
tests = ["runtime.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/app.spx",
                "module runtime.app; use function @id(\"runtime.add\") from runtime.core as add; @id(\"runtime.main\") fn main()->i64 {add(20,22)}",
            ),
            (
                "src/core.spx",
                "module runtime.core; @id(\"runtime.add\") fn add(left:i64,right:i64)->i64 {left+right}",
            ),
            (
                "src/tests.spx",
                "module runtime.tests; use function @id(\"runtime.add\") from runtime.core as add; @id(\"runtime.tests.main\") fn main()->i64 {if add(20,22)==42 {0} else {1}}",
            ),
        ] {
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }

    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn candidate(revision: &Arc<ProjectRevision>, value: i64) -> ProjectCandidate {
    let base = ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap();
    let intent = json!({"kind":"replace_function_body","target":"runtime.add","body":{
        "kind":"binary","op":"+","left":{"kind":"place","name":"left"},
        "right":{"kind":"i64","value":value}
    }});
    let change = SemanticChange::new(base.revision().project_revision(), &intent).unwrap();
    base.apply(base.candidate_digest(), &change).unwrap()
}

fn policy() -> CandidateTestPolicy {
    CandidateTestPolicy::new(100_000, 65_536, 2 * 1024 * 1024).unwrap()
}

fn area<'a>(report: &'a Value, name: &str) -> &'a Value {
    let rows = report["areas"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["area"] == name)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    rows[0]
}

fn failed<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let diagnostics = result.err().expect("invalid runtime evidence accepted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code == code),
        "{diagnostics:?}"
    );
}

#[test]
fn exact_coverage_and_test_report_change_only_the_runtime_blind_spot() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let candidate = candidate(&revision, 22);
    let candidate_json = candidate.to_json().to_owned();
    let coverage_text = candidate
        .analysis_coverage(candidate.candidate_digest())
        .unwrap();
    let coverage: Value = serde_json::from_str(&coverage_text).unwrap();
    let test = candidate
        .execute_tests(candidate.candidate_digest(), &policy())
        .unwrap();
    assert!(test.passed());
    let test_value: Value = serde_json::from_str(test.to_json()).unwrap();

    let text = candidate
        .analysis_runtime_evidence(candidate.candidate_digest(), &policy())
        .unwrap();
    assert!(text.len() <= MAX_PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_BYTES);
    assert!(!text.ends_with('\n'));
    let report: Value = serde_json::from_str(&text).unwrap();

    assert_eq!(
        report["schema"],
        PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_SCHEMA
    );
    assert_eq!(report["candidate_revision"], candidate.candidate_digest());
    assert_eq!(report["base_project_revision"], revision.project_revision());
    assert_eq!(
        report["project_revision"],
        candidate.revision().project_revision()
    );
    assert_eq!(
        report["workspace_revision"],
        candidate.revision().workspace_revision()
    );
    assert_eq!(report["candidate_test_report"], test_value);
    assert_eq!(report["candidate_test_report_digest"], test.report_digest());
    assert_eq!(report.as_object().unwrap().len(), 23);
    assert_eq!(report["areas"].as_array().unwrap().len(), 8);

    let mut expected_root = coverage.clone();
    expected_root["schema"] = json!(PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_SCHEMA);
    expected_root["evidence_class"] =
        json!("retained_source_and_bounded_reference_interpreter_evidence");
    expected_root["execution"] = json!(true);
    expected_root["reference_interpreter_execution"] = json!(true);
    expected_root["target_execution"] = json!(false);
    expected_root["candidate_test_report_digest"] = json!(test.report_digest());
    expected_root["candidate_test_report"] = test_value;
    expected_root["nonclaims"].as_array_mut().unwrap().extend(
        [
            "reference_interpreter_only_not_native_wasm_generated_or_deployed_runtime",
            "no_dynamic_path_coverage_trace_liveness_or_environment_drift_observation",
            "one_declared_test_closure_is_not_full_quality_or_external_contract_proof",
            "no_current_filesystem_deployment_external_provider_or_consumer_authentication",
            "no_source_publication_target_process_network_or_external_io_authority",
        ]
        .into_iter()
        .map(|value| json!(value)),
    );

    for original in coverage["areas"].as_array().unwrap() {
        let name = original["area"].as_str().unwrap();
        if name == "runtime_environment" {
            assert_eq!(
                area(&report, name),
                &json!({
                    "area":"runtime_environment",
                    "status":"partial",
                    "basis":"exact_candidate_replay_and_bounded_reference_interpreter_test_closure_attempt",
                    "limitations":[
                        "reference_interpreter_only_not_native_wasm_generated_or_deployed_runtime",
                        "one_manifest_declared_test_closure_is_not_dynamic_path_coverage",
                        "no_trace_liveness_environment_configuration_or_drift_observation",
                        "pass_is_not_full_quality_compatibility_external_api_or_deployment_proof",
                        "nonpassing_outcome_is_one_bounded_attempt_not_complete_failure_classification"
                    ],
                    "required_evidence":[
                        "native_and_wasm_runtime_conformance_bound_to_this_candidate",
                        "authenticated_deployment_environment_and_external_provider_evidence",
                        "dynamic_coverage_and_full_quality_profile_evidence"
                    ]
                })
            );
        } else {
            assert_eq!(area(&report, name), original);
        }
    }

    expected_root["areas"] = report["areas"].clone();
    assert_eq!(report, expected_root);

    assert_eq!(report["execution"], true);
    assert_eq!(report["reference_interpreter_execution"], true);
    assert_eq!(report["target_execution"], false);
    for field in [
        "source_authority",
        "candidate_retained",
        "publication_authority",
    ] {
        assert_eq!(report[field], false);
    }
    assert_eq!(
        report["evidence_class"],
        "retained_source_and_bounded_reference_interpreter_evidence"
    );
    for claim in [
        "reference_interpreter_only_not_native_wasm_generated_or_deployed_runtime",
        "no_dynamic_path_coverage_trace_liveness_or_environment_drift_observation",
        "one_declared_test_closure_is_not_full_quality_or_external_contract_proof",
        "no_current_filesystem_deployment_external_provider_or_consumer_authentication",
        "no_source_publication_target_process_network_or_external_io_authority",
    ] {
        assert!(report["nonclaims"]
            .as_array()
            .unwrap()
            .contains(&json!(claim)));
    }
    assert_eq!(candidate.to_json(), candidate_json);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn failed_reference_test_is_exact_partial_runtime_evidence_not_a_passing_gate() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let candidate = candidate(&revision, 0);
    let test = candidate
        .execute_tests(candidate.candidate_digest(), &policy())
        .unwrap();
    assert!(!test.passed());
    let expected: Value = serde_json::from_str(test.to_json()).unwrap();
    assert_eq!(expected["passed"], false);

    let report: Value = serde_json::from_str(
        &candidate
            .analysis_runtime_evidence(candidate.candidate_digest(), &policy())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report["candidate_test_report"], expected);
    assert_eq!(area(&report, "runtime_environment")["status"], "partial");
    assert_eq!(report["execution"], true);
    assert_eq!(report["target_execution"], false);
    assert!(report["nonclaims"].as_array().unwrap().contains(&json!(
        "one_declared_test_closure_is_not_full_quality_or_external_contract_proof"
    )));
}

#[test]
fn subject_mismatch_is_rejected_and_composition_is_deterministic_and_immutable() {
    assert_eq!(
        MAX_PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_BYTES,
        4 * 1024 * 1024
    );
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let left = candidate(&revision, 22);
    let right = candidate(&revision, 23);
    let left_json = left.to_json().to_owned();
    let right_json = right.to_json().to_owned();
    failed(
        left.analysis_runtime_evidence(right.candidate_digest(), &policy()),
        "SPX-G224",
    );
    failed(
        left.analysis_runtime_evidence("not-a-digest", &policy()),
        "SPX-G222",
    );

    let first = left
        .analysis_runtime_evidence(left.candidate_digest(), &policy())
        .unwrap();
    let second = left
        .analysis_runtime_evidence(left.candidate_digest(), &policy())
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(left.to_json(), left_json);
    assert_eq!(right.to_json(), right_json);
    assert_eq!(fixture.bytes(), disk);
}
