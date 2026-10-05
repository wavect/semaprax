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
    "python-test-suites",
    "installed-journey",
    "kernel0-lean-proof-gate",
    "supply-chain",
    "component-runtime-v3",
    "wasm-scalar-exports-browser-v1",
    "project-product-acceptance-v1",
    "project-v1",
    "native-rust-owned-data-sdk-v1",
    "native-rust-sdk-v1",
    "verify",
    "verify-build",
    "verify-tests",
    "unix-source-repair",
    "windows-agent-runtime-rest",
    "windows-source-repair",
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

/// Jobs that run only on `refs/tags/v*` and so are never release blockers.
const TAG_ONLY: [&str; 3] = ["release-gate", "release-artifacts", "publish-release"];

/// Jobs deliberately outside `release-gate`'s `needs:` because the hosted
/// runner cannot yet execute the thing they gate, so requiring them would
/// require a check that only ever takes its own skip path.
///
/// **Empty, and kept empty deliberately.** `kernel0-lean-proof-gate` was the
/// standing entry, on the stated grounds that hosted runners ship no Lean
/// toolchain and that AGENTS.md's no-build-time-network invariant forbade
/// fetching one. That reading conflated two different acts: the invariant
/// bans a *build* reaching the network, not a setup step provisioning a
/// pinned toolchain -- which this very workflow already does for Rust, Node
/// and TypeScript. The job now installs a sha256-pinned elan, builds the
/// proof for real, and runs both Lean gates with `--require-kernel` so a
/// provisioning regression fails instead of skipping. It is a blocker above.
///
/// This list is not a parking space. `a_not_yet_hosted_job_declares_that_status_in_the_workflow`
/// requires each entry to say so in its own job body, and forbids an entry
/// that is simultaneously listed as a blocker. An entry added here needs a
/// reason that survives the question asked above: is the thing genuinely
/// unrunnable hosted, or merely unwired?
const NOT_YET_HOSTED: [&str; 0] = [];

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

fn named_step_positions(job: &str, names: &[&str]) -> Result<Vec<usize>, String> {
    names
        .iter()
        .map(|name| {
            let marker = format!("      - name: {name}\n");
            let count = job.matches(&marker).count();
            if count != 1 {
                return Err(format!(
                    "expected exactly one {marker:?} step, found {count}"
                ));
            }
            Ok(job
                .find(&marker)
                .expect("the counted step must be findable"))
        })
        .collect()
}

/// The aggregate must verify the retained bundle that corresponds to each held
/// archive before it derives either checksums or the signed release inventory.
/// Keep this as a small parser rather than a list of witnesses: ordering inside
/// one shell step is security-relevant.
fn archive_attestation_gate_positions(job: &str) -> Result<(usize, usize, usize), String> {
    for required in [
        "gh attestation verify \"dist/$archive\"",
        "--bundle \"dist/$attestation\"",
        "--custom-trusted-root dist/trusted_root.jsonl",
        "--repo wavect/semaprax",
        "--signer-workflow wavect/semaprax/.github/workflows/ci.yml",
        "--source-digest \"$GITHUB_SHA\"",
        "--source-ref \"$GITHUB_REF\"",
        "--deny-self-hosted-runners",
    ] {
        if job.matches(required).count() != 1 {
            return Err(format!(
                "the archive-attestation gate must contain exactly one {required:?}"
            ));
        }
    }
    let root = job
        .find("gh attestation trusted-root")
        .ok_or_else(|| "the archive-attestation gate must freeze a trusted root".to_owned())?;
    let verify = job
        .find("gh attestation verify \"dist/$archive\"")
        .expect("required above");
    let checksums = job
        .find("(cd dist && sha256sum")
        .ok_or_else(|| "the aggregate must write checksums after verification".to_owned())?;
    if !(root < verify && verify < checksums) {
        return Err(
            "trusted-root capture, archive-attestation verification, and checksums are out of order"
                .to_owned(),
        );
    }
    Ok((root, verify, checksums))
}

#[test]
fn release_artifacts_are_attested_signed_and_packaged_for_offline_replay_before_uploading() {
    let workflow = workflow();
    let publisher = job(&workflow, "publish-release");
    let producers = job(&workflow, "release-artifacts");

    for exact in [
        "if: ${{ startsWith(github.ref, 'refs/tags/v') && success() }}",
        "actions: read",
        "contents: write",
        "id-token: write",
        "uses: sigstore/cosign-installer@6f9f17788090df1f26f669e9d70d6ae9567deba6 # v4.1.2",
        "cosign-release: v3.1.3",
        "--tag \"$GITHUB_REF_NAME\"",
        "--commit \"$GITHUB_SHA\"",
        "--workflow-identity \"wavect/semaprax/.github/workflows/ci.yml@refs/tags/$tag\"",
        "--run-id \"$GITHUB_RUN_ID\"",
        "--run-attempt \"$GITHUB_RUN_ATTEMPT\"",
        "--host-class github-hosted-ubuntu-24.04",
        "gh attestation verify \"dist/$archive\"",
        "--bundle \"dist/$attestation\"",
        "--custom-trusted-root dist/trusted_root.jsonl",
        "--repo wavect/semaprax",
        "--signer-workflow wavect/semaprax/.github/workflows/ci.yml",
        "--source-digest \"$GITHUB_SHA\"",
        "--source-ref \"$GITHUB_REF\"",
        "--deny-self-hosted-runners",
        "cosign sign-blob --yes \\\n            --bundle dist/release-provenance.bundle \\\n            dist/release-provenance.json",
        "python3 scripts/release-signature-claim.py",
        "--output dist/release-signature-claim.json",
        "--check dist/release-signature-claim.json",
        "doctor verify-release dist \\\n            --trusted-root-sha256 \"$root_digest\"",
        "tar -xOzf \"$archive\" \"$package/semaprax\"",
        "test ! -e dist/release-signature-claim.json",
        "test ! -e dist/trusted_root.jsonl",
        "gh attestation trusted-root \\\n            | head -c 4194305 > dist/trusted_root.jsonl",
        "test \"$(wc -c < dist/trusted_root.jsonl | tr -d ' ')\" -le 4194304",
        "dist/release-manifest.json dist/release-provenance.json \\\n            dist/release-provenance.bundle dist/release-signature-claim.json \\\n            dist/trusted_root.jsonl",
        "dist/release-attestation-x86_64-unknown-linux-gnu.json \\\n            dist/release-attestation-aarch64-apple-darwin.json \\\n            dist/release-attestation-x86_64-pc-windows-msvc.json",
        "find dist -maxdepth 1 -type f -name 'release-attestation-*.json'",
    ] {
        assert!(
            publisher.contains(exact),
            "release publisher lost exact Sigstore/inventory contract: {exact}"
        );
    }
    for script in [
        "scripts/release-manifest.py",
        "scripts/release-provenance.py",
    ] {
        assert_eq!(
            publisher.matches(script).count(),
            1,
            "the final publisher must build exactly one {script} document"
        );
    }
    assert_eq!(
        publisher
            .matches("scripts/release-signature-claim.py")
            .count(),
        2,
        "the publisher must build and byte-replay exactly one signature claim"
    );
    assert_eq!(
        publisher.matches("gh attestation trusted-root").count(),
        1,
        "the publisher must package exactly one explicit offline trusted-root set"
    );
    assert_eq!(
        publisher.matches("gh attestation verify").count(),
        1,
        "the aggregate must cryptographically bind every retained archive to its staged bundle"
    );
    archive_attestation_gate_positions(publisher)
        .expect("archive attestation validation must precede checksums and final release state");
    let missing_commit_pin = publisher.replacen(
        "--source-digest \"$GITHUB_SHA\"",
        "--source-digest \"unbound\"",
        1,
    );
    assert!(
        archive_attestation_gate_positions(&missing_commit_pin).is_err(),
        "the archive gate must reject a superficially valid command that loses its exact source pin"
    );
    assert_eq!(publisher.matches("id-token: write").count(), 1);
    for exact in [
        "attestations: write",
        "contents: read",
        "id-token: write",
        "uses: actions/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8 # v4.2.2",
        "subject-path: dist/semaprax-${{ github.ref_name }}-${{ matrix.target }}.${{ matrix.extension }}",
        "ATTESTATION_BUNDLE: ${{ steps.attest-release-archive.outputs.bundle-path }}",
        "RELEASE_BUNDLE: dist/release-attestation-${{ matrix.target }}.json",
        "cp -- \"$ATTESTATION_BUNDLE\" \"$RELEASE_BUNDLE\"",
        "dist/release-attestation-${{ matrix.target }}.json",
    ] {
        assert!(
            producers.contains(exact),
            "release artifact producer lost exact SLSA attestation contract: {exact}"
        );
    }
    assert_eq!(
        producers.matches("id-token: write").count(),
        1,
        "the archive producer requires its own OIDC token for GitHub provenance attestation"
    );
    assert!(
        !workflow
            .split_once("\njobs:\n")
            .expect("workflow must declare jobs")
            .0
            .contains("id-token: write"),
        "OIDC minting authority must be job-scoped, never a workflow default"
    );
    assert!(
        publisher.matches("set -euo pipefail").count() >= 5,
        "every archive/inventory/provenance/sign/publish shell boundary must fail closed"
    );
    let producer_positions = named_step_positions(
        producers,
        &[
            "Build, package, and smoke-test the Unix release artifact",
            "Build, package, and smoke-test the Windows release artifact",
            "Attest the exact smoke-tested target archive",
            "Stage the exact target attestation bundle for release publication",
            "Retain the exact target archive for aggregate publication",
        ],
    )
    .expect("every archive producer must smoke-test, attest, then retain its archive");
    assert!(
        producer_positions.windows(2).all(|pair| pair[0] < pair[1]),
        "the provenance action must attest the smoke-tested archive before publication aggregation"
    );
    let positions = named_step_positions(
        publisher,
        &[
            "Authenticate the complete archive inventory and produce SHA256SUMS",
            "Generate the final release manifest",
            "Generate release provenance from the final manifest",
            "Install pinned cosign",
            "Sign final release provenance with keyless Sigstore",
            "Derive the signature claim after archive-attestation verification",
            "Independently verify the signed release before publication",
            "Publish the release archives only after complete aggregation",
        ],
    )
    .expect("each final-inventory/signing step must be present once");
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "a release may only sign after final inventory and provenance, derive replay material, then upload the exact signed set"
    );

    // Mutation control: changing the signing step into a second provenance
    // step is syntactically harmless YAML but makes the signing boundary
    // disappear. `named_step_positions` must reject it, so this contract is
    // not merely a collection of positive substring witnesses.
    let mutated = publisher.replacen(
        "      - name: Sign final release provenance with keyless Sigstore\n",
        "      - name: Generate release provenance from the final manifest\n",
        1,
    );
    assert!(
        named_step_positions(
            &mutated,
            &[
                "Authenticate the complete archive inventory and produce SHA256SUMS",
                "Generate the final release manifest",
                "Generate release provenance from the final manifest",
                "Install pinned cosign",
                "Sign final release provenance with keyless Sigstore",
                "Publish the release archives only after complete aggregation",
            ],
        )
        .is_err(),
        "the order guard must reject a mutation that removes the signing step"
    );
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
        workflow.contains("concurrency:\n  # CI is a latest-ref health signal."),
        "the workflow must document its latest-ref cancellation policy"
    );
    assert!(
        workflow.contains("  group: ci-${{ github.workflow }}-${{ github.ref }}\n"),
        "all runs for one ref must share a concurrency group"
    );
    assert!(
        workflow.contains("  cancel-in-progress: true\n"),
        "a newer run must cancel the superseded run and save CI minutes"
    );
    assert!(!workflow.contains("github.sha || 'ref-tip'"));
    assert!(!workflow.contains("github.ref != 'refs/heads/main'"));
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

#[test]
fn docs_workflow_cancels_superseded_publish_runs() {
    let workflow = docs_workflow();

    assert!(
        workflow.contains("  group: docs-${{ github.workflow }}-${{ github.ref }}\n"),
        "all documentation runs for one ref must share a concurrency group"
    );
    assert!(
        workflow.contains("  cancel-in-progress: true\n"),
        "only the newest documentation build should spend runner minutes"
    );
    assert!(!workflow.contains("github.sha || 'ref-tip'"));
    assert!(!workflow.contains("github.ref != 'refs/heads/main'"));
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
    let workflow = workflow();
    let jobs = workflow_jobs(&workflow);

    assert_eq!(
        jobs.len(),
        RELEASE_BLOCKERS.len() + TAG_ONLY.len() + NOT_YET_HOSTED.len(),
        "unexpected CI job inventory: {jobs:?}"
    );
    let blocking: Vec<&str> = jobs
        .iter()
        .map(String::as_str)
        .filter(|job| !TAG_ONLY.contains(job) && !NOT_YET_HOSTED.contains(job))
        .collect();
    assert_eq!(
        blocking, RELEASE_BLOCKERS,
        "every non-release job must be a declared release blocker"
    );
}

/// The exemption above is only as honest as its evidence. A job named in
/// `NOT_YET_HOSTED` must say so in its own body, in the workflow, next to the
/// reason -- otherwise the list degrades into a place to park any job someone
/// did not want to wire into the aggregate, and the inventory rule it carves
/// out of stops meaning anything.
///
/// The marker is deliberately a full sentence rather than a bare token, so it
/// cannot be pasted onto a job without also stating what is not hosted and
/// why.
#[test]
fn a_not_yet_hosted_job_declares_that_status_in_the_workflow() {
    let workflow = workflow();
    for exempt in NOT_YET_HOSTED {
        let body = job(&workflow, exempt);
        assert!(
            body.contains("# NOT-YET-HOSTED, deliberately:"),
            "`{exempt}` is exempt from the release-blocker inventory but its \
             job body does not carry the `# NOT-YET-HOSTED, deliberately:` \
             marker and the reason; either wire it into `release-gate`'s \
             `needs:` and add it to RELEASE_BLOCKERS, or state in the \
             workflow what cannot run hosted yet"
        );
        assert!(
            !release_gate_needs(&workflow).contains(&exempt.to_string()),
            "`{exempt}` claims not-yet-hosted status but *is* listed in \
             `release-gate`'s `needs:`; a job cannot be both exempt and a \
             blocker -- drop it from NOT_YET_HOSTED once it is wired in"
        );
    }
}

/// The Lean job is only worth being a release blocker while it actually
/// runs a kernel. Both gate scripts exit 0 with a `SKIP` when no toolchain
/// is present -- the correct default for a developer machine, and a silent
/// no-op on a runner. `--require-kernel` converts that skip into a failure,
/// so dropping the flag would turn a green required check into a check of
/// nothing while every other assertion in this file still passed.
///
/// The provisioning half is pinned for the same reason: an unpinned
/// installer, or an archive unpacked without a digest check, would make the
/// kernel whatever the network served that morning, and a proof re-checked
/// by an unknown kernel is not re-checked.
#[test]
fn the_lean_gates_run_a_real_kernel_rather_than_taking_their_skip_path() {
    let workflow = workflow();
    let gate = job(&workflow, "kernel0-lean-proof-gate");

    for exact in [
        "python3 scripts/kernel0-lean-gate.py --require-kernel",
        "python3 scripts/lean-export-gate.py --require-kernel",
    ] {
        assert!(
            gate.contains(exact),
            "the Lean job must run `{exact}`; without `--require-kernel` a \
             missing toolchain is a skip that exits 0, so the job would \
             report success having checked no proof at all"
        );
    }

    // Single-sourced pin: the job reads the same `lean-toolchain` file that
    // `scripts/lean-export-gate.py` and `proof_export`'s PINNED_TOOLCHAIN
    // read, so a bump cannot land in one place and not the others.
    assert!(
        gate.contains("$(cat proofs/kernel0-lean/lean-toolchain)"),
        "the provisioned toolchain must come from the repository's own pin \
         file, not a version literal in the workflow"
    );
    assert!(
        gate.contains("sha256sum --check --strict"),
        "the elan archive must be verified against a recorded digest before \
         it is unpacked; a tag alone is mutable"
    );
    // Not a `curl | sh` installer. Pinned positively rather than by
    // forbidding the literal `| sh`, which this job's own `| sha256sum`
    // pipe contains -- a negative pin that matches the safe construct is a
    // test that fails for the wrong reason and gets "fixed" by deleting it.
    assert!(
        gate.contains("--output \"$archive\"") && gate.contains("tar -xzf \"$archive\""),
        "the elan archive must be downloaded to a file, digest-checked, and \
         unpacked from that file -- never piped straight into a shell"
    );

    // The corpus lane this job also arms is the other half of the same
    // failure mode -- a test that returns early is as blind as no test.
    assert!(
        gate.contains("SEMAPRAX_REQUIRE_KERNEL_ZERO_CROSS_BACKEND: \"1\""),
        "the Kernel-0 cross-backend differential lane must be armed here; \
         unarmed it skips on every runner while its step reports success"
    );

    for forbidden in ["continue-on-error", "# NOT-YET-HOSTED, deliberately:"] {
        assert!(
            !gate.contains(forbidden),
            "the Lean job is a release blocker and must fail closed, found \
             `{forbidden}`"
        );
    }
}

/// The job identifiers listed under `release-gate`'s `needs:`.
fn release_gate_needs(workflow: &str) -> Vec<String> {
    let gate = job(workflow, "release-gate");
    gate.split_once("    needs:\n")
        .expect("release-gate must declare needs")
        .1
        .lines()
        .map_while(|line| line.strip_prefix("      - ").map(str::to_owned))
        .collect()
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
    assert_eq!(
        build
            .matches(
                "if [[ \"$RUNNER_OS\" == Windows ]]; then exclude=(--exclude semaprax-harness); fi"
            )
            .count(),
        4,
        "Windows-only harness exclusion must cover all four workspace build checks"
    );
    for command in [
        "cargo fmt --all --check",
        "cargo clippy --locked --workspace \"${exclude[@]}\" --all-targets --all-features -- -D warnings",
        "cargo test --locked --workspace \"${exclude[@]}\" --all-features --doc",
        "cargo doc --locked --workspace \"${exclude[@]}\" --all-features --no-deps",
        "cargo build --locked --workspace \"${exclude[@]}\" --release",
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

/// A harness that gates its own tool-dependent lanes behind a
/// `SEMAPRAX_REQUIRE_*` environment flag only actually runs those lanes where
/// something sets the flag. Nothing did for three of them
/// (`SEMAPRAX_REQUIRE_VIEW_OWNERSHIP_COMPOSITION`,
/// `SEMAPRAX_REQUIRE_AGENT_PROPOSAL_CLIENTS`,
/// `SEMAPRAX_REQUIRE_INTERPRETER_BACKEND_PARITY`), so those lanes had never
/// executed on any runner while the steps selecting their harnesses still
/// reported success -- a test that passes by returning early is the same
/// blackout as a test with no CI selector at all, and is harder to see.
///
/// This walks the declared flags out of the test sources rather than pinning
/// a fixed list, so a newly introduced flag is refused until it is armed or
/// deliberately recorded below.
#[test]
fn every_declared_require_flag_is_armed_by_some_workflow() {
    /// Flags that are deliberately never armed in this repository, each with
    /// the reason it cannot be. Adding an entry here is a claim that the flag
    /// *must not* run in CI, not a way to silence this test.
    const DELIBERATELY_UNARMED: [(&str, &str); 0] = [];

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut declared = std::collections::BTreeSet::new();
    let mut stack = vec![root.join("tests"), root.join("src"), root.join("crates")];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = fs::read_to_string(&path).unwrap_or_default();
                let mut rest = source.as_str();
                while let Some(start) = rest.find("SEMAPRAX_REQUIRE_") {
                    let tail = &rest[start..];
                    let end = tail
                        .find(|character: char| {
                            !character.is_ascii_uppercase()
                                && character != '_'
                                && !character.is_ascii_digit()
                        })
                        .unwrap_or(tail.len());
                    declared.insert(tail[..end].to_string());
                    rest = &tail[end..];
                }
            }
        }
    }
    assert!(
        declared.len() > 5,
        "found only {} declared SEMAPRAX_REQUIRE_* flags; the walk is not \
         reaching the test sources and would pass vacuously",
        declared.len()
    );

    let mut workflows = String::new();
    for entry in fs::read_dir(root.join(".github/workflows"))
        .expect("workflow directory must be readable")
        .flatten()
    {
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "yml") {
            workflows.push_str(&fs::read_to_string(&path).unwrap_or_default());
        }
    }

    let unarmed = declared
        .iter()
        .filter(|flag| !workflows.contains(flag.as_str()))
        .filter(|flag| {
            !DELIBERATELY_UNARMED
                .iter()
                .any(|(recorded, _)| recorded == flag)
        })
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        unarmed.is_empty(),
        "these SEMAPRAX_REQUIRE_* flags gate tool-dependent test lanes but no \
         workflow sets them, so those lanes skip themselves on every runner \
         while their step still reports success: {}",
        unarmed.join(", ")
    );
}
