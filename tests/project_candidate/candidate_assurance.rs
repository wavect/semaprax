//! #129 (SPX-AI-030): candidate-bound assurance facts joined from existing
//! `semaprax.assurance-manifest.v1` evidence, and an independent acceptance
//! boundary a candidate cannot grant to itself.
//! See `docs/PROJECT-CANDIDATE-ASSURANCE-ACCEPTANCE-V1.md`.
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::assurance_manifest::{self, AssuranceClass, AssuranceManifestOptions};
use semaprax::project::{
    with_authenticated_project, CandidateAssuranceInput, ProjectCandidate, ProjectRevision,
    SemanticChange,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

// `assurance_manifest::generate` parses and verifies exactly the file it is
// pointed at, standalone (no project-linked imports, and requiring its own
// `main`), matching `capability_manifest`/`region_report`. Within a project,
// only the entry module and a listed `tests` module may declare `main`
// (any other source is a "provider module" and `SPX-G172` rejects a `main`
// on it), so the one candidate source this suite binds assurance envelopes
// to is always the entry module itself, carrying both the contracted
// `divide` function and its own `main`.
const APP_WITH_CONTRACTS: &str = "module assurance.app;\n\
@id(\"assurance.divide\") fn divide(left: i64, right: i64) -> i64\n\
    requires right != 0\n\
{\n    left / right\n}\n\
@id(\"assurance.main\") fn main() -> i64 { divide(4, 2) }\n";

impl Fixture {
    fn new(app: &str, tests_body: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-assurance-harness-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "candidate-assurance-harness"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "assurance.app"
sources = ["src/app.spx", "src/tests.spx"]
web_exports = []
tests = ["assurance.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            ("src/app.spx", app.to_owned()),
            (
                "src/tests.spx",
                format!("module assurance.tests;\n{tests_body}\n"),
            ),
        ] {
            let parsed = semaprax::parse(&text, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root)
    }

    fn app_path(&self) -> PathBuf {
        self.0.join("src/app.spx")
    }

    fn tests_path(&self) -> PathBuf {
        self.0.join("src/tests.spx")
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
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

fn manifest_for(path: &std::path::Path) -> String {
    assurance_manifest::generate(path, &AssuranceManifestOptions::default()).unwrap()
}

const PLAIN_TESTS_BODY: &str = "@id(\"assurance.check\") fn main()->i64 {0}";

/// Add a sibling function to `src/app.spx` that calls the existing
/// `assurance.divide`, changing that file's canonical text (and therefore
/// its `graph::revision`) without touching `divide`'s own obligations.
fn add_sibling_wrapper(candidate: &ProjectCandidate) -> ProjectCandidate {
    let intent = json!({"kind":"add_declaration","target":"assurance.divide","declaration":{
        "id":"assurance.divide-wrapper","name":"divide_wrapper",
        "parameters":[{"name":"left","type":"i64","mode":"value"},{"name":"right","type":"i64","mode":"value"}],
        "return_type":"i64","effects":[],"requires":[],"ensures":[],
        "body":{"kind":"call","target":"assurance.divide","arguments":[
            {"kind":"place","name":"left"},{"kind":"place","name":"right"}
        ]}
    }});
    let change = SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap();
    candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap()
}

#[test]
fn joins_a_genuine_envelope_bound_to_the_exact_candidate() {
    let fixture = Fixture::new(APP_WITH_CONTRACTS, PLAIN_TESTS_BODY);
    let revision = fixture.revision();
    let candidate = open(&revision);
    let envelope = manifest_for(&fixture.app_path());
    let inputs = [CandidateAssuranceInput {
        path: "src/app.spx",
        envelope: &envelope,
    }];

    let summary = candidate
        .candidate_assurance_summary(candidate.candidate_digest(), &inputs)
        .unwrap();
    let value: Value = serde_json::from_str(&summary).unwrap();
    assert_eq!(
        value["schema"],
        json!("semaprax.project-candidate-assurance-summary.v1")
    );
    assert_eq!(
        value["candidate_revision"],
        json!(candidate.candidate_digest())
    );
    // One precondition (`requires right != 0`) plus two ownership_parameter
    // obligations (`left`, `right`); `main` has no parameters or contracts.
    assert_eq!(value["obligations_total"], json!(3));
    assert_eq!(value["publication_authority"], json!(false));
    assert_eq!(value["acceptance_authority"], json!(false));
    let not_observed: Vec<&str> = value["sources_not_observed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(not_observed, vec!["src/tests.spx"]);
    assert_eq!(value["kinds_not_yet_derived"].as_array().unwrap().len(), 6);
}

#[test]
fn a_stale_envelope_from_before_a_real_candidate_edit_is_rejected() {
    let fixture = Fixture::new(APP_WITH_CONTRACTS, PLAIN_TESTS_BODY);
    let revision = fixture.revision();
    let base_candidate = open(&revision);
    // Generated against the on-disk bytes, before the candidate below ever
    // mutates anything: a completely genuine envelope, just stale by the
    // time it is presented.
    let stale_envelope = manifest_for(&fixture.app_path());

    let mutated_candidate = add_sibling_wrapper(&base_candidate);
    let inputs = [CandidateAssuranceInput {
        path: "src/app.spx",
        envelope: &stale_envelope,
    }];
    let error = mutated_candidate
        .candidate_assurance_summary(mutated_candidate.candidate_digest(), &inputs)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-G932");

    // A freshly regenerated envelope over the mutated candidate's own text
    // (materialized here since `generate` reads real files) binds cleanly
    // and now reports the wrapper's own ownership_parameter obligations too.
    let mutated_app_text = mutated_candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/app.spx")
        .unwrap()
        .source()
        .to_owned();
    std::fs::write(fixture.app_path(), &mutated_app_text).unwrap();
    let fresh_envelope = manifest_for(&fixture.app_path());
    let fresh_inputs = [CandidateAssuranceInput {
        path: "src/app.spx",
        envelope: &fresh_envelope,
    }];
    let summary = mutated_candidate
        .candidate_assurance_summary(mutated_candidate.candidate_digest(), &fresh_inputs)
        .unwrap();
    let value: Value = serde_json::from_str(&summary).unwrap();
    // 3 for `divide` (1 precondition + 2 ownership_parameter) plus 2
    // ownership_parameter obligations for the new wrapper.
    assert_eq!(value["obligations_total"], json!(5));
}

#[test]
fn an_envelope_for_a_path_outside_the_candidate_is_rejected() {
    let fixture = Fixture::new(APP_WITH_CONTRACTS, PLAIN_TESTS_BODY);
    let revision = fixture.revision();
    let candidate = open(&revision);
    let envelope = manifest_for(&fixture.app_path());
    let inputs = [CandidateAssuranceInput {
        path: "src/nonexistent.spx",
        envelope: &envelope,
    }];
    let error = candidate
        .candidate_assurance_summary(candidate.candidate_digest(), &inputs)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-G930");
}

#[test]
fn a_candidate_cannot_grant_its_own_acceptance() {
    let fixture = Fixture::new(APP_WITH_CONTRACTS, PLAIN_TESTS_BODY);
    let revision = fixture.revision();
    let candidate = open(&revision);
    let app_envelope = manifest_for(&fixture.app_path());
    let tests_envelope = manifest_for(&fixture.tests_path());
    let inputs = [
        CandidateAssuranceInput {
            path: "src/app.spx",
            envelope: &app_envelope,
        },
        CandidateAssuranceInput {
            path: "src/tests.spx",
            envelope: &tests_envelope,
        },
    ];

    // The candidate's own author supplying itself as reviewer is refused
    // outright, before the summary's content is even evaluated.
    let error = candidate
        .grant_candidate_acceptance(
            candidate.candidate_digest(),
            &inputs,
            AssuranceClass::Open,
            "agent-under-review",
            "agent-under-review",
        )
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-G933");

    // The exact same inputs, with a genuinely distinct reviewer, can be
    // granted: the boundary refuses self-acceptance specifically, not
    // acceptance in general.
    let granted = candidate
        .grant_candidate_acceptance(
            candidate.candidate_digest(),
            &inputs,
            AssuranceClass::Open,
            "agent-under-review",
            "independent-reviewer",
        )
        .unwrap();
    let granted: Value = serde_json::from_str(&granted).unwrap();
    assert_eq!(granted["granted"], json!(true));
    assert_eq!(granted["acceptance_authority"], json!(false));
}

#[test]
fn editing_the_visible_test_file_does_not_move_the_production_obligations() {
    let honest = Fixture::new(
        APP_WITH_CONTRACTS,
        "@id(\"assurance.check\") fn main()->i64 { 1 }",
    );
    let lying = Fixture::new(
        APP_WITH_CONTRACTS,
        "@id(\"assurance.check\") fn main()->i64 { 0 }",
    );

    let honest_candidate = open(&honest.revision());
    let lying_candidate = open(&lying.revision());
    let honest_envelope = manifest_for(&honest.app_path());
    let lying_envelope = manifest_for(&lying.app_path());
    // src/app.spx bytes are identical in both fixtures, so their assurance
    // obligations are identical too (the envelopes' `source.path` differ
    // only because each fixture uses its own temporary directory).
    let obligations_of = |envelope: &str| -> Value {
        serde_json::from_str::<Value>(envelope).unwrap()["payload"]["obligations"].clone()
    };
    assert_eq!(
        obligations_of(&honest_envelope),
        obligations_of(&lying_envelope)
    );

    let honest_inputs = [CandidateAssuranceInput {
        path: "src/app.spx",
        envelope: &honest_envelope,
    }];
    let lying_inputs = [CandidateAssuranceInput {
        path: "src/app.spx",
        envelope: &lying_envelope,
    }];
    let honest_summary: Value = serde_json::from_str(
        &honest_candidate
            .candidate_assurance_summary(honest_candidate.candidate_digest(), &honest_inputs)
            .unwrap(),
    )
    .unwrap();
    let lying_summary: Value = serde_json::from_str(
        &lying_candidate
            .candidate_assurance_summary(lying_candidate.candidate_digest(), &lying_inputs)
            .unwrap(),
    )
    .unwrap();

    assert_ne!(
        honest_summary["candidate_revision"],
        lying_summary["candidate_revision"]
    );
    assert_eq!(honest_summary["obligations"], lying_summary["obligations"]);
    assert_eq!(honest_summary["by_class"], lying_summary["by_class"]);

    // Neither candidate's acceptance for the bound `src/app.spx` obligation
    // set depends on what the (unobserved) test file claims either: both
    // withhold acceptance identically because `src/tests.spx` was never
    // supplied as an input here.
    let honest_grant: Value = serde_json::from_str(
        &honest_candidate
            .grant_candidate_acceptance(
                honest_candidate.candidate_digest(),
                &honest_inputs,
                AssuranceClass::Open,
                "proposer",
                "reviewer",
            )
            .unwrap(),
    )
    .unwrap();
    let lying_grant: Value = serde_json::from_str(
        &lying_candidate
            .grant_candidate_acceptance(
                lying_candidate.candidate_digest(),
                &lying_inputs,
                AssuranceClass::Open,
                "proposer",
                "reviewer",
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(honest_grant["granted"], lying_grant["granted"]);
    assert_eq!(
        honest_grant["unmet_obligations"],
        lying_grant["unmet_obligations"]
    );
}
