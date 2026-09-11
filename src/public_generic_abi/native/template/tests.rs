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
