//! Source-backed archive evidence authored; deliberately unrun locally.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateArchive, SemanticChange,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-archive-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(file), root.join(file)).unwrap();
        }
        Self(root)
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn rename(candidate: &ProjectCandidate, target: &str, name: &str) -> ProjectCandidate {
    let change = SemanticChange::new(
        candidate.revision().project_revision(),
        &json!({"kind":"rename_declaration","target":target,"name":name}),
    )
    .unwrap();
    candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap()
}
fn archive(candidate: &ProjectCandidate) -> ProjectCandidateArchive {
    ProjectCandidateArchive::prepare(candidate, candidate.candidate_digest()).unwrap()
}
fn restore(archive: &ProjectCandidateArchive) -> ProjectCandidate {
    ProjectCandidateArchive::restore(
        archive.to_json().as_bytes(),
        archive.archive_digest(),
        archive.candidate_digest(),
    )
    .unwrap()
}
fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    format!("{value}\n")
}
fn remint(mut value: Value) -> (String, String) {
    value.as_object_mut().unwrap().remove("archive_digest");
    let bytes = canonical(value.clone());
    let mut hash = Sha256::new();
    hash.update(b"semaprax.project-candidate-archive.payload.v1\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes.as_bytes());
    let digest = format!("sha256:{:x}", hash.finalize());
    value["archive_digest"] = json!(digest);
    (canonical(value), digest)
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == expected),
            "{errors:?}"
        ),
    }
}

#[test]
fn deleted_original_sources_restore_exact_candidate_without_recreating_files() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let one = rename(&root, "calculator.add", "sum");
    let candidate = rename(&one, "calculator.add", "total");
    let archive = archive(&candidate);
    std::fs::remove_dir_all(&fixture.0).unwrap();
    let restored = restore(&archive);
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(
        restored.recovery_capsule().unwrap(),
        candidate.recovery_capsule().unwrap()
    );
    assert_eq!(
        ProjectCandidateArchive::prepare(&restored, restored.candidate_digest())
            .unwrap()
            .to_json(),
        archive.to_json()
    );
    assert!(
        !fixture.0.exists(),
        "archive restore grants no filesystem publication"
    );
    assert_eq!(
        rename(&restored, "calculator.add", "addition").to_json(),
        rename(&candidate, "calculator.add", "addition").to_json()
    );
    let value: Value = serde_json::from_str(archive.to_json()).unwrap();
    assert_eq!(value["source_authority"], false);
    assert_eq!(value["approval_authority"], false);
    assert_eq!(value["trusted_hir"], false);
    assert!(value.get("root").is_none());
    assert!(value.get("approval").is_none());
    assert_eq!(value["sources"].as_array().unwrap().len(), 3);
}

#[test]
fn rebase_archives_its_actual_new_base_and_remains_independent_of_raw_edits() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let left = rename(&root, "calculator.add", "sum");
    let right = rename(&root, "calculator.multiply", "product");
    let rebased = left
        .rebase(
            left.candidate_digest(),
            Arc::clone(right.revision()),
            right.revision().project_revision(),
        )
        .unwrap()
        .into_candidate();
    let archive = archive(&rebased);
    assert_eq!(archive.base_revision(), right.revision().project_revision());
    std::fs::write(fixture.0.join("src/core.spx"), "invalid manual edit\n").unwrap();
    assert_eq!(restore(&archive).to_json(), rebased.to_json());
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("src/core.spx")).unwrap(),
        "invalid manual edit\n"
    );
}

#[test]
fn wrong_expected_identity_noncanonical_bytes_and_untrusted_claims_reject() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let changed = rename(&root, "calculator.add", "sum");
    let archive = archive(&changed);
    code(
        ProjectCandidateArchive::prepare(&changed, root.candidate_digest()),
        "SPX-G298",
    );
    code(
        ProjectCandidateArchive::restore(
            archive.to_json().as_bytes(),
            archive.archive_digest(),
            root.candidate_digest(),
        ),
        "SPX-G298",
    );
    code(
        ProjectCandidateArchive::restore(
            format!("{}\n", archive.to_json()).as_bytes(),
            archive.archive_digest(),
            archive.candidate_digest(),
        ),
        "SPX-G296",
    );
    let mut value: Value = serde_json::from_str(archive.to_json()).unwrap();
    value["approval_authority"] = json!(true);
    let (bytes, digest) = remint(value);
    code(
        ProjectCandidateArchive::restore(bytes.as_bytes(), &digest, archive.candidate_digest()),
        "SPX-G296",
    );
}

#[test]
fn reminting_content_cannot_substitute_an_incorrect_base_or_candidate() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let changed = rename(&root, "calculator.add", "sum");
    let archive = archive(&changed);
    let original: Value = serde_json::from_str(archive.to_json()).unwrap();
    let mut wrong_base = original.clone();
    wrong_base["base_revision"] = json!(changed.revision().project_revision());
    let (bytes, digest) = remint(wrong_base);
    code(
        ProjectCandidateArchive::restore(bytes.as_bytes(), &digest, archive.candidate_digest()),
        "SPX-G298",
    );
    let mut wrong_candidate = original.clone();
    wrong_candidate["candidate_digest"] = json!(root.candidate_digest());
    let (bytes, digest) = remint(wrong_candidate);
    code(
        ProjectCandidateArchive::restore(bytes.as_bytes(), &digest, root.candidate_digest()),
        "SPX-G298",
    );
    let mut wrong_source = original;
    wrong_source["sources"][0]["source_digest"] = json!(root.candidate_digest());
    let (bytes, digest) = remint(wrong_source);
    code(
        ProjectCandidateArchive::restore(bytes.as_bytes(), &digest, archive.candidate_digest()),
        "SPX-G298",
    );
}

#[test]
fn raw_depth_and_node_preflight_reject_before_json_tree_allocation() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let archive = archive(&candidate);
    let deep = format!("{}0{}", "[".repeat(17), "]".repeat(17));
    code(
        ProjectCandidateArchive::restore(
            deep.as_bytes(),
            archive.archive_digest(),
            archive.candidate_digest(),
        ),
        "SPX-G297",
    );
    let wide = format!("[{}]", vec!["0"; 1025].join(","));
    code(
        ProjectCandidateArchive::restore(
            wide.as_bytes(),
            archive.archive_digest(),
            archive.candidate_digest(),
        ),
        "SPX-G297",
    );
}
