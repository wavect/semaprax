use std::fs;
use std::path::Path;

const RELEASE_BLOCKERS: &[&str] = &[
    "supply-chain",
    "component-runtime-v3",
    "wasm-scalar-exports-browser-v1",
    "project-product-acceptance-v1",
    "project-v1",
    "native-rust-sdk-v1",
    "verify",
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
fn release_gate_is_default_success_over_the_complete_blocker_set() {
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
        "if: ${{ success() }}",
        "runs-on: ubuntu-24.04",
        "timeout-minutes: 5",
        "Confirm every release blocker passed",
    ] {
        assert!(
            release.contains(exact),
            "release gate lost contract: {exact}"
        );
    }
    for forbidden in [
        "always()",
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
    assert_eq!(
        workflow.matches("continue-on-error: true").count(),
        1,
        "the sole recent non-blocking diagnostic is an explicit, bounded exception"
    );
    assert!(verify.contains(
        "name: Diagnose bounded Windows Native Rust Interop unit evidence (non-blocking)"
    ));

    for blocker in RELEASE_BLOCKERS {
        if *blocker != "verify" {
            assert!(
                !job(&workflow, blocker).contains("continue-on-error"),
                "release blocker `{blocker}` must not mask failures"
            );
        }
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
