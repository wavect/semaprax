//! `scripts/release-manifest.py` (#167): the canonical, machine-readable,
//! cross-archive release manifest -- version, tag, commit, the required-check
//! inventory, the artifact inventory (platform/size/digest), the prerelease
//! flag, and the changelog-section digest. This is generated evidence, not a
//! publication step: building or checking a manifest here creates no GitHub
//! Release and calls no network API.

use crate::ci_release_gate::RELEASE_BLOCKERS;
use std::fs;
use std::path::Path;
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The manifest's required-check inventory must be parsed from the real CI
/// workflow, not restated: it must exactly equal (as a set, and in count)
/// the same 22-job list `tests/offline_package/ci_release_gate.rs`
/// independently pins from the same `release-gate` `needs:` block.
/// `RELEASE_BLOCKERS`'s own declared order there is not itself the
/// authoritative order (that Rust test only checks membership and count,
/// never sequence) -- the manifest's `required_checks` preserves the real
/// workflow's own `needs:` declaration order instead, which is what
/// `release_manifest_cli_builds_and_checks_against_real_archives` and the
/// synthetic-workflow case in `release_manifest_pure_checks` pin directly.
#[test]
fn required_checks_match_the_pinned_release_gate_inventory_as_a_set() {
    let output = Command::new("python3")
        .args([
            "-c",
            "import importlib.util, json, sys\n\
             spec = importlib.util.spec_from_file_location('rm', 'scripts/release-manifest.py')\n\
             m = importlib.util.module_from_spec(spec)\n\
             spec.loader.exec_module(m)\n\
             workflow = open('.github/workflows/ci.yml', encoding='utf-8').read()\n\
             print(json.dumps(m.parse_required_checks(workflow)))\n",
        ])
        .current_dir(root())
        .output()
        .expect("python3 must run the manifest's required-checks parser");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Vec<String> =
        serde_json::from_str(stdout.trim()).expect("parser must print a JSON array");
    assert_eq!(
        parsed.len(),
        RELEASE_BLOCKERS.len(),
        "release-manifest.py's required-check count drifted from the pinned gate inventory"
    );
    let mut parsed_sorted = parsed.clone();
    parsed_sorted.sort_unstable();
    let mut pinned_sorted: Vec<&str> = RELEASE_BLOCKERS.to_vec();
    pinned_sorted.sort_unstable();
    assert_eq!(
        parsed_sorted, pinned_sorted,
        "release-manifest.py's required-check inventory drifted from the pinned gate inventory"
    );
    // The real workflow's `needs:` list must never repeat a job.
    let mut deduped = parsed.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(
        deduped.len(),
        parsed.len(),
        "a required check was declared more than once"
    );
}

const MANIFEST_PURE_CHECKS: &str = r#"
import importlib.util
spec = importlib.util.spec_from_file_location('rm', 'scripts/release-manifest.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)

# --- job_block / parse_required_checks over a synthetic workflow -----------
WORKFLOW = (
    "jobs:\n"
    "  alpha:\n"
    "    runs-on: ubuntu-latest\n"
    "    steps: []\n"
    "  release-gate:\n"
    "    if: ${{ always() }}\n"
    "    needs:\n"
    "      - alpha\n"
    "      - beta\n"
    "      - gamma\n"
    "    runs-on: ubuntu-24.04\n"
    "    steps: []\n"
    "  release-artifacts:\n"
    "    needs: release-gate\n"
    "    runs-on: ubuntu-latest\n"
)
assert m.parse_required_checks(WORKFLOW) == ["alpha", "beta", "gamma"]

# An empty needs: list must fail closed, not silently produce zero checks.
EMPTY = WORKFLOW.replace(
    "    needs:\n      - alpha\n      - beta\n      - gamma\n", "    needs:\n"
)
try:
    m.parse_required_checks(EMPTY)
    raise AssertionError("empty needs: list must be rejected")
except ValueError as error:
    assert "empty" in str(error)

# A missing release-gate job must fail closed too.
try:
    m.parse_required_checks("jobs:\n  alpha:\n    runs-on: ubuntu-latest\n")
    raise AssertionError("missing release-gate job must be rejected")
except ValueError as error:
    assert "missing `release-gate` job" in str(error)

# --- changelog_section_digest: same section -> same digest, differs on edit -
CHANGELOG = '## 9.9.9 — 2027-01-01\n\nnotes here\n\n## 9.9.8 — 2026-12-01\n\nolder\n'
first = m.changelog_section_digest(CHANGELOG, "9.9.9")
second = m.changelog_section_digest(CHANGELOG, "9.9.9")
assert first == second
assert first.startswith("sha256:")
edited = CHANGELOG.replace("notes here", "notes here, revised")
assert m.changelog_section_digest(edited, "9.9.9") != first
# Digest is over the 9.9.9 section only; a change to the 9.9.8 section must
# not move it.
touched_other = CHANGELOG.replace("older", "older, revised")
assert m.changelog_section_digest(touched_other, "9.9.9") == first

# --- build_manifest: validates version/tag/commit before touching archives --
import tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as scratch:
    scratch = Path(scratch)
    try:
        m.build_manifest("9.9.9", "v9.9.8", "a" * 40, True, WORKFLOW, CHANGELOG, scratch)
        raise AssertionError("tag/version mismatch must be rejected")
    except ValueError as error:
        assert "does not equal v plus the version" in str(error)
    try:
        m.build_manifest("9.9.9", "v9.9.9", "not-hex", True, WORKFLOW, CHANGELOG, scratch)
        raise AssertionError("non-hex commit must be rejected")
    except ValueError as error:
        assert "40 lowercase hexadecimal" in str(error)
    try:
        m.build_manifest("9.9.9", "v9.9.9", "a" * 40, True, WORKFLOW, CHANGELOG, scratch)
        raise AssertionError("missing archives must be rejected")
    except ValueError as error:
        assert "missing release artifact" in str(error)

print("manifest pure checks ok")
"#;

#[test]
fn release_manifest_pure_checks() {
    let output = Command::new("python3")
        .args(["-B", "-c", MANIFEST_PURE_CHECKS])
        .current_dir(root())
        .output()
        .expect("python3 must run the manifest pure checks");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "manifest pure checks ok"
    );
}

/// End-to-end: build three synthetic archives with real per-archive
/// `semaprax.release-artifact.v1` manifests inside (the exact shape
/// `scripts/package-release.sh`/`.ps1` produce), then run the real
/// `scripts/release-manifest.py` CLI over them against the real repository's
/// own `CHANGELOG.md` and `.github/workflows/ci.yml`, and finally `--check`
/// the written manifest against itself (must agree) and against a
/// deliberately altered copy (must disagree, naming the exact field).
#[test]
fn release_manifest_cli_builds_and_checks_against_real_archives() {
    let root = root();
    let version = env!("CARGO_PKG_VERSION");
    let tag = format!("v{version}");
    let commit = "6".repeat(40);
    let scratch = std::env::temp_dir().join(format!(
        "semaprax-release-manifest-cli-{}",
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

    let manifest_text = fs::read_to_string(&manifest_path).expect("manifest must be written");
    assert!(manifest_text.contains("\"schema\": \"semaprax.release-manifest.v1\""));
    assert!(manifest_text.contains(&format!("\"version\": \"{version}\"")));
    assert!(manifest_text.contains(&format!("\"tag\": \"{tag}\"")));
    assert!(manifest_text.contains(&format!("\"commit\": \"{commit}\"")));
    assert!(manifest_text.contains("\"prerelease\": true"));
    assert!(manifest_text.contains("\"changelog_section_digest\": \"sha256:"));
    assert!(manifest_text.contains("\"platform\": \"x86_64-unknown-linux-gnu\""));
    assert!(manifest_text.contains("\"platform\": \"aarch64-apple-darwin\""));
    assert!(manifest_text.contains("\"platform\": \"x86_64-pc-windows-msvc\""));
    for blocker in RELEASE_BLOCKERS {
        assert!(
            manifest_text.contains(&format!("\"{blocker}\"")),
            "manifest lost required check {blocker}"
        );
    }

    // --check against itself: must agree, exit 0.
    let check_ok = Command::new("python3")
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
        .args(["--check"])
        .arg(&manifest_path)
        .current_dir(root)
        .output()
        .expect("release manifest --check must run");
    assert!(
        check_ok.status.success(),
        "self-check must agree: {}",
        String::from_utf8_lossy(&check_ok.stderr)
    );
    assert!(String::from_utf8_lossy(&check_ok.stdout).contains("agrees with recomputed evidence"));

    // --check against a deliberately altered manifest: must disagree, naming
    // the exact field, and exit nonzero.
    let altered_path = scratch.join("altered-manifest.json");
    let altered_text = manifest_text.replace(
        &format!("\"commit\": \"{commit}\""),
        "\"commit\": \"dddddddddddddddddddddddddddddddddddddddd\"",
    );
    assert_ne!(
        altered_text, manifest_text,
        "the substitution must actually apply"
    );
    fs::write(&altered_path, altered_text).expect("altered manifest must be writable");
    let check_bad = Command::new("python3")
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
        .args(["--check"])
        .arg(&altered_path)
        .current_dir(root)
        .output()
        .expect("release manifest --check must run against the altered manifest");
    assert!(
        !check_bad.status.success(),
        "altered manifest must be rejected"
    );
    let stderr = String::from_utf8_lossy(&check_bad.stderr);
    assert!(
        stderr.contains("commit mismatch"),
        "check must name the exact disagreeing field: {stderr}"
    );

    fs::remove_dir_all(&scratch).ok();
}

/// A digest mismatch between an archive's real bytes and a sibling
/// `SHA256SUMS` (the same file `publish-release` generates from the same
/// bytes) must be caught before the manifest is trusted, not silently
/// accepted.
#[test]
fn release_manifest_rejects_a_sha256sums_digest_mismatch() {
    let root = root();
    let scratch = std::env::temp_dir().join(format!(
        "semaprax-release-manifest-sums-{}",
        std::process::id()
    ));
    fs::create_dir_all(&scratch).expect("scratch dir must be creatable");
    let version = env!("CARGO_PKG_VERSION");
    let tag = format!("v{version}");
    let commit = "7".repeat(40);

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
         write_zip(f'semaprax-{{tag}}-x86_64-pc-windows-msvc.zip', 'x86_64-pc-windows-msvc')\n\
         (scratch / 'SHA256SUMS').write_text('{{}}  semaprax-{{}}-x86_64-unknown-linux-gnu.tar.gz\\n'.format('0' * 64, tag), encoding='utf-8')\n"
    );
    let build = Command::new("python3")
        .args(["-c", &build_script])
        .current_dir(root)
        .output()
        .expect("python3 must build synthetic archives and a wrong SHA256SUMS");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let result = Command::new("python3")
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
        .current_dir(root)
        .output()
        .expect("release manifest CLI must run");
    assert!(!result.status.success(), "digest mismatch must be rejected");
    assert!(String::from_utf8_lossy(&result.stderr).contains("digest mismatch"));

    fs::remove_dir_all(&scratch).ok();
}
