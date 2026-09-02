//! Closed source-review evidence, authored and intentionally unrun.
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, SemanticChange,
    MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES, PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-source-review-{}-{}",
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
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn sources(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|file| std::fs::read(self.0.join(file)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}
fn review(candidate: &ProjectCandidate) -> Value {
    let text = candidate
        .source_review(candidate.candidate_digest())
        .unwrap();
    assert!(text.len() <= MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES);
    assert!(text.ends_with('\n'));
    let report: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(report.as_object().unwrap().len(), 7);
    assert_eq!(report["schema"], PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA);
    assert_eq!(report["candidate_revision"], candidate.candidate_digest());
    assert_eq!(
        report["base_project_revision"],
        candidate.base_revision().project_revision()
    );
    assert_eq!(
        report["candidate_project_revision"],
        candidate.revision().project_revision()
    );
    assert_eq!(report["source_authority"], false);
    let mut body = report.clone();
    let revision = body
        .as_object_mut()
        .unwrap()
        .remove("report_revision")
        .unwrap();
    assert_eq!(
        revision,
        digest(
            b"semaprax.project-candidate-source-review.v1\0",
            format!("{body}\n").as_bytes()
        )
    );
    report
}

#[test]
fn unchanged_candidate_has_a_closed_empty_review_and_stale_selectors_fail() {
    let fixture = Fixture::new();
    let disk = fixture.sources();
    let candidate = fixture.candidate();
    let report = review(&candidate);
    assert_eq!(report["files"], json!([]));
    assert_eq!(review(&fixture.candidate()), report);
    let stale = format!("sha256:{}", "0".repeat(64));
    assert_ne!(candidate.candidate_digest(), stale);
    assert_eq!(
        candidate.source_review(&stale).unwrap_err()[0].code,
        "SPX-G224"
    );
    assert_eq!(
        candidate.source_review("invalid").unwrap_err()[0].code,
        "SPX-G222"
    );
    assert_eq!(fixture.sources(), disk);
}

#[test]
fn signature_evolution_review_reconstructs_exact_source_pairs_and_existing_diffs() {
    let fixture = Fixture::new();
    let disk = fixture.sources();
    let base = fixture.candidate();
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({
            "kind":"change_function_signature", "target":"calculator.add",
            "append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]
        }),
    )
    .unwrap();
    let candidate = base.apply(base.candidate_digest(), &change).unwrap();
    let old_bytes = candidate.to_json().to_owned();
    let old_report: Value = serde_json::from_str(&old_bytes).unwrap();
    let report = review(&candidate);
    let files = report["files"].as_array().unwrap();
    assert!(files.len() >= 2);
    assert!(files
        .windows(2)
        .all(|rows| rows[0]["path"].as_str() < rows[1]["path"].as_str()));
    let old_changes = old_report["source_changes"].as_array().unwrap();
    assert_eq!(files.len(), old_changes.len());
    for row in files {
        assert_eq!(row.as_object().unwrap().len(), 7);
        let path = row["path"].as_str().unwrap();
        let before = base
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .unwrap();
        let after = candidate
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .unwrap();
        assert_ne!(before.source(), after.source());
        assert_eq!(row["base_source"], before.source());
        assert_eq!(row["candidate_source"], after.source());
        assert_eq!(row["base_digest"], before.source_digest());
        assert_eq!(row["candidate_digest"], after.source_digest());
        for (source, field) in [
            (before.source(), "base_digest"),
            (after.source(), "candidate_digest"),
        ] {
            assert_eq!(
                row[field],
                digest(
                    b"semaprax.semantic-review.source-digest.v1\0",
                    source.as_bytes()
                )
            );
        }
        let old = old_changes.iter().find(|old| old["path"] == path).unwrap();
        assert_eq!(row["candidate_source"], old["replacement_source"]);
        assert_eq!(row["source_diff"], old["source_diff"]);
        assert_eq!(row["source_diff_digest"], old["source_diff_digest"]);
        assert_eq!(
            row["source_diff_digest"],
            digest(
                b"semaprax.candidate.source-diff.v1\0",
                row["source_diff"].as_str().unwrap().as_bytes()
            )
        );
    }
    let replayed = ProjectCandidate::replay(
        Arc::clone(base.revision()),
        base.revision().project_revision(),
        &[change],
        old_bytes.as_bytes(),
    )
    .unwrap();
    assert_eq!(review(&replayed), report);
    assert_eq!(candidate.to_json(), old_bytes);
    assert_eq!(fixture.sources(), disk);
}

#[test]
fn sibling_reviews_and_live_checkout_bytes_do_not_replace_retained_source_identity() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let replace = |value| {
        let change = SemanticChange::new(base.revision().project_revision(), &json!({
            "kind":"replace_function_body", "target":"calculator.add", "body":{"kind":"i64","value":value}
        })).unwrap();
        base.apply(base.candidate_digest(), &change).unwrap()
    };
    let left = replace(23);
    let right = replace(24);
    let left_report = review(&left);
    let right_report = review(&right);
    assert_ne!(
        left_report["report_revision"],
        right_report["report_revision"]
    );
    assert_eq!(
        left_report["base_project_revision"],
        right_report["base_project_revision"]
    );
    assert_eq!(
        left.source_review(right.candidate_digest()).unwrap_err()[0].code,
        "SPX-G224"
    );
    std::fs::write(fixture.0.join("src/core.spx"), b"manual checkout drift\n").unwrap();
    // The pure library reviews its retained revision, never an arbitrary path.
    // Live-source authentication belongs to the separate transport/host owner.
    assert_eq!(review(&left), left_report);
    assert_eq!(review(&right), right_report);
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        b"manual checkout drift\n"
    );
}
