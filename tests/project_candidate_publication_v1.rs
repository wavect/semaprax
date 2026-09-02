//! Managed publication bridge regressions.
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use fs2::FileExt;
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    apply_candidate_publication, prepare_candidate_publication, with_authenticated_project,
    ProjectCandidate, ProjectCandidatePublication, SemanticChange,
};
use semaprax::{semantic_workspace, workspace_graph};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-publication-{}-{}",
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
        let paths = root.join("paths.json");
        std::fs::write(&paths,"{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"src/app.spx\"},{\"path\":\"src/core.spx\"},{\"path\":\"src/tests.spx\"}]}\n").unwrap();
        let root = root.canonicalize().unwrap();
        semantic_workspace::initialize(&root, &paths).unwrap();
        Self(root)
    }
    fn root_candidate(&self) -> ProjectCandidate {
        let revision =
            with_authenticated_project(&self.manifest(), |snapshot| Ok(snapshot.retain_revision()))
                .unwrap();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
    fn candidate(&self) -> ProjectCandidate {
        let root = self.root_candidate();
        let change=SemanticChange::new(root.revision().project_revision(),&json!({"kind":"change_function_signature","target":"calculator.add","append_parameters":[{"name":"unused","type":"i64","argument":{"kind":"i64","value":0}}]})).unwrap();
        root.apply(root.candidate_digest(), &change).unwrap()
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn workspace_revision(&self) -> String {
        workspace_graph::snapshot(&self.0, "calculator.app")
            .unwrap()
            .workspace_revision()
            .to_owned()
    }
    fn prepare(&self, candidate: &ProjectCandidate) -> ProjectCandidatePublication {
        prepare_candidate_publication(
            candidate,
            candidate.candidate_digest(),
            &self.0,
            &self.manifest(),
            &self.workspace_revision(),
        )
        .unwrap()
    }
    fn lock(&self) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.0.join(".semaprax-workspace/LOCK"))
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), Vec::new());
                visit(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}
fn diagnostics<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|error| error.code == code), "{errors:?}"),
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
fn prepare_is_read_only_and_apply_changes_only_the_managed_active_generation() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let before = inventory(&fixture.0);
    let base = fixture.workspace_revision();
    let proof = fixture.prepare(&candidate);
    assert_eq!(inventory(&fixture.0), before);
    let proposal: Value = serde_json::from_str(proof.proposal()).unwrap();
    assert!(proposal["changes"].as_array().unwrap().len() >= 2);
    assert_eq!(proposal["base_workspace_revision"], base);
    let repeated = fixture.prepare(&candidate);
    assert_eq!(proof.to_json(), repeated.to_json());
    let receipt = apply_candidate_publication(
        &candidate,
        candidate.candidate_digest(),
        &fixture.0,
        &fixture.manifest(),
        &base,
        proof.to_json().as_bytes(),
    )
    .unwrap();
    let receipt: Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(receipt["result"], "managed_generation_published");
    assert_eq!(receipt["git_commit"], "not_performed");
    let managed = workspace_graph::snapshot(&fixture.0, "calculator.app").unwrap();
    assert_eq!(
        managed.workspace_revision(),
        proof.candidate_workspace_revision()
    );
    assert_ne!(managed.workspace_revision(), base);
    for module in managed.modules() {
        let expected = candidate
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == module.path())
            .unwrap();
        assert_eq!(module.source_graph_schema(), expected.source_graph_schema());
        assert_eq!(module.source_revision(), expected.source_revision());
        assert_eq!(module.source_digest(), expected.source_digest());
    }
    for expected in candidate.revision().sources() {
        assert_eq!(
            managed_source(&fixture.0, managed.workspace_revision(), expected.path()),
            expected.source()
        );
        assert_eq!(
            std::fs::read(fixture.0.join(expected.path())).unwrap(),
            before[Path::new(expected.path())]
        );
    }
    assert_eq!(
        std::fs::read(fixture.manifest()).unwrap(),
        before[Path::new("semaprax.toml")]
    );
    let after = inventory(&fixture.0);
    diagnostics(
        apply_candidate_publication(
            &candidate,
            candidate.candidate_digest(),
            &fixture.0,
            &fixture.manifest(),
            &base,
            proof.to_json().as_bytes(),
        ),
        "SPX-G247",
    );
    assert_eq!(inventory(&fixture.0), after);
}

#[test]
fn proof_tamper_approval_and_host_substitution_reject_before_any_generation_write() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let proof = fixture.prepare(&candidate);
    let before = inventory(&fixture.0);
    let base = fixture.workspace_revision();
    let mut bad = proof.to_json().as_bytes().to_vec();
    bad.push(b' ');
    diagnostics(
        apply_candidate_publication(
            &candidate,
            candidate.candidate_digest(),
            &fixture.0,
            &fixture.manifest(),
            &base,
            &bad,
        ),
        "SPX-G247",
    );
    diagnostics(
        apply_candidate_publication(
            &candidate,
            &format!("sha256:{}", "0".repeat(64)),
            &fixture.0,
            &fixture.manifest(),
            &base,
            proof.to_json().as_bytes(),
        ),
        "SPX-G247",
    );
    let other = Fixture::new();
    let other_before = inventory(&other.0);
    diagnostics(
        apply_candidate_publication(
            &candidate,
            candidate.candidate_digest(),
            &other.0,
            &other.manifest(),
            &base,
            proof.to_json().as_bytes(),
        ),
        "SPX-G247",
    );
    assert_eq!(inventory(&other.0), other_before);
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn existing_exclusive_lock_is_required_before_replay_or_candidate_approval_checks() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let proof = fixture.prepare(&candidate);
    let base = fixture.workspace_revision();
    let before = inventory(&fixture.0);
    let lock = fixture.lock();
    lock.try_lock_exclusive().unwrap();
    let wrong = format!("sha256:{}", "0".repeat(64));
    diagnostics(
        prepare_candidate_publication(&candidate, &wrong, &fixture.0, &fixture.manifest(), &base),
        "SPX-I210",
    );
    diagnostics(
        apply_candidate_publication(
            &candidate,
            &wrong,
            &fixture.0,
            &fixture.manifest(),
            &base,
            proof.to_json().as_bytes(),
        ),
        "SPX-I210",
    );
    FileExt::unlock(&lock).unwrap();
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn raw_source_drift_and_single_changed_file_never_pad_or_publish() {
    let fixture = Fixture::new();
    let root = fixture.root_candidate();
    let before = inventory(&fixture.0);
    let renamed = root
        .apply(
            root.candidate_digest(),
            &SemanticChange::new(
                root.revision().project_revision(),
                &json!({"kind":"rename_declaration","target":"calculator.add","name":"sum"}),
            )
            .unwrap(),
        )
        .unwrap();
    diagnostics(
        prepare_candidate_publication(
            &renamed,
            renamed.candidate_digest(),
            &fixture.0,
            &fixture.manifest(),
            &fixture.workspace_revision(),
        ),
        "SPX-G245",
    );
    assert_eq!(inventory(&fixture.0), before);
    let candidate = fixture.candidate();
    let proof = fixture.prepare(&candidate);
    let base = fixture.workspace_revision();
    let path = fixture.0.join("src/core.spx");
    let original = std::fs::read_to_string(&path).unwrap();
    std::fs::write(path, original.replace("left + right", "left - right")).unwrap();
    let changed = inventory(&fixture.0);
    assert!(apply_candidate_publication(
        &candidate,
        candidate.candidate_digest(),
        &fixture.0,
        &fixture.manifest(),
        &base,
        proof.to_json().as_bytes()
    )
    .is_err());
    assert_eq!(inventory(&fixture.0), changed);
    assert_eq!(fixture.workspace_revision(), base);
}
