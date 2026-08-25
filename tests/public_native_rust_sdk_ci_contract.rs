use std::fs;
use std::path::Path;

fn read(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

#[test]
fn hosted_matrix_routes_public_sdk_evidence_on_all_three_operating_systems() {
    let workflow = read(".github/workflows/ci.yml");
    let public_sdk = workflow
        .split("\n  native-rust-sdk-v1:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  verify:\n").next())
        .expect("Public Native Rust SDK CI job");
    for runner in ["ubuntu-latest", "macos-latest", "windows-latest"] {
        assert!(workflow.contains(runner), "CI matrix is missing {runner}");
    }
    for required in [
        "public_native_rust_sdk_v1",
        "public_native_rust_sdk_ci_contract",
        "examples/calculator-rust/Cargo.toml",
        "SEMAPRAX_ARCHIVER",
        "SEMAPRAX_REQUIRE_PUBLIC_NATIVE_RUST_SDK",
    ] {
        assert!(
            workflow.contains(required),
            "hosted public Native Rust SDK evidence is missing `{required}`"
        );
    }
    assert!(workflow.contains("--locked"));
    assert!(workflow.contains("--offline"));
    for required in [
        "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=%SEMAPRAX_LINKER%",
        "echo LINK=",
        "echo _LINK_=",
        "continue-on-error: ${{ runner.os == 'Windows' }}",
    ] {
        assert!(
            public_sdk.contains(required),
            "hosted public SDK linker boundary is missing `{required}`"
        );
    }
    assert_eq!(
        public_sdk
            .matches("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=")
            .count(),
        1,
        "Public SDK Windows setup must bind Cargo's target linker exactly once"
    );
}

#[test]
fn external_consumers_share_one_bounded_nested_cargo_linker_path_binder() {
    let binder = read("tests/support/native_rust_cargo.rs");
    for required in [
        "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
        "SEMAPRAX_LINKER",
        "SEMAPRAX_VCTOOLS",
        "command.env_remove(\"LINK\")",
        "command.env_remove(\"_LINK_\")",
        "does not hold the linker image or its ancestors",
        "close a same-path substitution race",
    ] {
        assert!(binder.contains(required), "Cargo binder lost `{required}`");
    }
    for forbidden in ["command.env(\"PATH\"", "RUSTFLAGS", "-Clinker="] {
        assert!(
            !binder.contains(forbidden),
            "Cargo binder admitted forbidden linker configuration `{forbidden}`"
        );
    }

    let consumers = read("tests/public_native_rust_sdk_v1.rs");
    assert_eq!(
        consumers
            .matches("native_rust_cargo::cargo_command()")
            .count(),
        10,
        "nine effectful setup/lock/consumer commands plus the command-inspection test must use the binder"
    );
    for required in [
        ".env(\"LINK\", \".obj\")",
        ".env(\"_LINK_\", \".obj\")",
        "Some(Some(linker.as_os_str()))",
        "configured(\"LINK\"), Some(None)",
        "configured(\"_LINK_\"), Some(None)",
        "configured(\"LIB\")",
        "configured(\"INCLUDE\")",
        "configured(\"PATH\"), None",
        "configured(\"RUSTFLAGS\"), None",
    ] {
        assert!(
            consumers.contains(required),
            "poisoned nested-Cargo command inspection lost `{required}`"
        );
    }
}

#[test]
fn archive_failures_without_settlement_evidence_are_fail_stop() {
    let platform = read("crates/semaprax-native-rust-interop-platform/src/lib.rs");
    let legacy = platform
        .split("#[cfg(not(target_os = \"macos\"))]")
        .nth(1)
        .and_then(|tail| tail.split("result.map(HeldRegularFile)").next())
        .expect("non-Darwin archive facade branch");
    assert!(legacy.contains("phase: ArchiveToolFailurePhase::Platform"));
    assert!(legacy.contains("settlement: ArchiveToolSettlement::Uncertain"));
    assert!(!legacy.contains("settlement: ArchiveToolSettlement::Settled"));
}

#[test]
fn quality_gate_and_example_are_promotion_evidence_not_a_private_claim() {
    let gates = read("docs/QUALITY-GATES.md");
    for required in [
        "public_native_rust_sdk_v1",
        "public_native_rust_sdk_ci_contract",
        "examples/calculator-rust",
    ] {
        assert!(
            gates.contains(required),
            "quality gates are missing `{required}`"
        );
    }

    let readme = read("examples/calculator-rust/README.md");
    for required in [
        "standalone example",
        "generated `semaprax-generated-native-rust-sdk` package",
        "no source or workspace dependency",
        "locked offline mode",
    ] {
        assert!(
            readme.contains(required),
            "example contract is missing `{required}`"
        );
    }
}

#[test]
fn root_package_does_not_create_a_dependency_cycle_to_the_builder() {
    let root_manifest = read("Cargo.toml");
    let dependencies = root_manifest
        .split("[dependencies]")
        .nth(1)
        .and_then(|tail| tail.split("[features]").next())
        .unwrap();
    assert!(!dependencies.contains("semaprax-native-rust-interop"));
    let builder_manifest = read("crates/semaprax-native-rust-interop-builder/Cargo.toml");
    assert!(builder_manifest.contains("semaprax = {"));
}
