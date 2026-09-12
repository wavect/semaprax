//! `scripts/release-provenance.py` (#168): the `semaprax.release-provenance.v1`
//! document built from an already-built `semaprax.release-manifest.v1`
//! (#167), plus `src/release_provenance.rs`'s independent Rust-side binding
//! verification. Building or verifying here creates no GitHub Release, signs
//! nothing, and grants no authority -- see that module's doc comment and
//! `docs/RELEASE-SIGNING-POLICY-V1.md`.

use std::fs;
use std::path::Path;
use std::process::Command;

use semaprax::release_provenance::{
    verify_manifest_artifacts_on_disk, verify_provenance_binds_manifest, ARCHIVE_PLATFORMS,
    TRUSTED_REPOSITORY, TRUSTED_WORKFLOW_PATH,
};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// `src/release_provenance.rs`'s [`ARCHIVE_PLATFORMS`] cannot import
/// `scripts/release-reconcile.py`'s `ARCHIVE_TARGETS` (a Python tuple), so
/// both independently name the three admitted release platforms. Cross-check
/// they agree, the same way `ci_release_gate.rs` cross-checks its
/// `RELEASE_BLOCKERS` against `release-manifest.py`'s independent parse of
/// the CI workflow.
#[test]
fn archive_platforms_match_release_reconcile_targets() {
    let output = Command::new("python3")
        .args([
            "-c",
            "import importlib.util, json\n\
             spec = importlib.util.spec_from_file_location('rr', 'scripts/release-reconcile.py')\n\
             m = importlib.util.module_from_spec(spec)\n\
             spec.loader.exec_module(m)\n\
             print(json.dumps(sorted(target for target, _ in m.ARCHIVE_TARGETS)))\n",
        ])
        .current_dir(root())
        .output()
        .expect("python3 must run release-reconcile.py's ARCHIVE_TARGETS");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Vec<String> = serde_json::from_str(String::from_utf8_lossy(&output.stdout).trim())
        .expect("parser must print a JSON array");
    let mut expected: Vec<&str> = ARCHIVE_PLATFORMS.to_vec();
    expected.sort_unstable();
    assert_eq!(parsed, expected);
}

/// `src/release_provenance.rs`'s `TRUSTED_REPOSITORY`/`TRUSTED_WORKFLOW_PATH`
/// must exactly match `scripts/release-provenance.py`'s constants of the
/// same name, and both must exactly match the literal values recorded in
/// `docs/RELEASE-SIGNING-POLICY-V1.md`'s "Trusted identity policy v1" table.
/// A drift in any one of the three would mean the enforced code, the
/// document-generating script, and the human-reviewable policy no longer
/// agree on what "trusted" means.
#[test]
fn trusted_identity_constants_match_the_script_and_the_policy_document() {
    let output = Command::new("python3")
        .args([
            "-c",
            "import importlib.util, json\n\
             spec = importlib.util.spec_from_file_location('rp', 'scripts/release-provenance.py')\n\
             m = importlib.util.module_from_spec(spec)\n\
             spec.loader.exec_module(m)\n\
             print(json.dumps({'repository': m.TRUSTED_REPOSITORY, 'workflow_path': m.TRUSTED_WORKFLOW_PATH}))\n",
        ])
        .current_dir(root())
        .output()
        .expect("python3 must run release-provenance.py's trusted-identity constants");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&output.stdout).trim())
            .expect("parser must print a JSON object");
    assert_eq!(parsed["repository"], TRUSTED_REPOSITORY);
    assert_eq!(parsed["workflow_path"], TRUSTED_WORKFLOW_PATH);

    let policy = fs::read_to_string(root().join("docs/RELEASE-SIGNING-POLICY-V1.md"))
        .expect("docs/RELEASE-SIGNING-POLICY-V1.md must exist");
    assert!(
        policy.contains(TRUSTED_REPOSITORY),
        "policy document must record the exact trusted repository string"
    );
    assert!(
        policy.contains(TRUSTED_WORKFLOW_PATH),
        "policy document must record the exact trusted workflow path string"
    );
    assert!(
        policy.contains("https://token.actions.githubusercontent.com"),
        "policy document must record the exact trusted OIDC issuer string"
    );
}

/// End-to-end: build three synthetic archives and a real
/// `scripts/release-manifest.py` manifest (mirroring
/// `release_manifest_cli_builds_and_checks_against_real_archives`), then run
/// the real `scripts/release-provenance.py` CLI over it, and finally verify
/// the written provenance document with the independent Rust module -- both
/// that it binds to the manifest and that the manifest's own artifact
/// digests agree with the synthetic archives' real bytes.
#[test]
fn release_provenance_cli_builds_and_rust_module_accepts_it() {
    let root = root();
    let version = env!("CARGO_PKG_VERSION");
    let tag = format!("v{version}");
    let commit = "6".repeat(40);
    let scratch = std::env::temp_dir().join(format!(
        "semaprax-release-provenance-cli-{}",
        std::process::id()
    ));
    fs::create_dir_all(&scratch).expect("scratch dir must be creatable");

    let build_script = format!(
        "import io, json, tarfile, zipfile\n\
         from pathlib import Path\n\
         scratch = Path({scratch:?})\n\
         version = {version:?}\n\
         tag = {tag:?}\n\
         commit = {commit:?}\n\
         def manifest_bytes(target):\n\
         \treturn json.dumps({{'schema': 'semaprax.release-artifact.v1', 'version': version, 'commit': commit, 'target': target, 'maturity': 'pre-alpha', 'binaries': ['semaprax', 'semapraxd'], 'nonclaims': []}}).encode('utf-8')\n\
         def write_tar(name, target):\n\
         \twith tarfile.open(scratch / name, 'w:gz') as archive:\n\
         \t\tdata = manifest_bytes(target)\n\
         \t\tinfo = tarfile.TarInfo(name=f'semaprax-{{tag}}-{{target}}/release-manifest.json')\n\
         \t\tinfo.size = len(data)\n\
         \t\tarchive.addfile(info, io.BytesIO(data))\n\
         def write_zip(name, target):\n\
         \twith zipfile.ZipFile(scratch / name, 'w') as archive:\n\
         \t\tarchive.writestr(f'semaprax-{{tag}}-{{target}}/release-manifest.json', manifest_bytes(target))\n\
         write_tar(f'semaprax-{{tag}}-x86_64-unknown-linux-gnu.tar.gz', 'x86_64-unknown-linux-gnu')\n\
         write_tar(f'semaprax-{{tag}}-aarch64-apple-darwin.tar.gz', 'aarch64-apple-darwin')\n\
         write_zip(f'semaprax-{{tag}}-x86_64-pc-windows-msvc.zip', 'x86_64-pc-windows-msvc')\n"
    );
    let build = Command::new("python3")
        .args(["-c", &build_script])
        .current_dir(root)
        .output()
        .expect("python3 must build synthetic archives");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let manifest_path = scratch.join("release-manifest.json");
    let build_manifest = Command::new("python3")
        .args([
            "scripts/release-manifest.py",
            "--version",
            version,
            "--tag",
            &tag,
            "--commit",
            &commit,
            "--archives-dir",
        ])
        .arg(&scratch)
        .args(["--output"])
        .arg(&manifest_path)
        .current_dir(root)
        .output()
        .expect("release manifest CLI must run");
    assert!(
        build_manifest.status.success(),
        "manifest build failed: {}",
        String::from_utf8_lossy(&build_manifest.stderr)
    );

    let workflow_identity = format!("{TRUSTED_REPOSITORY}/{TRUSTED_WORKFLOW_PATH}@refs/tags/{tag}");
    let provenance_path = scratch.join("release-provenance.json");
    let build_provenance = Command::new("python3")
        .args(["scripts/release-provenance.py", "--manifest"])
        .arg(&manifest_path)
        .args([
            "--workflow-identity",
            &workflow_identity,
            "--run-id",
            "424242",
            "--run-attempt",
            "1",
            "--rustc-version",
            "1.88.0",
            "--host-class",
            "github-hosted-ubuntu-24.04",
            "--output",
        ])
        .arg(&provenance_path)
        .current_dir(root)
        .output()
        .expect("release provenance CLI must run");
    assert!(
        build_provenance.status.success(),
        "provenance build failed: {}",
        String::from_utf8_lossy(&build_provenance.stderr)
    );

    let manifest_bytes = fs::read(&manifest_path).expect("manifest must be written");
    let provenance_bytes = fs::read(&provenance_path).expect("provenance must be written");

    verify_provenance_binds_manifest(&provenance_bytes, &manifest_bytes)
        .expect("a provenance document built from this exact manifest must bind to it");
    verify_manifest_artifacts_on_disk(&manifest_bytes, &scratch)
        .expect("the manifest's artifact digests must agree with the real synthetic archive bytes");

    // --check against itself must agree.
    let check_ok = Command::new("python3")
        .args(["scripts/release-provenance.py", "--manifest"])
        .arg(&manifest_path)
        .args([
            "--workflow-identity",
            &workflow_identity,
            "--run-id",
            "424242",
            "--run-attempt",
            "1",
            "--rustc-version",
            "1.88.0",
            "--host-class",
            "github-hosted-ubuntu-24.04",
            "--check",
        ])
        .arg(&provenance_path)
        .current_dir(root)
        .output()
        .expect("release provenance --check must run");
    assert!(
        check_ok.status.success(),
        "self-check must agree: {}",
        String::from_utf8_lossy(&check_ok.stderr)
    );

    // A single mutated byte in the on-disk provenance document must be
    // rejected by the independent Rust module (it re-derives the manifest
    // digest and every bound field from the exact bytes under test, not from
    // a cached struct).
    let mut tampered = provenance_bytes.clone();
    let position = tampered
        .windows(commit.len())
        .position(|window| window == commit.as_bytes())
        .expect("provenance must contain the commit literal");
    tampered[position] = if tampered[position] == b'6' {
        b'7'
    } else {
        b'6'
    };
    verify_provenance_binds_manifest(&tampered, &manifest_bytes)
        .expect_err("a single mutated byte in the on-disk provenance document must be rejected");

    fs::remove_dir_all(&scratch).ok();
}

/// A `--workflow-identity` that does not match the pinned trusted repository
/// and workflow path for the requested tag must be rejected before any
/// document is written -- this is the "wrong repository/workflow identity"
/// hostile case applied to the *builder*, not only the verifier.
#[test]
fn release_provenance_cli_rejects_a_mismatched_workflow_identity() {
    let root = root();
    let scratch = std::env::temp_dir().join(format!(
        "semaprax-release-provenance-bad-identity-{}",
        std::process::id()
    ));
    fs::create_dir_all(&scratch).expect("scratch dir must be creatable");
    let manifest_path = scratch.join("release-manifest.json");
    fs::write(
        &manifest_path,
        r#"{
  "schema": "semaprax.release-manifest.v1",
  "version": "9.9.9",
  "tag": "v9.9.9",
  "commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "prerelease": true,
  "required_checks": ["alpha"],
  "changelog_section_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "artifacts": [{"name": "x", "platform": "x86_64-unknown-linux-gnu", "size": 1, "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]
}"#,
    )
    .expect("manifest fixture must be writable");

    let result = Command::new("python3")
        .args(["scripts/release-provenance.py", "--manifest"])
        .arg(&manifest_path)
        .args([
            "--workflow-identity",
            "attacker/semaprax/.github/workflows/ci.yml@refs/tags/v9.9.9",
            "--run-id",
            "1",
            "--run-attempt",
            "1",
            "--rustc-version",
            "1.88.0",
            "--host-class",
            "github-hosted-ubuntu-24.04",
        ])
        .current_dir(root)
        .output()
        .expect("release provenance CLI must run");
    assert!(
        !result.status.success(),
        "a mismatched workflow identity must be rejected"
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("does not match the expected"));

    fs::remove_dir_all(&scratch).ok();
}

/// An unrecognized `--host-class` must be rejected by argument parsing
/// itself (a closed vocabulary, not a free-form string).
#[test]
fn release_provenance_cli_rejects_an_unknown_host_class() {
    let root = root();
    let scratch = std::env::temp_dir().join(format!(
        "semaprax-release-provenance-bad-host-{}",
        std::process::id()
    ));
    fs::create_dir_all(&scratch).expect("scratch dir must be creatable");
    let manifest_path = scratch.join("release-manifest.json");
    fs::write(
        &manifest_path,
        r#"{
  "schema": "semaprax.release-manifest.v1",
  "version": "9.9.9",
  "tag": "v9.9.9",
  "commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "prerelease": true,
  "required_checks": ["alpha"],
  "changelog_section_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "artifacts": [{"name": "x", "platform": "x86_64-unknown-linux-gnu", "size": 1, "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]
}"#,
    )
    .expect("manifest fixture must be writable");

    let result = Command::new("python3")
        .args(["scripts/release-provenance.py", "--manifest"])
        .arg(&manifest_path)
        .args([
            "--workflow-identity",
            &format!("{TRUSTED_REPOSITORY}/{TRUSTED_WORKFLOW_PATH}@refs/tags/v9.9.9"),
            "--run-id",
            "1",
            "--run-attempt",
            "1",
            "--rustc-version",
            "1.88.0",
            "--host-class",
            "my-laptop",
        ])
        .current_dir(root)
        .output()
        .expect("release provenance CLI must run");
    assert!(
        !result.status.success(),
        "an unrecognized host class must be rejected"
    );

    fs::remove_dir_all(&scratch).ok();
}
