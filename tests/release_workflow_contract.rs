use std::fs;
use std::path::Path;
#[cfg(unix)]
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
        "if: ${{ startsWith(github.ref, 'refs/tags/v') }}",
        "needs: release-gate",
        "timeout-minutes: 30",
        "fail-fast: false",
        "os: ubuntu-24.04\n            target: x86_64-unknown-linux-gnu\n            extension: tar.gz",
        "os: macos-15\n            target: aarch64-apple-darwin\n            extension: tar.gz",
        "os: windows-2025\n            target: x86_64-pc-windows-msvc\n            extension: zip",
        "toolchain: 1.97.1",
        "scripts/package-release.sh \"$GITHUB_REF_NAME\" \"$GITHUB_SHA\" \"${{ matrix.target }}\" dist",
        "scripts/package-release.ps1 -Tag $env:GITHUB_REF_NAME -Commit $env:GITHUB_SHA -Target \"${{ matrix.target }}\" -OutputRoot dist",
        "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
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
        "actions/download-artifact@634f93cb2916e3fdff6788551b99b062d0335ce0",
        "pattern: release-*",
        "merge-multiple: true",
        "sha256sum \"${archives[@]}\"",
        "gh release create \"$GITHUB_REF_NAME\"",
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
