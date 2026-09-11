use std::fs;
use std::path::Path;
use std::process::Command;

fn read(path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn job<'a>(workflow: &'a str, name: &str) -> &'a str {
    let marker = format!("  {name}:\n");
    let tail = workflow
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing `{name}` job"))
        .1;
    tail.match_indices('\n')
        .find_map(|(end, _)| {
            let line = tail[end + 1..].lines().next()?;
            (line.starts_with("  ") && !line.starts_with("    ") && line.ends_with(':'))
                .then_some(&tail[..end])
        })
        .unwrap_or(tail)
}

#[test]
fn workspace_ci_keeps_bounded_test_executables_and_fail_fast_coverage() {
    let workflow = read(".github/workflows/ci.yml");
    // ci_msrv_sharding_contract independently checks the router's actual
    // workspace inventory and exact Cargo selectors, including shared names.
    for (name, test_command) in [
        (
            "verify-tests",
            "python3 scripts/ci-msrv.py --label \"Rust $RUNNER_OS\" --shard \"${{ matrix.shard }}\"",
        ),
        (
            "msrv",
            "python3 scripts/ci-msrv.py --shard \"${{ matrix.shard }}\"",
        ),
    ] {
        let selected = job(&workflow, name);
        assert!(
            selected.contains("CARGO_PROFILE_DEV_DEBUG: \"0\""),
            "{name}"
        );
        assert!(
            selected.contains("CARGO_PROFILE_TEST_DEBUG: \"0\""),
            "{name}"
        );
        assert!(selected.contains(test_command), "{name}");
        assert!(!selected.contains("--no-fail-fast"));
        assert!(!selected.contains("continue-on-error"));
    }
}

#[test]
fn tag_artifacts_are_exact_blocking_children_of_the_release_gate() {
    let workflow = read(".github/workflows/ci.yml");
    let artifacts = job(&workflow, "release-artifacts");
    for exact in [
        // Written out rather than relying on GitHub's undocumented implicit
        // `success()` insertion on a bare `if:` with no status function: this
        // job must never build or upload a tag's artifacts unless the
        // exact-tag `release-gate` already succeeded in this same run.
        "if: ${{ success() && startsWith(github.ref, 'refs/tags/v') }}",
        "needs: release-gate",
        // The gate binds `github.sha` to the checked-out commit once, in its
        // own job; this re-derives that same fact locally in the job that
        // actually builds the artifacts, so a future checkout override (a
        // stray `ref:`) cannot silently bind the built archive to a
        // different commit than the gate verified.
        "test \"$(git rev-parse HEAD)\" = \"$GITHUB_SHA\"",
        "timeout-minutes: 30",
        "fail-fast: false",
        "os: ubuntu-24.04\n            target: x86_64-unknown-linux-gnu\n            extension: tar.gz",
        "os: macos-15\n            target: aarch64-apple-darwin\n            extension: tar.gz",
        "os: windows-2025\n            target: x86_64-pc-windows-msvc\n            extension: zip",
        "toolchain: 1.97.1",
        "scripts/package-release.sh \"$GITHUB_REF_NAME\" \"$GITHUB_SHA\" \"${{ matrix.target }}\" dist",
        "scripts/package-release.ps1 -Tag $env:GITHUB_REF_NAME -Commit $env:GITHUB_SHA -Target \"${{ matrix.target }}\" -OutputRoot dist",
        "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
        "if-no-files-found: error",
        "compression-level: 0",
    ] {
        assert!(artifacts.contains(exact), "artifact job lost: {exact}");
    }
    for forbidden in [
        "continue-on-error",
        "retry",
        "contents: write",
        "permissions:",
    ] {
        assert!(
            !artifacts.contains(forbidden),
            "artifact builder gained forbidden behavior: {forbidden}"
        );
    }
}

#[test]
fn publication_waits_for_all_artifacts_and_owns_the_only_write_authority() {
    let workflow = read(".github/workflows/ci.yml");
    let publish = job(&workflow, "publish-release");
    for exact in [
        "if: ${{ startsWith(github.ref, 'refs/tags/v') && success() }}",
        "      - release-gate",
        "      - release-artifacts",
        "actions: read",
        "contents: write",
        // The job that actually acquires `contents: write` and calls `gh
        // release create` re-derives, in its own job, the same fact
        // `release-gate` verified about itself: the checkout is the exact
        // commit GitHub reports for this run, not a `ref:`-overridden one.
        // This is what makes "the gate's commit binding is dropped" a local
        // test failure here rather than only inside `release-gate`.
        "test \"$(git rev-parse HEAD)\" = \"$GITHUB_SHA\"",
        "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
        "pattern: release-*",
        "merge-multiple: true",
        "sha256sum \"${archives[@]}\"",
        "gh release create \"$GITHUB_REF_NAME\"",
        "python3 scripts/release-notes.py --version \"$version\"",
        "--notes-file \"$RUNNER_TEMP/release-notes.md\"",
        "--verify-tag",
        "--prerelease",
    ] {
        assert!(publish.contains(exact), "publication job lost: {exact}");
    }
    assert_eq!(workflow.matches("contents: write").count(), 1);
    for archive in [
        "semaprax-v$version-x86_64-unknown-linux-gnu.tar.gz",
        "semaprax-v$version-aarch64-apple-darwin.tar.gz",
        "semaprax-v$version-x86_64-pc-windows-msvc.zip",
    ] {
        assert!(publish.contains(archive));
    }
    for forbidden in [
        "continue-on-error",
        "retry",
        "always()",
        "failure()",
        "cancelled()",
    ] {
        assert!(
            !publish.contains(forbidden),
            "publisher must fail closed: {forbidden}"
        );
    }
}

#[test]
fn release_automation_checks_version_surfaces_and_renders_only_one_changelog_bucket() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let version = env!("CARGO_PKG_VERSION");
    let check = Command::new("python3")
        .args([
            "scripts/prepare-release.py",
            "--check",
            "--version",
            version,
        ])
        .env("PYTHONUTF8", "0")
        .env("PYTHONIOENCODING", "utf-8")
        .current_dir(root)
        .output()
        .expect("release preparation checker must run");
    assert!(
        check.status.success(),
        "release preparation checker failed: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    let check = String::from_utf8(check.stdout).expect("checker output must be UTF-8");
    assert!(check.starts_with(&format!("release surfaces agree on v{version} (")));
    assert!(check.trim_end().ends_with(')'));

    let notes = Command::new("python3")
        .args(["scripts/release-notes.py", "--version", version])
        .env("PYTHONUTF8", "0")
        .env("PYTHONIOENCODING", "utf-8")
        .current_dir(root)
        .output()
        .expect("release notes renderer must run");
    assert!(
        notes.status.success(),
        "release notes renderer failed: {}",
        String::from_utf8_lossy(&notes.stderr)
    );
    let notes = String::from_utf8(notes.stdout).expect("release notes must be UTF-8");
    let title = format!("SEMAPRAX v{version} is pre-alpha research software.");
    // Three sampled entries of the current bucket, taken from the top, middle,
    // and bottom of its section, plus the fixed frame. Samples are re-picked
    // each release; the point they hold is that the renderer emits this
    // bucket's own content, whole.
    for exact in [
        title.as_str(),
        "## Changes",
        "Public Generic Type Grammar v1",
        "Add the bundled `std.env.policy` package",
        "Extended Exact Program Context v2",
        "These unsigned archives are not notarized",
        "SHA-256 checksums are integrity facts, not signatures.",
    ] {
        assert!(notes.contains(exact), "release notes lost: {exact}");
    }
    // Every other bucket stays out, including the one immediately before this
    // release: a renderer that walked past its section would pick that up
    // first.
    for other in ["## 0.4.0", "## 0.3.5", "## Unreleased"] {
        assert!(
            !notes.contains(other),
            "release notes leaked another bucket: {other}"
        );
    }
}

/// #167 required outcome: "Oversized release notes are detected before
/// publication and produce the documented bounded summary." A prior fix
/// (the v0.4.0 hotfix in the history of this file) only special-cased the
/// literal version string `"0.4.0"`, so this exact requirement passed by
/// accident for one past tag and would have silently regressed -- an
/// oversized section under any OTHER version rendered whole, over the
/// GitHub 125,000-byte limit, with no truncation and no test able to catch
/// it. This drives the renderer over a synthetic changelog via
/// `--changelog` so the behaviour is pinned independently of which real
/// version happens to be tagged next, and independently of `CHANGELOG.md`'s
/// current contents.
#[test]
fn release_notes_bounds_an_oversized_section_for_any_version() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = std::env::temp_dir().join(format!(
        "semaprax-release-notes-bound-{}",
        std::process::id()
    ));
    fs::create_dir_all(&scratch).expect("scratch dir must be creatable");
    let oversized_changelog = scratch.join("OVERSIZED-CHANGELOG.md");
    // One line repeated past the 118,000-char bound the renderer applies,
    // followed by a distinctive tail line that must NOT survive truncation.
    let mut body = String::from("## 9.9.9 — 2027-01-01\n\n");
    while body.len() < 120_000 {
        body.push_str("- a synthetic changelog line that pads this section\n");
    }
    body.push_str("- TAIL LINE THAT MUST BE TRUNCATED AWAY\n");
    body.push_str("## 9.9.8 — 2026-12-01\n\nolder notes\n");
    fs::write(&oversized_changelog, &body).expect("synthetic changelog must be writable");

    let oversized = Command::new("python3")
        .args([
            "scripts/release-notes.py",
            "--version",
            "9.9.9",
            "--changelog",
        ])
        .arg(&oversized_changelog)
        .current_dir(root)
        .output()
        .expect("release notes renderer must run over a synthetic changelog");
    assert!(
        oversized.status.success(),
        "renderer must succeed by truncating, not fail: {}",
        String::from_utf8_lossy(&oversized.stderr)
    );
    let oversized = String::from_utf8(oversized.stdout).expect("release notes must be UTF-8");
    assert!(
        oversized.len() < 125_000,
        "bounded release notes must stay under GitHub's release-body limit, got {} bytes",
        oversized.len()
    );
    assert!(
        oversized.contains("truncated for GitHub's 125,000-byte release-note limit"),
        "oversized notes must carry the documented bounded-summary notice"
    );
    assert!(
        oversized.contains("see CHANGELOG.md for the complete v9.9.9 notes"),
        "the bounded notice must name the exact truncated version"
    );
    assert!(
        !oversized.contains("TAIL LINE THAT MUST BE TRUNCATED AWAY"),
        "content past the bound must actually be cut, not merely flagged"
    );

    // Positive control: a section under the bound is rendered whole, with no
    // truncation notice at all -- proving the two cases actually diverge.
    let small_changelog = scratch.join("SMALL-CHANGELOG.md");
    fs::write(
        &small_changelog,
        "## 9.9.9 — 2027-01-01\n\nsmall notes only\n\n## 9.9.8 — 2026-12-01\n\nolder\n",
    )
    .expect("small synthetic changelog must be writable");
    let small = Command::new("python3")
        .args([
            "scripts/release-notes.py",
            "--version",
            "9.9.9",
            "--changelog",
        ])
        .arg(&small_changelog)
        .current_dir(root)
        .output()
        .expect("release notes renderer must run over a small synthetic changelog");
    assert!(small.status.success());
    let small = String::from_utf8(small.stdout).expect("release notes must be UTF-8");
    assert!(small.contains("small notes only"));
    assert!(!small.contains("truncated for GitHub's"));

    fs::remove_dir_all(&scratch).ok();
}

#[test]
fn both_packagers_bind_version_commit_manifest_inventory_and_smoke() {
    let unix = read("scripts/package-release.sh");
    let windows = read("scripts/package-release.ps1");
    for (name, source) in [("Unix", unix.as_str()), ("Windows", windows.as_str())] {
        for exact in [
            "semaprax.release-artifact.v1",
            "production-ready",
            "stable language ABI",
            "stable public protocol",
            "safety-critical suitability",
            "pre-alpha",
            "release-manifest.json",
            "smoke/meaning.spx",
            "semaprax.version.v1",
            "semaprax",
            "semapraxd",
            "LICENSE",
            "README.md",
            "--version",
            "version --json",
            " check ",
            " run ",
            "cargo build --locked --release",
            "-p semaprax-toolchain",
            "--bin semaprax-full",
        ] {
            assert!(source.contains(exact), "{name} packager lost: {exact}");
        }
        for forbidden in ["unsafe", "retry", "notarize", "codesign", "signtool"] {
            assert!(
                !source.contains(forbidden),
                "{name} packager gained: {forbidden}"
            );
        }
    }
    assert!(unix.contains("*[!0-9a-f]*"));
    assert!(windows.contains("^[0-9a-f]{40}$"));
    assert!(windows.contains("[System.IO.Compression.ZipFile]::CreateFromDirectory("));
    assert!(windows.contains("[System.IO.Compression.CompressionLevel]::Optimal"));
    assert!(!windows.contains("Compress-Archive"));
}

#[test]
fn windows_packager_gives_literal_zip_extraction_sole_smoke_root_creation() {
    let windows = read("scripts/package-release.ps1");
    assert!(windows.contains("[System.IO.Compression.ZipFile]::ExtractToDirectory("));
    assert!(!windows.contains("New-Item -ItemType Directory -Path $smokeRoot"));
    let absence_check = windows
        .find("foreach ($path in @($packageRoot, $archive, $smokeRoot, $buildRoot))")
        .expect("smoke extraction root must be checked for absence");
    let extraction = windows
        .find("[System.IO.Compression.ZipFile]::ExtractToDirectory(")
        .expect("literal ZIP extraction must be present");
    assert!(absence_check < extraction);
}

#[test]
fn release_documentation_preserves_all_nonclaims() {
    let docs = read("docs/RELEASE-PROCESS.md");
    for exact in [
        "unsigned",
        "not notarized",
        "No cross-host reproducible build is claimed",
        "integrity facts, not signatures",
        "does not promote any completion-matrix row",
        "pre-alpha",
    ] {
        assert!(docs.contains(exact), "release nonclaim lost: {exact}");
    }
}

#[cfg(unix)]
#[test]
fn unix_packager_rejects_tag_and_commit_drift_before_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = std::env::temp_dir().join(format!(
        "semaprax-release-contract-no-output-{}",
        std::process::id()
    ));
    assert!(!output.exists(), "hostile-test output unexpectedly exists");
    for (tag, commit) in [
        ("v9.9.9", "64aec43b52277a53cb0f18d19fce9a37ca2dccaf"),
        ("v0.2.0", "64AEC43B52277A53CB0F18D19FCE9A37CA2DCCAF"),
    ] {
        let result = Command::new("sh")
            .arg("scripts/package-release.sh")
            .args([tag, commit, "aarch64-apple-darwin"])
            .arg(&output)
            .current_dir(root)
            .output()
            .expect("Unix release packager must be runnable through sh");
        assert!(
            !result.status.success(),
            "hostile input unexpectedly passed"
        );
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
        assert!(!output.exists(), "rejected input created release output");
    }
}

/// `scripts/release-reconcile.py` (#167): a README "published" claim must be
/// backed by a `docs/RELEASE-PROCESS.md` evidence section, a matching
/// CHANGELOG.md heading, and an exactly-cited commit/date/anchor -- and the
/// tool never mutates a file or the network by default.
#[test]
fn release_reconcile_agrees_with_the_real_published_v0_4_1_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // CI checks out with `fetch-depth: 1` and `fetch-tags: false`, so a
    // shallow job has no local `v0.4.1` tag and the reconcile reports
    // `no-candidate` even though the repository does have a published
    // release. Ensure the tag is present before reconciling; this is a
    // read-only `git fetch` and does not mutate any repository file.
    let tag_check = Command::new("git")
        .args(["rev-list", "-n1", "v0.4.1"])
        .current_dir(root)
        .output()
        .expect("git rev-list must run");
    if !tag_check.status.success() {
        let _ = Command::new("git")
            .args(["fetch", "--tags", "--prune", "--prune-tags"])
            .current_dir(root)
            .output();
    }
    let output = Command::new("python3")
        .args(["scripts/release-reconcile.py", "--version", "0.4.1"])
        .current_dir(root)
        .output()
        .expect("release reconcile must run");
    assert!(
        output.status.success(),
        "reconcile reported problems against the real repository: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        "release reconcile: v0.4.1 state=published-documented"
    );
}

const RECONCILE_PURE_CHECKS: &str = r#"
import runpy

module = runpy.run_path('scripts/release-reconcile.py')
reconcile_doc_claim = module['reconcile_doc_claim']
candidate_state = module['candidate_state']
verify_local_release_directory = module['verify_local_release_directory']
live_release_agrees = module['live_release_agrees']

def readme(date, commit, anchor, version='9.9.9'):
    return (
        'The published tag is the\n'
        f'[v{version} prerelease](https://github.com/wavect/semaprax/releases/tag/v{version})\n'
        f'({date}, `{commit}`) with smoke-tested archives, SHA256 checksums, and\n'
        'hosted release evidence in the\n'
        f'[release process](docs/RELEASE-PROCESS.md#{anchor}).\n'
    )

CHANGELOG_OK = '## 9.9.9 — 2027-01-01\n\nnotes\n\n## 9.9.8 — 2026-12-01\n\nolder notes\n'
COMMIT = 'a' * 40
EVIDENCE_OK = (
    '## 9.9.9 hosted release evidence\n\n'
    f'The annotated `v9.9.9` tag resolves to exact commit\n`{COMMIT}`.\n'
)

# --- reconcile_doc_claim: healthy citation has zero problems -----------------
problems = reconcile_doc_claim(
    readme('2027-01-01', COMMIT[:8], '999-hosted-release-evidence'),
    CHANGELOG_OK,
    EVIDENCE_OK,
)
assert problems == [], problems

# --- the exact real #167 shape: claim with no recorded evidence section -----
problems = reconcile_doc_claim(
    readme('2026-09-10', 'dfc15e2d', '040-hosted-release-evidence'),
    CHANGELOG_OK,
    '## 9.9.8 hosted release evidence\n\nexact commit\n`' + ('b' * 40) + '`.\n',
)
assert any("no '## 9.9.9 hosted release evidence' section" in p for p in problems), problems

# --- evidence section exists, but README cites the wrong commit prefix ------
problems = reconcile_doc_claim(
    readme('2027-01-01', 'deadbeef', '999-hosted-release-evidence'),
    CHANGELOG_OK,
    EVIDENCE_OK,
)
assert any('evidence section records' in p for p in problems), problems

# --- evidence section exists, but README cites the wrong date ---------------
problems = reconcile_doc_claim(
    readme('2027-06-06', COMMIT[:8], '999-hosted-release-evidence'),
    CHANGELOG_OK,
    EVIDENCE_OK,
)
assert any('CHANGELOG.md dates it' in p for p in problems), problems

# --- evidence section exists, but README's evidence link is stale -----------
problems = reconcile_doc_claim(
    readme('2027-01-01', COMMIT[:8], '040-hosted-release-evidence'),
    CHANGELOG_OK,
    EVIDENCE_OK,
)
assert any('points at #040-hosted-release-evidence' in p for p in problems), problems

# --- candidate_state: no tag at all ------------------------------------------
state, problems = candidate_state('9.9.9', None, None, None)
assert (state, problems) == ('no-candidate', [])

# --- candidate_state: tagged but no evidence, and README makes no claim --
# This is the healthy shape a release that failed after tagging but before
# the documentation step leaves behind: an explicit non-published state and
# NO misleading claim, because nothing claims it.
state, problems = candidate_state('9.9.9', COMMIT, None, None)
assert (state, problems) == ('tagged-unpublished', [])

# --- candidate_state: same tagged-but-undocumented shape, but README DOES
# claim it is published -- the exact misleading-claim case criterion 3 must
# catch. This is the negative control: only the claim changed, and it alone
# must flip the verdict.
state, problems = candidate_state('9.9.9', COMMIT, None, '9.9.9')
assert state == 'inconsistent', (state, problems)
assert any('not' in p and "'published-documented'" in p for p in problems), problems

# --- candidate_state: fully consistent published claim ----------------------
state, problems = candidate_state(
    '9.9.9', COMMIT, {'commit': COMMIT, 'anchor': 'x'}, '9.9.9'
)
assert (state, problems) == ('published-documented', [])

# --- candidate_state: evidence commit disagrees with the local tag ----------
state, problems = candidate_state(
    '9.9.9', COMMIT, {'commit': 'b' * 40, 'anchor': 'x'}, None
)
assert state == 'inconsistent'
assert any('disagrees with local tag commit' in p for p in problems), problems

# --- live_release_agrees: pure function, no network ---------------------
assert live_release_agrees('9.9.9', COMMIT, {
    'draft': False, 'published_at': '2027-01-01T00:00:00Z', 'tag_name': 'v9.9.9',
}) == []
assert any('draft' in p for p in live_release_agrees('9.9.9', COMMIT, {
    'draft': True, 'published_at': '2027-01-01T00:00:00Z', 'tag_name': 'v9.9.9',
}))
assert any('published_at' in p for p in live_release_agrees('9.9.9', COMMIT, {
    'draft': False, 'published_at': None, 'tag_name': 'v9.9.9',
}))
assert any('tag_name' in p for p in live_release_agrees('9.9.9', COMMIT, {
    'draft': False, 'published_at': '2027-01-01T00:00:00Z', 'tag_name': 'v9.9.8',
}))

# --- reconcile_changelog_summary: catches a stale "current tag" claim -------
reconcile_changelog_summary = module['reconcile_changelog_summary']
SUMMARY_OK = '## Latest published milestone\n\n- `v9.9.9` is the current prerelease tag used by installation and distribution docs.\n'
assert reconcile_changelog_summary(SUMMARY_OK, '9.9.9') == []
problems = reconcile_changelog_summary(SUMMARY_OK, '9.9.10')
assert any('claims v9.9.9' in p and 'is 9.9.10' in p for p in problems), problems
# A file that makes no such claim at all is not itself a problem.
assert reconcile_changelog_summary('no claim here\n', '9.9.9') == []

print('reconcile pure checks ok')
"#;

#[test]
fn release_reconcile_doc_claim_and_candidate_state_pure_checks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .args(["-B", "-c", RECONCILE_PURE_CHECKS])
        .current_dir(root)
        .output()
        .expect("python3 must run the reconcile pure checks");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "reconcile pure checks ok"
    );
}

/// The three disagreement classes over a local directory of built/downloaded
/// release archives, exercised against real `.tar.gz`/`.zip` archives built
/// in-process (the same manifest shape `scripts/package-release.sh` writes),
/// plus the positive (fully agreeing) case.
const RECONCILE_ARCHIVE_DIRECTORY_CHECKS: &str = r#"
import json
import runpy
import tarfile
import tempfile
import zipfile
from pathlib import Path

module = runpy.run_path('scripts/release-reconcile.py')
verify_local_release_directory = module['verify_local_release_directory']

VERSION = '9.9.9'
COMMIT = 'c' * 40

def manifest_bytes(version, commit, target):
    return json.dumps({
        'schema': 'semaprax.release-artifact.v1',
        'version': version,
        'commit': commit,
        'target': target,
        'maturity': 'pre-alpha',
        'binaries': ['semaprax', 'semapraxd'],
        'nonclaims': [],
    }).encode('utf-8')

def write_tar_gz(directory, name, target, version, commit):
    package = f'semaprax-v{VERSION}-{target}'
    path = directory / name
    with tarfile.open(path, 'w:gz') as archive:
        data = manifest_bytes(version, commit, target)
        info = tarfile.TarInfo(name=f'{package}/release-manifest.json')
        info.size = len(data)
        archive.addfile(info, __import__('io').BytesIO(data))

def write_zip(directory, name, target, version, commit):
    package = f'semaprax-v{VERSION}-{target}'
    path = directory / name
    with zipfile.ZipFile(path, 'w') as archive:
        archive.writestr(f'{package}/release-manifest.json', manifest_bytes(version, commit, target))

LINUX = f'semaprax-v{VERSION}-x86_64-unknown-linux-gnu.tar.gz'
MACOS = f'semaprax-v{VERSION}-aarch64-apple-darwin.tar.gz'
WINDOWS = f'semaprax-v{VERSION}-x86_64-pc-windows-msvc.zip'

# --- positive: all three agree -----------------------------------------------
with tempfile.TemporaryDirectory() as scratch:
    scratch = Path(scratch)
    write_tar_gz(scratch, LINUX, 'x86_64-unknown-linux-gnu', VERSION, COMMIT)
    write_tar_gz(scratch, MACOS, 'aarch64-apple-darwin', VERSION, COMMIT)
    write_zip(scratch, WINDOWS, 'x86_64-pc-windows-msvc', VERSION, COMMIT)
    assert verify_local_release_directory(scratch, VERSION, COMMIT) == []

# --- missing artifact ---------------------------------------------------------
with tempfile.TemporaryDirectory() as scratch:
    scratch = Path(scratch)
    write_tar_gz(scratch, LINUX, 'x86_64-unknown-linux-gnu', VERSION, COMMIT)
    write_tar_gz(scratch, MACOS, 'aarch64-apple-darwin', VERSION, COMMIT)
    problems = verify_local_release_directory(scratch, VERSION, COMMIT)
    assert problems == [f'missing artifact: {WINDOWS}'], problems

# --- wrong version embedded in one archive's manifest ------------------------
with tempfile.TemporaryDirectory() as scratch:
    scratch = Path(scratch)
    write_tar_gz(scratch, LINUX, 'x86_64-unknown-linux-gnu', VERSION, COMMIT)
    write_tar_gz(scratch, MACOS, 'aarch64-apple-darwin', '1.1.1', COMMIT)
    write_zip(scratch, WINDOWS, 'x86_64-pc-windows-msvc', VERSION, COMMIT)
    problems = verify_local_release_directory(scratch, VERSION, COMMIT)
    assert any('wrong version' in p and MACOS in p for p in problems), problems
    assert not any('wrong commit' in p for p in problems), problems

# --- wrong commit embedded in one archive's manifest --------------------------
with tempfile.TemporaryDirectory() as scratch:
    scratch = Path(scratch)
    write_tar_gz(scratch, LINUX, 'x86_64-unknown-linux-gnu', VERSION, COMMIT)
    write_tar_gz(scratch, MACOS, 'aarch64-apple-darwin', VERSION, COMMIT)
    write_zip(scratch, WINDOWS, 'x86_64-pc-windows-msvc', VERSION, 'd' * 40)
    problems = verify_local_release_directory(scratch, VERSION, COMMIT)
    assert any('wrong commit' in p and WINDOWS in p for p in problems), problems
    assert not any('wrong version' in p for p in problems), problems

# --- SHA256SUMS digest disagreement -------------------------------------------
with tempfile.TemporaryDirectory() as scratch:
    scratch = Path(scratch)
    write_tar_gz(scratch, LINUX, 'x86_64-unknown-linux-gnu', VERSION, COMMIT)
    write_tar_gz(scratch, MACOS, 'aarch64-apple-darwin', VERSION, COMMIT)
    write_zip(scratch, WINDOWS, 'x86_64-pc-windows-msvc', VERSION, COMMIT)
    (scratch / 'SHA256SUMS').write_text(f'{"0" * 64}  {LINUX}\n', encoding='utf-8')
    problems = verify_local_release_directory(scratch, VERSION, COMMIT)
    assert any('digest mismatch' in p and LINUX in p for p in problems), problems

print('reconcile archive directory checks ok')
"#;

#[test]
fn release_reconcile_local_archive_directory_disagreement_classes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .args(["-B", "-c", RECONCILE_ARCHIVE_DIRECTORY_CHECKS])
        .current_dir(root)
        .output()
        .expect("python3 must run the reconcile archive-directory checks");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "reconcile archive directory checks ok"
    );
}

#[test]
fn release_process_documents_state_and_reconciliation() {
    let docs = read("docs/RELEASE-PROCESS.md");
    for exact in [
        "## Release state and reconciliation",
        "python3 scripts/release-reconcile.py --version 0.4.1",
        "`no-candidate`",
        "`tagged-unpublished`",
        "`published-documented`",
        "`inconsistent`",
        "### The failure and recovery path",
        "Never move or recreate a published release tag",
    ] {
        assert!(docs.contains(exact), "release process lost: {exact}");
    }
}
