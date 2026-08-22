use std::fs;
use std::path::Path;

fn read(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

#[test]
fn hosted_matrix_routes_public_sdk_evidence_on_all_three_operating_systems() {
    let workflow = read(".github/workflows/ci.yml");
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
