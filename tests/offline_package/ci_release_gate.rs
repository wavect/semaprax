use std::fs;
use std::path::Path;
use std::process::Command;

const RELEASE_BLOCKERS: &[&str] = &[
    "supply-chain",
    "component-runtime-v3",
    "wasm-scalar-exports-browser-v1",
    "project-product-acceptance-v1",
    "project-v1",
    "native-rust-sdk-v1",
    "verify",
    "verify-tests",
    "desktop-native-product",
    "ios-static-cross-check",
    "ios-swift-app-cross-check",
    "android-emulator-cross-check",
    "android-jni-app-cross-check",
    "callable-host-sanitizers",
    "rust-host-address-sanitizer",
    "msrv",
];

fn workflow() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml"))
        .expect("CI workflow must be readable")
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
fn desktop_product_is_an_exact_dedicated_release_blocker() {
    let workflow = workflow();
    let verify = job(&workflow, "verify");
    let desktop = job(&workflow, "desktop-native-product");

    for command in [
        "platform-tests/desktop-native/package-windows.ps1",
        "platform-tests/desktop-native/package-ui-windows.ps1",
        "platform-tests/desktop-native/package-macos.sh",
        "platform-tests/desktop-native/package-ui-macos.sh",
    ] {
        assert!(
            !verify.contains(command),
            "desktop command `{command}` must not remain in verify"
        );
        assert_eq!(
            desktop.matches(command).count(),
            1,
            "desktop command `{command}` must occur exactly once in its dedicated job"
        );
    }

    for exact in [
        "name: Private desktop + native UI product (${{ matrix.os }})",
        "runs-on: ${{ matrix.os }}",
        "timeout-minutes: 60",
        "fail-fast: false",
        "os: [windows-2025, macos-15]",
        "toolchain: 1.97.1",
        "run: cargo fetch --locked",
        "platform-tests/desktop-native/package-windows.ps1 `\n            -OutputRoot \"$env:RUNNER_TEMP/semaprax-private-desktop-v3\"",
        "platform-tests/desktop-native/package-ui-windows.ps1 `\n            -OutputRoot \"$env:RUNNER_TEMP/semaprax-private-desktop-ui-v1\" `\n            -EngineRoot \"$env:RUNNER_TEMP/semaprax-private-desktop-v3\"",
        "platform-tests/desktop-native/package-macos.sh \\\n            \"$RUNNER_TEMP/semaprax-private-desktop-v3\"",
        "platform-tests/desktop-native/package-ui-macos.sh \\\n            \"$RUNNER_TEMP/semaprax-private-desktop-ui-v1\" \\\n            \"$RUNNER_TEMP/semaprax-private-desktop-v3\"",
    ] {
        assert!(desktop.contains(exact), "desktop job lost exact contract: {exact}");
    }

    assert!(!desktop.contains("continue-on-error"));
    assert!(!desktop.contains("retry"));
    assert!(!desktop.contains("actions/cache"));
    assert!(!desktop.contains("actions/upload-artifact"));
    assert!(!desktop.contains("actions/download-artifact"));
}

#[test]
fn release_gate_fails_closed_over_the_complete_blocker_set() {
    let workflow = workflow();
    let release = job(&workflow, "release-gate");
    let needs = release
        .split_once("    needs:\n")
        .expect("release gate must declare needs")
        .1
        .split_once("    runs-on:")
        .expect("release needs must precede runs-on")
        .0;

    for blocker in RELEASE_BLOCKERS {
        assert!(
            release.contains(&format!("      - {blocker}\n")),
            "release gate must depend on `{blocker}`"
        );
    }
    assert_eq!(
        needs
            .lines()
            .filter(|line| line.starts_with("      - "))
            .count(),
        RELEASE_BLOCKERS.len(),
        "release gate dependency inventory must stay exact"
    );
    for exact in [
        "name: Release gate",
        // `success()` would skip this job whenever a blocker did not succeed,
        // and GitHub counts a skipped check run as a satisfied required status
        // check. The gate must run unconditionally and decide in the script.
        "if: ${{ always() }}",
        "runs-on: ubuntu-24.04",
        "timeout-minutes: 5",
        "Confirm every release blocker passed",
        "SEMAPRAX_CI_NEEDS: ${{ toJSON(needs) }}",
        "--sha \"${{ github.sha }}\"",
        "--head-sha \"$(git rev-parse HEAD)\"",
    ] {
        assert!(
            release.contains(exact),
            "release gate lost contract: {exact}"
        );
    }
    assert!(
        release.contains(&format!(
            "python3 scripts/ci-required-checks.py \\\n            --min-jobs {} \\\n",
            RELEASE_BLOCKERS.len()
        )),
        "release gate must aggregate exactly its declared blocker count"
    );
    for forbidden in [
        "success()",
        "failure()",
        "cancelled()",
        "continue-on-error",
        "actions/cache",
        "actions/upload-artifact",
        "actions/download-artifact",
    ] {
        assert!(
            !release.contains(forbidden),
            "release gate must fail closed, found `{forbidden}`"
        );
    }
}

#[test]
fn existing_core_matrix_and_global_authority_remain_bounded() {
    let workflow = workflow();
    let verify = job(&workflow, "verify");

    assert!(workflow.contains("permissions:\n  contents: read\n"));
    assert!(workflow.contains(
        "concurrency:\n  group: ci-${{ github.workflow }}-${{ github.ref }}\n  cancel-in-progress: true\n"
    ));
    assert!(verify.contains("fail-fast: false"));
    assert!(verify.contains("os: [ubuntu-latest, macos-latest, windows-latest]"));
    assert!(!workflow.contains("continue-on-error: true"));

    for blocker in RELEASE_BLOCKERS {
        assert!(
            !job(&workflow, blocker).contains("continue-on-error"),
            "release blocker `{blocker}` must not mask failures"
        );
    }
    for blocker in RELEASE_BLOCKERS {
        for forbidden in [
            "actions/cache",
            "actions/upload-artifact",
            "actions/download-artifact",
        ] {
            assert!(
                !job(&workflow, blocker).contains(forbidden),
                "release blocker `{blocker}` must not acquire cache/artifact coupling"
            );
        }
    }
}

/// Every top-level job identifier declared under `jobs:`, in declaration order.
fn workflow_jobs(workflow: &str) -> Vec<String> {
    workflow
        .split_once("\njobs:\n")
        .expect("workflow must declare jobs")
        .1
        .lines()
        .filter_map(|line| {
            let name = line.strip_prefix("  ")?.strip_suffix(':')?;
            (!name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'))
            .then(|| name.to_owned())
        })
        .collect()
}

/// A new job must be wired into the gate. Deriving the inventory from the
/// workflow, rather than restating it, is what makes a *missing* shard --
/// present in CI but absent from `needs` -- a local test failure instead of a
/// silently narrower aggregate.
#[test]
fn every_job_that_is_not_a_tag_only_release_step_is_a_release_blocker() {
    const TAG_ONLY: [&str; 3] = ["release-gate", "release-artifacts", "publish-release"];
    let workflow = workflow();
    let jobs = workflow_jobs(&workflow);

    assert_eq!(
        jobs.len(),
        RELEASE_BLOCKERS.len() + TAG_ONLY.len(),
        "unexpected CI job inventory: {jobs:?}"
    );
    let blocking: Vec<&str> = jobs
        .iter()
        .map(String::as_str)
        .filter(|job| !TAG_ONLY.contains(job))
        .collect();
    assert_eq!(
        blocking, RELEASE_BLOCKERS,
        "every non-release job must be a declared release blocker"
    );
}

/// Exercises the gate's verdict directly over synthetic `needs` contexts. The
/// hosted behaviour this pins cannot be observed from reading the workflow.
const GATE_VERDICTS: &str = r#"
import contextlib
import io
import json
import runpy

gate = runpy.run_path('scripts/ci-required-checks.py')
verdict = gate['failures']
SHA = 'a' * 40
OTHER = 'b' * 40

green = {f'job-{index}': {'result': 'success'} for index in range(16)}
assert verdict(green, 16, SHA, SHA) == []

# A blocker that failed, was skipped, or was cancelled is never success. GitHub
# reports the last two on an aggregate that used `if: success()`.
for result in ('failure', 'skipped', 'cancelled', None, '', 'Success'):
    broken = dict(green, **{'job-7': {'result': result}})
    assert verdict(broken, 16, SHA, SHA) == [
        f"upstream job 'job-7' result is {result!r}, not 'success'"
    ], (result, verdict(broken, 16, SHA, SHA))
malformed = dict(green, **{'job-7': 'success'})
assert verdict(malformed, 16, SHA, SHA) == [
    "upstream job 'job-7' result is None, not 'success'"
]

# A missing shard: green, but fewer jobs than the gate aggregates.
missing = dict(green)
del missing['job-3']
assert any('expected at least 16' in reason for reason in verdict(missing, 16, SHA, SHA))
# An emptied `needs:` must not pass vacuously.
assert any('expected at least 16' in reason for reason in verdict({}, 16, SHA, SHA))
assert any('at least one job' in reason for reason in verdict(green, 0, SHA, SHA))

# Results belonging to another commit.
assert verdict(green, 16, SHA, OTHER) == [
    f'gate ran on checked-out commit {OTHER}, not the reported commit {SHA}'
]
for bad in ('HEAD', '', SHA.upper(), SHA[:39]):
    assert any('hexadecimal commit' in reason for reason in verdict(green, 16, bad, SHA))
assert any('JSON object' in reason for reason in verdict([], 16, SHA, SHA))

# main() reads the context from the environment and never from argv.
arguments = ['--min-jobs', '16', '--sha', SHA, '--head-sha', SHA]
log = io.StringIO()
with contextlib.redirect_stderr(log):
    assert gate['main'](arguments, {}) == 1
    assert gate['main'](arguments, {'SEMAPRAX_CI_NEEDS': 'not json'}) == 1
    assert gate['main'](arguments, {'SEMAPRAX_CI_NEEDS': json.dumps(broken)}) == 1
assert 'SEMAPRAX_CI_NEEDS must carry toJSON(needs)' in log.getvalue()
assert 'is not valid JSON' in log.getvalue()
assert "result is 'Success', not 'success'" in log.getvalue()

passed = io.StringIO()
with contextlib.redirect_stdout(passed):
    assert gate['main'](arguments, {'SEMAPRAX_CI_NEEDS': json.dumps(green)}) == 0
assert passed.getvalue() == f'release gate: 16 upstream jobs succeeded at {SHA}\n'
print('gate verdicts checked')
"#;

#[test]
fn aggregate_gate_rejects_failed_skipped_cancelled_missing_and_foreign_results() {
    let output = Command::new("python3")
        .args(["-B", "-c", GATE_VERDICTS])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
        .output()
        .expect("python3 must run the aggregate gate");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "gate verdicts checked"
    );
}
