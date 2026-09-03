#![cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use semaprax::candidate_archive_store::CandidateArchiveStore;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateArchive, ProjectCandidateDraft,
    ProjectCandidateDraftArchive,
};
use semaprax::semantic_retention::{RetentionPolicy, MAX_RETENTION_TOTAL_BYTES};
use semaprax::semantic_retention_lifecycle::{
    AutomaticRetentionLifecycle, DurableLifecycleReplayReceipt, RestoredPendingSubject,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    archives: PathBuf,
    registry: PathBuf,
    candidate: Arc<ProjectCandidate>,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-automatic-retention-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("project/src")).unwrap();
        let root = root.canonicalize().unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            fs::copy(sample.join(path), root.join("project").join(path)).unwrap();
        }
        let candidate =
            with_authenticated_project(&root.join("project/semaprax.toml"), |snapshot| {
                ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                    .map(Arc::new)
            })
            .unwrap();
        let archives = root.join("archives");
        let registry = root.join("registry");
        fs::create_dir(&archives).unwrap();
        fs::create_dir(&registry).unwrap();
        fs::create_dir(registry.join("metadata")).unwrap();
        for path in [&archives, &registry, &registry.join("metadata")] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self {
            root,
            archives,
            registry,
            candidate,
        }
    }
    fn policy(&self) -> RetentionPolicy {
        RetentionPolicy::new(32, MAX_RETENTION_TOTAL_BYTES, 1).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn archive_precedes_exact_current_and_restart_replays_without_checkout() {
    let fixture = Fixture::new();
    let archive =
        ProjectCandidateArchive::prepare(&fixture.candidate, fixture.candidate.candidate_digest())
            .unwrap();
    let mut lifecycle = AutomaticRetentionLifecycle::open(
        &fixture.archives,
        &fixture.registry,
        fixture.policy(),
        None,
    )
    .unwrap();
    let outcome = lifecycle.persist_candidate(&archive).unwrap();
    assert!(outcome.lifecycle().registry_advanced());
    let transaction = outcome.transaction().unwrap();
    assert!(transaction
        .to_json()
        .contains("\"operation_kind\":\"archive_candidate_then_checkpoint\""));
    assert!(fixture
        .archives
        .join(format!("{}.json", &outcome.receipt().archive_digest()[7..]))
        .is_file());
    DurableLifecycleReplayReceipt::restore(
        transaction.to_json().as_bytes(),
        transaction.transaction_digest(),
    )
    .unwrap();
    let mut reminted = transaction.to_json().as_bytes().to_vec();
    reminted[10] ^= 1;
    assert!(
        DurableLifecycleReplayReceipt::restore(&reminted, transaction.transaction_digest())
            .is_err()
    );
    let original: serde_json::Value = serde_json::from_str(transaction.to_json()).unwrap();
    for field in ["cursor_digest", "checkpoint_digest", "plan_digest"] {
        let mut changed = original.clone();
        changed[field] = serde_json::json!("sha256:ABC");
        let mut bytes = serde_json::to_vec(&changed).unwrap();
        bytes.push(b'\n');
        assert!(
            DurableLifecycleReplayReceipt::restore(&bytes, transaction.transaction_digest())
                .is_err()
        );
    }
    for field in ["archive_digest", "content_digest", "base_revision"] {
        let mut changed = original.clone();
        changed["subject"][field] = serde_json::json!("sha256:ABC");
        let mut bytes = serde_json::to_vec(&changed).unwrap();
        bytes.push(b'\n');
        assert!(
            DurableLifecycleReplayReceipt::restore(&bytes, transaction.transaction_digest())
                .is_err()
        );
    }
    let cursor = outcome.lifecycle().cursor_digest().unwrap().to_owned();
    drop(lifecycle);
    fs::remove_dir_all(fixture.root.join("project")).unwrap();
    let restarted = AutomaticRetentionLifecycle::open(
        &fixture.archives,
        &fixture.registry,
        fixture.policy(),
        Some(&cursor),
    )
    .unwrap();
    let restored = restarted.restore_pending().unwrap();
    assert_eq!(restored.len(), 1);
    assert!(
        matches!(&restored[0], RestoredPendingSubject::Candidate(value) if value.candidate_digest() == fixture.candidate.candidate_digest())
    );
}

#[test]
fn candidate_and_draft_kinds_remain_distinct_across_consecutive_generations() {
    let fixture = Fixture::new();
    let candidate_archive =
        ProjectCandidateArchive::prepare(&fixture.candidate, fixture.candidate.candidate_digest())
            .unwrap();
    let draft = ProjectCandidateDraft::open(Arc::clone(&fixture.candidate)).unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "calculator.add", "pending-add")
        .unwrap();
    let draft_archive =
        ProjectCandidateDraftArchive::prepare(&draft, draft.draft_digest()).unwrap();
    let mut lifecycle = AutomaticRetentionLifecycle::open(
        &fixture.archives,
        &fixture.registry,
        fixture.policy(),
        None,
    )
    .unwrap();
    let first = lifecycle.persist_candidate(&candidate_archive).unwrap();
    assert_eq!(first.lifecycle().sequence(), Some(1));
    let second = lifecycle.persist_draft(&draft_archive).unwrap();
    assert_eq!(second.lifecycle().sequence(), Some(2));
    assert!(second
        .transaction()
        .unwrap()
        .to_json()
        .contains("\"operation_kind\":\"archive_draft_then_checkpoint\""));
    let restored = lifecycle.restore_pending().unwrap();
    assert_eq!(
        restored
            .iter()
            .filter(|value| matches!(value, RestoredPendingSubject::Candidate(_)))
            .count(),
        1
    );
    assert_eq!(
        restored
            .iter()
            .filter(|value| matches!(value, RestoredPendingSubject::Draft(_)))
            .count(),
        1
    );
}

#[test]
fn stale_current_is_sticky_and_never_rolls_back_the_second_archive() {
    let fixture = Fixture::new();
    let archive =
        ProjectCandidateArchive::prepare(&fixture.candidate, fixture.candidate.candidate_digest())
            .unwrap();
    let mut winner = AutomaticRetentionLifecycle::open(
        &fixture.archives,
        &fixture.registry,
        fixture.policy(),
        None,
    )
    .unwrap();
    let mut stale = AutomaticRetentionLifecycle::open(
        &fixture.archives,
        &fixture.registry,
        fixture.policy(),
        None,
    )
    .unwrap();
    winner.persist_candidate(&archive).unwrap();
    let draft = ProjectCandidateDraft::open(Arc::clone(&fixture.candidate)).unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "calculator.add", "pending-add")
        .unwrap();
    let draft_archive =
        ProjectCandidateDraftArchive::prepare(&draft, draft.draft_digest()).unwrap();
    let outcome = stale.persist_draft(&draft_archive).unwrap();
    assert!(!outcome.lifecycle().registry_advanced());
    assert!(outcome.transaction().is_none());
    assert!(fixture
        .archives
        .join(format!("{}.json", &outcome.receipt().archive_digest()[7..]))
        .is_file());
    assert!(outcome.recovery_required());
}

#[test]
fn held_archive_root_substitution_cannot_redirect_startup_replay() {
    let fixture = Fixture::new();
    let archive =
        ProjectCandidateArchive::prepare(&fixture.candidate, fixture.candidate.candidate_digest())
            .unwrap();
    let mut lifecycle = AutomaticRetentionLifecycle::open(
        &fixture.archives,
        &fixture.registry,
        fixture.policy(),
        None,
    )
    .unwrap();
    lifecycle.persist_candidate(&archive).unwrap();
    let displaced = fixture.root.join("held-archives");
    fs::rename(&fixture.archives, &displaced).unwrap();
    fs::create_dir(&fixture.archives).unwrap();
    fs::set_permissions(&fixture.archives, fs::Permissions::from_mode(0o700)).unwrap();
    let replacement = fixture
        .archives
        .join(format!("{}.json", &archive.archive_digest()[7..]));
    fs::copy(
        displaced.join(format!("{}.json", &archive.archive_digest()[7..])),
        &replacement,
    )
    .unwrap();
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(lifecycle.restore_pending().is_err());
}

#[test]
fn orphaned_archive_resumes_once_and_already_checkpointed_resume_is_idempotent() {
    let fixture = Fixture::new();
    let archive =
        ProjectCandidateArchive::prepare(&fixture.candidate, fixture.candidate.candidate_digest())
            .unwrap();
    let store = CandidateArchiveStore::open(&fixture.archives).unwrap();
    let receipt = store.persist(&archive).unwrap();
    drop(store);
    fs::remove_dir_all(fixture.root.join("project")).unwrap();

    let mut lifecycle = AutomaticRetentionLifecycle::open(
        &fixture.archives,
        &fixture.registry,
        fixture.policy(),
        None,
    )
    .unwrap();
    let resumed = lifecycle
        .resume_candidate(
            receipt.archive_digest(),
            receipt.candidate_digest(),
            receipt.base_revision(),
        )
        .unwrap();
    assert_eq!(resumed.sequence(), 1);
    assert!(!resumed.already_checkpointed());
    let cursor = resumed.cursor_digest().unwrap().to_owned();
    drop(lifecycle);

    let mut reopened = AutomaticRetentionLifecycle::open(
        &fixture.archives,
        &fixture.registry,
        fixture.policy(),
        Some(&cursor),
    )
    .unwrap();
    let settled = reopened
        .resume_candidate(
            receipt.archive_digest(),
            receipt.candidate_digest(),
            receipt.base_revision(),
        )
        .unwrap();
    assert!(settled.already_checkpointed());
    assert!(!settled.recovery_required());
    assert_eq!(settled.sequence(), 1);
    assert_eq!(settled.cursor_digest(), Some(cursor.as_str()));
}

#[test]
fn resumed_archive_rejects_cross_kind_and_reminted_base() {
    let fixture = Fixture::new();
    let archive =
        ProjectCandidateArchive::prepare(&fixture.candidate, fixture.candidate.candidate_digest())
            .unwrap();
    let store = CandidateArchiveStore::open(&fixture.archives).unwrap();
    let receipt = store.persist(&archive).unwrap();
    drop(store);
    let mut lifecycle = AutomaticRetentionLifecycle::open(
        &fixture.archives,
        &fixture.registry,
        fixture.policy(),
        None,
    )
    .unwrap();
    assert!(lifecycle
        .resume_draft(
            receipt.archive_digest(),
            receipt.candidate_digest(),
            receipt.base_revision(),
        )
        .is_err());
    assert!(lifecycle
        .resume_candidate(
            receipt.archive_digest(),
            receipt.candidate_digest(),
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        )
        .is_err());
    assert!(!fixture.registry.join("CURRENT").exists());
}
