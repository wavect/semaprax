//! Integrated managed-generation scenario.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::project::{
    apply_candidate_publication, prepare_candidate_publication, with_authenticated_project,
    CandidateTestPolicy, ProjectCandidate, ProjectSemanticImage, SemanticChange,
};
use semaprax::{semantic_workspace, workspace_graph};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf, String);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-graph-workflow-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let root = root.canonicalize().unwrap();
        let paths = root.join("paths.json");
        std::fs::write(
            &paths,
            concat!(
                "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[",
                "{\"path\":\"src/app.spx\"},{\"path\":\"src/core.spx\"},",
                "{\"path\":\"src/tests.spx\"}]}\n"
            ),
        )
        .unwrap();
        let workspace_revision = semantic_workspace::initialize(&root, &paths).unwrap();
        Self(root, workspace_revision)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn managed_source(root: &Path, workspace_revision: &str, path: &str) -> String {
    let revision = workspace_revision.strip_prefix("sha256:").unwrap();
    std::fs::read_to_string(
        root.join(".semaprax-workspace")
            .join("generations")
            .join(revision)
            .join("files")
            .join(path),
    )
    .unwrap()
}

#[test]
fn signature_evolution_merge_reports_tests_and_separate_managed_publication() {
    let fixture = Fixture::new();
    let manifest = fixture.0.join("semaprax.toml");
    let revision =
        with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision())).unwrap();
    let image =
        ProjectSemanticImage::derive(Arc::clone(&revision), revision.project_revision()).unwrap();
    let symbol = image
        .symbol(image.image_digest(), "calculator.add")
        .unwrap();
    assert!(symbol.contains("calculator.add"));
    let root = ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let signature = SemanticChange::new(
        revision.project_revision(),
        &json!({
            "kind":"change_function_signature", "target":"calculator.add",
            "append_parameters":[{"name":"unused", "type":"i64",
                "argument":{"kind":"i64", "value":0}}]
        }),
    )
    .unwrap();
    let left = root.apply(root.candidate_digest(), &signature).unwrap();
    let rename = SemanticChange::new(
        revision.project_revision(),
        &json!({
            "kind":"rename_declaration", "target":"calculator.multiply", "name":"times"
        }),
    )
    .unwrap();
    let right = root.apply(root.candidate_digest(), &rename).unwrap();
    let merged = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap()
        .into_candidate();
    // Every admitted candidate has already passed canonical source rebuilding,
    // identity/manifest invariants, contract/ownership checking and target projection.
    let report: Value = serde_json::from_str(merged.to_json()).unwrap();
    assert!(report["source_changes"].as_array().unwrap().len() >= 2);
    assert!(report["core_targets"].get("candidate").is_some());
    assert!(report["impact"].is_object() || report["impact"].is_array());
    let delta = merged
        .semantic_delta(merged.candidate_digest(), "calculator.add")
        .unwrap();
    merged
        .verify_semantic_delta(
            merged.candidate_digest(),
            "calculator.add",
            delta.as_bytes(),
        )
        .unwrap();
    let plan: Value =
        serde_json::from_str(&merged.test_plan(merged.candidate_digest()).unwrap()).unwrap();
    assert_eq!(plan["schema"], "semaprax.project-candidate-test-plan.v1");
    let policy = CandidateTestPolicy::new(100_000, 65_536, 262_144).unwrap();
    assert!(merged
        .execute_tests(merged.candidate_digest(), &policy)
        .unwrap()
        .passed());
    // A competing signature is a conflict even if both candidates were admitted.
    let competing_change = SemanticChange::new(
        revision.project_revision(),
        &json!({
            "kind":"change_function_signature", "target":"calculator.add",
            "append_parameters":[{"name":"other", "type":"i64",
                "argument":{"kind":"i64", "value":1}}]
        }),
    )
    .unwrap();
    let competing = root
        .apply(root.candidate_digest(), &competing_change)
        .unwrap();
    assert!(left
        .merge(
            left.candidate_digest(),
            &competing,
            competing.candidate_digest()
        )
        .is_err());
    let base = fixture.1.clone();
    let proof = prepare_candidate_publication(
        &merged,
        merged.candidate_digest(),
        &fixture.0,
        &manifest,
        &base,
    )
    .unwrap();
    assert_eq!(
        workspace_graph::snapshot(&fixture.0, "calculator.app")
            .unwrap()
            .workspace_revision(),
        base
    );
    let receipt = apply_candidate_publication(
        &merged,
        merged.candidate_digest(),
        &fixture.0,
        &manifest,
        &base,
        proof.to_json().as_bytes(),
    )
    .unwrap();
    let receipt: Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(receipt["result"], "managed_generation_published");
    assert_eq!(receipt["git_commit"], "not_performed");
    let active = workspace_graph::snapshot(&fixture.0, "calculator.app").unwrap();
    assert_eq!(
        active.workspace_revision(),
        proof.candidate_workspace_revision()
    );
    for actual in active.modules() {
        let expected = merged
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == actual.path())
            .unwrap();
        assert_eq!(actual.source_graph_schema(), expected.source_graph_schema());
        assert_eq!(actual.source_revision(), expected.source_revision());
        assert_eq!(actual.source_digest(), expected.source_digest());
    }
    for expected in merged.revision().sources() {
        assert_eq!(
            managed_source(&fixture.0, active.workspace_revision(), expected.path()),
            expected.source()
        );
    }
    for source in revision.sources() {
        assert_eq!(
            std::fs::read_to_string(fixture.0.join(source.path())).unwrap(),
            source.source()
        );
    }
    let errors = apply_candidate_publication(
        &merged,
        merged.candidate_digest(),
        &fixture.0,
        &manifest,
        &base,
        proof.to_json().as_bytes(),
    )
    .unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-G247"));
    assert_eq!(
        workspace_graph::snapshot(&fixture.0, "calculator.app")
            .unwrap()
            .workspace_revision(),
        proof.candidate_workspace_revision()
    );
}
