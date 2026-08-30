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
        assert!(public_sdk.contains(runner), "CI matrix is missing {runner}");
    }
    for required in [
        "public_native_rust_sdk_v1",
        "public_native_rust_sdk_ci_contract",
        "examples/calculator-rust/Cargo.toml",
        "SEMAPRAX_ARCHIVER",
        "SEMAPRAX_REQUIRE_PUBLIC_NATIVE_RUST_SDK",
        "SEMAPRAX_REQUIRE_DARWIN_REAL_ARCHIVE",
        "tests::darwin_real_d_archive_is_exact_and_reproducible_across_tool_versions",
        "cargo test --locked --offline -p semaprax-native-rust-interop-platform-sys --lib tests::darwin_real_d_archive_is_exact_and_reproducible_across_tool_versions",
    ] {
        assert!(
            public_sdk.contains(required),
            "hosted public Native Rust SDK evidence is missing `{required}`"
        );
    }
    assert!(public_sdk.contains("--locked"));
    assert!(public_sdk.contains("--offline"));
    let dependency_fetch = public_sdk
        .find("Fetch the root workspace and standalone Rust consumer dependency closures")
        .expect("Public SDK dependency fetch");
    let darwin_gate = public_sdk
        .find("Require exact reproducible Darwin archive admission")
        .expect("Public SDK Darwin archive gate");
    assert!(
        dependency_fetch < darwin_gate,
        "the Darwin archive gate must run offline after the complete dependency fetch",
    );
    for required in [
        "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=%SEMAPRAX_LINKER%",
        "echo LINK=",
        "echo _LINK_=",
        "Require minimal effectful public SDK build on Windows",
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
    assert!(
        !public_sdk.contains("continue-on-error"),
        "every Public Native Rust SDK host and step must be blocking"
    );
}

#[test]
fn node_entrypoint_stays_relative_to_the_canonical_fixture_authority() {
    let consumer = read("tests/public_native_rust_sdk_v1.rs");
    let helper = consumer
        .split_once("fn calculator_node_command(fixture: &Path) -> Command {")
        .and_then(|(_, tail)| tail.split_once("\n}\n"))
        .map(|(body, _)| body)
        .expect("calculator Node command helper");
    assert!(helper.contains("command.current_dir(fixture).arg(\"calculator.mjs\");"));
    assert_eq!(
        consumer
            .matches("calculator_node_command(&fixture.0)")
            .count(),
        2,
        "the command-inspection regression and real Wasm execution must share the helper"
    );
    assert!(
        !consumer.contains(".arg(fixture.0.join(\"calculator.mjs\"))"),
        "Node 22 on Windows cannot resolve a verbatim absolute entrypoint"
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

    for (path, expected_commands) in [
        ("tests/frame_payload_product_v1.rs", 1),
        ("tests/public_native_rust_owned_data_sdk_v1.rs", 3),
        ("tests/project_native_rust_owned_utf8_v1.rs", 1),
    ] {
        let consumer = read(path);
        assert!(consumer.contains("mod native_rust_cargo;"), "{path}");
        assert_eq!(
            consumer
                .matches("native_rust_cargo::cargo_command()")
                .count(),
            expected_commands,
            "{path} must bind every generated-package Cargo invocation"
        );
        assert!(!consumer.contains("Command::new(\"cargo\")"), "{path}");
        assert!(
            !consumer.contains("Command::new(env!(\"CARGO\"))"),
            "{path}"
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
fn general_windows_matrix_binds_the_same_explicit_nested_cargo_linker() {
    let workflow = read(".github/workflows/ci.yml");
    let verify = workflow
        .split_once("\n  verify:\n")
        .map(|(_, tail)| tail)
        .and_then(|tail| tail.split_once("\n  desktop-native-product:\n"))
        .map(|(verify, _)| verify)
        .expect("general Rust verification matrix");
    for required in [
        "echo CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=%SEMAPRAX_LINKER%",
        "echo LINK=",
        "echo _LINK_=",
    ] {
        assert!(
            verify.contains(required),
            "general Windows matrix lost `{required}`"
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
    let mut normal_dependencies = false;
    for line in root_manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            normal_dependencies = line == "[dependencies]" || line.ends_with(".dependencies]");
        } else if normal_dependencies {
            assert!(
                !line.starts_with("semaprax-"),
                "private normal dependency: {line}"
            );
        }
    }
    let builder_manifest = read("crates/semaprax-native-rust-interop-builder/Cargo.toml");
    assert!(builder_manifest.contains("semaprax = {"));
}
