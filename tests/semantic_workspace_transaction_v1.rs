use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use semaprax::workspace;
use sha2::{Digest, Sha256};

static FIXTURE: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let serial = FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-workspace-{label}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct LockHolder {
    child: Child,
    release: PathBuf,
}

impl LockHolder {
    fn release(mut self) {
        std::fs::write(&self.release, b"release").unwrap();
        assert!(self.child.wait().unwrap().success());
    }

    fn kill(mut self) {
        self.child.kill().unwrap();
        let _ = self.child.wait().unwrap();
    }
}

impl Drop for LockHolder {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.release, b"release");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_lock_holder(root: &Path, mode: &str, ordinal: u64) -> LockHolder {
    let ready = root.join(format!("holder-{ordinal}.ready"));
    let release = root.join(format!("holder-{ordinal}.release"));
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "workspace_lock_holder_child", "--nocapture"])
        .env("SEMAPRAX_WORKSPACE_LOCK_CHILD", "1")
        .env(
            "SEMAPRAX_WORKSPACE_LOCK_PATH",
            root.join(".semaprax-workspace/LOCK"),
        )
        .env("SEMAPRAX_WORKSPACE_LOCK_MODE", mode)
        .env("SEMAPRAX_WORKSPACE_LOCK_READY", &ready)
        .env("SEMAPRAX_WORKSPACE_LOCK_RELEASE", &release)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.exists() {
        assert!(
            Instant::now() < deadline,
            "lock-holder child did not become ready"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    LockHolder { child, release }
}

fn canonical(source: &str, path: &str) -> String {
    let program = semaprax::parse(source, path).unwrap();
    semaprax::format::canonical(&program)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fixture(label: &str) -> (TempRoot, PathBuf, String, String) {
    let root = TempRoot::new(label);
    let alpha = canonical(
        "module workspace.alpha; @id(\"workspace.alpha.helper\") fn helper()->i64{1} fn main()->i64{helper()}",
        "alpha.spx",
    );
    let beta = canonical(
        "module workspace.beta; @id(\"workspace.beta.helper\") fn helper()->i64{2} fn main()->i64{helper()}",
        "nested/beta.spx",
    );
    std::fs::write(root.path().join("alpha.spx"), &alpha).unwrap();
    std::fs::create_dir(root.path().join("nested")).unwrap();
    std::fs::write(root.path().join("nested/beta.spx"), &beta).unwrap();
    let path_set = root.path().join("paths.json");
    std::fs::write(
        &path_set,
        "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"nested/beta.spx\"}]}\n",
    )
    .unwrap();
    (root, path_set, alpha, beta)
}

#[test]
fn phase_a_initializes_authenticates_and_previews_without_raw_source_writes() {
    let (root, path_set, alpha, beta) = fixture("preview");
    let revision = workspace::initialize(root.path(), &path_set).unwrap();
    let snapshot = workspace::snapshot(root.path()).unwrap();
    assert_eq!(
        revision,
        "sha256:9a7368825342cee138d02a8037248e9a41ed0479d4f7c32a21c7ee7141cf280c"
    );
    assert_eq!(
        sha256(snapshot.to_json().as_bytes()),
        "3646097c9fb8c47bced51cf2c404b886755f657c73c57afb18d25282574f0b80"
    );
    assert_eq!(snapshot.workspace_revision(), revision);
    assert_eq!(snapshot.files().len(), 2);
    assert_eq!(
        std::fs::read_to_string(root.path().join("alpha.spx")).unwrap(),
        alpha
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("nested/beta.spx")).unwrap(),
        beta
    );

    let alpha_revision = snapshot
        .files()
        .iter()
        .find(|file| file.path() == "alpha.spx")
        .unwrap()
        .source_revision();
    let beta_revision = snapshot
        .files()
        .iter()
        .find(|file| file.path() == "nested/beta.spx")
        .unwrap()
        .source_revision();
    let alpha_patch =
        format!("base {alpha_revision}\nrename workspace.alpha.helper to renamed_alpha\n");
    let beta_patch =
        format!("base {beta_revision}\nrename workspace.beta.helper to renamed_beta\n");
    let patch = root.path().join("workspace-patch.json");
    std::fs::write(
        &patch,
        format!(
            "{{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"base_workspace_revision\":\"{revision}\",\"files\":[{{\"path\":\"alpha.spx\",\"patch\":{}}},{{\"path\":\"nested/beta.spx\",\"patch\":{}}}]}}\n",
            serde_json::to_string(&alpha_patch).unwrap(),
            serde_json::to_string(&beta_patch).unwrap()
        ),
    )
    .unwrap();
    let preview = workspace::preview(root.path(), &patch).unwrap();
    assert_eq!(
        sha256(preview.as_bytes()),
        "a4f1a9467d535aada97e7f253cf51c0d2168b5557a5a400d11692ac6966776b4"
    );
    assert!(preview.contains("\"schema\":\"semaprax.semantic-workspace-preview.v1\""));
    assert!(preview.contains("\"used_operations\":2"));
    assert!(!preview.contains("\"used_total_candidate_source_bytes\":0"));
    assert_eq!(
        std::fs::read_to_string(root.path().join("alpha.spx")).unwrap(),
        alpha
    );
    assert_eq!(
        workspace::apply(root.path(), &patch).unwrap_err()[0].code,
        "SPX-I212"
    );
}

#[test]
fn shared_compiler_owned_prelude_ids_do_not_conflict_across_files() {
    let (root, path_set, _, _) = fixture("prelude");
    workspace::initialize(root.path(), &path_set).unwrap();
    assert_eq!(workspace::snapshot(root.path()).unwrap().files().len(), 2);
}

#[test]
fn initialize_rejects_noncanonical_base_before_control_publication() {
    let (root, path_set, _, _) = fixture("canonical");
    std::fs::write(
        root.path().join("alpha.spx"),
        "module  workspace.alpha; @id(\"workspace.alpha.helper\") fn helper()->i64{1}\n",
    )
    .unwrap();
    let error = workspace::initialize(root.path(), &path_set).unwrap_err();
    assert_eq!(error[0].code, "SPX-G150");
    assert!(!root.path().join(".semaprax-workspace").exists());
}

#[cfg(any(unix, windows))]
#[test]
fn initialize_rejects_hardlinked_sources_before_control_publication() {
    let (root, path_set, _, _) = fixture("hardlink");
    std::fs::remove_file(root.path().join("nested/beta.spx")).unwrap();
    std::fs::hard_link(
        root.path().join("alpha.spx"),
        root.path().join("nested/beta.spx"),
    )
    .unwrap();
    let error = workspace::initialize(root.path(), &path_set).unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert!(!root.path().join(".semaprax-workspace").exists());
}

#[test]
fn workspace_lock_holder_child() {
    if std::env::var_os("SEMAPRAX_WORKSPACE_LOCK_CHILD").is_none() {
        return;
    }
    let lock_path = PathBuf::from(std::env::var_os("SEMAPRAX_WORKSPACE_LOCK_PATH").unwrap());
    let ready = PathBuf::from(std::env::var_os("SEMAPRAX_WORKSPACE_LOCK_READY").unwrap());
    let release = PathBuf::from(std::env::var_os("SEMAPRAX_WORKSPACE_LOCK_RELEASE").unwrap());
    let mode = std::env::var("SEMAPRAX_WORKSPACE_LOCK_MODE").unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_path)
        .unwrap();
    match mode.as_str() {
        "shared" => fs2::FileExt::lock_shared(&file).unwrap(),
        "exclusive" => fs2::FileExt::lock_exclusive(&file).unwrap(),
        _ => panic!("unexpected child lock mode"),
    }
    std::fs::write(ready, b"ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !release.exists() {
        assert!(Instant::now() < deadline, "lock-holder release timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    fs2::FileExt::unlock(&file).unwrap();
}

#[test]
fn permanent_lock_allows_two_real_shared_process_holders() {
    let (root, path_set, _, _) = fixture("shared-locks");
    workspace::initialize(root.path(), &path_set).unwrap();
    let first = spawn_lock_holder(root.path(), "shared", 1);
    let second = spawn_lock_holder(root.path(), "shared", 2);
    assert_eq!(workspace::snapshot(root.path()).unwrap().files().len(), 2);
    second.release();
    first.release();
}

#[test]
fn permanent_lock_reports_shared_exclusive_and_exclusive_exclusive_busy() {
    let (root, path_set, _, _) = fixture("busy-locks");
    workspace::initialize(root.path(), &path_set).unwrap();
    let shared = spawn_lock_holder(root.path(), "shared", 1);
    let contender = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.path().join(".semaprax-workspace/LOCK"))
        .unwrap();
    assert!(fs2::FileExt::try_lock_exclusive(&contender).is_err());
    shared.release();

    let exclusive = spawn_lock_holder(root.path(), "exclusive", 2);
    let error = workspace::snapshot(root.path()).unwrap_err();
    assert_eq!(error[0].code, "SPX-I210");
    assert!(fs2::FileExt::try_lock_exclusive(&contender).is_err());
    exclusive.release();
    fs2::FileExt::try_lock_exclusive(&contender).unwrap();
    fs2::FileExt::unlock(&contender).unwrap();
}

#[test]
fn killing_real_lock_holder_releases_the_permanent_lock() {
    let (root, path_set, _, _) = fixture("killed-lock");
    workspace::initialize(root.path(), &path_set).unwrap();
    let exclusive = spawn_lock_holder(root.path(), "exclusive", 1);
    assert_eq!(
        workspace::snapshot(root.path()).unwrap_err()[0].code,
        "SPX-I210"
    );
    exclusive.kill();
    assert_eq!(workspace::snapshot(root.path()).unwrap().files().len(), 2);
}
