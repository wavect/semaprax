//! Real filesystem regressions authored, not executed in this change.
use super::*;
use crate::project::{with_authenticated_project, SemanticChange};
use serde_json::json;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    store: PathBuf,
    candidate: ProjectCandidate,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-archive-store-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("project/src")).unwrap();
        let root = root.canonicalize().unwrap();
        let store = root.join("archives");
        fs::create_dir(&store).unwrap();
        fs::set_permissions(&store, fs::Permissions::from_mode(0o700)).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            fs::copy(example.join(path), root.join("project").join(path)).unwrap();
        }
        let revision = with_authenticated_project(&root.join("project/semaprax.toml"), |s| {
            Ok(s.retain_revision())
        })
        .unwrap();
        let candidate =
            ProjectCandidate::open(revision.clone(), revision.project_revision()).unwrap();
        let change = SemanticChange::new(
            candidate.revision().project_revision(),
            &json!({"kind":"rename_declaration","target":"calculator.add","name":"addition"}),
        )
        .unwrap();
        let candidate = candidate
            .apply(candidate.candidate_digest(), &change)
            .unwrap();
        Self {
            root,
            store,
            candidate,
        }
    }
    fn archive(&self) -> ProjectCandidateArchive {
        ProjectCandidateArchive::prepare(&self.candidate, self.candidate.candidate_digest())
            .unwrap()
    }
    fn other_archive(&self) -> ProjectCandidateArchive {
        let change = SemanticChange::new(self.candidate.revision().project_revision(),&json!({"kind":"rename_declaration","target":"calculator.subtract","name":"difference"})).unwrap();
        let candidate = self
            .candidate
            .apply(self.candidate.candidate_digest(), &change)
            .unwrap();
        ProjectCandidateArchive::prepare(&candidate, candidate.candidate_digest()).unwrap()
    }
    fn entry(&self, archive: &ProjectCandidateArchive) -> PathBuf {
        self.store.join(format!(
            "{}.json",
            digest_hex(archive.archive_digest()).unwrap()
        ))
    }
    fn names(&self) -> Vec<String> {
        let mut names = fs::read_dir(&self.store)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        names.sort();
        names
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn code<T>(result: Result<T>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(errors.iter().any(|e| e.code == expected), "{errors:?}"),
    }
}
fn private_file(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn exact_archive_survives_original_source_removal_without_becoming_source_authority() {
    let fixture = Fixture::new();
    let archive = fixture.archive();
    let receipt = persist(&fixture.store, &archive).unwrap();
    assert_eq!(receipt.archive_digest(), archive.archive_digest());
    assert_eq!(
        receipt.candidate_digest(),
        fixture.candidate.candidate_digest()
    );
    assert_eq!(
        receipt.base_revision(),
        fixture.candidate.base_revision().project_revision()
    );
    assert_eq!(
        fs::read(fixture.entry(&archive)).unwrap(),
        archive.to_json().as_bytes()
    );
    assert_eq!(
        fs::metadata(fixture.entry(&archive))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o600
    );
    fs::remove_dir_all(fixture.root.join("project")).unwrap();
    let replay = load(
        &fixture.store,
        archive.archive_digest(),
        archive.candidate_digest(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), fixture.candidate.to_json());
    assert_eq!(
        replay.revision().project_revision(),
        fixture.candidate.revision().project_revision()
    );
    assert!(!fixture.root.join("project").exists());
    assert_eq!(fixture.names().len(), 1);
}

#[test]
fn successful_store_receipts_produce_a_deterministic_authority_neutral_gc_plan() {
    let fixture = Fixture::new();
    let first_archive = fixture.archive();
    let second_archive = fixture.other_archive();
    let first_receipt = persist(&fixture.store, &first_archive).unwrap();
    let second_receipt = persist(&fixture.store, &second_archive).unwrap();
    assert_eq!(
        first_receipt.stored_bytes(),
        first_archive.to_json().len() as u64
    );
    assert_eq!(
        second_receipt.stored_bytes(),
        second_archive.to_json().len() as u64
    );

    let policy = RetentionPolicy::new(1, semantic_retention::MAX_RETENTION_TOTAL_BYTES, 0).unwrap();
    let receipts = [
        RetainedArchiveReceipt::Candidate(&first_receipt),
        RetainedArchiveReceipt::Candidate(&second_receipt),
    ];
    let transition = checkpoint_retained_archives(None, None, 1, policy, &receipts).unwrap();
    let reversed =
        checkpoint_retained_archives(None, None, 1, policy, &[receipts[1], receipts[0]]).unwrap();

    assert_eq!(
        transition.checkpoint().to_json(),
        reversed.checkpoint().to_json()
    );
    assert_eq!(transition.plan_json(), reversed.plan_json());
    assert_eq!(
        transition.checkpoint().authority(),
        semantic_retention::RetentionAuthority::None
    );
    assert_eq!(
        transition.plan().authority(),
        semantic_retention::RetentionAuthority::None
    );
    assert_eq!(transition.checkpoint().retained_subjects().len(), 1);
    assert_eq!(transition.evicted_subjects().len(), 1);
    assert!(fixture.entry(&first_archive).exists());
    assert!(fixture.entry(&second_archive).exists());
}

#[test]
fn no_adoption_wrong_binding_and_same_length_content_tamper_fail_closed() {
    let fixture = Fixture::new();
    let archive = fixture.archive();
    persist(&fixture.store, &archive).unwrap();
    code(persist(&fixture.store, &archive), "SPX-G302");
    assert_eq!(
        fs::read(fixture.entry(&archive)).unwrap(),
        archive.to_json().as_bytes()
    );
    let wrong = format!("sha256:{}", "0".repeat(64));
    assert!(load(&fixture.store, archive.archive_digest(), &wrong).is_err());
    let mut tampered = archive.to_json().as_bytes().to_vec();
    let position = tampered.iter().position(|byte| *byte == b'a').unwrap();
    tampered[position] = b'b';
    private_file(&fixture.entry(&archive), &tampered);
    assert!(load(
        &fixture.store,
        archive.archive_digest(),
        archive.candidate_digest()
    )
    .is_err());
    assert_eq!(fs::read(fixture.entry(&archive)).unwrap(), tampered);
}

#[test]
fn root_spelling_permissions_links_and_cooperative_lock_reject_before_stage_creation() {
    let fixture = Fixture::new();
    let archive = fixture.archive();
    code(
        persist(Path::new("relative-archive-store"), &archive),
        "SPX-G300",
    );
    code(persist(&fixture.store.join("."), &archive), "SPX-G300");
    fs::set_permissions(&fixture.store, fs::Permissions::from_mode(0o755)).unwrap();
    code(persist(&fixture.store, &archive), "SPX-G302");
    fs::set_permissions(&fixture.store, fs::Permissions::from_mode(0o700)).unwrap();
    let link = fixture.root.join("archive-link");
    symlink(&fixture.store, &link).unwrap();
    code(persist(&link, &archive), "SPX-G302");
    let lock = fs::File::open(&fixture.store).unwrap();
    fs2::FileExt::try_lock_exclusive(&lock).unwrap();
    code(persist(&fixture.store, &archive), "SPX-G302");
    fs2::FileExt::unlock(&lock).unwrap();
    assert!(fixture.names().is_empty());
}

#[test]
fn selected_symlink_hardlink_and_foreign_inventory_do_not_get_followed_or_cleaned() {
    let fixture = Fixture::new();
    let archive = fixture.archive();
    let outside = fixture.root.join("outside");
    private_file(&outside, archive.to_json().as_bytes());
    symlink(&outside, fixture.entry(&archive)).unwrap();
    code(
        load(
            &fixture.store,
            archive.archive_digest(),
            archive.candidate_digest(),
        ),
        "SPX-G302",
    );
    fs::remove_file(fixture.entry(&archive)).unwrap();
    fs::hard_link(&outside, fixture.entry(&archive)).unwrap();
    code(
        load(
            &fixture.store,
            archive.archive_digest(),
            archive.candidate_digest(),
        ),
        "SPX-G302",
    );
    fs::remove_file(fixture.entry(&archive)).unwrap();
    private_file(&fixture.store.join("foreign"), b"do not remove");
    code(persist(&fixture.store, &archive), "SPX-G302");
    assert_eq!(fs::read(&outside).unwrap(), archive.to_json().as_bytes());
    assert_eq!(
        fs::read(fixture.store.join("foreign")).unwrap(),
        b"do not remove"
    );
}

#[test]
fn retained_entries_have_bounded_metadata_inventory_not_implied_content_authentication() {
    let fixture = Fixture::new();
    let archive = fixture.archive();
    persist(&fixture.store, &archive).unwrap();
    for index in 0..31 {
        let path = fixture.store.join(format!("{index:064x}.json"));
        assert_ne!(path, fixture.entry(&archive));
        private_file(&path, b"unselected bytes are not semantically admitted");
    }
    assert_eq!(
        load(
            &fixture.store,
            archive.archive_digest(),
            archive.candidate_digest()
        )
        .unwrap()
        .candidate_digest(),
        archive.candidate_digest()
    );
    code(
        persist(&fixture.store, &fixture.other_archive()),
        "SPX-G301",
    );
    assert_eq!(fixture.names().len(), 32);
    private_file(
        &fixture.store.join(format!("{:064x}.json", 32)),
        b"one too many",
    );
    code(
        load(
            &fixture.store,
            archive.archive_digest(),
            archive.candidate_digest(),
        ),
        "SPX-G301",
    );
}

#[test]
fn failed_stage_is_retained_blocks_new_publication_but_does_not_hide_completed_archive() {
    let fixture = Fixture::new();
    let complete = fixture.archive();
    persist(&fixture.store, &complete).unwrap();
    let other = fixture.other_archive();
    let result = unix::persist_with_hook(&fixture.store, &other, |point, _| {
        if point == unix::StorePoint::AfterStageCreate {
            Err(std::io::Error::other("injected pre-pivot failure"))
        } else {
            Ok(())
        }
    });
    code(result, "SPX-I360");
    let stage = fixture.store.join(format!(
        ".stage-{}",
        digest_hex(other.archive_digest()).unwrap()
    ));
    assert_eq!(fs::metadata(&stage).unwrap().len(), 0);
    code(persist(&fixture.store, &other), "SPX-G302");
    assert_eq!(
        load(
            &fixture.store,
            complete.archive_digest(),
            complete.candidate_digest()
        )
        .unwrap()
        .candidate_digest(),
        complete.candidate_digest()
    );
    assert!(stage.exists());
}

#[test]
fn substitution_before_publication_preserves_outside_bytes_and_does_not_publish() {
    let fixture = Fixture::new();
    let archive = fixture.archive();
    let outside = fixture.root.join("outside");
    private_file(&outside, b"outside must remain unchanged");
    let stage = fixture.store.join(format!(
        ".stage-{}",
        digest_hex(archive.archive_digest()).unwrap()
    ));
    let result = unix::persist_with_hook(&fixture.store, &archive, |point, _| {
        if point == unix::StorePoint::BeforePublish {
            fs::remove_file(&stage)?;
            symlink(&outside, &stage)?;
        }
        Ok(())
    });
    code(result, "SPX-G302");
    assert!(!fixture.entry(&archive).exists());
    assert_eq!(
        fs::read(&outside).unwrap(),
        b"outside must remain unchanged"
    );
    assert!(fs::symlink_metadata(&stage)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn post_pivot_observation_failure_is_uncertain_and_resolved_only_by_exact_load() {
    let fixture = Fixture::new();
    let archive = fixture.archive();
    let result = unix::persist_with_hook(&fixture.store, &archive, |point, _| {
        if point == unix::StorePoint::AfterPublish {
            Err(std::io::Error::other(
                "injected observation failure after real rename",
            ))
        } else {
            Ok(())
        }
    });
    code(result, "SPX-I361");
    assert!(fixture.entry(&archive).exists());
    code(persist(&fixture.store, &archive), "SPX-G302");
    assert_eq!(
        load(
            &fixture.store,
            archive.archive_digest(),
            archive.candidate_digest()
        )
        .unwrap()
        .to_json(),
        fixture.candidate.to_json()
    );
}
