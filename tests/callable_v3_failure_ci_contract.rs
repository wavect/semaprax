use std::fs;
use std::path::Path;

#[test]
fn callable_v3_failure_injection_evidence_is_mandatory() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .expect("read the pinned CI workflow");
    let provider = fs::read_to_string(root.join("src/codegen/native_callable_provider_v3.rs"))
        .expect("read the private callable-v3 provider");
    let abi = fs::read_to_string(root.join("docs/NATIVE-CALLABLE-ABI-V3.md"))
        .expect("read the callable-v3 ABI");
    let test_name =
        "physical_failure_injection_and_durable_settlement_boundaries_are_exact_at_o0_o2";

    assert!(workflow.matches(test_name).count() >= 2);
    for required in [
        "SEMAPRAX_REQUIRE_CALLABLE_V3_SANITIZERS: \"1\"",
        "ASAN_OPTIONS: detect_leaks=0:halt_on_error=1:abort_on_error=1",
        "UBSAN_OPTIONS: halt_on_error=1:print_stacktrace=1",
    ] {
        assert!(workflow.contains(required), "CI lost `{required}`");
    }
    for required in [
        "fn physical_failure_injection_and_durable_settlement_boundaries_are_exact_at_o0_o2()",
        "spx_v3_maybe_physical_failure",
        "SPX_V3_FAULT_RESPONSE_OFFSET",
        "SPX_V3_FAULT_FRAME_OFFSET",
        "SPX_V3_FAULT_CANDIDATE_OFFSET",
        "SPX_V3_FAULT_FINALIZER_BOUNDARY",
    ] {
        assert!(provider.contains(required), "provider lost `{required}`");
    }
    for required in [
        "Pre-execute",
        "`AbortHostUnwind` uses frame return tag 3",
        "reserved sentinel `0xFFFF_FFFE`",
        "execute is not entered",
        "certified abort settlement",
        "host receipt",
    ] {
        assert!(abi.contains(required), "ABI lost `{required}`");
    }
}
