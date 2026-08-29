#[cfg(windows)]
use super::canonical_root;
use super::{
    acquire_snapshot, apply, apply_with_hook, bounded_manifest, count_directories_bounded,
    count_entries_bounded, file_facts, identity_from_path, initialize, initialize_with_hook,
    map_post_publication_candidate_diagnostics, parse_path_set,
    prepare_candidate_generation_with_hook, require_distinct_path_identities,
    require_exact_path_association, validate_staging_inventory, ApplyPoint, FileFact,
    GenerationPoint, InitializePoint,
};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    path_set: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-workspace-unit-{label}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let alpha = canonical(
                "module workspace.hook_alpha; @id(\"workspace.hook_alpha.helper\") fn helper()->i64{1} fn main()->i64{helper()}",
                "alpha.spx",
            );
        let beta = canonical(
                "module workspace.hook_beta; @id(\"workspace.hook_beta.helper\") fn helper()->i64{2} fn main()->i64{helper()}",
                "beta.spx",
            );
        std::fs::write(root.join("alpha.spx"), alpha).unwrap();
        std::fs::write(root.join("beta.spx"), beta).unwrap();
        let path_set = root.join("paths.json");
        std::fs::write(&path_set, "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"beta.spx\"}]}\n").unwrap();
        Self { root, path_set }
    }

    fn active(&self) -> PathBuf {
        self.root.join(".semaprax-workspace/ACTIVE")
    }

    fn initialize_and_patch(&self, label: &str) -> PathBuf {
        let revision = initialize(&self.root, &self.path_set).unwrap();
        let snapshot = super::snapshot(&self.root).unwrap();
        let alpha = snapshot
            .files()
            .iter()
            .find(|file| file.path() == "alpha.spx")
            .unwrap();
        let beta = snapshot
            .files()
            .iter()
            .find(|file| file.path() == "beta.spx")
            .unwrap();
        let alpha_patch = format!(
            "base {}\nrename workspace.hook_alpha.helper to alpha_{label}\n",
            alpha.source_revision()
        );
        let beta_patch = format!(
            "base {}\nrename workspace.hook_beta.helper to beta_{label}\n",
            beta.source_revision()
        );
        let path = self.root.join(format!("{label}.wspatch"));
        std::fs::write(
                &path,
                format!(
                    "{{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"base_workspace_revision\":\"{revision}\",\"files\":[{{\"path\":\"alpha.spx\",\"patch\":{}}},{{\"path\":\"beta.spx\",\"patch\":{}}}]}}\n",
                    serde_json::to_string(&alpha_patch).unwrap(),
                    serde_json::to_string(&beta_patch).unwrap()
                ),
            )
            .unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn spawn_phase_c_process(
    fixture: &Fixture,
    patch: &Path,
    boundary: &str,
) -> (Child, PathBuf, PathBuf) {
    let ready = fixture.root.join(format!("phase-c-{boundary}.ready"));
    let release = fixture.root.join(format!("phase-c-{boundary}.release"));
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "workspace::tests::phase_c_apply_process_child",
            "--nocapture",
        ])
        .env("SEMAPRAX_PHASE_C_CHILD", "1")
        .env("SEMAPRAX_PHASE_C_ROOT", &fixture.root)
        .env("SEMAPRAX_PHASE_C_PATCH", patch)
        .env("SEMAPRAX_PHASE_C_BOUNDARY", boundary)
        .env("SEMAPRAX_PHASE_C_READY", &ready)
        .env("SEMAPRAX_PHASE_C_RELEASE", &release)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.exists() {
        assert!(
            Instant::now() < deadline,
            "Phase-C child did not reach {boundary}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    (child, ready, release)
}

#[test]
fn phase_c_apply_process_child() {
    if std::env::var_os("SEMAPRAX_PHASE_C_CHILD").is_none() {
        return;
    }
    let root = PathBuf::from(std::env::var_os("SEMAPRAX_PHASE_C_ROOT").unwrap());
    let patch = PathBuf::from(std::env::var_os("SEMAPRAX_PHASE_C_PATCH").unwrap());
    let boundary = std::env::var("SEMAPRAX_PHASE_C_BOUNDARY").unwrap();
    let ready = PathBuf::from(std::env::var_os("SEMAPRAX_PHASE_C_READY").unwrap());
    let release = PathBuf::from(std::env::var_os("SEMAPRAX_PHASE_C_RELEASE").unwrap());
    apply_with_hook(&root, &patch, |point, _, _, _| {
        let selected = match boundary.as_str() {
            "pre" => point == ApplyPoint::BeforeActiveReplace,
            "post" => point == ApplyPoint::AfterActiveReplace,
            _ => false,
        };
        if selected {
            std::fs::write(&ready, "ready\n")?;
            let deadline = Instant::now() + Duration::from_secs(30);
            while !release.exists() {
                assert!(Instant::now() < deadline, "Phase-C child release timed out");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        Ok(())
    })
    .unwrap();
}

fn canonical(source: &str, path: &str) -> String {
    let program = crate::parse(source, Path::new(path)).unwrap();
    crate::format::canonical(&program)
}

#[test]
fn evidence_preflight_paths_are_keyed_not_positional() {
    let expected = vec!["alpha.spx".to_owned(), "beta.spx".to_owned()];
    let reordered = vec!["beta.spx".to_owned(), "alpha.spx".to_owned()];
    require_exact_path_association(&expected, &reordered).unwrap();

    let duplicate = vec!["alpha.spx".to_owned(), "alpha.spx".to_owned()];
    assert!(require_exact_path_association(&expected, &duplicate).is_err());
    let missing = vec!["alpha.spx".to_owned()];
    assert!(require_exact_path_association(&expected, &missing).is_err());
    let foreign = vec!["alpha.spx".to_owned(), "gamma.spx".to_owned()];
    assert!(require_exact_path_association(&expected, &foreign).is_err());
}

#[test]
fn source_mutation_hook_prevents_active_publication() {
    let fixture = Fixture::new("source-race");
    let source = fixture.root.join("alpha.spx");
    let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
        if matches!(point, InitializePoint::GenerationBeforeRename) {
            std::fs::write(&source, "externally mutated\n").unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert!(!fixture.active().exists());
}

#[test]
fn staged_manifest_mutation_hook_prevents_active_publication() {
    let fixture = Fixture::new("stage-race");
    let manifest = fixture
        .root
        .join(".semaprax-workspace/staging/0/manifest.json");
    let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
        if matches!(point, InitializePoint::GenerationBeforeRename) {
            std::fs::write(&manifest, "{}\n").unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert!(!fixture.active().exists());
}

#[test]
fn path_set_mutation_hook_prevents_active_publication() {
    let fixture = Fixture::new("path-set-race");
    let path_set = fixture.path_set.clone();
    let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
        if matches!(point, InitializePoint::ActiveBeforeRename) {
            std::fs::write(&path_set, "{}\n").unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert!(!fixture.active().exists());
}

#[test]
fn staged_active_byte_mutation_hook_prevents_publication() {
    let fixture = Fixture::new("active-byte-race");
    let staged = fixture.root.join(".semaprax-workspace/staging/0");
    let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
        if matches!(point, InitializePoint::ActiveBeforeRename) {
            std::fs::write(&staged, "{}\n").unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert!(!fixture.active().exists());
}

#[test]
fn staged_active_same_byte_replacement_hook_prevents_publication() {
    let fixture = Fixture::new("active-replacement-race");
    let staged = fixture.root.join(".semaprax-workspace/staging/0");
    let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
        if matches!(point, InitializePoint::ActiveBeforeRename) {
            let bytes = std::fs::read(&staged).unwrap();
            std::fs::remove_file(&staged).unwrap();
            std::fs::write(&staged, bytes).unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert!(!fixture.active().exists());
}

#[test]
fn initializer_preserves_foreign_generation_and_active_destinations() {
    for kind in ["file", "directory"] {
        let fixture = Fixture::new(&format!("foreign-generation-{kind}"));
        let mut foreign_generation = None;
        let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
            if matches!(point, InitializePoint::GenerationDestinationChecked) {
                let slot = fixture.root.join(".semaprax-workspace/staging/0");
                let manifest = std::fs::read_to_string(slot.join("manifest.json")).unwrap();
                let revision = super::workspace_revision(&manifest);
                let destination = fixture
                    .root
                    .join(".semaprax-workspace/generations")
                    .join(super::revision_hex(&revision).unwrap());
                if kind == "file" {
                    std::fs::write(&destination, "foreign-generation\n").unwrap();
                } else {
                    std::fs::create_dir(&destination).unwrap();
                }
                foreign_generation = Some(destination);
            }
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I211");
        let foreign_generation = foreign_generation.unwrap();
        assert_eq!(foreign_generation.is_file(), kind == "file");
        assert_eq!(foreign_generation.is_dir(), kind == "directory");
        assert!(!fixture.active().exists());
    }

    for kind in ["file", "directory"] {
        let fixture = Fixture::new(&format!("foreign-active-{kind}"));
        let foreign_active = fixture.active();
        let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
            if matches!(point, InitializePoint::ActiveDestinationChecked) {
                if kind == "file" {
                    std::fs::write(&foreign_active, "foreign-active\n").unwrap();
                } else {
                    std::fs::create_dir(&foreign_active).unwrap();
                }
            }
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I212");
        assert_eq!(foreign_active.is_file(), kind == "file");
        assert_eq!(foreign_active.is_dir(), kind == "directory");
    }
}

#[test]
fn initializer_relocation_fingerprint_rejects_post_rename_corruption() {
    let fixture = Fixture::new("generation-relocation-corruption");
    let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
        if matches!(point, InitializePoint::GenerationRelocated) {
            let generation =
                std::fs::read_dir(fixture.root.join(".semaprax-workspace/generations"))
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
            let manifest = generation.join("manifest.json");
            let bytes = std::fs::read(&manifest).unwrap();
            std::fs::remove_file(&manifest).unwrap();
            std::fs::write(&manifest, bytes).unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert!(!fixture.active().exists());

    let fixture = Fixture::new("active-relocation-corruption");
    let active = fixture.active();
    let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
        if matches!(point, InitializePoint::ActiveRelocated) {
            let bytes = std::fs::read(&active).unwrap();
            std::fs::remove_file(&active).unwrap();
            std::fs::write(&active, bytes).unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I212");
    assert!(active.exists());
}

#[test]
fn final_guard_rejects_valid_staging_inventory_drift() {
    let fixture = Fixture::new("staging-drift");
    initialize(&fixture.root, &fixture.path_set).unwrap();
    let mut guard = acquire_snapshot(&fixture.root, false).unwrap();
    std::fs::create_dir(fixture.root.join(".semaprax-workspace/staging/0")).unwrap();
    let error = guard.recheck().unwrap_err();
    assert_eq!(error[0].code, "SPX-G152");
}

#[test]
fn final_guard_rejects_valid_retained_generation_drift() {
    let fixture = Fixture::new("generation-drift");
    initialize(&fixture.root, &fixture.path_set).unwrap();
    let donor = Fixture::new("generation-donor");
    std::fs::write(
            donor.root.join("beta.spx"),
            canonical(
                "module workspace.hook_beta; @id(\"workspace.hook_beta.helper\") fn helper()->i64{3} fn main()->i64{helper()}",
                "beta.spx",
            ),
        )
        .unwrap();
    initialize(&donor.root, &donor.path_set).unwrap();
    let donor_generations = donor.root.join(".semaprax-workspace/generations");
    let donor_generation = std::fs::read_dir(&donor_generations)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let target_generations = fixture.root.join(".semaprax-workspace/generations");
    let target = target_generations.join(donor_generation.file_name());
    let mut guard = acquire_snapshot(&fixture.root, false).unwrap();
    copy_tree(&donor_generation.path(), &target);
    let error = guard.recheck().unwrap_err();
    assert_eq!(error[0].code, "SPX-G152");
}

#[test]
fn snapshot_releases_shared_lock_before_returning_owned_data() {
    let fixture = Fixture::new("snapshot-lock-handoff");
    let revision = initialize(&fixture.root, &fixture.path_set).unwrap();
    let lock_path = fixture.root.join(".semaprax-workspace/LOCK");

    for _ in 0..128 {
        let snapshot = super::snapshot(&fixture.root).unwrap();
        assert_eq!(snapshot.workspace_revision(), revision);
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&lock)
            .expect("snapshot must release its shared lock before returning");
        fs2::FileExt::unlock(&lock).unwrap();
    }
}

#[test]
fn commit_failures_release_exclusive_lock_before_returning() {
    let fixture = Fixture::new("commit-error-lock-handoff");
    let patch = fixture.initialize_and_patch("handoff");
    let old_revision = super::snapshot(&fixture.root)
        .unwrap()
        .workspace_revision
        .to_owned();
    let lock_path = fixture.root.join(".semaprax-workspace/LOCK");

    for _ in 0..64 {
        let error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
            if point == ApplyPoint::AfterCandidatePrepared {
                return Err(std::io::Error::other("reject before ACTIVE staging"));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I211");

        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&lock)
            .expect("failed commit must synchronously release its exclusive lock");
        fs2::FileExt::unlock(&lock).unwrap();
        assert_eq!(
            super::snapshot(&fixture.root).unwrap().workspace_revision,
            old_revision
        );
    }
}

#[test]
fn apply_pretransfer_failures_release_exclusive_lock_before_returning() {
    let fixture = Fixture::new("apply-pretransfer-lock-handoff");
    let patch = fixture.initialize_and_patch("pretransfer");
    let old_revision = super::snapshot(&fixture.root)
        .unwrap()
        .workspace_revision
        .to_owned();
    let lock_path = fixture.root.join(".semaprax-workspace/LOCK");
    let missing = fixture.root.join("missing.wspatch");
    let malformed = fixture.root.join("malformed.wspatch");
    std::fs::write(&malformed, "{}\n").unwrap();
    let stale = fixture.root.join("stale.wspatch");
    let stale_revision = format!("sha256:{:064x}", 0usize);
    let stale_source =
        std::fs::read_to_string(&patch)
            .unwrap()
            .replacen(&old_revision, &stale_revision, 1);
    std::fs::write(&stale, stale_source).unwrap();

    for _ in 0..64 {
        let missing_error = apply(&fixture.root, &missing).unwrap_err();
        assert_eq!(missing_error[0].code, "SPX-I209");
        assert_apply_lock_handoff(&fixture, &lock_path, &old_revision);

        let malformed_error = apply(&fixture.root, &malformed).unwrap_err();
        assert_eq!(malformed_error[0].code, "SPX-G150");
        assert_apply_lock_handoff(&fixture, &lock_path, &old_revision);

        let hook_error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
            if point == ApplyPoint::AfterPatchRead {
                return Err(std::io::Error::other("reject after patch ownership"));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(hook_error[0].code, "SPX-I209");
        assert_apply_lock_handoff(&fixture, &lock_path, &old_revision);

        let stale_error = apply(&fixture.root, &stale).unwrap_err();
        assert_eq!(stale_error[0].code, "SPX-G152");
        assert_apply_lock_handoff(&fixture, &lock_path, &old_revision);
    }
}

fn assert_apply_lock_handoff(fixture: &Fixture, lock_path: &Path, revision: &str) {
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_path)
        .unwrap();
    fs2::FileExt::try_lock_exclusive(&lock)
        .expect("failed apply must synchronously release its exclusive lock");
    fs2::FileExt::unlock(&lock).unwrap();
    assert_eq!(
        super::snapshot(&fixture.root).unwrap().workspace_revision,
        revision
    );
}

fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), destination).unwrap();
        }
    }
}

fn assert_active_unchanged(fixture: &Fixture, bytes: &[u8], identity: super::FileIdentity) {
    assert_eq!(std::fs::read(fixture.active()).unwrap(), bytes);
    assert_eq!(
        identity_from_path(&fixture.active(), "SPX-I209").unwrap(),
        identity
    );
}

#[test]
fn path_identity_relation_is_biconditional() {
    let fixture = Fixture::new("path-identity-relation");
    let alpha = fixture.root.join("alpha.spx");
    let beta = fixture.root.join("beta.spx");
    let alpha_identity = identity_from_path(&alpha, "SPX-I209").unwrap();
    let beta_identity = identity_from_path(&beta, "SPX-I209").unwrap();
    require_distinct_path_identities(&[
        (&alpha, &alpha_identity),
        (&alpha, &alpha_identity),
        (&beta, &beta_identity),
    ])
    .unwrap();
    assert!(require_distinct_path_identities(&[
        (&alpha, &alpha_identity),
        (&alpha, &beta_identity),
    ])
    .is_err());
    assert!(require_distinct_path_identities(&[
        (&alpha, &alpha_identity),
        (&beta, &alpha_identity),
    ])
    .is_err());
}

#[test]
fn post_publication_candidate_mapping_is_narrow() {
    let structural = map_post_publication_candidate_diagnostics(vec![super::Diagnostic::io(
        "SPX-I209",
        "workspace directory must be real and non-aliased",
    )]);
    assert_eq!(structural[0].code, "SPX-G153");
    let genuine_io = map_post_publication_candidate_diagnostics(vec![super::Diagnostic::io(
        "SPX-I209",
        "cannot inspect directory: access denied",
    )]);
    assert_eq!(genuine_io[0].code, "SPX-I209");
}

#[test]
fn phase_b_creates_then_deep_reuses_candidate_without_active_pivot() {
    let fixture = Fixture::new("phase-b-reuse");
    let patch = fixture.initialize_and_patch("reuse");
    let active_bytes = std::fs::read(fixture.active()).unwrap();
    let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
    let first =
        prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {}).unwrap();
    assert!(fixture
        .root
        .join(".semaprax-workspace/generations")
        .join(super::revision_hex(&first).unwrap())
        .is_dir());
    assert_eq!(
        super::snapshot(&fixture.root).unwrap().retained_generations,
        2
    );
    let second =
        prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {}).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        super::snapshot(&fixture.root).unwrap().retained_generations,
        2
    );
    assert_active_unchanged(&fixture, &active_bytes, active_identity);
}

#[test]
fn phase_c_pivots_only_active_and_second_apply_is_stale() {
    let fixture = Fixture::new("phase-c-success");
    let alpha = std::fs::read(fixture.root.join("alpha.spx")).unwrap();
    let beta = std::fs::read(fixture.root.join("beta.spx")).unwrap();
    let patch = fixture.initialize_and_patch("commit");
    let old_active = std::fs::read(fixture.active()).unwrap();
    let old_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
    let preview = super::preview(&fixture.root, &patch).unwrap();
    let expected = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
        ["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();

    let applied = apply(&fixture.root, &patch).unwrap();
    assert_eq!(applied, expected);
    assert_ne!(std::fs::read(fixture.active()).unwrap(), old_active);
    assert_ne!(
        identity_from_path(&fixture.active(), "SPX-I209").unwrap(),
        old_identity
    );
    assert_eq!(
        super::snapshot(&fixture.root).unwrap().workspace_revision,
        expected
    );
    assert_eq!(
        std::fs::read(fixture.root.join("alpha.spx")).unwrap(),
        alpha
    );
    assert_eq!(std::fs::read(fixture.root.join("beta.spx")).unwrap(), beta);

    let committed_active = std::fs::read(fixture.active()).unwrap();
    let committed_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
    let error = apply(&fixture.root, &patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G152");
    assert_active_unchanged(&fixture, &committed_active, committed_identity);
}

#[test]
fn phase_c_final_checks_reject_owned_input_and_authority_drift_before_pivot() {
    for case in [
        "patch",
        "active",
        "stage",
        "candidate",
        "candidate_source",
        "candidate_inventory",
        "staging_inventory",
        "generation_inventory",
        "before_replace_stage",
    ] {
        let fixture = Fixture::new(&format!("phase-c-final-{case}"));
        let patch = fixture.initialize_and_patch(case);
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let error = apply_with_hook(
            &fixture.root,
            &patch,
            |point, active, staged_active, candidate| {
                match (case, point) {
                    ("patch", ApplyPoint::AfterPatchRead) => {
                        std::fs::write(&patch, "{}\n")?;
                    }
                    ("active", ApplyPoint::BeforeSecondFinalCheck) => {
                        let bytes = std::fs::read(active)?;
                        std::fs::remove_file(active)?;
                        std::fs::write(active, bytes)?;
                    }
                    ("stage", ApplyPoint::BeforeSecondFinalCheck) => {
                        let path = staged_active.unwrap();
                        let bytes = std::fs::read(path)?;
                        std::fs::remove_file(path)?;
                        std::fs::write(path, bytes)?;
                    }
                    ("candidate", ApplyPoint::BeforeSecondFinalCheck) => {
                        let path = candidate.unwrap().join("manifest.json");
                        let bytes = std::fs::read(&path)?;
                        std::fs::remove_file(&path)?;
                        std::fs::write(path, bytes)?;
                    }
                    ("candidate_source", ApplyPoint::BeforeSecondFinalCheck) => {
                        let path = candidate.unwrap().join("files/alpha.spx");
                        let bytes = std::fs::read(&path)?;
                        std::fs::remove_file(&path)?;
                        std::fs::write(path, bytes)?;
                    }
                    ("candidate_inventory", ApplyPoint::BeforeSecondFinalCheck) => {
                        std::fs::write(candidate.unwrap().join("files/extra.spx"), "foreign\n")?;
                    }
                    ("staging_inventory", ApplyPoint::BeforeSecondFinalCheck) => {
                        std::fs::write(
                            fixture.root.join(".semaprax-workspace/staging/31"),
                            "foreign\n",
                        )?;
                    }
                    ("generation_inventory", ApplyPoint::BeforeSecondFinalCheck) => {
                        std::fs::create_dir(fixture.root.join(format!(
                            ".semaprax-workspace/generations/{:064x}",
                            987_654usize
                        )))?;
                    }
                    ("before_replace_stage", ApplyPoint::BeforeActiveReplace) => {
                        std::fs::write(staged_active.unwrap(), "{}\n")?;
                    }
                    _ => {}
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert!(matches!(error[0].code, "SPX-G153" | "SPX-I209"));
        if case == "active" {
            assert_eq!(std::fs::read(fixture.active()).unwrap(), active_bytes);
            assert_ne!(
                identity_from_path(&fixture.active(), "SPX-I209").unwrap(),
                active_identity
            );
        } else {
            assert_active_unchanged(&fixture, &active_bytes, active_identity);
        }
    }
}

#[test]
fn phase_c_each_final_boundary_rejects_identity_and_inventory_substitution() {
    for boundary in [
        ApplyPoint::BeforeFirstFinalCheck,
        ApplyPoint::BeforeActiveReplace,
    ] {
        for case in [
            "patch",
            "active",
            "stage",
            "manifest",
            "source",
            "staging_inventory",
            "generation_inventory",
        ] {
            let fixture = Fixture::new(&format!("phase-c-{boundary:?}-{case}"));
            let patch = fixture.initialize_and_patch(case);
            let old_revision = super::snapshot(&fixture.root)
                .unwrap()
                .workspace_revision
                .to_owned();
            let error =
                apply_with_hook(&fixture.root, &patch, |point, active, staged, candidate| {
                    if point != boundary {
                        return Ok(());
                    }
                    match case {
                        "patch" => std::fs::write(&patch, "{}\n")?,
                        "active" => replace_with_same_bytes(active)?,
                        "stage" => replace_with_same_bytes(staged.unwrap())?,
                        "manifest" => {
                            replace_with_same_bytes(&candidate.unwrap().join("manifest.json"))?
                        }
                        "source" => {
                            replace_with_same_bytes(&candidate.unwrap().join("files/alpha.spx"))?
                        }
                        "staging_inventory" => std::fs::write(
                            fixture.root.join(".semaprax-workspace/staging/31"),
                            "foreign\n",
                        )?,
                        "generation_inventory" => std::fs::create_dir(fixture.root.join(format!(
                            ".semaprax-workspace/generations/{:064x}",
                            123_456usize
                        )))?,
                        _ => unreachable!(),
                    }
                    Ok(())
                })
                .unwrap_err();
            assert!(matches!(error[0].code, "SPX-G153" | "SPX-I209"));
            assert_eq!(
                super::snapshot(&fixture.root).unwrap().workspace_revision,
                old_revision
            );
        }
    }
}

fn replace_with_same_bytes(path: &Path) -> std::io::Result<()> {
    let bytes = std::fs::read(path)?;
    std::fs::remove_file(path)?;
    std::fs::write(path, bytes)
}

#[test]
fn phase_c_pre_pivot_rejection_retains_active_and_staging_residue() {
    let fixture = Fixture::new("phase-c-reject-pivot");
    let patch = fixture.initialize_and_patch("reject");
    let active_bytes = std::fs::read(fixture.active()).unwrap();
    let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
    let error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
        if point == ApplyPoint::BeforeActiveReplace {
            return Err(std::io::Error::other("injected ACTIVE rename rejection"));
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I211");
    assert_active_unchanged(&fixture, &active_bytes, active_identity);
    assert!(
        std::fs::read_dir(fixture.root.join(".semaprax-workspace/staging"))
            .unwrap()
            .next()
            .is_some()
    );
}

#[cfg(unix)]
#[test]
fn phase_c_atomic_active_rename_failure_preserves_old_active() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("phase-c-rename-failure");
    let patch = fixture.initialize_and_patch("rename_failure");
    let active_bytes = std::fs::read(fixture.active()).unwrap();
    let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
    let control = fixture.root.join(".semaprax-workspace");
    let original_permissions = std::fs::metadata(&control).unwrap().permissions();
    let error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
        if point == ApplyPoint::BeforeActiveReplace {
            std::fs::set_permissions(&control, std::fs::Permissions::from_mode(0o500))?;
        }
        Ok(())
    })
    .unwrap_err();
    std::fs::set_permissions(&control, original_permissions).unwrap();
    assert_eq!(error[0].code, "SPX-I211");
    assert_active_unchanged(&fixture, &active_bytes, active_identity);
}

#[test]
fn phase_c_bounded_final_source_growth_fails_before_pivot() {
    for boundary in [
        ApplyPoint::BeforeFirstFinalCheck,
        ApplyPoint::BeforeActiveReplace,
    ] {
        let fixture = Fixture::new(&format!("phase-c-growth-{boundary:?}"));
        let patch = fixture.initialize_and_patch("growth");
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let base_revision = super::snapshot(&fixture.root)
            .unwrap()
            .workspace_revision
            .strip_prefix("sha256:")
            .unwrap()
            .to_owned();
        let base_source = fixture
            .root
            .join(".semaprax-workspace/generations")
            .join(base_revision)
            .join("files/alpha.spx");
        let error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
            if point == boundary {
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&base_source)?
                    .set_len((super::MAX_TOTAL_SOURCE_BYTES + 1) as u64)?;
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
        assert_eq!(
            std::fs::metadata(base_source).unwrap().len(),
            (super::MAX_TOTAL_SOURCE_BYTES + 1) as u64
        );
    }
}

#[test]
fn phase_c_post_pivot_uncertainty_retains_new_generation_and_foreign_residue() {
    let fixture = Fixture::new("phase-c-post-pivot");
    let patch = fixture.initialize_and_patch("post_pivot");
    let preview = super::preview(&fixture.root, &patch).unwrap();
    let expected = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
        ["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let residue = fixture.root.join(".semaprax-workspace/staging/31");
    let error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
        if point == ApplyPoint::AfterActiveReplace {
            std::fs::write(&residue, "foreign\n")?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I212");
    assert_eq!(
        super::snapshot(&fixture.root).unwrap().workspace_revision,
        expected
    );
    assert_eq!(std::fs::read_to_string(residue).unwrap(), "foreign\n");
}

#[test]
fn phase_c_unwind_boundaries_leave_exactly_old_or_new_active() {
    for point in [
        ApplyPoint::BeforeActiveReplace,
        ApplyPoint::AfterActiveReplace,
    ] {
        let fixture = Fixture::new(&format!("phase-c-unwind-{point:?}"));
        let patch = fixture.initialize_and_patch("unwind");
        let old_revision = super::snapshot(&fixture.root)
            .unwrap()
            .workspace_revision
            .to_owned();
        let preview = super::preview(&fixture.root, &patch).unwrap();
        let candidate = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
            ["candidate_workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = apply_with_hook(&fixture.root, &patch, |observed, _, _, _| {
                if observed == point {
                    panic!("simulated process termination boundary");
                }
                Ok(())
            });
        }));
        assert!(result.is_err());
        let current = super::snapshot(&fixture.root).unwrap();
        if point == ApplyPoint::BeforeActiveReplace {
            assert_eq!(current.workspace_revision, old_revision);
            assert_eq!(apply(&fixture.root, &patch).unwrap(), candidate);
        } else {
            assert_eq!(current.workspace_revision, candidate);
            assert_eq!(
                apply(&fixture.root, &patch).unwrap_err()[0].code,
                "SPX-G152"
            );
        }
    }
}

#[test]
fn phase_c_killed_process_boundaries_recover_as_exact_old_or_new() {
    for boundary in ["pre", "post"] {
        let fixture = Fixture::new(&format!("phase-c-kill-{boundary}"));
        let patch = fixture.initialize_and_patch("killed");
        let old_revision = super::snapshot(&fixture.root)
            .unwrap()
            .workspace_revision
            .to_owned();
        let preview = super::preview(&fixture.root, &patch).unwrap();
        let candidate = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
            ["candidate_workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        let (mut child, _, _) = spawn_phase_c_process(&fixture, &patch, boundary);
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());

        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(fixture.root.join(".semaprax-workspace/LOCK"))
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&lock).unwrap();
        fs2::FileExt::unlock(&lock).unwrap();
        let current = super::snapshot(&fixture.root).unwrap();
        assert_eq!(
            std::fs::read_dir(fixture.root.join(".semaprax-workspace/generations"))
                .unwrap()
                .count(),
            2
        );
        if boundary == "pre" {
            assert_eq!(current.workspace_revision, old_revision);
            assert!(
                std::fs::read_dir(fixture.root.join(".semaprax-workspace/staging"))
                    .unwrap()
                    .next()
                    .is_some()
            );
            assert_eq!(apply(&fixture.root, &patch).unwrap(), candidate);
        } else {
            assert_eq!(current.workspace_revision, candidate);
            assert_eq!(
                apply(&fixture.root, &patch).unwrap_err()[0].code,
                "SPX-G152"
            );
        }
    }
}

#[test]
fn phase_c_live_writer_exposes_no_partial_snapshot_to_cooperative_reader() {
    let fixture = Fixture::new("phase-c-live-reader");
    let patch = fixture.initialize_and_patch("live_reader");
    let old_active = std::fs::read(fixture.active()).unwrap();
    let preview = super::preview(&fixture.root, &patch).unwrap();
    let candidate = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
        ["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let (mut child, _, release) = spawn_phase_c_process(&fixture, &patch, "pre");
    assert_eq!(
        super::snapshot(&fixture.root).unwrap_err()[0].code,
        "SPX-I210"
    );
    assert_eq!(std::fs::read(fixture.active()).unwrap(), old_active);
    std::fs::write(release, "release\n").unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(
        super::snapshot(&fixture.root).unwrap().workspace_revision,
        candidate
    );
}

#[test]
fn phase_c_active_permission_drift_fails_both_final_boundaries() {
    for (target, point) in [
        ("old", ApplyPoint::BeforeFirstFinalCheck),
        ("stage", ApplyPoint::BeforeFirstFinalCheck),
        ("old", ApplyPoint::BeforeSecondFinalCheck),
        ("stage", ApplyPoint::BeforeSecondFinalCheck),
    ] {
        let fixture = Fixture::new(&format!("phase-c-permission-{target}-{point:?}"));
        let patch = fixture.initialize_and_patch("permissions");
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let original_permissions = std::fs::metadata(fixture.active()).unwrap().permissions();
        let error = apply_with_hook(&fixture.root, &patch, |observed, active, staged, _| {
            if observed == point {
                let path = if target == "old" {
                    active
                } else {
                    staged.unwrap()
                };
                let mut permissions = std::fs::metadata(path)?.permissions();
                permissions.set_readonly(!permissions.readonly());
                std::fs::set_permissions(path, permissions)?;
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
        std::fs::set_permissions(fixture.active(), original_permissions.clone()).unwrap();
        for entry in std::fs::read_dir(fixture.root.join(".semaprax-workspace/staging")).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                std::fs::set_permissions(path, original_permissions.clone()).unwrap();
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn phase_c_success_preserves_active_mode() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("phase-c-mode");
    let patch = fixture.initialize_and_patch("mode");
    let active = fixture.active();
    std::fs::set_permissions(&active, std::fs::Permissions::from_mode(0o640)).unwrap();
    apply(&fixture.root, &patch).unwrap();
    assert_eq!(
        std::fs::metadata(active).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[test]
fn phase_b_skips_valid_residue_and_preserves_staging_objects() {
    let fixture = Fixture::new("phase-b-slots");
    let patch = fixture.initialize_and_patch("slots");
    let staging = fixture.root.join(".semaprax-workspace/staging");
    std::fs::write(staging.join("0"), "residue-zero\n").unwrap();
    std::fs::create_dir(staging.join("1")).unwrap();
    let mut observed = None;
    prepare_candidate_generation_with_hook(&fixture.root, &patch, |point, slot, _| {
        if point == GenerationPoint::AfterSlotCreate {
            observed = slot.file_name().map(|name| name.to_owned());
        }
    })
    .unwrap();
    assert_eq!(observed.unwrap(), "2");
    assert_eq!(
        std::fs::read_to_string(staging.join("0")).unwrap(),
        "residue-zero\n"
    );
    assert!(staging.join("1").is_dir());
}

#[test]
fn phase_b_rejects_staging_and_retention_exhaustion_without_active_change() {
    let fixture = Fixture::new("phase-b-exhausted");
    let patch = fixture.initialize_and_patch("exhausted");
    let active_bytes = std::fs::read(fixture.active()).unwrap();
    let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
    let staging = fixture.root.join(".semaprax-workspace/staging");
    for ordinal in 0..super::MAX_STAGING_ATTEMPTS {
        std::fs::write(staging.join(ordinal.to_string()), "residue\n").unwrap();
    }
    let error =
        prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {}).unwrap_err();
    assert_eq!(error[0].code, "SPX-G151");
    assert_active_unchanged(&fixture, &active_bytes, active_identity);

    for ordinal in 0..super::MAX_STAGING_ATTEMPTS {
        std::fs::remove_file(staging.join(ordinal.to_string())).unwrap();
    }
    let generations = fixture.root.join(".semaprax-workspace/generations");
    let active_generation = std::fs::read_dir(&generations)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name();
    let mut made = 0usize;
    for ordinal in 0usize..64 {
        let name = format!("{ordinal:064x}");
        if name == active_generation.to_string_lossy() {
            continue;
        }
        std::fs::create_dir(generations.join(name)).unwrap();
        made += 1;
        if made == super::MAX_RETAINED_GENERATIONS - 1 {
            break;
        }
    }
    let error =
        prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {}).unwrap_err();
    assert_eq!(error[0].code, "SPX-G151");
    assert_active_unchanged(&fixture, &active_bytes, active_identity);
}

#[test]
fn phase_b_detects_manifest_file_and_destination_races_without_active_pivot() {
    for (label, point) in [
        ("manifest", GenerationPoint::AfterManifestWrite),
        ("file", GenerationPoint::AfterFilesWrite),
    ] {
        let fixture = Fixture::new(&format!("phase-b-{label}"));
        let patch = fixture.initialize_and_patch(label);
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let error =
            prepare_candidate_generation_with_hook(&fixture.root, &patch, |current, slot, _| {
                if current == point {
                    let path = if label == "manifest" {
                        slot.join("manifest.json")
                    } else {
                        slot.join("files/alpha.spx")
                    };
                    let bytes = std::fs::read(&path).unwrap();
                    std::fs::remove_file(&path).unwrap();
                    std::fs::write(path, bytes).unwrap();
                }
            })
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
    }

    for kind in ["file", "directory"] {
        let fixture = Fixture::new(&format!("phase-b-destination-{kind}"));
        let patch = fixture.initialize_and_patch(&format!("destination_{kind}"));
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let mut foreign = None;
        let error = prepare_candidate_generation_with_hook(
            &fixture.root,
            &patch,
            |point, _, destination| {
                if point == GenerationPoint::DestinationChecked {
                    if kind == "file" {
                        std::fs::write(destination, "foreign-generation\n").unwrap();
                    } else {
                        std::fs::create_dir(destination).unwrap();
                    }
                    foreign = Some(destination.to_path_buf());
                }
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I211");
        let foreign = foreign.unwrap();
        assert_eq!(foreign.is_file(), kind == "file");
        assert_eq!(foreign.is_dir(), kind == "directory");
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
    }
}

#[test]
fn phase_b_corrupt_existing_generation_fails_closed_without_staging_or_active_change() {
    let fixture = Fixture::new("phase-b-corrupt-reuse");
    let patch = fixture.initialize_and_patch("corrupt_reuse");
    let revision =
        prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {}).unwrap();
    let candidate = fixture
        .root
        .join(".semaprax-workspace/generations")
        .join(super::revision_hex(&revision).unwrap());
    std::fs::write(candidate.join("manifest.json"), "{}\n").unwrap();
    let staging = fixture.root.join(".semaprax-workspace/staging");
    let active_bytes = std::fs::read(fixture.active()).unwrap();
    let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
    let error =
        prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {}).unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert_eq!(std::fs::read_dir(staging).unwrap().count(), 0);
    assert_eq!(
        std::fs::read_to_string(candidate.join("manifest.json")).unwrap(),
        "{}\n"
    );
    assert_active_unchanged(&fixture, &active_bytes, active_identity);
}

#[test]
fn phase_b_rejects_extra_phase_inventory_and_preserves_foreign_objects() {
    for kind in ["staging", "generation"] {
        let fixture = Fixture::new(&format!("phase-b-extra-{kind}"));
        let patch = fixture.initialize_and_patch(&format!("extra_{kind}"));
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let mut foreign = None;
        let error = prepare_candidate_generation_with_hook(&fixture.root, &patch, |point, _, _| {
            if point == GenerationPoint::BeforeStageValidation {
                let path = if kind == "staging" {
                    fixture.root.join(".semaprax-workspace/staging/31")
                } else {
                    fixture.root.join(format!(
                        ".semaprax-workspace/generations/{:064x}",
                        65_535usize
                    ))
                };
                if kind == "staging" {
                    std::fs::write(&path, "foreign\n").unwrap();
                } else {
                    std::fs::create_dir(&path).unwrap();
                }
                foreign = Some(path);
            }
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert!(foreign.unwrap().exists());
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
    }
}

#[cfg(unix)]
#[test]
fn phase_b_rejects_staged_symlink_and_hardlink_aliases() {
    use std::os::unix::fs::symlink;

    for kind in ["symlink", "hardlink"] {
        let fixture = Fixture::new(&format!("phase-b-{kind}"));
        let patch = fixture.initialize_and_patch(kind);
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let error =
            prepare_candidate_generation_with_hook(&fixture.root, &patch, |point, slot, _| {
                if point == GenerationPoint::AfterFilesWrite {
                    let alpha = slot.join("files/alpha.spx");
                    std::fs::remove_file(&alpha).unwrap();
                    if kind == "symlink" {
                        symlink(fixture.root.join("alpha.spx"), &alpha).unwrap();
                    } else {
                        std::fs::hard_link(slot.join("files/beta.spx"), &alpha).unwrap();
                    }
                }
            })
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
    }
}

#[test]
fn phase_b_post_publish_corruption_is_reported_without_active_pivot() {
    let fixture = Fixture::new("phase-b-post-publish");
    let patch = fixture.initialize_and_patch("post_publish");
    let active_bytes = std::fs::read(fixture.active()).unwrap();
    let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
    let mut published = None;
    let error =
        prepare_candidate_generation_with_hook(&fixture.root, &patch, |point, _, destination| {
            if point == GenerationPoint::AfterGenerationPublish {
                let manifest = destination.join("manifest.json");
                let bytes = std::fs::read(&manifest).unwrap();
                std::fs::remove_file(&manifest).unwrap();
                std::fs::write(&manifest, bytes).unwrap();
                published = Some(destination.to_path_buf());
            }
        })
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert!(published.unwrap().exists());
    assert_active_unchanged(&fixture, &active_bytes, active_identity);
}

#[test]
fn injected_small_inventory_limit_stops_before_unbounded_collection() {
    let fixture = Fixture::new("inventory-limit");
    for name in ["one", "two", "three"] {
        std::fs::write(fixture.root.join(name), "x").unwrap();
    }
    let error = count_entries_bounded(&fixture.root, 2).unwrap_err();
    assert_eq!(error[0].code, "SPX-G151");
}

#[test]
fn manifest_bound_rejects_expansion() {
    let fact = FileFact {
        path: "alpha.spx".to_owned(),
        module: "workspace.alpha".to_owned(),
        source_graph_schema: "x".repeat(super::MAX_MANIFEST_BYTES),
        source_revision: format!("sha256:{}", "0".repeat(64)),
        source_digest: format!("sha256:{}", "0".repeat(64)),
        source: String::new(),
        declarations: Vec::new(),
        declaration_count: 0,
        callable_count: 0,
        call_count: 0,
    };
    let error = bounded_manifest(&[fact]).unwrap_err();
    assert_eq!(error[0].code, "SPX-G151");
}

#[test]
fn managed_path_count_accepts_exact_and_rejects_over() {
    let render = |count: usize| {
        format!(
            "{{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{}]}}\n",
            (0..count)
                .map(|index| format!("{{\"path\":\"file{index:02}.spx\"}}"))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    assert_eq!(parse_path_set(&render(16)).unwrap().len(), 16);
    assert_eq!(parse_path_set(&render(17)).unwrap_err()[0].code, "SPX-G151");
}

#[test]
fn aggregate_callable_budget_accepts_exact_and_rejects_over_before_hir() {
    let exact = vec![
        module_with_callables("workspace.budget_a", 512),
        module_with_callables("workspace.budget_b", 512),
    ];
    assert_eq!(file_facts(exact, true).unwrap().len(), 2);
    let over = vec![
        module_with_callables("workspace.budget_c", 512),
        module_with_callables("workspace.budget_d", 513),
    ];
    let Err(error) = file_facts(over, true) else {
        panic!("over-limit aggregate callables must fail");
    };
    assert_eq!(error[0].code, "SPX-G151");
}

fn module_with_callables(module: &str, count: usize) -> (String, String) {
    let path = format!("{}.spx", module.replace('.', "_"));
    let mut source = format!("module {module};\n");
    for index in 0..count.saturating_sub(1) {
        source.push_str(&format!("fn helper{index}()->i64{{{index}}}\n"));
    }
    source.push_str("fn main()->i64{0}\n");
    (path.clone(), canonical(&source, &path))
}

#[test]
fn staging_and_retained_inventory_bounds_are_exact() {
    let fixture = Fixture::new("inventory-exact-over");
    let staging = fixture.root.join("staging-bounds");
    std::fs::create_dir(&staging).unwrap();
    for attempt in 0..super::MAX_STAGING_ATTEMPTS {
        std::fs::create_dir(staging.join(attempt.to_string())).unwrap();
    }
    assert_eq!(validate_staging_inventory(&staging).unwrap().0, 32);
    std::fs::create_dir(staging.join("32")).unwrap();
    let Err(error) = validate_staging_inventory(&staging) else {
        panic!("over-limit staging inventory must fail");
    };
    assert!(matches!(error[0].code, "SPX-G151" | "SPX-G153"));

    let retained = fixture.root.join("retained-bounds");
    std::fs::create_dir(&retained).unwrap();
    std::fs::create_dir(retained.join("0".repeat(64))).unwrap();
    std::fs::create_dir(retained.join("1".repeat(64))).unwrap();
    assert_eq!(count_directories_bounded(&retained, 2).unwrap().0, 2);
    let Err(error) = count_directories_bounded(&retained, 1) else {
        panic!("over-limit retained generations must fail");
    };
    assert_eq!(error[0].code, "SPX-G151");
}

#[cfg(windows)]
#[test]
fn canonical_root_rejects_windows_directory_reparse_points() {
    let fixture = Fixture::new("root-reparse");
    let alias = fixture.root.with_extension("reparse");
    std::os::windows::fs::symlink_dir(&fixture.root, &alias).unwrap();
    let error = canonical_root(&alias).unwrap_err();
    assert_eq!(error[0].code, "SPX-I209");
    std::fs::remove_dir(&alias).unwrap();
}

#[cfg(windows)]
#[test]
fn phase_b_rejects_nested_windows_junction_and_preserves_its_target() {
    use std::process::Command;

    let fixture = Fixture::new("phase-b-windows-junction");
    let patch = fixture.initialize_and_patch("windows_junction");
    let active_bytes = std::fs::read(fixture.active()).unwrap();
    let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
    let foreign = fixture.root.join("foreign-junction-target");
    let mut junction = None;
    let error = prepare_candidate_generation_with_hook(
        &fixture.root,
        &patch,
        |point, slot, destination| {
            if point == GenerationPoint::DestinationChecked {
                let files = slot.join("files");
                std::fs::rename(&files, &foreign).unwrap();
                let status = Command::new("cmd")
                    .args(["/C", "mklink", "/J"])
                    .arg(&files)
                    .arg(&foreign)
                    .status()
                    .unwrap();
                assert!(status.success(), "mklink /J failed");
                junction = Some(destination.join("files"));
            }
        },
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G153");
    assert!(foreign.join("alpha.spx").is_file());
    assert_active_unchanged(&fixture, &active_bytes, active_identity);
    let junction = junction.unwrap();
    assert!(super::metadata_is_reparse(
        &std::fs::symlink_metadata(&junction).unwrap()
    ));
    std::fs::remove_dir(junction).unwrap();
    assert!(foreign.join("alpha.spx").is_file());
}

#[cfg(windows)]
#[test]
fn windows_held_handle_relocation_publishes_exact_initializer_and_candidate_maps() {
    let fixture = Fixture::new("windows-relocation-success");
    let patch = fixture.initialize_and_patch("windows_relocation");
    let active_bytes = std::fs::read(fixture.active()).unwrap();
    let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
    let candidate =
        prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {}).unwrap();
    assert!(fixture
        .root
        .join(".semaprax-workspace/generations")
        .join(super::revision_hex(&candidate).unwrap())
        .is_dir());
    assert_active_unchanged(&fixture, &active_bytes, active_identity);
}
