//! A disposable-repo dry-run harness for issue #167's required outcome "the
//! complete flow succeeds in a disposable test repository or dry-run
//! harness" and its "[a] simulated failure between artifact upload and
//! Release publication is recoverable without duplicate assets or false
//! docs" required test.
//!
//! `bd777928`'s own commit message recorded that no mid-release failure had
//! been simulated anywhere in this repository; `docs/RELEASE-PROCESS.md`'s
//! "The failure and recovery path" section is a documented manual procedure,
//! not an exercised mechanism. This module exercises it mechanically, using
//! the real `scripts/release-manifest.py`, `scripts/release-notes.py` and
//! `scripts/release-publish-simulate.py` CLIs over a disposable scratch
//! directory built from scratch for each test -- never the real checkout's
//! tags, README, or CHANGELOG.md.
//!
//! `scripts/release-publish-simulate.py` is an explicit, offline simulation
//! of `gh release create`/`gh release view`, not a GitHub client: it never
//! makes a network call, and a record it writes is a fixture, never
//! release-promotion evidence. This harness therefore proves the local
//! tag -> manifest -> artifacts -> (simulated) publish -> reconciliation
//! state machine's mechanics -- duplicate-publish refusal, no-duplicate-asset
//! recovery, and "no claim before the record exists" -- but it does NOT
//! exercise the 22 hosted release-blocker CI jobs themselves (see
//! `.github/workflows/ci.yml`'s `release-gate`), which remain hosted
//! evidence recorded in `docs/RELEASE-PROCESS.md`'s dated evidence sections.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn scratch_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "semaprax-release-dry-run-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).expect("scratch dir must be creatable");
    dir
}

/// Build three synthetic archives (the exact per-archive
/// `semaprax.release-artifact.v1` shape `scripts/package-release.sh`/`.ps1`
/// produce) for `tag`/`version`/`commit` inside `archives_dir`.
fn build_synthetic_archives(archives_dir: &Path, tag: &str, version: &str, commit: &str) {
    let script = format!(
        "import io, json, tarfile, zipfile\n\
         from pathlib import Path\n\
         archives_dir = Path({archives_dir:?})\n\
         version = {version:?}\n\
         tag = {tag:?}\n\
         commit = {commit:?}\n\
         def manifest_bytes(target):\n\
         \treturn json.dumps({{'schema': 'semaprax.release-artifact.v1', 'version': version, 'commit': commit, 'target': target, 'maturity': 'pre-alpha', 'binaries': ['semaprax', 'semapraxd'], 'nonclaims': []}}).encode('utf-8')\n\
         def write_tar(name, target):\n\
         \twith tarfile.open(archives_dir / name, 'w:gz') as archive:\n\
         \t\tdata = manifest_bytes(target)\n\
         \t\tinfo = tarfile.TarInfo(name=f'semaprax-{{tag}}-{{target}}/release-manifest.json')\n\
         \t\tinfo.size = len(data)\n\
         \t\tarchive.addfile(info, io.BytesIO(data))\n\
         def write_zip(name, target):\n\
         \twith zipfile.ZipFile(archives_dir / name, 'w') as archive:\n\
         \t\tarchive.writestr(f'semaprax-{{tag}}-{{target}}/release-manifest.json', manifest_bytes(target))\n\
         write_tar(f'semaprax-{{tag}}-x86_64-unknown-linux-gnu.tar.gz', 'x86_64-unknown-linux-gnu')\n\
         write_tar(f'semaprax-{{tag}}-aarch64-apple-darwin.tar.gz', 'aarch64-apple-darwin')\n\
         write_zip(f'semaprax-{{tag}}-x86_64-pc-windows-msvc.zip', 'x86_64-pc-windows-msvc')\n"
    );
    let build = Command::new("python3")
        .args(["-c", &script])
        .current_dir(root())
        .output()
        .expect("python3 must build synthetic archives");
    assert!(
        build.status.success(),
        "synthetic archive build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
}

fn archive_paths(archives_dir: &Path, tag: &str) -> Vec<PathBuf> {
    vec![
        archives_dir.join(format!("semaprax-{tag}-x86_64-unknown-linux-gnu.tar.gz")),
        archives_dir.join(format!("semaprax-{tag}-aarch64-apple-darwin.tar.gz")),
        archives_dir.join(format!("semaprax-{tag}-x86_64-pc-windows-msvc.zip")),
    ]
}

/// Stage `artifacts-built`: real `release-manifest.py` over the real
/// repository's own `CHANGELOG.md` and `.github/workflows/ci.yml`, applied
/// to the disposable synthetic archives. Returns the manifest path.
fn build_manifest(archives_dir: &Path, tag: &str, version: &str, commit: &str) -> PathBuf {
    let manifest_path = archives_dir.join("release-manifest.json");
    let output = Command::new("python3")
        .args([
            "scripts/release-manifest.py",
            "--version",
            version,
            "--tag",
            tag,
            "--commit",
            commit,
            "--archives-dir",
        ])
        .arg(archives_dir)
        .args(["--output"])
        .arg(&manifest_path)
        .current_dir(root())
        .output()
        .expect("release-manifest.py must run");
    assert!(
        output.status.success(),
        "manifest stage failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    manifest_path
}

fn view_simulated_release(store: &Path, tag: &str) -> std::process::Output {
    Command::new("python3")
        .args(["scripts/release-publish-simulate.py", "view", "--store"])
        .arg(store)
        .args(["--tag", tag])
        .current_dir(root())
        .output()
        .expect("release-publish-simulate.py view must run")
}

fn create_simulated_release(
    store: &Path,
    tag: &str,
    commit: &str,
    manifest_path: &Path,
    notes_path: &Path,
    assets: &[PathBuf],
) -> std::process::Output {
    let mut command = Command::new("python3");
    command
        .args(["scripts/release-publish-simulate.py", "create", "--store"])
        .arg(store)
        .args(["--tag", tag, "--commit", commit, "--manifest"])
        .arg(manifest_path)
        .args(["--notes-file"])
        .arg(notes_path)
        .args(assets)
        .current_dir(root());
    command
        .output()
        .expect("release-publish-simulate.py create must run")
}

/// The full disposable-repo flow: manifest -> notes -> simulated publish ->
/// exactly one release with exactly three assets -> a retried `create`
/// (simulating an operator re-running the job) is refused as a duplicate.
/// This is the "complete flow succeeds in a disposable test repository or
/// dry-run harness" required test.
#[test]
fn dry_run_full_flow_builds_manifest_and_publishes_exactly_once() {
    let version = env!("CARGO_PKG_VERSION");
    let tag = format!("v{version}");
    let commit = "1".repeat(40);
    let scratch = scratch_dir("full-flow");
    let archives_dir = scratch.join("dist");
    fs::create_dir_all(&archives_dir).unwrap();

    build_synthetic_archives(&archives_dir, &tag, version, &commit);
    let manifest_path = build_manifest(&archives_dir, &tag, version, &commit);

    let notes_path = scratch.join("notes.md");
    let notes = Command::new("python3")
        .args(["scripts/release-notes.py", "--version", version])
        .current_dir(root())
        .output()
        .expect("release-notes.py must run");
    assert!(
        notes.status.success(),
        "{}",
        String::from_utf8_lossy(&notes.stderr)
    );
    fs::write(&notes_path, &notes.stdout).unwrap();

    let store = scratch.join("mock-releases.json");
    let assets = archive_paths(&archives_dir, &tag);

    // Before publication: no simulated release exists yet -- the same
    // "artifacts-built, not yet published" shape `candidate_state` in
    // `scripts/release-reconcile.py` calls `tagged-unpublished`.
    let before = view_simulated_release(&store, &tag);
    assert!(
        !before.status.success(),
        "no release must exist before publish"
    );

    let created =
        create_simulated_release(&store, &tag, &commit, &manifest_path, &notes_path, &assets);
    assert!(
        created.status.success(),
        "simulated publish failed: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(String::from_utf8_lossy(&created.stdout).contains("created"));

    let after = view_simulated_release(&store, &tag);
    assert!(after.status.success(), "release must exist after publish");
    let record: serde_json::Value =
        serde_json::from_slice(&after.stdout).expect("view must print JSON");
    assert_eq!(record["tag"], tag);
    assert_eq!(record["commit"], commit);
    assert_eq!(record["prerelease"], serde_json::Value::Bool(true));
    let recorded_assets = record["assets"]
        .as_array()
        .expect("assets must be an array");
    assert_eq!(
        recorded_assets.len(),
        3,
        "exactly three assets, no more, no fewer"
    );

    // A retried `create` (an operator re-running the publish job after it
    // already succeeded) must be refused, not silently duplicate the
    // release or its assets.
    let retried =
        create_simulated_release(&store, &tag, &commit, &manifest_path, &notes_path, &assets);
    assert!(
        !retried.status.success(),
        "a duplicate publish must be refused"
    );
    assert!(String::from_utf8_lossy(&retried.stderr).contains("already exists"));
    let still = view_simulated_release(&store, &tag);
    let record: serde_json::Value = serde_json::from_slice(&still.stdout).unwrap();
    assert_eq!(
        record["assets"].as_array().unwrap().len(),
        3,
        "a refused retry must not have appended duplicate assets"
    );

    fs::remove_dir_all(&scratch).ok();
}

/// Simulates a mid-release failure: artifacts are built and the manifest
/// exists (`artifacts-built`), but the process is interrupted before the
/// simulated Release is ever created -- exactly the gap between
/// `release-artifacts` succeeding and `publish-release` completing that
/// `docs/RELEASE-PROCESS.md`'s recovery section describes. Recovery (a
/// second attempt, as if the underlying failure were fixed and the job
/// re-run) must produce exactly one clean release with no duplicate assets
/// and no residue from the interrupted attempt.
#[test]
fn dry_run_mid_publish_failure_recovers_without_duplicate_assets() {
    let version = env!("CARGO_PKG_VERSION");
    let tag = format!("v{version}");
    let commit = "2".repeat(40);
    let scratch = scratch_dir("mid-failure");
    let archives_dir = scratch.join("dist");
    fs::create_dir_all(&archives_dir).unwrap();

    build_synthetic_archives(&archives_dir, &tag, version, &commit);
    let manifest_path = build_manifest(&archives_dir, &tag, version, &commit);
    let notes_path = scratch.join("notes.md");
    let notes = Command::new("python3")
        .args(["scripts/release-notes.py", "--version", version])
        .current_dir(root())
        .output()
        .expect("release-notes.py must run");
    assert!(notes.status.success());
    fs::write(&notes_path, &notes.stdout).unwrap();
    let store = scratch.join("mock-releases.json");
    let assets = archive_paths(&archives_dir, &tag);

    // --- the crash: artifacts and manifest exist, but publish never ran ----
    // (deliberately not calling create_simulated_release here)
    let observed_before_recovery = view_simulated_release(&store, &tag);
    assert!(
        !observed_before_recovery.status.success(),
        "an interrupted release must leave no simulated Release behind"
    );
    // The store file itself must not exist either: nothing was ever written,
    // matching the real recovery doc's "explicit candidate state, no
    // misleading claim" property (the absence of a doc edit / recorded
    // publication, not a doc edit that races ahead of it).
    assert!(
        !store.exists(),
        "no store file may exist before any publish attempt"
    );

    // --- recovery: fix the underlying failure and retry -------------------
    let recovered =
        create_simulated_release(&store, &tag, &commit, &manifest_path, &notes_path, &assets);
    assert!(
        recovered.status.success(),
        "recovery publish must succeed cleanly: {}",
        String::from_utf8_lossy(&recovered.stderr)
    );

    let after = view_simulated_release(&store, &tag);
    assert!(after.status.success());
    let record: serde_json::Value = serde_json::from_slice(&after.stdout).unwrap();
    let recorded_assets = record["assets"].as_array().unwrap();
    assert_eq!(
        recorded_assets.len(),
        3,
        "recovery must publish exactly the three manifest artifacts, no duplicates"
    );
    let mut names: Vec<&str> = recorded_assets
        .iter()
        .map(|asset| asset["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names.len(),
        3,
        "asset names must be pairwise distinct, not repeated by a partial retry"
    );

    // A second recovery attempt (an operator running the retry twice) is
    // refused the same way an already-successful publish is.
    let repeated =
        create_simulated_release(&store, &tag, &commit, &manifest_path, &notes_path, &assets);
    assert!(!repeated.status.success());
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("already exists"));

    fs::remove_dir_all(&scratch).ok();
}

/// A release attempt whose assets do not match the manifest's own recorded
/// digest/size (a corrupted or substituted archive reaching the publish
/// step) must be refused, not published.
#[test]
fn dry_run_publish_refuses_an_asset_that_disagrees_with_the_manifest() {
    let version = env!("CARGO_PKG_VERSION");
    let tag = format!("v{version}");
    let commit = "3".repeat(40);
    let scratch = scratch_dir("tampered-asset");
    let archives_dir = scratch.join("dist");
    fs::create_dir_all(&archives_dir).unwrap();

    build_synthetic_archives(&archives_dir, &tag, version, &commit);
    let manifest_path = build_manifest(&archives_dir, &tag, version, &commit);
    let notes_path = scratch.join("notes.md");
    fs::write(&notes_path, "tampered-asset dry run notes\n").unwrap();
    let store = scratch.join("mock-releases.json");
    let assets = archive_paths(&archives_dir, &tag);

    // Tamper with one archive's bytes after the manifest already recorded
    // its digest and size.
    let linux_archive = &assets[0];
    let mut tampered = fs::read(linux_archive).unwrap();
    tampered.push(0xFF);
    fs::write(linux_archive, tampered).unwrap();

    let result =
        create_simulated_release(&store, &tag, &commit, &manifest_path, &notes_path, &assets);
    assert!(!result.status.success(), "a tampered asset must be refused");
    assert!(String::from_utf8_lossy(&result.stderr).contains("digest"));
    assert!(
        !store.exists(),
        "a refused publish must not write a store record"
    );

    fs::remove_dir_all(&scratch).ok();
}
