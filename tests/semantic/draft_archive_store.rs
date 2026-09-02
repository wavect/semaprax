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
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateArchive, ProjectCandidateDraft,
    ProjectCandidateDraftArchive,
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
