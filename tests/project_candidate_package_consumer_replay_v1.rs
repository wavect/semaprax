//! Explicit candidate-era package-consumer replay; authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::package_lock_v2::{self, Coordinate};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::package_resolver::{self, Requirement, ResolutionInput, ResolutionOptions};
use semaprax::package_source_capsule::{self, PackageSource, SourceCapsuleOptions};
use semaprax::project::{
    with_authenticated_project, CandidatePackageConsumerReplayInput, ProjectCandidate,
    ProjectRevision, SemanticChange, MAX_PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_BYTES,
    PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_SCHEMA,
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
            "spx-candidate-package-consumers-{}-{}",
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
entry = "lib.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["lib.answer", "lib.unused"]
tests = ["lib.tests"]
"#,
        )
        .unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        fixture.write(
            "app",
            r#"module lib.app;
use function @id("lib.answer") from libmath as answer;
@id("lib.main") fn main()->i64 {answer()}
"#,
        );
        fixture.write(
            "core",
            r#"module libmath;
@id("lib.answer") fn answer()->i64 {41}
@id("lib.unused") fn unused()->i64 {99}
"#,
        );
        fixture.write(
            "tests",
            r#"module lib.tests;
use function @id("lib.answer") from libmath as answer;
@id("lib.test") fn main()->i64 {if answer()==41 {0}else{1}}
"#,
        );
        fixture
    }
    fn write(&self, module: &str, text: &str) {
        let logical = format!("src/{module}.spx");
        std::fs::write(self.0.join(&logical), canonical(text, &logical)).unwrap();
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
    let intent = json!({"kind":"replace_function_body","target":TARGET,
        "body":{"kind":"i64","value":value}});
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
    )
    .unwrap()
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
    fn candidate_era(root: &Path, candidate: &ProjectCandidate) -> Self {
        let candidate_source = candidate
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == PROVIDER_PATH)
            .unwrap()
            .source()
            .to_owned();
        let provider_path = root.join("candidate-provider.spx");
        std::fs::write(&provider_path, &candidate_source).unwrap();
        let provider_report =
            package_report_v2::generate(&provider_path, &PackageReportV2Options::default())
                .unwrap();
        let consumer_interface = canonical(
            "module app.main;\n@id(\"app.main\") fn main()->i64 {0}\n",
            "consumer-interface.spx",
        );
        let consumer_interface_path = root.join("consumer-interface.spx");
        std::fs::write(&consumer_interface_path, consumer_interface).unwrap();
        let consumer_report = package_report_v2::generate(
            &consumer_interface_path,
            &PackageReportV2Options::default(),
        )
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
        let consumer_source = canonical(
            r#"module app.main;
use function @id("lib.answer") from libmath as answer;
use function @id("lib.unused") from libmath as unused;
@id("app.main") fn main()->i64 {answer()+1}
fn private_helper()->i64 {answer()}
"#,
            "consumer.spx",
        );
        let sources = vec![
            PackageSource {
                package: "app.main".into(),
                report: consumer_report,
                source: consumer_source,
            },
            PackageSource {
                package: "libmath".into(),
                report: provider_report,
                source: candidate_source,
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
    fn replay<'a>(&'a self, target: &'a str) -> CandidatePackageConsumerReplayInput<'a> {
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

fn report(candidate: &ProjectCandidate, corpus: &Corpus, target: &str) -> Value {
    serde_json::from_str(
        &candidate
            .package_consumer_replay(candidate.candidate_digest(), &corpus.replay(target))
            .unwrap(),
    )
    .unwrap()
}
fn failed<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let diagnostics = result.err().expect("invalid package replay accepted");
    assert!(
        diagnostics.iter().any(|error| error.code == code),
        "{diagnostics:?}"
    );
}

#[test]
fn candidate_era_capsule_replays_called_and_import_only_consumers_with_exact_source_binding() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let candidate = changed(&base, 42);
    let base_json = base.to_json().to_owned();
    let candidate_json = candidate.to_json().to_owned();
    let corpus = Corpus::candidate_era(&fixture.0, &candidate);
    let called = report(&candidate, &corpus, TARGET);

    assert_eq!(
        called["schema"],
        PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_SCHEMA
    );
    assert_eq!(called.as_object().unwrap().len(), 26);
    assert_eq!(called["candidate_revision"], candidate.candidate_digest());
    assert_eq!(called["base_project_revision"], revision.project_revision());
    assert_eq!(
        called["candidate_project_revision"],
        candidate.revision().project_revision()
    );
    assert_eq!(
        called["provider"],
        json!({"package":"libmath","version":"1.0.0"})
    );
    assert_eq!(called["provider_source"]["path"], PROVIDER_PATH);
    let candidate_source = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == PROVIDER_PATH)
        .unwrap();
    assert_eq!(called["provider_source"].as_object().unwrap().len(), 9);
    assert_eq!(
        called["provider_source"]["candidate_source_revision"],
        candidate_source.source_revision()
    );
    assert_eq!(
        called["provider_source"]["provider_interface_source_revision"],
        candidate_source.source_revision()
    );
    assert_eq!(
        called["provider_source"]["source_bytes"],
        candidate_source.source().len()
    );
    assert_ne!(
        called["provider_source"]["provider_package_source_digest"],
        called["provider_source"]["candidate_source_digest"]
    );
    assert_eq!(called["provider_source"]["changed_from_base"], true);
    assert_eq!(called["target"], TARGET);
    assert_eq!(
        called["counts"],
        json!({"packages":2,"imports":1,"calls":2})
    );
    assert_eq!(called["imports"].as_array().unwrap().len(), 1);
    assert_eq!(called["calls"].as_array().unwrap().len(), 2);
    assert!(called["calls"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["caller"] == "app.main"));
    assert!(called["calls"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["caller"] != "app.main"));

    let imported = report(&candidate, &corpus, "lib.unused");
    assert_eq!(
        imported["counts"],
        json!({"packages":2,"imports":1,"calls":0})
    );
    assert_eq!(imported["imports"].as_array().unwrap().len(), 1);
    assert_eq!(imported["calls"], json!([]));
    for field in [
        "source_authority",
        "execution",
        "publication_authority",
        "candidate_retained",
        "graph_retained",
    ] {
        assert_eq!(called[field], false);
        assert_eq!(imported[field], false);
    }
    assert_eq!(
        called["project_association"],
        "candidate_provider_source_projection_only"
    );
    assert_eq!(called["nonclaims"].as_array().unwrap().len(), 8);
    assert!(called["nonclaims"]
        .as_array()
        .unwrap()
        .contains(&json!("no_ambient_consumer_discovery_or_completeness")));
    assert!(called["nonclaims"]
        .as_array()
        .unwrap()
        .contains(&json!("not_api_abi_or_behavioral_compatibility")));
    assert!(called["nonclaims"].as_array().unwrap().contains(&json!(
        "calls_are_static_authenticated_source_sites_not_runtime_execution"
    )));
    assert!(
        serde_json::to_string(&called).unwrap().len()
            <= MAX_PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_BYTES
    );
    assert_eq!(base.to_json(), base_json);
    assert_eq!(candidate.to_json(), candidate_json);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn stale_foreign_and_tampered_candidate_package_inputs_fail_closed() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let left = changed(&base, 42);
    let right = changed(&base, 43);
    let base_corpus = Corpus::candidate_era(&fixture.0, &base);
    let left_corpus = Corpus::candidate_era(&fixture.0, &left);
    let right_corpus = Corpus::candidate_era(&fixture.0, &right);

    failed(
        left.package_consumer_replay(right.candidate_digest(), &left_corpus.replay(TARGET)),
        "SPX-G224",
    );
    failed(
        left.package_consumer_replay("not-a-digest", &left_corpus.replay(TARGET)),
        "SPX-G222",
    );
    failed(
        left.package_consumer_replay(left.candidate_digest(), &base_corpus.replay(TARGET)),
        "SPX-G336",
    );
    failed(
        left.package_consumer_replay(left.candidate_digest(), &right_corpus.replay(TARGET)),
        "SPX-G336",
    );

    let mut foreign = left_corpus.replay(TARGET);
    let coordinate = Coordinate {
        package: "other".into(),
        version: "1.0.0".into(),
    };
    foreign.provider = &coordinate;
    failed(
        left.package_consumer_replay(left.candidate_digest(), &foreign),
        "SPX-G336",
    );
    let mut wrong_path = left_corpus.replay(TARGET);
    wrong_path.provider_source_path = "src/app.spx";
    failed(
        left.package_consumer_replay(left.candidate_digest(), &wrong_path),
        "SPX-G336",
    );

    let mut tampered_sources = left_corpus.sources.clone();
    tampered_sources[0].source = tampered_sources[0]
        .source
        .replace("answer()+1", "answer()+2");
    let mut tampered = left_corpus.replay(TARGET);
    tampered.sources = &tampered_sources;
    failed(
        left.package_consumer_replay(left.candidate_digest(), &tampered),
        "SPX-PS507",
    );
    let tampered_capsule = left_corpus.capsule.replacen("sha256:", "sha256:0", 1);
    let mut tampered = left_corpus.replay(TARGET);
    tampered.capsule = &tampered_capsule;
    assert!(left
        .package_consumer_replay(left.candidate_digest(), &tampered)
        .is_err());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn sibling_replay_is_deterministic_history_isolated_and_retains_nothing() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let left = changed(&base, 42);
    let right = changed(&base, 43);
    let left_json = left.to_json().to_owned();
    let right_json = right.to_json().to_owned();
    let left_corpus = Corpus::candidate_era(&fixture.0, &left);
    let right_corpus = Corpus::candidate_era(&fixture.0, &right);
    let left_first = left
        .package_consumer_replay(left.candidate_digest(), &left_corpus.replay(TARGET))
        .unwrap();
    let left_again = left
        .package_consumer_replay(left.candidate_digest(), &left_corpus.replay(TARGET))
        .unwrap();
    let right_report = right
        .package_consumer_replay(right.candidate_digest(), &right_corpus.replay(TARGET))
        .unwrap();
    assert_eq!(left_first, left_again);
    assert_ne!(left_first, right_report);
    let left_value: Value = serde_json::from_str(&left_first).unwrap();
    let right_value: Value = serde_json::from_str(&right_report).unwrap();
    assert_ne!(
        left_value["candidate_revision"],
        right_value["candidate_revision"]
    );
    assert_ne!(
        left_value["package_graph_revision"],
        right_value["package_graph_revision"]
    );
    assert_ne!(
        left_value["provider_source"]["candidate_source_revision"],
        right_value["provider_source"]["candidate_source_revision"]
    );
    assert_eq!(left.to_json(), left_json);
    assert_eq!(right.to_json(), right_json);
    assert_eq!(fixture.bytes(), disk);
}
