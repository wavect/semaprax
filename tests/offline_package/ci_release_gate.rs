use std::fs;
use std::path::Path;
use std::process::Command;

/// The exact release-gate required-check inventory, in declaration order.
/// `pub(crate)` rather than private: `release_manifest.rs` cross-checks that
/// `scripts/release-manifest.py`'s own independent parse of the same
/// `release-gate` `needs:` block in `.github/workflows/ci.yml` produces this
/// exact list, so the manifest's required-check inventory and this pinned
/// gate inventory cannot silently drift apart.
pub(crate) const RELEASE_BLOCKERS: &[&str] = &[
    "agent-proposal-clients",
    "gen05b-generic-instance-closure",
    "public-generic-ownership-milestone",
    "std-library-depth",
    "release-claim-reconcile",
    "supply-chain",
    "component-runtime-v3",
    "wasm-scalar-exports-browser-v1",
    "project-product-acceptance-v1",
    "project-v1",
    "native-rust-sdk-v1",
    "verify",
    "verify-build",
    "verify-tests",
    "desktop-native-product",
    "doctor-macos-confinement",
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

fn docs_workflow() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/docs.yml"))
        .expect("Docs workflow must be readable")
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
    assert!(
        workflow.contains("concurrency:\n  group: ci-${{ github.workflow }}-${{ github.ref }}\n")
    );
    // Pinned exactly, because the weaker `cancel-in-progress: true` cancelled 98
    // of 100 consecutive `main` runs and left the required gates with no verdict
    // of either colour. A push to `main` must always be allowed to finish;
    // anything else may still be superseded by its own tip.
    assert!(
        workflow.contains("  cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}\n"),
        "`main` runs must never be cancelled by a later push"
    );
    assert!(!workflow.contains("  cancel-in-progress: true\n"));
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

/// `docs.yml`'s `deploy` job publishes the GitHub Pages site and, unlike
/// `ci.yml`, has no release gate downstream to notice a missed publish. Issue
/// #169 fixed the identical defect in `ci.yml`: the unconditional
/// `cancel-in-progress: true` cancelled a `main` run whenever a later push
/// landed before it finished, and a cancellation is neither a pass nor a
/// failure, so nothing ever turned red. For `docs.yml` this means a
/// cancelled run's commit is simply never published to the site, silently,
/// with the previously published commit left standing and no signal that it
/// is stale. Pinned exactly, and the negative assertion kept alongside it,
/// because a weaker positive-only check would not catch a regression that
/// restored the unconditional form.
#[test]
fn docs_workflow_never_cancels_a_completed_main_publish() {
    let workflow = docs_workflow();

    assert!(
        workflow.contains("concurrency:\n  group: docs-${{ github.workflow }}-${{ github.ref }}\n")
    );
    assert!(
        workflow.contains("  cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}\n"),
        "a cancelled `Docs` run on `main` means that commit's site is never \
         published, and a cancellation is neither a pass nor a failure, so \
         nothing detects the gap; `main` runs must never be cancelled by a \
         later push"
    );
    assert!(
        !workflow.contains("  cancel-in-progress: true\n"),
        "the unconditional form must not return once the ref-scoped \
         expression is in place"
    );
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

green = {f'job-{index}': {'result': 'success'} for index in range(18)}
assert verdict(green, 18, SHA, SHA) == []

# A blocker that failed, was skipped, or was cancelled is never success. GitHub
# reports the last two on an aggregate that used `if: success()`.
for result in ('failure', 'skipped', 'cancelled', None, '', 'Success'):
    broken = dict(green, **{'job-7': {'result': result}})
    assert verdict(broken, 18, SHA, SHA) == [
        f"upstream job 'job-7' result is {result!r}, not 'success'"
    ], (result, verdict(broken, 18, SHA, SHA))
malformed = dict(green, **{'job-7': 'success'})
assert verdict(malformed, 18, SHA, SHA) == [
    "upstream job 'job-7' result is None, not 'success'"
]

# A missing shard: green, but fewer jobs than the gate aggregates.
missing = dict(green)
del missing['job-3']
assert any('expected at least 18' in reason for reason in verdict(missing, 18, SHA, SHA))
# An emptied `needs:` must not pass vacuously.
assert any('expected at least 18' in reason for reason in verdict({}, 18, SHA, SHA))
assert any('at least one job' in reason for reason in verdict(green, 0, SHA, SHA))

# Results belonging to another commit.
assert verdict(green, 18, SHA, OTHER) == [
    f'gate ran on checked-out commit {OTHER}, not the reported commit {SHA}'
]
for bad in ('HEAD', '', SHA.upper(), SHA[:39]):
    assert any('hexadecimal commit' in reason for reason in verdict(green, 18, bad, SHA))
assert any('JSON object' in reason for reason in verdict([], 18, SHA, SHA))

# main() reads the context from the environment and never from argv.
arguments = ['--min-jobs', '18', '--sha', SHA, '--head-sha', SHA]
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
assert passed.getvalue() == f'release gate: 18 upstream jobs succeeded at {SHA}\n'
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

#[test]
fn build_validation_runs_independently_without_losing_platform_coverage() {
    let workflow = workflow();
    let build = job(&workflow, "verify-build");
    let evidence = job(&workflow, "verify");
    for lane in [build, evidence] {
        assert!(lane.contains("os: [ubuntu-latest, macos-latest, windows-latest]"));
        assert!(lane.contains("fail-fast: false"));
        assert!(
            !lane.contains("    needs:"),
            "validation lanes must start independently"
        );
        assert!(lane.contains("toolchain: 1.97.1"));
        assert!(lane.contains("CARGO_PROFILE_TEST_DEBUG: \"0\""));
    }
    for command in [
        "cargo fmt --all --check",
        "cargo clippy --locked --workspace --all-targets --all-features -- -D warnings",
        "cargo test --locked --workspace --all-features --doc",
        "cargo doc --locked --workspace --all-features --no-deps",
        "cargo build --locked --workspace --release",
        "cargo package --locked -p semaprax",
        "node scripts/verify-web.mjs target/control-flow-web",
    ] {
        assert_eq!(
            build.matches(command).count(),
            1,
            "missing or duplicate {command}"
        );
        assert!(
            !evidence.contains(command),
            "build work still serializes evidence: {command}"
        );
    }
    assert!(build.contains("RUSTDOCFLAGS: -D warnings"));
    for gate in [
        "Require Windows callable-v2 and private callable-v3 physical evidence",
        "Require native cleanup sanitizers (Linux)",
        "Require private Native Rust Interop ASan + UBSan round trip (Linux)",
    ] {
        assert!(
            evidence.contains(gate),
            "lost distinct physical evidence: {gate}"
        );
        assert!(!build.contains(gate));
    }
}

/// Drift class this test exists to catch: a job joins `release-gate`'s
/// `needs:` (and `RELEASE_BLOCKERS` above is updated to match, so the gate
/// itself stays fail-closed) but `docs/CI-REQUIRED-CHECKS-V1.md`'s
/// human-readable inventory table is never told about it. That happened in
/// this repository's history for `release-claim-reconcile`: the job and this
/// file's `RELEASE_BLOCKERS` entry landed in one commit
/// (`ci: make the release-claim reconciliation an actual release blocker`),
/// and the doc's table, blocking-job count, and `--min-jobs` prose were only
/// corrected later, with no test failing in between.
#[test]
fn required_checks_doc_names_every_release_blocker() {
    let doc = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/CI-REQUIRED-CHECKS-V1.md"),
    )
    .expect("required-checks doc must be readable");

    // `agent-proposal-clients` (the three AGENT-06 client contexts) and
    // `gen05b-generic-instance-closure` (the GEN-05B closure context) are
    // documented by prose alias rather than literal job id, because each
    // expands to several published check-context names that the doc already
    // enumerates individually in its own paragraph. Every other release
    // blocker must appear by its literal job id somewhere in the doc.
    let prose_aliased: &[&str] = &["agent-proposal-clients", "gen05b-generic-instance-closure"];

    for blocker in RELEASE_BLOCKERS {
        if prose_aliased.contains(blocker) {
            continue;
        }
        assert!(
            doc.contains(blocker),
            "release blocker `{blocker}` is not named anywhere in \
             docs/CI-REQUIRED-CHECKS-V1.md; add its published check-context \
             row to the inventory table (or, if it expands to several \
             contexts already described in prose, add it to `prose_aliased` \
             in this test) whenever a new job joins `release-gate`'s `needs:`"
        );
    }
}

/// The doc's `--min-jobs` prose is hand-written English, not derived from
/// `ci.yml` or `RELEASE_BLOCKERS`, so nothing forced it to move when either
/// did. This pins it to the live workflow value instead of a hardcoded
/// number, so it fails the moment a blocker is added or removed without the
/// doc being updated to match -- exactly the gap that let
/// `release-claim-reconcile` land without the doc noticing.
#[test]
fn required_checks_doc_min_jobs_matches_the_live_gate() {
    let workflow = workflow();
    let gate = job(&workflow, "release-gate");
    let marker = "--min-jobs ";
    let start = gate
        .find(marker)
        .unwrap_or_else(|| panic!("release-gate must pass `{marker}`"))
        + marker.len();
    let digits: String = gate[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let live_min_jobs: usize = digits
        .parse()
        .unwrap_or_else(|_| panic!("`--min-jobs` value must be numeric, got {digits:?}"));
    assert_eq!(
        live_min_jobs,
        RELEASE_BLOCKERS.len(),
        "ci.yml's `--min-jobs {live_min_jobs}` must equal \
         RELEASE_BLOCKERS.len() ({}); keep the aggregate's floor in step with \
         its own blocker inventory",
        RELEASE_BLOCKERS.len()
    );

    let doc = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/CI-REQUIRED-CHECKS-V1.md"),
    )
    .expect("required-checks doc must be readable");
    let needle = format!("--min-jobs {live_min_jobs}");
    assert!(
        doc.contains(&needle),
        "docs/CI-REQUIRED-CHECKS-V1.md must state the live value (`{needle}`); \
         update its prose when the release-gate blocker count changes"
    );
}
