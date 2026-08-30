//! Host archive recovery evidence, authored and deliberately unrun locally.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateArchive, SemanticChange,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-session-archive-{}-{}",
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
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: true,
                ..VNextPolicy::default()
            },
        )
        .unwrap()
    }
    fn candidate(&self) -> ProjectCandidate {
        let revision =
            with_authenticated_project(&self.manifest(), |snapshot| Ok(snapshot.retain_revision()))
                .unwrap();
        let expected = revision.project_revision().to_owned();
        ProjectCandidate::open(revision, &expected).unwrap()
    }
    fn renamed(&self) -> ProjectCandidate {
        let base = self.candidate();
        let change = SemanticChange::new(
            base.revision().project_revision(),
            &json!({"kind":"rename_declaration","target":"calculator.add","name":"sum"}),
        )
        .unwrap();
        base.apply(base.candidate_digest(), &change).unwrap()
    }
    fn edit_app(&self) {
        let path = self.0.join("src/app.spx");
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(source.contains("multiply(6, 7)"));
        let source = source.replace("multiply(6, 7)", "multiply(6, 8)");
        let program = semaprax::parse(&source, "src/app.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn archive(candidate: &ProjectCandidate) -> ProjectCandidateArchive {
    ProjectCandidateArchive::prepare(candidate, candidate.candidate_digest()).unwrap()
}
fn restore(session: &mut VNextSession, archive: &ProjectCandidateArchive) -> Value {
    serde_json::from_str(
        &session
            .restore_candidate_archive(
                archive.to_json().as_bytes(),
                archive.archive_digest(),
                archive.candidate_digest(),
            )
            .unwrap(),
    )
    .unwrap()
}
fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if method != "protocol/capabilities" {
        params["image_revision"] = json!(session.image_revision());
    }
    let frame = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(frame.as_bytes()).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}

#[test]
fn sibling_restart_restores_historical_candidate_then_explicitly_rebases() {
    let original = Fixture::new();
    let candidate = original.renamed();
    let saved = archive(&candidate);
    drop(candidate);
    drop(original); // The archive must not depend on its former source root.
    let sibling = Fixture::new();
    sibling.edit_app();
    let before = sibling.bytes();
    let current = sibling.candidate();
    assert_ne!(current.revision().project_revision(), saved.base_revision());
    let mut session = sibling.session();
    let image = session.image_revision().to_owned();
    let handle = restore(&mut session, &saved);
    assert_eq!(handle["base_revision"], saved.base_revision());
    assert_eq!(handle["candidate_revision"], saved.candidate_digest());
    assert_eq!(handle["source_authority"], false);
    assert_eq!(session.image_revision(), image);
    assert_eq!(
        session
            .export_candidate_archive(&image, saved.candidate_digest())
            .unwrap()
            .to_json(),
        saved.to_json()
    );
    let query = payload(call(
        &mut session,
        "candidate/query",
        json!({"candidate_revision":saved.candidate_digest(),"offset":0,"chunk_bytes":1024}),
    ));
    assert_eq!(query["report_schema"], "semaprax.project-candidate.v1");
    let live = payload(call(&mut session, "candidate/open", json!({})));
    let rebased = payload(call(
        &mut session,
        "candidate/rebase",
        json!({"candidate_revision":saved.candidate_digest(),"new_base_candidate_revision":live["candidate_revision"]}),
    ));
    assert_eq!(
        rebased["candidate"]["base_revision"],
        current.revision().project_revision()
    );
    assert_eq!(
        rebased["report"]["validation"],
        "complete_candidate_source_replay"
    );
    assert_eq!(sibling.bytes(), before);
}

#[test]
fn restoration_requires_startup_and_candidate_grant_without_enabling_commit() {
    let fixture = Fixture::new();
    let saved = archive(&fixture.renamed());
    let mut readonly = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    let errors = readonly
        .restore_candidate_archive(
            saved.to_json().as_bytes(),
            saved.archive_digest(),
            saved.candidate_digest(),
        )
        .unwrap_err();
    assert_eq!(errors[0].code, "SPX-G303");
    let image = readonly.image_revision().to_owned();
    assert_eq!(
        readonly
            .export_candidate_archive(&image, saved.candidate_digest())
            .err()
            .unwrap()[0]
            .code,
        "SPX-G303"
    );
    let mut started = fixture.session();
    assert!(started.handle_frame(b"{").is_some());
    assert_eq!(
        started
            .restore_candidate_archive(
                saved.to_json().as_bytes(),
                saved.archive_digest(),
                saved.candidate_digest()
            )
            .unwrap_err()[0]
            .code,
        "SPX-G303"
    );
    let mut session = fixture.session();
    restore(&mut session, &saved);
    let caps = payload(call(&mut session, "protocol/capabilities", json!({})));
    assert_eq!(caps["source_authority"], false);
    assert!(!caps["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|name| name == "candidate/commit"));
    assert_eq!(
        session
            .restore_candidate_archive(
                saved.to_json().as_bytes(),
                saved.archive_digest(),
                saved.candidate_digest()
            )
            .unwrap_err()[0]
            .code,
        "SPX-G303"
    );
}

#[test]
fn tamper_and_wrong_typed_handoff_leave_existing_registry_entry_unchanged() {
    let fixture = Fixture::new();
    let saved = archive(&fixture.renamed());
    let before = fixture.bytes();
    let mut session = fixture.session();
    restore(&mut session, &saved);
    let mut tampered = saved.to_json().as_bytes().to_vec();
    tampered[0] = b'[';
    assert!(session
        .restore_candidate_archive(&tampered, saved.archive_digest(), saved.candidate_digest())
        .is_err());
    let other = fixture.candidate();
    let other_digest = other.candidate_digest().to_owned();
    assert_eq!(
        session
            .retain_archived_candidate(other, saved.candidate_digest())
            .unwrap_err()[0]
            .code,
        "SPX-G224"
    );
    let image = session.image_revision().to_owned();
    assert_eq!(
        session
            .export_candidate_archive(&image, saved.candidate_digest())
            .unwrap()
            .to_json(),
        saved.to_json()
    );
    assert_eq!(
        session
            .export_candidate_archive(&image, &other_digest)
            .err()
            .unwrap()[0]
            .code,
        "SPX-G224"
    );
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn live_drift_blocks_installation_and_export_until_explicit_refresh() {
    let fixture = Fixture::new();
    let saved = archive(&fixture.renamed());
    let mut empty = fixture.session();
    let mut retained = fixture.session();
    restore(&mut retained, &saved);
    fixture.edit_app();
    let before = fixture.bytes();
    assert!(empty
        .restore_candidate_archive(
            saved.to_json().as_bytes(),
            saved.archive_digest(),
            saved.candidate_digest()
        )
        .is_err());
    let old_image = retained.image_revision().to_owned();
    assert!(retained
        .export_candidate_archive(&old_image, saved.candidate_digest())
        .is_err());
    let current = fixture.candidate();
    for session in [&mut empty, &mut retained] {
        payload(call(
            session,
            "workspace/refresh",
            json!({"expected_new_project_revision":current.revision().project_revision()}),
        ));
    }
    let image = empty.image_revision().to_owned();
    assert_eq!(
        empty
            .export_candidate_archive(&image, saved.candidate_digest())
            .err()
            .unwrap()[0]
            .code,
        "SPX-G224"
    );
    let image = retained.image_revision().to_owned();
    assert!(retained
        .export_candidate_archive(&old_image, saved.candidate_digest())
        .is_err());
    assert_eq!(
        retained
            .export_candidate_archive(&image, saved.candidate_digest())
            .unwrap()
            .to_json(),
        saved.to_json()
    );
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn independently_restored_typed_candidate_uses_the_same_handle_without_disk_writes() {
    let fixture = Fixture::new();
    let saved = archive(&fixture.renamed());
    let restored = ProjectCandidateArchive::restore(
        saved.to_json().as_bytes(),
        saved.archive_digest(),
        saved.candidate_digest(),
    )
    .unwrap();
    let mut session = fixture.session();
    let before = fixture.bytes();
    let handle: Value = serde_json::from_str(
        &session
            .retain_archived_candidate(restored, saved.candidate_digest())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(handle, restore(&mut session, &saved)); // duplicate identity does not consume a new slot
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn archive_from_a_different_manifest_is_not_installed() {
    let original = Fixture::new();
    let saved = archive(&original.renamed());
    let foreign = Fixture::new();
    let manifest = std::fs::read_to_string(foreign.manifest()).unwrap();
    assert!(manifest.contains("name = \"calculator\""));
    std::fs::write(
        foreign.manifest(),
        manifest.replace("name = \"calculator\"", "name = \"different\""),
    )
    .unwrap();
    let mut session = foreign.session();
    assert_eq!(
        session
            .restore_candidate_archive(
                saved.to_json().as_bytes(),
                saved.archive_digest(),
                saved.candidate_digest()
            )
            .unwrap_err()[0]
            .code,
        "SPX-G303"
    );
    let image = session.image_revision().to_owned();
    assert_eq!(
        session
            .export_candidate_archive(&image, saved.candidate_digest())
            .err()
            .unwrap()[0]
            .code,
        "SPX-G224"
    );
}
