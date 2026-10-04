//! Standalone lockfile freshness.
//!
//! Several manifests outside the root workspace depend on `semaprax` by path:
//! the example projects and the isolated platform-test runners. Each carries
//! its own committed `Cargo.lock`. Adding a dependency to the root crate
//! therefore invalidates every one of them, and nothing in an ordinary local
//! build notices: the root workspace resolves fine, the tests pass, and the
//! break only appears on a runner, where `cargo fetch --locked` and
//! `cargo-deny --locked` fail before anything is compiled.
//!
//! That has now happened three times — `1232e985` (a new crate), `9be496e5`
//! (ed25519-dalek) and `a643f39f` (the Sigstore verification tree) — and each
//! time it reddened most of CI for a reason unrelated to the change under
//! test. This gate moves the discovery to the working tree, in seconds.
//!
//! The check is the same one the workflows perform:
//!
//!     cargo metadata --locked --offline --manifest-path <manifest>
//!
//! To fix a failure, refresh the named lockfile minimally and commit it:
//!
//!     cargo fetch --manifest-path <manifest>
//!
//! This gate deliberately does not link the compiler; like `module_size`, it
//! reads the tree and shells out.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Manifests that cannot be resolved standalone, with the reason.
///
/// An entry here is a structural impossibility, never a convenience. Each is
/// asserted to still be unresolvable, so an entry that becomes resolvable
/// fails the gate rather than quietly excusing a real manifest. A stale
/// lockfile can never be hidden by adding it here: that would be weakening a
/// gate to make it pass.
const NOT_RESOLVABLE_STANDALONE: &[(&str, &str)] = &[
    (
        "examples/frame-payload-rust/Cargo.toml",
        "depends on examples/frame-payload-generated-sdk/, which the gate generates \
         and which is absent from a fresh checkout; no workflow fetches it --locked",
    ),
    (
        "crates/semaprax-native-rust-interop-builder/tests/fixtures/rich-rust-missing-vendor-fixture/Cargo.toml",
        "deliberately names the unavailable missing-vendor package",
    ),
    (
        "crates/semaprax-native-rust-interop-builder/tests/fixtures/rich-rust-shape-fixture/Cargo.toml",
        "deliberately names the unavailable shape-dual package",
    ),
    (
        "crates/semaprax-native-rust-interop-builder/tests/fixtures/rich-rust-vendored-fixture/Cargo.toml",
        "deliberately names the unavailable fixture-vendor package",
    ),
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Every committed `Cargo.lock` outside the root workspace, in stable order.
fn standalone_manifests(root: &Path) -> Vec<String> {
    let listed = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "*Cargo.lock"])
        .output()
        .expect("git ls-files must run from the repository root");
    assert!(
        listed.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&listed.stderr)
    );

    let mut manifests: Vec<String> = String::from_utf8(listed.stdout)
        .expect("git prints UTF-8 paths here")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        // The root lock belongs to the workspace this test runs in.
        .filter(|lock| lock.ends_with("/Cargo.lock"))
        .map(|lock| lock.replace("Cargo.lock", "Cargo.toml"))
        .collect();
    manifests.sort();
    manifests
}

fn resolves_locked_and_offline(root: &Path, manifest: &str) -> Result<(), String> {
    let resolved = Command::new(env!("CARGO"))
        .arg("metadata")
        .args(["--locked", "--offline", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(root.join(manifest))
        .output()
        .expect("cargo must be invocable");
    if resolved.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&resolved.stderr).trim().to_owned())
}

#[test]
fn every_standalone_lockfile_resolves_against_its_manifest() {
    let root = repository_root();
    let manifests = standalone_manifests(&root);
    assert!(
        !manifests.is_empty(),
        "no standalone lockfiles were discovered; this gate would pass over \
         nothing. Check that `git ls-files '*Cargo.lock'` still works here."
    );

    let mut stale = Vec::new();
    let mut checked = 0usize;
    for manifest in &manifests {
        if NOT_RESOLVABLE_STANDALONE
            .iter()
            .any(|(excused, _)| excused == manifest)
        {
            continue;
        }
        checked += 1;
        if let Err(reason) = resolves_locked_and_offline(&root, manifest) {
            stale.push(format!("  {manifest}\n    {reason}"));
        }
    }

    assert!(
        checked > 0,
        "every discovered manifest was excused; this gate would pass over nothing"
    );
    assert!(
        stale.is_empty(),
        "{} standalone lockfile(s) are stale against their manifest.\n\n{}\n\n\
         A root-crate dependency change invalidates every standalone lock. \
         Refresh each one minimally and commit it:\n\n{}\n",
        stale.len(),
        stale.join("\n"),
        stale
            .iter()
            .filter_map(|entry| entry.split_whitespace().next())
            .map(|manifest| format!("    cargo fetch --manifest-path {manifest}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn every_excused_manifest_is_still_genuinely_unresolvable() {
    let root = repository_root();
    for (manifest, reason) in NOT_RESOLVABLE_STANDALONE {
        assert!(
            root.join(manifest).exists(),
            "{manifest} is excused from the lockfile gate but no longer exists; \
             drop the entry"
        );
        assert!(
            resolves_locked_and_offline(&root, manifest).is_err(),
            "{manifest} is excused from the lockfile gate as: {reason}\n\
             It now resolves standalone, so the excuse is obsolete. Remove the \
             entry so the manifest is checked like every other one."
        );
    }
}

#[test]
fn the_discovered_set_covers_every_manifest_the_workflows_fetch_locked() {
    let root = repository_root();
    let workflow = std::fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .expect("the CI workflow must be readable");
    let discovered = standalone_manifests(&root);

    let mut unknown = Vec::new();
    for line in workflow.lines() {
        let Some(rest) = line.split("--manifest-path").nth(1) else {
            continue;
        };
        if !line.contains("--locked") {
            continue;
        }
        let manifest = rest.split_whitespace().next().unwrap_or_default();
        if manifest.is_empty() || manifest.contains("${{") {
            continue;
        }
        if !discovered.iter().any(|known| known == manifest) {
            unknown.push(manifest.to_owned());
        }
    }
    unknown.sort();
    unknown.dedup();

    assert!(
        unknown.is_empty(),
        "the CI workflow fetches these manifests with --locked, but they carry \
         no committed lockfile this gate can check:\n  {}\n\
         Commit the lockfile, or the runner resolves it from the network.",
        unknown.join("\n  ")
    );
}
