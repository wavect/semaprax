//! Candidate-bound analysis-boundary inventory; authored and intentionally unrun.
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, ProjectSemanticImage,
    SemanticChange, IMAGE_ANALYSIS_COVERAGE_SCHEMA, MAX_IMAGE_ANALYSIS_COVERAGE_BYTES,
    MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_BYTES, PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum ImportFixture {
    None,
    NonNative,
    Native,
}

struct Fixture(PathBuf);
impl Fixture {
    fn new(import: ImportFixture) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-analysis-coverage-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "candidate-analysis-coverage"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "coverage.app"
sources = ["src/app.spx", "src/core.spx", "src/generated.spx", "src/tests.spx"]
web_exports = ["coverage.public"]
tests = ["coverage.tests"]
"#,
        )
        .unwrap();
        let mut core =
            "module coverage.core;\n@id(\"coverage.public\") fn public_value(value:i64)->i64 {value}\n"
                .to_owned();
        match import {
            ImportFixture::None => {}
            ImportFixture::NonNative => core.push_str(
                r#"@id("coverage.token") resource Token {
    @id("coverage.token.drop") drop trivial;
}
@id("coverage.host") interface Host permits {} {
    @id("coverage.host.observe") import fn observe(value: own Token) -> unit
        effects {} failure infallible consumes value always;
}
// A source-local lookalike remains an ordinary function, not provider evidence.
@id("coverage.provider-like") fn provider_like(value: own Token) -> unit {observe(value)}
"#,
            ),
            ImportFixture::Native => core.push_str(
                r#"@id("coverage.host") interface Host permits {} {
    @id("coverage.host.echo") import rust fn echo(value:i64)->unit
        effects {} failure infallible;
}
"#,
            ),
        }
        for (path, text) in [
            (
                "src/app.spx",
                "module coverage.app;\n@id(\"coverage.main\") fn main()->i64 {0}\n",
            ),
            ("src/core.spx", core.as_str()),
            (
                "src/generated.spx",
                "module coverage.generated;\n@id(\"coverage.generated-seed\") fn seed(value:i64)->i64 {value}\n",
            ),
            (
                "src/tests.spx",
                "module coverage.tests;\n@id(\"coverage.test\") fn main()->i64 {0}\n",
            ),
        ] {
            let parsed = semaprax::parse(text, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root)
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
            "src/generated.spx",
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

fn open(revision: &Arc<ProjectRevision>) -> ProjectCandidate {
    ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap()
}

fn introduce(candidate: &ProjectCandidate, id: &str, name: &str) -> ProjectCandidate {
    let intent = json!({"kind":"add_declaration","target":"coverage.generated-seed","declaration":{
        "id":id,"name":name,
        "parameters":[{"name":"value","type":"i64","mode":"value"}],
        "return_type":"i64","effects":[],"requires":[],"ensures":[],
        "body":{"kind":"call","target":"coverage.generated-seed","arguments":[{"kind":"place","name":"value"}]}
    }});
    let change = SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap();
    candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap()
}

fn report(candidate: &ProjectCandidate) -> (String, Value) {
    let text = candidate
        .analysis_coverage(candidate.candidate_digest())
        .unwrap();
    assert!(text.len() <= MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_BYTES);
    assert!(!text.ends_with('\n'));
    let value = serde_json::from_str(&text).unwrap();
    (text, value)
}

fn area<'a>(report: &'a Value, name: &str) -> &'a Value {
    let matches = report["areas"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["area"] == name)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    matches[0]
}

fn failed<T>(result: Result<T, Vec<semaprax::diagnostic::Diagnostic>>, code: &str) {
    let diagnostics = result.err().expect("invalid coverage selection accepted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code == code),
        "{diagnostics:?}"
    );
}

#[test]
fn final_candidate_report_is_the_independently_derived_image_inventory_with_exact_bindings() {
    let fixture = Fixture::new(ImportFixture::None);
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let candidate = introduce(&base, "coverage.generated-added", "generated_added");
    let base_json = base.to_json().to_owned();
    let candidate_json = candidate.to_json().to_owned();

    let image = ProjectSemanticImage::derive(
        Arc::clone(candidate.revision()),
        candidate.revision().project_revision(),
    )
    .unwrap();
    let image_text = image.analysis_coverage(image.image_digest()).unwrap();
    assert!(image_text.len() <= MAX_IMAGE_ANALYSIS_COVERAGE_BYTES);
    let mut expected: Value = serde_json::from_str(&image_text).unwrap();
    assert_eq!(expected["schema"], IMAGE_ANALYSIS_COVERAGE_SCHEMA);
    expected["schema"] = json!(PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA);
    expected["candidate_revision"] = json!(candidate.candidate_digest());
    expected["base_project_revision"] = json!(revision.project_revision());
    expected["candidate_retained"] = json!(false);
    expected["publication_authority"] = json!(false);

    let (_, actual) = report(&candidate);
    assert_eq!(actual, expected);
    assert_eq!(actual.as_object().unwrap().len(), 19);
    assert_eq!(actual["image_revision"], image.image_digest());
    assert_eq!(
        actual["project_revision"],
        candidate.revision().project_revision()
    );
    assert_eq!(
        actual["workspace_revision"],
        candidate.revision().workspace_revision()
    );
    assert_eq!(
        actual["project_graph_digest"],
        candidate.revision().semantic_graph_digest()
    );
    assert_eq!(actual["inventory"]["functions"].as_u64().unwrap(), 5);

    let base_image =
        ProjectSemanticImage::derive(Arc::clone(&revision), revision.project_revision()).unwrap();
    let base_coverage: Value = serde_json::from_str(
        &base_image
            .analysis_coverage(base_image.image_digest())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        actual["inventory"]["functions"].as_u64().unwrap(),
        base_coverage["inventory"]["functions"].as_u64().unwrap() + 1
    );
    for row in actual["sources"].as_array().unwrap() {
        let before = base_coverage["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["path"] == row["path"])
            .unwrap();
        if row["path"] == "src/generated.spx" {
            assert_ne!(row["source_revision"], before["source_revision"]);
            assert_ne!(row["source_digest"], before["source_digest"]);
        } else {
            assert_eq!(row, before);
        }
    }

    assert_eq!(actual["areas"].as_array().unwrap().len(), 8);
    assert_eq!(area(&actual, "declared_source_inputs")["status"], "known");
    for name in [
        "declared_external_contracts",
        "deployment_configuration",
        "generated_file_provenance",
        "generated_artifacts",
        "external_api_behavior",
        "runtime_environment",
        "external_consumers",
    ] {
        assert_eq!(area(&actual, name)["status"], "not_inspected");
    }
    assert!(area(&actual, "generated_file_provenance")["limitations"]
        .as_array()
        .unwrap()
        .contains(&json!("listed_generated_spx_is_checked_as_source")));
    for field in [
        "source_authority",
        "external_io",
        "execution",
        "candidate_retained",
        "publication_authority",
    ] {
        assert_eq!(actual[field], false);
    }
    assert!(actual["nonclaims"].as_array().unwrap().contains(&json!(
        "no_absence_proof_for_undeclared_files_services_or_external_callers"
    )));
    assert_eq!(base.to_json(), base_json);
    assert_eq!(candidate.to_json(), candidate_json);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn declared_external_contract_stays_partial_and_local_lookalike_is_not_provider_evidence() {
    let fixture = Fixture::new(ImportFixture::NonNative);
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let candidate = introduce(&open(&revision), "coverage.local-observe", "observe_local");
    let (_, value) = report(&candidate);

    assert_eq!(value["inventory"]["interfaces"], 1);
    assert_eq!(value["inventory"]["interface_imports"], 1);
    assert_eq!(value["external_contracts"].as_array().unwrap().len(), 1);
    assert_eq!(
        value["external_contracts"][0],
        json!({"path":"src/core.spx","module":"coverage.core",
            "interface_id":"coverage.host","import_id":"coverage.host.observe",
            "name":"observe","import_key":"coverage.host.observe","native_rust":false,
            "effects":[],"required_authority":[]})
    );
    let contracts = value["external_contracts"].to_string();
    assert!(!contracts.contains("coverage.provider-like"));
    assert!(!contracts.contains("coverage.local-observe"));
    assert_eq!(
        area(&value, "declared_external_contracts")["status"],
        "partial"
    );
    assert!(area(&value, "declared_external_contracts")["limitations"]
        .as_array()
        .unwrap()
        .contains(&json!(
            "declarations_are_not_external_implementation_evidence"
        )));
    assert_eq!(
        area(&value, "external_api_behavior")["status"],
        "not_inspected"
    );
    assert_eq!(
        area(&value, "external_consumers")["status"],
        "not_inspected"
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn native_import_admission_still_fails_before_any_candidate_can_claim_coverage() {
    let fixture = Fixture::new(ImportFixture::Native);
    let disk = fixture.bytes();
    let errors = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
    })
    .err()
    .expect("Native Rust import unexpectedly produced a candidate");
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-G218"),
        "{errors:?}"
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn sibling_and_stale_selectors_unlisted_deployment_inputs_and_reads_remain_isolated() {
    let fixture = Fixture::new(ImportFixture::None);
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let left = introduce(&base, "coverage.left", "left_generated");
    let right = introduce(&base, "coverage.right", "right_generated");
    let base_json = base.to_json().to_owned();
    let left_json = left.to_json().to_owned();
    let right_json = right.to_json().to_owned();
    let (_, left_report) = report(&left);
    let (_, right_report) = report(&right);

    assert_ne!(left.candidate_digest(), right.candidate_digest());
    assert_ne!(
        left_report["image_revision"],
        right_report["image_revision"]
    );
    assert_ne!(
        left_report["project_revision"],
        right_report["project_revision"]
    );
    assert_eq!(
        left_report["base_project_revision"],
        revision.project_revision()
    );
    assert_eq!(
        right_report["base_project_revision"],
        revision.project_revision()
    );
    failed(left.analysis_coverage(right.candidate_digest()), "SPX-G224");
    failed(left.analysis_coverage("not-a-digest"), "SPX-G222");
    failed(
        left.analysis_coverage(&format!("sha256:{}", "0".repeat(64))),
        "SPX-G224",
    );

    let before = left.analysis_coverage(left.candidate_digest()).unwrap();
    std::fs::write(
        fixture.0.join("deployment.secret"),
        b"unlisted deployment input must remain outside retained evidence",
    )
    .unwrap();
    let after = left.analysis_coverage(left.candidate_digest()).unwrap();
    assert_eq!(after, before);
    assert!(!after.contains("unlisted deployment input"));
    assert_eq!(
        area(&left_report, "deployment_configuration")["status"],
        "not_inspected"
    );
    assert_eq!(base.to_json(), base_json);
    assert_eq!(left.to_json(), left_json);
    assert_eq!(right.to_json(), right_json);
    assert_eq!(fixture.bytes(), disk);
    assert_eq!(
        std::fs::read(fixture.0.join("deployment.secret")).unwrap(),
        b"unlisted deployment input must remain outside retained evidence"
    );
}
