//! Candidate-bound pathless artifact evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ImageArtifactKind, ProjectCandidate, ProjectRevision,
    SemanticChange, MAX_PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_BYTES,
    PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_SCHEMA,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

enum FixtureKind {
    Scalar,
    Owned,
    Api,
}

struct Fixture {
    root: PathBuf,
    paths: Vec<&'static str>,
}

impl Fixture {
    fn new(kind: FixtureKind) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-analysis-artifact-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        let paths = match kind {
            FixtureKind::Scalar => {
                copy_example(&root, "calculator-project", "src/core.spx");
                vec![
                    "semaprax.toml",
                    "src/app.spx",
                    "src/core.spx",
                    "src/tests.spx",
                ]
            }
            FixtureKind::Owned => {
                copy_example(&root, "frame-payload-project", "src/frame.spx");
                vec![
                    "semaprax.toml",
                    "src/app.spx",
                    "src/frame.spx",
                    "src/tests.spx",
                ]
            }
            FixtureKind::Api => {
                std::fs::write(
                    root.join("semaprax.toml"),
                    r#"schema = "semaprax.project.v1"
name = "candidate-analysis-artifact"
entry = "api.app"
sources = ["src/app.spx", "src/core.spx", "src/flags.spx", "src/tests.spx"]
web_exports = ["api.add", "api.flag"]
tests = ["api.tests"]
"#,
                )
                .unwrap();
                for (path, source) in [
                    (
                        "src/core.spx",
                        r#"module api.core;
@id("api.add") fn add(left:i64,right:i64)->i64 {left+right}
@id("api.hidden") fn hidden(value:i64)->i64 {value}
"#,
                    ),
                    (
                        "src/flags.spx",
                        r#"module api.flags;
@id("api.flag") fn invert(value:bool)->bool {!value}
"#,
                    ),
                    (
                        "src/app.spx",
                        r#"module api.app;
use function @id("api.add") from api.core as add;
use function @id("api.flag") from api.flags as invert;
@id("api.main") fn main()->i64 {if invert(false) {add(40,2)} else {0}}
"#,
                    ),
                    (
                        "src/tests.spx",
                        r#"module api.tests;
use function @id("api.add") from api.core as add;
@id("api.test") fn main()->i64 {if add(40,2)==42 {0}else{1}}
"#,
                    ),
                ] {
                    let parsed = semaprax::parse(source, path).unwrap();
                    std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
                }
                vec![
                    "semaprax.toml",
                    "src/app.spx",
                    "src/core.spx",
                    "src/flags.spx",
                    "src/tests.spx",
                ]
            }
        };
        Self { root, paths }
    }

    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.root.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }

    fn bytes(&self) -> Vec<Vec<u8>> {
        self.paths
            .iter()
            .map(|path| std::fs::read(self.root.join(path)).unwrap())
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn copy_example(root: &Path, name: &str, module: &str) {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name);
    for path in ["semaprax.toml", "src/app.spx", module, "src/tests.spx"] {
        std::fs::copy(sample.join(path), root.join(path)).unwrap();
    }
}

fn apply(candidate: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    candidate
        .apply(
            candidate.candidate_digest(),
            &SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap()
}

fn report(candidate: &ProjectCandidate, kind: ImageArtifactKind) -> (String, Value) {
    let text = candidate
        .analysis_artifact_evidence(candidate.candidate_digest(), kind)
        .unwrap();
    assert!(text.len() <= MAX_PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_BYTES);
    assert!(!text.ends_with('\n'));
    let value = serde_json::from_str(&text).unwrap();
    (text, value)
}

fn area<'a>(value: &'a Value, name: &str) -> &'a Value {
    let rows = value["areas"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["area"] == name)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    rows[0]
}

fn independently_composed(candidate: &ProjectCandidate, kind: ImageArtifactKind) -> Value {
    let mut coverage: Value = serde_json::from_str(
        &candidate
            .analysis_coverage(candidate.candidate_digest())
            .unwrap(),
    )
    .unwrap();
    let delta: Value = serde_json::from_str(
        &candidate
            .artifact_delta(candidate.candidate_digest(), kind)
            .unwrap(),
    )
    .unwrap();
    coverage["schema"] = json!(PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_SCHEMA);
    coverage["evidence_class"] =
        json!("retained_source_and_verified_pathless_candidate_artifact_evidence");
    coverage["areas"][4] = json!({
        "area":"generated_artifacts",
        "status":"partial",
        "basis":"independently_replayed_selected_pathless_candidate_artifact",
        "limitations":[
            "only_the_selected_web_npm_openapi_or_c_carrier_was_inspected",
            "artifact_report_omits_encoded_file_bodies_but_binds_verified_envelope_and_file_hashes",
            "no_filesystem_materialization_installation_deployment_or_runtime_execution",
            "outside_projection_is_not_platform_absence",
            "zero_selected_carrier_files_is_not_absence_of_other_artifact_kinds_or_deployed_artifacts"
        ],
        "required_evidence":[
            "authorized_materialization_and_deployment_binding_for_the_selected_artifact",
            "runtime_and_external_consumer_conformance_for_the_selected_artifact"
        ]
    });
    coverage["artifact_delta"] = delta;
    coverage
}

fn assert_exact_evidence(candidate: &ProjectCandidate, kind: ImageArtifactKind) -> Value {
    let (first, value) = report(candidate, kind);
    assert_eq!(value, independently_composed(candidate, kind));
    assert_eq!(value.as_object().unwrap().len(), 20);
    assert_eq!(first, report(candidate, kind).0);
    let kind_name = match kind {
        ImageArtifactKind::Web => "web",
        ImageArtifactKind::Npm => "npm",
        ImageArtifactKind::OpenApi => "openapi",
        ImageArtifactKind::C => "c",
    };
    assert_eq!(value["artifact_delta"]["kind"], kind_name);
    assert_eq!(
        value["artifact_delta"]["candidate_digest"],
        candidate.candidate_digest()
    );
    assert_eq!(
        value["artifact_delta"]["candidate"]["project_revision"],
        value["project_revision"]
    );
    assert_eq!(
        value["artifact_delta"]["candidate"]["project_graph_digest"],
        value["project_graph_digest"]
    );
    for source in value["sources"].as_array().unwrap() {
        let joined = value["artifact_delta"]["candidate"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["path"] == source["path"])
            .collect::<Vec<_>>();
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0]["source_revision"], source["source_revision"]);
        assert_eq!(joined[0]["source_digest"], source["source_digest"]);
    }
    assert_eq!(area(&value, "generated_artifacts")["status"], "partial");
    let baseline: Value = serde_json::from_str(
        &candidate
            .analysis_coverage(candidate.candidate_digest())
            .unwrap(),
    )
    .unwrap();
    for name in [
        "declared_source_inputs",
        "declared_external_contracts",
        "deployment_configuration",
        "generated_file_provenance",
        "external_api_behavior",
        "runtime_environment",
        "external_consumers",
    ] {
        assert_eq!(area(&value, name), area(&baseline, name));
    }
    for flag in [
        "source_authority",
        "external_io",
        "execution",
        "candidate_retained",
        "publication_authority",
    ] {
        assert_eq!(value[flag], false);
    }
    value
}

fn failed<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().expect("hostile artifact evidence accepted");
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn web_changed_and_unchanged_candidates_attach_exact_files_hashes_exports_and_sources() {
    let fixture = Fixture::new(FixtureKind::Scalar);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let unchanged = assert_exact_evidence(&base, ImageArtifactKind::Web);
    assert_eq!(
        unchanged["artifact_delta"]["comparison"]["artifact_bytes_equal"],
        true
    );
    let changed = apply(
        &base,
        json!({"kind":"change_function_signature","target":"calculator.add","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]}),
    );
    let evidence = assert_exact_evidence(&changed, ImageArtifactKind::Web);
    assert_eq!(
        evidence["artifact_delta"]["comparison"]["carrier_equal"],
        false
    );
    assert!(evidence["artifact_delta"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| {
            row["base"].is_null()
                || row["candidate"].is_null()
                || (row["base"]["sha256"]
                    .as_str()
                    .unwrap()
                    .starts_with("sha256:")
                    && row["candidate"]["sha256"]
                        .as_str()
                        .unwrap()
                        .starts_with("sha256:"))
        }));
    assert!(evidence["artifact_delta"]["exports"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == "calculator.add"));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn all_four_existing_pathless_carriers_are_explicit_selected_partial_evidence() {
    let owned = Fixture::new(FixtureKind::Owned);
    let api = Fixture::new(FixtureKind::Api);
    let owned_candidate = owned.candidate();
    let api_candidate = api.candidate();
    let npm = assert_exact_evidence(&owned_candidate, ImageArtifactKind::Npm);
    let openapi = assert_exact_evidence(&api_candidate, ImageArtifactKind::OpenApi);
    let c = assert_exact_evidence(&api_candidate, ImageArtifactKind::C);
    for (value, kind) in [(npm, "npm"), (openapi, "openapi"), (c, "c")] {
        assert_eq!(value["artifact_delta"]["kind"], kind);
        assert_eq!(
            area(&value, "generated_artifacts")["basis"],
            "independently_replayed_selected_pathless_candidate_artifact"
        );
        assert!(value["artifact_delta"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["path"].is_string()));
    }
}

#[test]
fn exact_candidate_selection_is_deterministic_immutable_and_ignores_unlisted_deployment_files() {
    let fixture = Fixture::new(FixtureKind::Scalar);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let left = apply(
        &base,
        json!({"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i64","value":7}}),
    );
    let right = apply(
        &base,
        json!({"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i64","value":8}}),
    );
    let left_json = left.to_json().to_owned();
    let first = report(&left, ImageArtifactKind::Web).0;
    std::fs::write(
        fixture.root.join("deployment.json"),
        br#"{"environment":"production"}"#,
    )
    .unwrap();
    assert_eq!(report(&left, ImageArtifactKind::Web).0, first);
    failed(
        left.analysis_artifact_evidence("sha256:broken", ImageArtifactKind::Web),
        "SPX-G222",
    );
    failed(
        left.analysis_artifact_evidence(right.candidate_digest(), ImageArtifactKind::Web),
        "SPX-G224",
    );
    assert_eq!(left.to_json(), left_json);
    assert_eq!(fixture.bytes(), disk);
    assert_eq!(
        area(
            &report(&left, ImageArtifactKind::Web).1,
            "deployment_configuration"
        )["status"],
        "not_inspected"
    );
}

#[test]
fn unsupported_carrier_admission_propagates_without_empty_or_partial_fabrication() {
    let fixture = Fixture::new(FixtureKind::Owned);
    let candidate = fixture.candidate();
    let errors = candidate
        .analysis_artifact_evidence(candidate.candidate_digest(), ImageArtifactKind::OpenApi)
        .err()
        .expect("unsupported owned OpenAPI carrier fabricated evidence");
    assert!(!errors.is_empty());
    assert!(errors.iter().all(|error| error.code != "SPX-G352"));
}
