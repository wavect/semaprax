use std::fs;
use std::path::Path;

#[test]
fn private_callable_v3_physical_ci_evidence_is_mandatory() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .expect("read the pinned CI workflow");
    let provider = fs::read_to_string(root.join("src/codegen/native_callable_provider_v3.rs"))
        .expect("read the private callable-v3 provider");
    let joint = fs::read_to_string(
        root.join("crates/semaprax-native-host/src/settlement_host_v3_integration.rs"),
    )
    .expect("read the private callable-v3 joint integration");

    for required in [
        "Require Windows callable-v2 and private callable-v3 physical evidence",
        "Require private callable-v3 provider sanitizers (Linux)",
        "SEMAPRAX_REQUIRE_CALLABLE_V3_SANITIZERS: \"1\"",
        "CLANG: clang",
        "codegen::native_callable_provider_v3::tests::authoritative_fourteen_case_graph_providers_execute_and_settle_at_o0_o2",
        "settlement_host_v3_integration::generated_provider_loader_host_v3_end_to_end_is_exact",
        "settlement_host_v3_integration::generated_provider_loader_host_v3_physical_failures_are_durable_at_o0_o2",
        "Run dynamically loaded callable-v2 and private callable-v3 paths under ASan and UBSan",
        "Private iOS static loader + host cross-check",
        "Require every private iOS static target to compile",
        "aarch64-apple-ios",
        "aarch64-apple-ios-sim",
        "x86_64-apple-ios",
        "aarch64-apple-ios-macabi",
        "x86_64-apple-ios-macabi",
        "cargo check --locked -p semaprax-native-loader --target \"$target\" --all-targets",
        "cargo check --locked -p semaprax-native-host --target \"$target\" --all-targets",
        "loader_tree=\"$(cargo tree --locked -p semaprax-native-loader --target \"$target\" -e normal)\"",
        "host_tree=\"$(cargo tree --locked -p semaprax-native-host --target \"$target\" -e normal)\"",
        "if grep -q libloading <<<\"$loader_tree\" || grep -q libloading <<<\"$host_tree\"",
        "iOS static loader/host target unexpectedly resolved libloading",
    ] {
        assert!(
            workflow.contains(required),
            "CI lost mandatory private callable-v3 evidence: {required}"
        );
    }
    assert!(provider
        .contains("fn authoritative_fourteen_case_graph_providers_execute_and_settle_at_o0_o2()"));
    assert!(provider.contains("SEMAPRAX_REQUIRE_CALLABLE_V3_SANITIZERS"));
    assert!(joint.contains("fn generated_provider_loader_host_v3_end_to_end_is_exact()"));
    assert!(joint
        .contains("fn generated_provider_loader_host_v3_physical_failures_are_durable_at_o0_o2()"));
    assert!(joint.contains("REQUIRED_SANITIZERS_ENV"));
}
