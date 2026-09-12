use super::*;
use crate::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};

fn sample_binding() -> NativeProviderBindingV1 {
    NativeProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            TargetProfile::NativeC11,
            "runtime:native-c11-fixture",
        ),
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        "spx_pg_endpoint_reverse_bytes_v1",
        "semaprax-0.4.1",
    )
}

#[test]
fn render_is_byte_deterministic_for_identical_inputs() {
    let descriptor_bytes = b"fixture-descriptor-bytes".to_vec();
    let first = render_reference_provider(&descriptor_bytes, &sample_binding());
    let second = render_reference_provider(&descriptor_bytes, &sample_binding());
    assert_eq!(first, second);
}

#[test]
fn render_embeds_exact_trusted_bytes_as_byte_arrays() {
    let descriptor_bytes = vec![0u8, 1, 2, 255];
    let rendered = render_reference_provider(&descriptor_bytes, &sample_binding());
    assert!(rendered.contains("SPX_PG_TRUSTED_DESCRIPTOR_BYTES[] = {0x00,0x01,0x02,0xff};"));
    assert!(rendered.contains("SPX_PG_TRUSTED_DESCRIPTOR_LEN = 4;"));
}

#[test]
fn render_handles_empty_descriptor_bytes_without_an_invalid_c_array() {
    let rendered = render_reference_provider(&[], &sample_binding());
    assert!(rendered.contains("SPX_PG_TRUSTED_DESCRIPTOR_BYTES[] = {0};"));
    assert!(rendered.contains("SPX_PG_TRUSTED_DESCRIPTOR_LEN = 0;"));
}

#[test]
fn render_differs_when_the_trusted_descriptor_bytes_differ() {
    let a = render_reference_provider(b"one", &sample_binding());
    let b = render_reference_provider(b"two", &sample_binding());
    assert_ne!(a, b);
}

#[test]
fn header_declares_the_exact_versioned_abi_surface() {
    for symbol in [
        "spx_pg_provider_open_v1",
        "spx_pg_input_prepare_v1",
        "spx_pg_call_v1",
        "spx_pg_result_export_v1",
        "spx_pg_value_release_v1",
        "spx_pg_result_release_v1",
        "spx_pg_provider_close_v1",
    ] {
        assert!(
            HEADER_V1.contains(symbol),
            "header is missing required ABI symbol {symbol}"
        );
    }
}

#[test]
fn header_declares_opaque_incomplete_public_types() {
    // Opaque: `typedef struct spx_pg_provider_v1 spx_pg_provider_v1;` with no
    // matching `struct spx_pg_provider_v1 { ... }` body in the header itself
    // — the definition lives only in provider_body.c, never in the public
    // header, so a foreign caller cannot see or copy internal layout.
    for name in ["spx_pg_provider_v1", "spx_pg_value_v1", "spx_pg_result_v1"] {
        assert!(HEADER_V1.contains(&format!("typedef struct {name} {name};")));
        assert!(
            !HEADER_V1.contains(&format!("struct {name} {{")),
            "{name} must stay incomplete/opaque in the public header"
        );
    }
    // The definitions do live in the body, which is never part of the public
    // header a foreign consumer includes.
    for name in ["spx_pg_provider_v1", "spx_pg_value_v1", "spx_pg_result_v1"] {
        assert!(BODY_V1.contains(&format!("struct {name} {{")));
    }
}

#[test]
fn header_fixes_status_width_and_a_closed_ok_value() {
    assert!(HEADER_V1.contains("typedef int32_t spx_pg_status_v1;"));
    assert!(HEADER_V1.contains("#define SPX_PG_STATUS_OK 0"));
}

#[test]
fn header_orders_trace_ordinals_exactly_like_the_logical_vocabulary() {
    use crate::public_generic_abi::carrier::trace::TraceLabel;
    let logical_order = [
        TraceLabel::FrameValidated,
        TraceLabel::LeafAllocationStarted,
        TraceLabel::LeafAllocationCommitted,
        TraceLabel::LeafPayloadCopied,
        TraceLabel::InputValuePrepared,
        TraceLabel::InputTransferCommitted,
        TraceLabel::ExecutionStarted,
        TraceLabel::ExecutionFinished,
        TraceLabel::ResultLeafAllocationStarted,
        TraceLabel::ResultLeafAllocationCommitted,
        TraceLabel::ResultValuePrepared,
        TraceLabel::ResultCommit,
        TraceLabel::LeafRelease,
        TraceLabel::CarrierRelease,
        TraceLabel::TerminalStatus,
    ];
    let native_order = [
        "SPX_PG_TRACE_FRAME_VALIDATED",
        "SPX_PG_TRACE_LEAF_ALLOCATION_STARTED",
        "SPX_PG_TRACE_LEAF_ALLOCATION_COMMITTED",
        "SPX_PG_TRACE_LEAF_PAYLOAD_COPIED",
        "SPX_PG_TRACE_INPUT_VALUE_PREPARED",
        "SPX_PG_TRACE_INPUT_TRANSFER_COMMITTED",
        "SPX_PG_TRACE_EXECUTION_STARTED",
        "SPX_PG_TRACE_EXECUTION_FINISHED",
        "SPX_PG_TRACE_RESULT_LEAF_ALLOCATION_STARTED",
        "SPX_PG_TRACE_RESULT_LEAF_ALLOCATION_COMMITTED",
        "SPX_PG_TRACE_RESULT_VALUE_PREPARED",
        "SPX_PG_TRACE_RESULT_COMMIT",
        "SPX_PG_TRACE_LEAF_RELEASE",
        "SPX_PG_TRACE_CARRIER_RELEASE",
        "SPX_PG_TRACE_TERMINAL_STATUS",
    ];
    assert_eq!(logical_order.len(), native_order.len());
    for (index, name) in native_order.iter().enumerate() {
        assert!(
            HEADER_V1.contains(&format!("#define {name} {index}")),
            "{name} must be defined as ordinal {index}, matching TraceLabel's own declared order"
        );
    }
}

#[test]
fn body_bounds_match_the_boundary_profile_constants_verbatim() {
    use crate::public_generic_abi::boundary_profile::{
        MAX_BYTES_PER_LEAF, MAX_OWNED_LEAVES_PER_INSTANCE, MAX_TOTAL_PAYLOAD_BYTES,
    };
    assert!(BODY_V1.contains(&format!(
        "#define SPX_PG_MAX_OWNED_LEAVES {}u",
        MAX_OWNED_LEAVES_PER_INSTANCE
    )));
    assert!(BODY_V1.contains(&format!(
        "#define SPX_PG_MAX_BYTES_PER_LEAF ({}u * 1024u)",
        MAX_BYTES_PER_LEAF / 1024
    )));
    assert!(BODY_V1.contains(&format!(
        "#define SPX_PG_MAX_TOTAL_PAYLOAD_BYTES ({}u * 1024u * 1024u)",
        MAX_TOTAL_PAYLOAD_BYTES / (1024 * 1024)
    )));
}

/// Real, compiled-and-executed regression for a defect found during issue
/// #134's audit: `spx_pg_result_release_v1` released its leaves with a
/// second, untraced per-leaf loop instead of routing through
/// `spx_pg_release_leaves` (the same helper `spx_pg_value_release_v1` already
/// used), so releasing a *result* handle recorded none of the normalized
/// trace's `LEAF_RELEASE`/`CARRIER_RELEASE` events while releasing a *value*
/// handle recorded both. This compiles the real rendered provider, drives one
/// full success round trip through the real C ABI, and asserts the exact two
/// trace events `spx_pg_result_release_v1` must append.
#[test]
fn result_release_records_the_same_normalized_trace_events_as_value_release() {
    let descriptor_bytes = b"template-tests-result-release-trace-fixture".to_vec();
    let provider_source = render_reference_provider(&descriptor_bytes, &sample_binding());

    // One owned leaf, byte 0x41: [u64 leaf_count=1][u64 leaf_len=1][0x41].
    const MAIN_FRAGMENT: &str = r#"
#include <stdio.h>

static const uint8_t CARRIER_BYTES[] = {
    0x01,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x01,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x41
};

int main(void) {
    spx_pg_provider_v1 *provider = NULL;
    if (spx_pg_provider_open_v1(SPX_PG_TRUSTED_DESCRIPTOR_BYTES, SPX_PG_TRUSTED_DESCRIPTOR_LEN,
                                 SPX_PG_TRUSTED_BINDING_BYTES, SPX_PG_TRUSTED_BINDING_LEN,
                                 &provider) != SPX_PG_STATUS_OK || provider == NULL) {
        return 1;
    }
    spx_pg_value_v1 *input = NULL;
    if (spx_pg_input_prepare_v1(provider, CARRIER_BYTES, sizeof(CARRIER_BYTES), &input) !=
            SPX_PG_STATUS_OK || input == NULL) {
        return 2;
    }
    spx_pg_result_v1 *result = NULL;
    if (spx_pg_call_v1(provider, input, &result) != SPX_PG_STATUS_OK || result == NULL) {
        return 3;
    }
    size_t before = spx_pg_test_trace_len_v1();
    if (spx_pg_result_release_v1(&result) != SPX_PG_STATUS_OK || result != NULL) {
        return 4;
    }
    size_t after = spx_pg_test_trace_len_v1();
    if (after != before + 2) {
        return 5;
    }
    if (spx_pg_test_trace_label_v1(after - 2) != SPX_PG_TRACE_LEAF_RELEASE) {
        return 6;
    }
    if (spx_pg_test_trace_label_v1(after - 1) != SPX_PG_TRACE_CARRIER_RELEASE) {
        return 7;
    }
    if (spx_pg_provider_close_v1(&provider) != SPX_PG_STATUS_OK) {
        return 8;
    }
    if (spx_pg_test_live_allocations_v1() != 0) {
        return 9;
    }
    printf("result-release-records-the-normalized-trace\n");
    return 0;
}
"#;

    let source = format!("{provider_source}\n{MAIN_FRAGMENT}");

    let compiler = std::env::var_os("CLANG").map_or_else(
        || std::path::PathBuf::from("clang"),
        std::path::PathBuf::from,
    );
    let root = std::env::temp_dir().join(format!(
        "semaprax-pg-native-result-release-trace-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let source_path = root.join("probe.c");
    std::fs::write(&source_path, &source).unwrap();
    let executable = root.join(format!("probe{}", std::env::consts::EXE_SUFFIX));
    let built = std::process::Command::new(&compiler)
        .current_dir(&root)
        .args(["-std=c11", "-O0", "-Wall", "-Wextra", "-Werror"])
        .arg(&source_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}: {}",
        root.display(),
        String::from_utf8_lossy(&built.stderr)
    );
    let ran = std::process::Command::new(&executable)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        ran.status.success(),
        "{}: stdout={} stderr={}",
        root.display(),
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    assert_eq!(ran.stdout, b"result-release-records-the-normalized-trace\n");
}
