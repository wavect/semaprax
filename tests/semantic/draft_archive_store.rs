//! Durable draft store evidence, authored and intentionally unrun.
#![cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
use semaprax::candidate_archive_store::{load, load_draft, persist, persist_draft};
use semaprax::diagnostic::Diagnostic;
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{
    persist_semantic_image, with_authenticated_project, ProjectCandidate, ProjectCandidateArchive,
    ProjectCandidateDraft, ProjectCandidateDraftArchive, ProjectSemanticImage,
};
use semaprax::semantic_retention::{
    checkpoint_receipts, RetentionAuthority, RetentionPolicy, RetentionReceipt,
    MAX_RETENTION_TOTAL_BYTES,
};
use semaprax::semantic_retention_lifecycle::{
    RetentionLifecycleCoordinator, SuccessfulRetentionReceipt,
};
use serde_json::json;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    store: PathBuf,
    base: Arc<ProjectCandidate>,
    draft: ProjectCandidateDraft,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-draft-store-{}-{}",
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
        let base = with_authenticated_project(&root.join("project/semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
        })
        .unwrap();
        let draft = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
        let draft = draft
            .with_body_hole(draft.draft_digest(), "calculator.add", "add")
            .unwrap();
        let draft = draft
            .with_body_hole(draft.draft_digest(), "calculator.subtract", "subtract")
            .unwrap();
        let draft = draft
            .fill_hole(
                draft.draft_digest(),
                "add",
                &json!({"kind":"i64","value":17}),
            )
            .unwrap();
        let store = root.join("archives");
        fs::create_dir(&store).unwrap();
        fs::set_permissions(&store, fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            root,
            store,
            base,
            draft,
        }
    }
    fn archive(&self) -> ProjectCandidateDraftArchive {
        ProjectCandidateDraftArchive::prepare(&self.draft, self.draft.draft_digest()).unwrap()
    }
    fn entry(&self, digest: &str) -> PathBuf {
        self.store.join(format!("{}.json", &digest[7..]))
    }
    fn names(&self) -> Vec<String> {
        let mut names = fs::read_dir(&self.store)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        names.sort();
        names
    }
    fn sources(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| fs::read(self.root.join("project").join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
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
fn private_file(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn deleted_origin_partial_history_restores_exact_context_then_continues_without_recreating_source()
{
    let fixture = Fixture::new();
    let archive = fixture.archive();
    let receipt = persist_draft(&fixture.store, &archive).unwrap();
    assert_eq!(receipt.archive_digest(), archive.archive_digest());
    assert_eq!(receipt.draft_digest(), fixture.draft.draft_digest());
    assert_eq!(
        receipt.base_revision(),
        fixture.base.base_revision().project_revision()
    );
    let entry = fixture.entry(archive.archive_digest());
    assert_eq!(fs::read(&entry).unwrap(), archive.to_json().as_bytes());
    assert_eq!(
        fs::metadata(&entry).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    let context = fixture
        .draft
        .hole_context(fixture.draft.draft_digest(), "subtract")
        .unwrap();
    fs::remove_dir_all(fixture.root.join("project")).unwrap();
    let restored = load_draft(
        &fixture.store,
        archive.archive_digest(),
        archive.draft_digest(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), fixture.draft.to_json());
    assert_eq!(
        restored.recovery_capsule().unwrap(),
        fixture.draft.recovery_capsule().unwrap()
    );
    assert_eq!(
        restored
            .hole_context(restored.draft_digest(), "subtract")
            .unwrap(),
        context
    );
    assert_eq!(
        ProjectCandidateDraftArchive::prepare(&restored, restored.draft_digest())
            .unwrap()
            .to_json(),
        archive.to_json()
    );
    code(restored.complete(restored.draft_digest()), "SPX-G232");
    let ready = restored
        .fill_hole(
            restored.draft_digest(),
            "subtract",
            &json!({"kind":"i64","value":23}),
        )
        .unwrap();
    let expected = fixture
        .draft
        .fill_hole(
            fixture.draft.draft_digest(),
            "subtract",
            &json!({"kind":"i64","value":23}),
        )
        .unwrap();
    assert_eq!(
        ready.complete(ready.draft_digest()).unwrap().to_json(),
        expected
            .complete(expected.draft_digest())
            .unwrap()
            .to_json()
    );
    let ready_archive =
        ProjectCandidateDraftArchive::prepare(&ready, ready.draft_digest()).unwrap();
    persist_draft(&fixture.store, &ready_archive).unwrap();
    let loaded_ready = load_draft(
        &fixture.store,
        ready_archive.archive_digest(),
        ready_archive.draft_digest(),
    )
    .unwrap();
    assert_eq!(
        loaded_ready
            .complete(loaded_ready.draft_digest())
            .unwrap()
            .to_json(),
        ready.complete(ready.draft_digest()).unwrap().to_json()
    );
    assert_eq!(fixture.names().len(), 2);
    assert!(!fixture.root.join("project").exists());
    assert_eq!(fs::read(entry).unwrap(), archive.to_json().as_bytes());
}

#[test]
fn duplicate_wrong_hash_and_same_length_content_tampering_never_overwrite_or_change_sources() {
    let fixture = Fixture::new();
    let sources = fixture.sources();
    let archive = fixture.archive();
    persist_draft(&fixture.store, &archive).unwrap();
    let entry = fixture.entry(archive.archive_digest());
    code(persist_draft(&fixture.store, &archive), "SPX-G302");
    assert_eq!(fs::read(&entry).unwrap(), archive.to_json().as_bytes());
    let wrong = format!("sha256:{}", "0".repeat(64));
    code(
        load_draft(&fixture.store, archive.archive_digest(), &wrong),
        "SPX-G342",
    );
    code(
        load_draft(&fixture.store, "not-a-digest", archive.draft_digest()),
        "SPX-G300",
    );
    code(
        load_draft(&fixture.store, archive.archive_digest(), "not-a-digest"),
        "SPX-G300",
    );
    let mut bytes = archive.to_json().as_bytes().to_vec();
    let position = bytes.iter().position(|byte| *byte == b'a').unwrap();
    bytes[position] = b'b';
    private_file(&entry, &bytes);
    assert!(load_draft(
        &fixture.store,
        archive.archive_digest(),
        archive.draft_digest()
    )
    .is_err());
    assert_eq!(fs::read(entry).unwrap(), bytes);
    assert_eq!(fixture.sources(), sources);
    assert_eq!(fixture.names().len(), 1);
}

#[test]
fn shared_store_keeps_candidate_and_draft_formats_distinct() {
    let fixture = Fixture::new();
    let sources = fixture.sources();
    let draft = fixture.archive();
    let candidate =
        ProjectCandidateArchive::prepare(&fixture.base, fixture.base.candidate_digest()).unwrap();
    persist(&fixture.store, &candidate).unwrap();
    persist_draft(&fixture.store, &draft).unwrap();
    code(
        load_draft(
            &fixture.store,
            candidate.archive_digest(),
            draft.draft_digest(),
        ),
        "SPX-G340",
    );
    code(
        load(
            &fixture.store,
            draft.archive_digest(),
            candidate.candidate_digest(),
        ),
        "SPX-G296",
    );
    assert_eq!(
        load(
            &fixture.store,
            candidate.archive_digest(),
            candidate.candidate_digest()
        )
        .unwrap()
        .to_json(),
        fixture.base.to_json()
    );
    assert_eq!(
        load_draft(&fixture.store, draft.archive_digest(), draft.draft_digest())
            .unwrap()
            .to_json(),
        fixture.draft.to_json()
    );
    assert_eq!(fixture.names().len(), 2);
    assert_eq!(fixture.sources(), sources);
}

#[test]
fn real_image_candidate_and_draft_receipts_share_one_authority_neutral_checkpoint() {
    let fixture = Fixture::new();
    let image_root = fixture.root.join("images");
    fs::create_dir(&image_root).unwrap();
    fs::set_permissions(&image_root, fs::Permissions::from_mode(0o700)).unwrap();
    let revision = Arc::clone(fixture.base.base_revision());
    let expected_revision = revision.project_revision().to_owned();
    let image = ProjectSemanticImage::derive(revision, &expected_revision).unwrap();
    let image_receipt = persist_semantic_image(&image_root, &image, image.image_digest()).unwrap();

    let candidate_archive =
        ProjectCandidateArchive::prepare(&fixture.base, fixture.base.candidate_digest()).unwrap();
    let candidate_receipt = persist(&fixture.store, &candidate_archive).unwrap();
    let draft_archive = fixture.archive();
    let draft_receipt = persist_draft(&fixture.store, &draft_archive).unwrap();

    let receipts: [&dyn RetentionReceipt; 3] = [&image_receipt, &candidate_receipt, &draft_receipt];
    let policy = RetentionPolicy::new(2, MAX_RETENTION_TOTAL_BYTES, 0).unwrap();
    let registry = fixture.root.join("retention-registry");
    fs::create_dir(&registry).unwrap();
    fs::create_dir(registry.join("metadata")).unwrap();
    fs::set_permissions(&registry, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(registry.join("metadata"), fs::Permissions::from_mode(0o700)).unwrap();
    let mut lifecycle = RetentionLifecycleCoordinator::open(&registry, policy, None).unwrap();
    let mut stale_lifecycle = RetentionLifecycleCoordinator::open(&registry, policy, None).unwrap();
    let typed_receipts = [
        SuccessfulRetentionReceipt::Image(&image_receipt),
        SuccessfulRetentionReceipt::Candidate(&candidate_receipt),
        SuccessfulRetentionReceipt::Draft(&draft_receipt),
    ];
    let lifecycle_report = lifecycle.checkpoint(&typed_receipts);
    assert!(lifecycle_report.registry_advanced());
    assert_eq!(lifecycle_report.sequence(), Some(1));
    assert!(lifecycle_report.diagnostics().is_empty());
    let lifecycle_json: serde_json::Value =
        serde_json::from_str(lifecycle_report.to_json()).unwrap();
    assert_eq!(lifecycle_json["successful_receipt_count"], 3);
    assert_eq!(
        lifecycle_json["subject_store_status"],
        "successful_receipts_precede_registry_attempt"
    );
    assert_eq!(lifecycle_json["registry_cursor_status"], "advanced");
    let first_cursor = lifecycle_report.cursor_digest().unwrap().to_owned();
    assert_eq!(lifecycle_json["cursor_digest"], first_cursor);
    let projected = lifecycle_json["successful_store_receipts"]
        .as_array()
        .unwrap();
    assert!(projected.contains(&json!({
        "kind":"image",
        "subject_digest":image_receipt.retention_observation().unwrap().subject().subject_digest(),
        "stored_bytes":image_receipt.retained_image_bytes(),
        "receipt_digest":image_receipt.receipt_digest(),
        "image_digest":image_receipt.image_digest(),
        "revision_store_entry":image_receipt.entry_digest(),
        "project_revision":image_receipt.project_revision(),
    })));
    assert!(projected.contains(&json!({
        "kind":"candidate",
        "subject_digest":candidate_receipt.retention_observation().unwrap().subject().subject_digest(),
        "stored_bytes":candidate_receipt.stored_bytes(),
        "archive_digest":candidate_receipt.archive_digest(),
        "candidate_digest":candidate_receipt.candidate_digest(),
        "base_revision":candidate_receipt.base_revision(),
    })));
    assert!(projected.contains(&json!({
        "kind":"draft",
        "subject_digest":draft_receipt.retention_observation().unwrap().subject().subject_digest(),
        "stored_bytes":draft_receipt.stored_bytes(),
        "archive_digest":draft_receipt.archive_digest(),
        "draft_digest":draft_receipt.draft_digest(),
        "base_revision":draft_receipt.base_revision(),
    })));

    let second = lifecycle.checkpoint(&typed_receipts);
    assert!(second.registry_advanced());
    assert_eq!(second.sequence(), Some(2));
    assert_ne!(second.cursor_digest(), Some(first_cursor.as_str()));
    let reopened =
        RetentionLifecycleCoordinator::open(&registry, policy, second.cursor_digest()).unwrap();
    assert_eq!(reopened.expected_cursor_digest(), second.cursor_digest());
    let stale_report = stale_lifecycle.checkpoint(&typed_receipts);
    assert!(!stale_report.registry_advanced());
    assert_eq!(stale_report.diagnostics()[0].code, "SPX-G467");
    let stale_json: serde_json::Value = serde_json::from_str(stale_report.to_json()).unwrap();
    assert_eq!(
        stale_json["subject_store_status"],
        "successful_receipts_precede_registry_attempt"
    );
    assert_eq!(
        stale_json["registry_cursor_status"],
        "registry_cursor_not_advanced_stale"
    );
    let poisoned = stale_lifecycle.checkpoint(&typed_receipts);
    assert_eq!(poisoned.diagnostics()[0].code, "SPX-G483");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(poisoned.to_json()).unwrap()
            ["registry_cursor_status"],
        "registry_attempt_blocked_reopen_required"
    );
    let over_capacity = vec![SuccessfulRetentionReceipt::Image(&image_receipt); 97];
    let blocked_over_capacity = stale_lifecycle.checkpoint(&over_capacity);
    assert_eq!(blocked_over_capacity.diagnostics()[0].code, "SPX-G483");
    let blocked_json: serde_json::Value =
        serde_json::from_str(blocked_over_capacity.to_json()).unwrap();
    assert_eq!(
        blocked_json["registry_cursor_status"],
        "registry_attempt_blocked_reopen_required"
    );
    assert_eq!(
        blocked_json["subject_store_status"],
        "successful_receipts_precede_registry_attempt"
    );
    let transition = checkpoint_receipts(None, None, 1, policy, &receipts).unwrap();
    let reversed = checkpoint_receipts(
        None,
        None,
        1,
        policy,
        &[receipts[2], receipts[1], receipts[0]],
    )
    .unwrap();

    assert_eq!(
        image_receipt.retained_image_bytes(),
        image.to_json().len() as u64
    );
    assert_eq!(
        transition.checkpoint().to_json(),
        reversed.checkpoint().to_json()
    );
    assert_eq!(transition.plan_json(), reversed.plan_json());
    assert_eq!(
        transition.checkpoint().authority(),
        RetentionAuthority::None
    );
    assert_eq!(transition.plan().authority(), RetentionAuthority::None);
    assert_eq!(transition.checkpoint().retained_subjects().len(), 2);
    assert_eq!(transition.evicted_subjects().len(), 1);
    assert!(image_root
        .join(&image_receipt.entry_digest()["sha256:".len()..])
        .exists());
    assert!(fixture.entry(candidate_archive.archive_digest()).exists());
    assert!(fixture.entry(draft_archive.archive_digest()).exists());
}

#[test]
fn lifecycle_holds_the_startup_registry_root_across_path_substitution() {
    let fixture = Fixture::new();
    let archive =
        ProjectCandidateArchive::prepare(&fixture.base, fixture.base.candidate_digest()).unwrap();
    let receipt = persist(&fixture.store, &archive).unwrap();
    let policy = RetentionPolicy::new(2, MAX_RETENTION_TOTAL_BYTES, 0).unwrap();
    let registry = fixture.root.join("held-retention-registry");
    fs::create_dir(&registry).unwrap();
    fs::create_dir(registry.join("metadata")).unwrap();
    fs::set_permissions(&registry, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(registry.join("metadata"), fs::Permissions::from_mode(0o700)).unwrap();
    let mut lifecycle = RetentionLifecycleCoordinator::open(&registry, policy, None).unwrap();

    let displaced = fixture.root.join("displaced-retention-registry");
    fs::rename(&registry, &displaced).unwrap();
    fs::create_dir(&registry).unwrap();
    fs::create_dir(registry.join("metadata")).unwrap();
    fs::set_permissions(&registry, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(registry.join("metadata"), fs::Permissions::from_mode(0o700)).unwrap();

    let outcome = lifecycle.checkpoint(&[SuccessfulRetentionReceipt::Candidate(&receipt)]);
    assert_eq!(outcome.diagnostics()[0].code, "SPX-G466");
    assert!(!registry.join("CURRENT").exists());
    assert!(!displaced.join("CURRENT").exists());

    fs::remove_dir_all(&registry).unwrap();
    fs::rename(&displaced, &registry).unwrap();
    let reopened = RetentionLifecycleCoordinator::open(&registry, policy, None).unwrap();
    assert_eq!(reopened.expected_cursor_digest(), None);
}

#[test]
fn v5_host_retains_explicit_registry_and_reports_post_store_stale_without_rollback() {
    let fixture = Fixture::new();
    let archive =
        ProjectCandidateArchive::prepare(&fixture.base, fixture.base.candidate_digest()).unwrap();
    let receipt = persist(&fixture.store, &archive).unwrap();
    let entry = fixture.entry(receipt.archive_digest());
    assert!(entry.exists());

    let policy = RetentionPolicy::new(2, MAX_RETENTION_TOTAL_BYTES, 0).unwrap();
    let registry = fixture.root.join("v5-retention-registry");
    fs::create_dir(&registry).unwrap();
    fs::create_dir(registry.join("metadata")).unwrap();
    fs::set_permissions(&registry, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(registry.join("metadata"), fs::Permissions::from_mode(0o700)).unwrap();
    let manifest = fixture.root.join("project/semaprax.toml");
    let mut first = VNextSession::open(&manifest, VNextPolicy::default())
        .unwrap()
        .with_retention_lifecycle(&registry, policy, None)
        .unwrap();
    let mut stale = VNextSession::open(&manifest, VNextPolicy::default())
        .unwrap()
        .with_retention_lifecycle(&registry, policy, None)
        .unwrap();
    let typed = [SuccessfulRetentionReceipt::Candidate(&receipt)];

    let advanced = first
        .checkpoint_successful_retention_receipts(&typed)
        .unwrap();
    assert!(advanced.registry_advanced());
    assert_eq!(advanced.sequence(), Some(1));
    assert_eq!(
        first.retention_lifecycle_outcome().unwrap().to_json(),
        advanced.to_json()
    );
    let stale = stale
        .checkpoint_successful_retention_receipts(&typed)
        .unwrap();
    assert!(!stale.registry_advanced());
    assert_eq!(stale.diagnostics()[0].code, "SPX-G467");
    let stale_json: serde_json::Value = serde_json::from_str(stale.to_json()).unwrap();
    assert_eq!(
        stale_json["subject_store_status"],
        "successful_receipts_precede_registry_attempt"
    );
    assert_eq!(
        stale_json["registry_cursor_status"],
        "registry_cursor_not_advanced_stale"
    );
    assert!(entry.exists());

    let mut unattached = VNextSession::open(&manifest, VNextPolicy::default()).unwrap();
    let errors = match unattached.checkpoint_successful_retention_receipts(&typed) {
        Ok(_) => panic!("unattached session accepted retention receipts"),
        Err(errors) => errors,
    };
    assert_eq!(errors[0].code, "SPX-G280");
    assert!(errors[0].message.contains("receipt remains valid"));
    assert!(entry.exists());
}

#[test]
fn roots_must_be_explicit_private_and_held_without_following_links() {
    let fixture = Fixture::new();
    let archive = fixture.archive();
    code(
        persist_draft(Path::new("relative-store"), &archive),
        "SPX-G300",
    );
    code(
        persist_draft(&fixture.store.join("."), &archive),
        "SPX-G300",
    );
    fs::set_permissions(&fixture.store, fs::Permissions::from_mode(0o755)).unwrap();
    code(persist_draft(&fixture.store, &archive), "SPX-G302");
    fs::set_permissions(&fixture.store, fs::Permissions::from_mode(0o700)).unwrap();
    let link = fixture.root.join("linked-store");
    symlink(&fixture.store, &link).unwrap();
    code(persist_draft(&link, &archive), "SPX-G302");
    let held = fs::File::open(&fixture.store).unwrap();
    fs2::FileExt::try_lock_exclusive(&held).unwrap();
    code(persist_draft(&fixture.store, &archive), "SPX-G302");
    fs2::FileExt::unlock(&held).unwrap();
    assert!(fixture.names().is_empty());
}

#[test]
fn hostile_selected_entries_and_inert_stage_are_never_followed_removed_or_adopted() {
    let fixture = Fixture::new();
    let archive = fixture.archive();
    let entry = fixture.entry(archive.archive_digest());
    let outside = fixture.root.join("outside.json");
    private_file(&outside, archive.to_json().as_bytes());
    symlink(&outside, &entry).unwrap();
    code(
        load_draft(
            &fixture.store,
            archive.archive_digest(),
            archive.draft_digest(),
        ),
        "SPX-G302",
    );
    assert!(fs::symlink_metadata(&entry)
        .unwrap()
        .file_type()
        .is_symlink());
    fs::remove_file(&entry).unwrap();
    fs::hard_link(&outside, &entry).unwrap();
    code(
        load_draft(
            &fixture.store,
            archive.archive_digest(),
            archive.draft_digest(),
        ),
        "SPX-G302",
    );
    fs::remove_file(&entry).unwrap();
    private_file(&fixture.store.join("foreign"), b"keep me");
    code(persist_draft(&fixture.store, &archive), "SPX-G302");
    assert_eq!(fs::read(fixture.store.join("foreign")).unwrap(), b"keep me");
    fs::remove_file(fixture.store.join("foreign")).unwrap();
    persist_draft(&fixture.store, &archive).unwrap();
    let stage = fixture.store.join(format!(".stage-{}", "0".repeat(64)));
    private_file(&stage, b"incomplete");
    assert_eq!(
        load_draft(
            &fixture.store,
            archive.archive_digest(),
            archive.draft_digest()
        )
        .unwrap()
        .to_json(),
        fixture.draft.to_json()
    );
    let ready = fixture
        .draft
        .fill_hole(
            fixture.draft.draft_digest(),
            "subtract",
            &json!({"kind":"i64","value":23}),
        )
        .unwrap();
    let ready = ProjectCandidateDraftArchive::prepare(&ready, ready.draft_digest()).unwrap();
    code(persist_draft(&fixture.store, &ready), "SPX-G302");
    assert_eq!(fs::read(stage).unwrap(), b"incomplete");
    assert_eq!(fs::read(outside).unwrap(), archive.to_json().as_bytes());
    assert_eq!(fs::read(entry).unwrap(), archive.to_json().as_bytes());
}

#[test]
fn typed_store_recovery_still_requires_startup_grant_exact_draft_and_matching_manifest() {
    use semaprax::image_transport::{VNextPolicy, VNextSession};
    use serde_json::Value;
    let fixture = Fixture::new();
    let archive = fixture.archive();
    persist_draft(&fixture.store, &archive).unwrap();
    let load = || {
        load_draft(
            &fixture.store,
            archive.archive_digest(),
            archive.draft_digest(),
        )
        .unwrap()
    };
    let open = |enabled| {
        VNextSession::open(
            &fixture.root.join("project/semaprax.toml"),
            VNextPolicy {
                candidate_prepare: enabled,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let mut readonly = open(false);
    code(
        readonly.retain_archived_draft(load(), archive.draft_digest()),
        "SPX-G303",
    );
    let mut stale = open(true);
    let wrong = format!("sha256:{}", "0".repeat(64));
    code(stale.retain_archived_draft(load(), &wrong), "SPX-G232");
    let image = stale.image_revision().to_owned();
    assert!(stale
        .export_draft_archive(&image, archive.draft_digest())
        .is_err());
    let handle: Value = serde_json::from_str(
        &stale
            .retain_archived_draft(load(), archive.draft_digest())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(handle["draft_revision"], archive.draft_digest());
    assert_eq!(handle["source_authority"], false);
    assert_eq!(
        stale
            .export_draft_archive(&image, archive.draft_digest())
            .unwrap()
            .to_json(),
        archive.to_json()
    );
    let frame=json!({"jsonrpc":"2.0","id":1,"method":"hole/query","params":{"image_revision":image,"draft_revision":archive.draft_digest(),"hole_id":"subtract"}}).to_string();
    let response: Value =
        serde_json::from_slice(&stale.handle_frame(frame.as_bytes()).unwrap()).unwrap();
    assert!(response.get("error").is_none(), "{response}");
    code(
        stale.retain_archived_draft(load(), archive.draft_digest()),
        "SPX-G303",
    );
    let sibling = Fixture::new();
    let path = sibling.root.join("project/semaprax.toml");
    let manifest = fs::read_to_string(&path).unwrap();
    let different = manifest.replace("name = \"calculator\"", "name = \"different-calculator\"");
    assert_ne!(different, manifest);
    fs::write(&path, different).unwrap();
    let mut foreign = VNextSession::open(
        &path,
        VNextPolicy {
            candidate_prepare: true,
            ..Default::default()
        },
    )
    .unwrap();
    code(
        foreign.retain_archived_draft(load(), archive.draft_digest()),
        "SPX-G342",
    );
    let image = foreign.image_revision().to_owned();
    assert!(foreign
        .export_draft_archive(&image, archive.draft_digest())
        .is_err());
    assert_eq!(
        fs::read(fixture.entry(archive.archive_digest())).unwrap(),
        archive.to_json().as_bytes()
    );
}
