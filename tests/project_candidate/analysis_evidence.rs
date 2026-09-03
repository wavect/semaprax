//! Candidate analysis evidence with one explicit package corpus.
use semaprax::diagnostic::Diagnostic;
use semaprax::package_lock_v2::{self, Coordinate};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::package_resolver::{self, Requirement, ResolutionInput, ResolutionOptions};
use semaprax::package_source_capsule::{self, PackageSource, SourceCapsuleOptions};
use semaprax::project::{
    with_authenticated_project, CandidatePackageConsumerReplayInput, ProjectCandidate,
    ProjectRevision, SemanticChange, MAX_PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_BYTES,
    PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_SCHEMA,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const PROVIDER_PATH: &str = "src/core.spx";
const TARGET: &str = "lib.answer";

fn canonical(text: &str, path: &str) -> String {
    semaprax::format::canonical(&semaprax::parse(text, path).unwrap())
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-analysis-evidence-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "libmath"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "libmath"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["lib.answer", "lib.unobserved", "lib.unused"]
tests = ["lib.tests"]
"#,
        )
        .unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        for (name, source) in [
            (
                "app",
                r#"module lib.app;
use function @id("lib.answer") from libmath as answer;
@id("lib.app-helper") fn app_helper()->i64 {answer()}
"#,
            ),
            (
                "core",
                r#"module libmath;
@id("lib.answer") fn answer()->i64 {41}
@id("lib.unobserved") fn unobserved()->i64 {7}
@id("lib.unused") fn unused()->i64 {99}
@id("lib.main") fn main()->i64 {answer()}
"#,
            ),
            (
                "tests",
                r#"module lib.tests;
use function @id("lib.answer") from libmath as answer;
@id("lib.test") fn main()->i64 {if answer()==41 {0}else{1}}
"#,
            ),
        ] {
            let path = format!("src/{name}.spx");
            std::fs::write(root.join(&path), canonical(source, &path)).unwrap();
        }
        fixture
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
            PROVIDER_PATH,
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
fn changed(base: &ProjectCandidate, value: i64) -> ProjectCandidate {
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"replace_function_body","target":TARGET,
            "body":{"kind":"i64","value":value}}),
    )
    .unwrap();
    base.apply(base.candidate_digest(), &change).unwrap()
}

struct Corpus {
    provider: Coordinate,
    sources: Vec<PackageSource>,
    input: ResolutionInput,
    resolution_options: ResolutionOptions,
    evidence: String,
    capsule_options: SourceCapsuleOptions,
    capsule: String,
}
impl Corpus {
    fn for_candidate(root: &Path, candidate: &ProjectCandidate) -> Self {
        let provider_source = candidate
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == PROVIDER_PATH)
            .unwrap()
            .source()
            .to_owned();
        let provider_path = root.join("analysis-provider.spx");
        std::fs::write(&provider_path, &provider_source).unwrap();
        let provider_report =
            package_report_v2::generate(&provider_path, &PackageReportV2Options::default())
                .unwrap();
        let interface_path = root.join("analysis-consumer-interface.spx");
        std::fs::write(
            &interface_path,
            canonical(
                "module app.main;\n@id(\"app.main\") fn main()->i64 {0}\n",
                "analysis-consumer-interface.spx",
            ),
        )
        .unwrap();
        let consumer_report =
            package_report_v2::generate(&interface_path, &PackageReportV2Options::default())
                .unwrap();
        let provider = Coordinate {
            package: "libmath".into(),
            version: "1.0.0".into(),
        };
        let consumer = Coordinate {
            package: "app.main".into(),
            version: "1.0.0".into(),
        };
        let provider_subject =
            package_lock_v2::create_subject(&provider, &provider_report, &[], &[]).unwrap();
        let consumer_subject = package_lock_v2::create_subject(
            &consumer,
            &consumer_report,
            std::slice::from_ref(&provider),
            &[],
        )
        .unwrap();
        let input = ResolutionInput {
            requirements: vec![Requirement {
                package: "app.main".into(),
                range: "=1.0.0".into(),
            }],
            subjects: vec![provider_subject, consumer_subject],
            target: "wasm32".into(),
            allowed_capabilities: vec![],
        };
        let resolution_options = ResolutionOptions::default();
        let evidence = package_resolver::generate(&input, &resolution_options).unwrap();
        let sources = vec![
            PackageSource {
                package: "app.main".into(),
                report: consumer_report,
                source: canonical(
                    r#"module app.main;
use function @id("lib.answer") from libmath as answer;
use function @id("lib.unused") from libmath as unused;
@id("app.main") fn main()->i64 {answer()+1}
fn private_helper()->i64 {answer()}
"#,
                    "analysis-consumer.spx",
                ),
            },
            PackageSource {
                package: "libmath".into(),
                report: provider_report,
                source: provider_source,
            },
        ];
        let capsule_options = SourceCapsuleOptions::default();
        let capsule = package_source_capsule::generate(
            &sources,
            &evidence,
            &input,
            &resolution_options,
            &capsule_options,
        )
        .unwrap();
        Self {
            provider,
            sources,
            input,
            resolution_options,
            evidence,
            capsule_options,
            capsule,
        }
    }
    fn input<'a>(&'a self, target: &'a str) -> CandidatePackageConsumerReplayInput<'a> {
        CandidatePackageConsumerReplayInput {
            provider: &self.provider,
            provider_source_path: PROVIDER_PATH,
            target,
            capsule: &self.capsule,
            sources: &self.sources,
            resolution_evidence: &self.evidence,
            resolution_input: &self.input,
            resolution_options: &self.resolution_options,
            capsule_options: &self.capsule_options,
        }
    }
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
fn report(candidate: &ProjectCandidate, corpus: &Corpus, target: &str) -> (String, Value) {
    let text = candidate
        .analysis_evidence(candidate.candidate_digest(), &corpus.input(target))
        .unwrap();
    assert!(text.len() <= MAX_PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_BYTES);
    assert!(!text.ends_with('\n'));
    let value = serde_json::from_str(&text).unwrap();
    (text, value)
}
fn failed<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let diagnostics = result.err().expect("invalid analysis evidence accepted");
    assert!(
        diagnostics.iter().any(|error| error.code == code),
        "{diagnostics:?}"
    );
}

#[test]
fn explicit_candidate_era_consumers_change_only_the_external_consumer_boundary_to_partial() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let candidate = changed(&base, 42);
    let base_json = base.to_json().to_owned();
    let candidate_json = candidate.to_json().to_owned();
    let corpus = Corpus::for_candidate(&fixture.0, &candidate);
    let coverage: Value = serde_json::from_str(
        &candidate
            .analysis_coverage(candidate.candidate_digest())
            .unwrap(),
    )
    .unwrap();
    let replay: Value = serde_json::from_str(
        &candidate
            .package_consumer_replay(candidate.candidate_digest(), &corpus.input(TARGET))
            .unwrap(),
    )
    .unwrap();
    let (_, actual) = report(&candidate, &corpus, TARGET);

    assert_eq!(actual["schema"], PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_SCHEMA);
    // `Compose candidate analysis boundary declarations` added the declared
    // external contract boundary to every analysis report.
    assert_eq!(actual.as_object().unwrap().len(), 21);
    assert!(!actual["external_contracts"].is_null());
    assert_eq!(
        actual["evidence_class"],
        "retained_source_and_explicit_package_consumer_evidence"
    );
    assert_eq!(actual["package_consumer_replay"], replay);
    assert_eq!(actual["candidate_revision"], candidate.candidate_digest());
    assert_eq!(actual["base_project_revision"], revision.project_revision());
    assert_eq!(
        actual["project_revision"],
        candidate.revision().project_revision()
    );
    assert_eq!(actual["areas"].as_array().unwrap().len(), 8);
    for (field, value) in coverage.as_object().unwrap() {
        if !["schema", "evidence_class", "areas"].contains(&field.as_str()) {
            assert_eq!(
                &actual[field.as_str()],
                value,
                "coverage field {field} changed"
            );
        }
    }
    for name in [
        "declared_source_inputs",
        "declared_external_contracts",
        "deployment_configuration",
        "generated_file_provenance",
        "generated_artifacts",
        "external_api_behavior",
        "runtime_environment",
    ] {
        assert_eq!(area(&actual, name), area(&coverage, name));
    }
    let external = area(&actual, "external_consumers");
    assert_eq!(
        area(&coverage, "external_consumers")["status"],
        "not_inspected"
    );
    assert_eq!(external["status"], "partial");
    assert_eq!(
        external["basis"],
        "explicit_authenticated_candidate_provider_package_consumer_source_replay"
    );
    assert!(external["limitations"].as_array().unwrap().contains(&json!(
        "absence_from_this_replay_is_not_absence_of_other_external_consumers"
    )));
    assert!(external["limitations"]
        .as_array()
        .unwrap()
        .contains(&json!("not_api_abi_or_behavioral_compatibility")));
    assert!(external["limitations"]
        .as_array()
        .unwrap()
        .contains(&json!("imports_and_static_calls_are_not_runtime_execution")));
    assert_eq!(
        replay["counts"],
        json!({"packages":2,"imports":1,"calls":2})
    );
    assert_eq!(
        replay["provider_source"]["candidate_source_revision"],
        candidate
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == PROVIDER_PATH)
            .unwrap()
            .source_revision()
    );
    for field in [
        "source_authority",
        "external_io",
        "execution",
        "candidate_retained",
        "publication_authority",
    ] {
        assert_eq!(actual[field], false);
    }
    assert_eq!(replay["graph_retained"], false);
    assert_eq!(base.to_json(), base_json);
    assert_eq!(candidate.to_json(), candidate_json);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn import_only_zero_call_evidence_is_partial_but_never_an_absence_claim() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let candidate = changed(&open(&revision), 42);
    let corpus = Corpus::for_candidate(&fixture.0, &candidate);
    let (_, actual) = report(&candidate, &corpus, "lib.unused");
    let replay = &actual["package_consumer_replay"];
    assert_eq!(
        replay["counts"],
        json!({"packages":2,"imports":1,"calls":0})
    );
    assert_eq!(replay["imports"].as_array().unwrap().len(), 1);
    assert_eq!(replay["calls"], json!([]));
    assert_eq!(area(&actual, "external_consumers")["status"], "partial");
    assert!(area(&actual, "external_consumers")["limitations"]
        .as_array()
        .unwrap()
        .contains(&json!(
            "absence_from_this_replay_is_not_absence_of_other_external_consumers"
        )));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn explicit_corpus_with_no_matching_import_or_call_remains_partial_and_requires_more_evidence() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let candidate = changed(&open(&revision), 42);
    let corpus = Corpus::for_candidate(&fixture.0, &candidate);
    let (_, actual) = report(&candidate, &corpus, "lib.unobserved");
    let replay = &actual["package_consumer_replay"];
    assert_eq!(
        replay["counts"],
        json!({"packages":2,"imports":0,"calls":0})
    );
    assert_eq!(replay["imports"], json!([]));
    assert_eq!(replay["calls"], json!([]));
    let external = area(&actual, "external_consumers");
    assert_eq!(external["status"], "partial");
    assert!(external["limitations"].as_array().unwrap().contains(&json!(
        "absence_from_this_replay_is_not_absence_of_other_external_consumers"
    )));
    assert!(external["required_evidence"]
        .as_array()
        .unwrap()
        .contains(&json!("authorized_installed_consumer_inventory")));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn stale_foreign_tampered_and_sibling_evidence_is_rejected_or_history_isolated() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let left = changed(&base, 42);
    let right = changed(&base, 43);
    let base_corpus = Corpus::for_candidate(&fixture.0, &base);
    let left_corpus = Corpus::for_candidate(&fixture.0, &left);
    let right_corpus = Corpus::for_candidate(&fixture.0, &right);
    failed(
        left.analysis_evidence(right.candidate_digest(), &left_corpus.input(TARGET)),
        "SPX-G224",
    );
    failed(
        left.analysis_evidence("not-a-digest", &left_corpus.input(TARGET)),
        "SPX-G222",
    );
    failed(
        left.analysis_evidence(left.candidate_digest(), &base_corpus.input(TARGET)),
        "SPX-G336",
    );
    failed(
        left.analysis_evidence(left.candidate_digest(), &right_corpus.input(TARGET)),
        "SPX-G336",
    );
    let mut tampered_sources = left_corpus.sources.clone();
    let original_consumer_source = tampered_sources[0].source.clone();
    tampered_sources[0].source = tampered_sources[0]
        .source
        .replace("answer() + 1", "answer() + 2");
    assert_ne!(tampered_sources[0].source, original_consumer_source);
    let mut tampered = left_corpus.input(TARGET);
    tampered.sources = &tampered_sources;
    failed(
        left.analysis_evidence(left.candidate_digest(), &tampered),
        "SPX-PS507",
    );

    let (first, left_report) = report(&left, &left_corpus, TARGET);
    let (again, _) = report(&left, &left_corpus, TARGET);
    let (_, right_report) = report(&right, &right_corpus, TARGET);
    assert_eq!(first, again);
    assert_ne!(
        left_report["candidate_revision"],
        right_report["candidate_revision"]
    );
    assert_ne!(
        left_report["package_consumer_replay"]["package_graph_revision"],
        right_report["package_consumer_replay"]["package_graph_revision"]
    );
    assert_eq!(fixture.bytes(), disk);
}
