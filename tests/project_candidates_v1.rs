//! Candidate integration evidence authored without executing local gates.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, SemanticChange,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-candidates-v1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
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

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if entry.file_type().unwrap().is_dir() {
                entries.insert(relative, Vec::new());
                visit(root, &path, entries);
            } else {
                entries.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

fn append(revision: &ProjectRevision) -> SemanticChange {
    SemanticChange::new(
        revision.project_revision(),
        &json!({
            "kind":"change_function_signature", "target":"calculator.add",
            "append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]
        }),
    )
    .unwrap()
}

#[test]
fn change_catalog_is_revision_bound_and_omits_unsupported_targets() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let candidate =
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let report: Value =
        serde_json::from_str(&candidate.change_catalog("calculator.add").unwrap()).unwrap();
    assert_eq!(report["candidate_digest"], candidate.candidate_digest());
    assert_eq!(report["requires_full_candidate_validation"], true);
    assert_eq!(report["admission"], "constructor_discovery_only");
    assert_eq!(report["operations"].as_array().unwrap().len(), 3);
    assert_eq!(
        report["operations"][1]["exactly_one_form"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let unknown: Value =
        serde_json::from_str(&candidate.change_catalog("unknown.id").unwrap()).unwrap();
    assert!(unknown["operations"].as_array().unwrap().is_empty());
    assert_code(candidate.change_catalog(""), "SPX-G222");
    assert_code(candidate.change_catalog(&"x".repeat(4097)), "SPX-G223");
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|error| error.code == code), "{errors:?}"),
    }
}

fn source<'a>(revision: &'a ProjectRevision, path: &str) -> &'a str {
    revision
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .unwrap()
        .source()
}

#[test]
fn append_signature_rebuilds_all_declared_callers_without_changing_live_sources() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let candidate =
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let original = candidate.to_json().to_owned();
    let change = append(&revision);
    assert_eq!(
        SemanticChange::from_json(change.to_json().as_bytes())
            .unwrap()
            .to_json(),
        change.to_json()
    );
    let amended = candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap();
    assert_ne!(
        amended.revision().project_revision(),
        revision.project_revision()
    );
    assert_eq!(candidate.to_json(), original);
    assert_eq!(
        amended.base_revision().project_revision(),
        revision.project_revision()
    );
    assert!(source(amended.revision(), "src/core.spx")
        .contains("fn add(left: i64, right: i64, offset: i64)"));
    assert!(source(amended.revision(), "src/tests.spx").contains("add(19, 23, 0)"));
    assert!(source(amended.revision(), "src/app.spx")
        .contains("add(multiply(6, 7), subtract(divide(4, 2), 2), 0)"));
    assert_eq!(
        amended.revision().manifest().to_canonical_toml(),
        revision.manifest().to_canonical_toml()
    );
    let evidence: Value = serde_json::from_str(amended.to_json()).unwrap();
    assert_eq!(evidence["schema"], "semaprax.project-candidate.v1");
    assert_eq!(evidence["validation"]["tests"], "not_run");
    assert_eq!(evidence["operations"][0]["migrated_calls"], 2);
    assert_eq!(evidence["source_changes"].as_array().unwrap().len(), 3);
    assert!(evidence["core_targets"]["candidate"]
        .as_array()
        .unwrap()
        .iter()
        .all(|target| target["admitted"] == true));
    let replayed = ProjectCandidate::replay(
        Arc::clone(&revision),
        revision.project_revision(),
        &[change],
        amended.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.candidate_digest(), amended.candidate_digest());
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn branches_are_immutable_and_sequential_changes_bind_the_current_revision() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let root = ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let left = root
        .apply(root.candidate_digest(), &append(&revision))
        .unwrap();
    let rename = SemanticChange::new(
        revision.project_revision(),
        &json!({
            "kind":"rename_declaration","target":"calculator.add","name":"sum"
        }),
    )
    .unwrap();
    let right = root.apply(root.candidate_digest(), &rename).unwrap();
    assert_ne!(left.candidate_digest(), right.candidate_digest());
    assert_eq!(
        root.revision().project_revision(),
        revision.project_revision()
    );
    assert!(source(right.revision(), "src/core.spx").contains("fn sum(left: i64, right: i64)"));
    assert_eq!(
        source(right.revision(), "src/app.spx"),
        source(&revision, "src/app.spx")
    );
    let comparison: Value = serde_json::from_str(&left.compare(&right).unwrap()).unwrap();
    assert_eq!(comparison["overlapping_targets"], json!(["calculator.add"]));
    assert_eq!(comparison["commit_authority"], false);
    assert_code(left.apply(left.candidate_digest(), &rename), "SPX-G224");
    let change = SemanticChange::new(left.revision().project_revision(), &json!({
        "kind":"replace_function_body","target":"calculator.add",
        "body":{"kind":"binary","op":"+",
            "left":{"kind":"binary","op":"+","left":{"kind":"place","name":"left"},"right":{"kind":"place","name":"right"}},
            "right":{"kind":"place","name":"offset"}}
    })).unwrap();
    let combined = left.apply(left.candidate_digest(), &change).unwrap();
    assert!(source(combined.revision(), "src/core.spx").contains("left + right + offset"));
    assert!(!source(left.revision(), "src/core.spx").contains("left + right + offset"));
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn typed_invalid_body_and_stale_selection_leave_candidate_and_disk_unchanged() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let root = ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let original = root.to_json().to_owned();
    let invalid = SemanticChange::new(revision.project_revision(), &json!({
        "kind":"replace_function_body","target":"calculator.add","body":{"kind":"bool","value":true}
    })).unwrap();
    // This expression passes the constructor grammar; the real Project type
    // verifier must reject bool as the body of the existing i64 function.
    assert!(root.apply(root.candidate_digest(), &invalid).is_err());
    let stale = format!("sha256:{}", "0".repeat(64));
    assert_code(
        ProjectCandidate::open(Arc::clone(&revision), &stale),
        "SPX-G224",
    );
    assert_code(root.apply(&stale, &append(&revision)), "SPX-G224");
    assert_eq!(root.to_json(), original);
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn replay_rejects_canonical_tampered_diff_and_noncanonical_or_weakened_change() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let root = ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let change = append(&revision);
    let candidate = root.apply(root.candidate_digest(), &change).unwrap();
    let mut changed: Value = serde_json::from_str(candidate.to_json()).unwrap();
    changed["source_changes"][0]["source_diff"] = json!("forged diff");
    changed["source_changes"][0]["source_diff_digest"] =
        json!(format!("sha256:{}", "0".repeat(64)));
    changed.sort_all_objects();
    let tampered = format!("{}\n", serde_json::to_string(&changed).unwrap());
    assert_code(
        ProjectCandidate::replay(
            Arc::clone(&revision),
            revision.project_revision(),
            &[change.clone()],
            tampered.as_bytes(),
        ),
        "SPX-G224",
    );
    assert_code(
        ProjectCandidate::replay(
            Arc::clone(&revision),
            revision.project_revision(),
            &[change.clone()],
            candidate.to_json().trim_end_matches('\n').as_bytes(),
        ),
        "SPX-G224",
    );
    assert_code(
        SemanticChange::from_json(change.to_json().trim_end_matches('\n').as_bytes()),
        "SPX-G222",
    );
    let mut weak: Value = serde_json::from_str(change.to_json()).unwrap();
    weak["requirements"] = json!([]);
    weak.sort_all_objects();
    assert_code(
        SemanticChange::from_json(
            format!("{}\n", serde_json::to_string(&weak).unwrap()).as_bytes(),
        ),
        "SPX-G222",
    );
    assert_eq!(inventory(&fixture.0), before);
}
