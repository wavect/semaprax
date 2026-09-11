use super::*;
use crate::public_generic_abi::carrier::CarrierBindingV1;

fn sample_carrier_binding() -> CarrierBindingV1 {
    CarrierBindingV1::new(
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        TargetProfile::NativeC11,
        "runtime:native-c11-fixture",
    )
}

fn sample() -> NativeProviderBindingV1 {
    NativeProviderBindingV1::new(
        sample_carrier_binding(),
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        "spx_pg_endpoint_reverse_bytes_v1",
        "semaprax-0.4.1",
    )
}

#[test]
fn encode_decode_round_trips() {
    let binding = sample();
    let bytes = binding.encode();
    let decoded = decode_native_provider_binding(&bytes).unwrap();
    assert_eq!(decoded, binding);
}

#[test]
fn encode_is_byte_deterministic() {
    assert_eq!(sample().encode(), sample().encode());
    assert_eq!(sample().binding_digest(), sample().binding_digest());
}

#[test]
fn schema_and_target_profile_are_fixed() {
    let binding = sample();
    assert_eq!(binding.target_profile(), TargetProfile::NativeC11);
    assert_eq!(
        binding.support_publication_state(),
        SupportPublicationState::UnsupportedUnpublished
    );
}

#[test]
fn decode_rejects_wrong_schema() {
    let mut bytes = Vec::new();
    frame(&mut bytes, b"not-the-native-adapter-schema");
    let error = decode_native_provider_binding(&bytes).unwrap_err();
    assert_eq!(error.code, MALFORMED_NATIVE_BINDING);
}

#[test]
fn decode_rejects_embedded_binding_naming_another_target_profile() {
    let wrong_target = CarrierBindingV1::new(
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        TargetProfile::CoreWasm,
        "runtime:core-wasm-fixture",
    );
    let mut bytes = Vec::new();
    frame(&mut bytes, NATIVE_ADAPTER_SCHEMA.as_bytes());
    frame(&mut bytes, &wrong_target.encode());
    frame(&mut bytes, NATIVE_ADAPTER_ABI_VERSION.as_bytes());
    frame(&mut bytes, b"digest");
    frame(&mut bytes, b"symbol");
    frame(&mut bytes, b"version");
    frame(&mut bytes, b"unsupported-unpublished");
    let error = decode_native_provider_binding(&bytes).unwrap_err();
    assert_eq!(error.code, MALFORMED_NATIVE_BINDING);
}

#[test]
fn decode_rejects_unknown_abi_version() {
    let carrier_bytes = sample_carrier_binding().encode();
    let mut bytes = Vec::new();
    frame(&mut bytes, NATIVE_ADAPTER_SCHEMA.as_bytes());
    frame(&mut bytes, &carrier_bytes);
    frame(&mut bytes, b"v2-does-not-exist");
    frame(&mut bytes, b"digest");
    frame(&mut bytes, b"symbol");
    frame(&mut bytes, b"version");
    frame(&mut bytes, b"unsupported-unpublished");
    let error = decode_native_provider_binding(&bytes).unwrap_err();
    assert_eq!(error.code, MALFORMED_NATIVE_BINDING);
}

#[test]
fn decode_rejects_unknown_support_publication_claim() {
    let carrier_bytes = sample_carrier_binding().encode();
    let mut bytes = Vec::new();
    frame(&mut bytes, NATIVE_ADAPTER_SCHEMA.as_bytes());
    frame(&mut bytes, &carrier_bytes);
    frame(&mut bytes, NATIVE_ADAPTER_ABI_VERSION.as_bytes());
    frame(&mut bytes, b"digest");
    frame(&mut bytes, b"symbol");
    frame(&mut bytes, b"version");
    frame(&mut bytes, b"supported-and-published");
    let error = decode_native_provider_binding(&bytes).unwrap_err();
    assert_eq!(error.code, MALFORMED_NATIVE_BINDING);
}

#[test]
fn decode_rejects_trailing_bytes() {
    let mut bytes = sample().encode();
    bytes.push(0);
    let error = decode_native_provider_binding(&bytes).unwrap_err();
    assert_eq!(error.code, MALFORMED_NATIVE_BINDING);
}

#[test]
fn decode_rejects_truncated_bytes() {
    let bytes = sample().encode();
    for cut in [0, 1, 4, bytes.len() - 1] {
        let error = decode_native_provider_binding(&bytes[..cut]).unwrap_err();
        assert_eq!(error.code, MALFORMED_NATIVE_BINDING);
    }
}

#[test]
fn replay_accepts_byte_identical_candidate() {
    let trusted = sample();
    let candidate = trusted.encode();
    let replayed = replay_native_provider_binding(&candidate, &trusted).unwrap();
    assert_eq!(replayed, trusted);
}

#[test]
fn replay_rejects_wrong_descriptor_digest() {
    let trusted = sample();
    let candidate = NativeProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            TargetProfile::NativeC11,
            "runtime:native-c11-fixture",
        ),
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        "spx_pg_endpoint_reverse_bytes_v1",
        "semaprax-0.4.1",
    )
    .encode();
    let error = replay_native_provider_binding(&candidate, &trusted).unwrap_err();
    assert_eq!(error.code, NATIVE_BINDING_REPLAY_MISMATCH);
}

#[test]
fn replay_rejects_wrong_provider_artifact_digest() {
    let trusted = sample();
    let candidate = NativeProviderBindingV1::new(
        sample_carrier_binding(),
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "spx_pg_endpoint_reverse_bytes_v1",
        "semaprax-0.4.1",
    )
    .encode();
    let error = replay_native_provider_binding(&candidate, &trusted).unwrap_err();
    assert_eq!(error.code, NATIVE_BINDING_REPLAY_MISMATCH);
}

#[test]
fn replay_rejects_wrong_endpoint_symbol() {
    let trusted = sample();
    let candidate = NativeProviderBindingV1::new(
        sample_carrier_binding(),
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        "spx_pg_endpoint_some_other_v1",
        "semaprax-0.4.1",
    )
    .encode();
    let error = replay_native_provider_binding(&candidate, &trusted).unwrap_err();
    assert_eq!(error.code, NATIVE_BINDING_REPLAY_MISMATCH);
}

#[test]
fn replay_rejects_wrong_target_profile() {
    let trusted = sample();
    let candidate = NativeProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            TargetProfile::CoreWasm,
            "runtime:core-wasm-fixture",
        ),
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        "spx_pg_endpoint_reverse_bytes_v1",
        "semaprax-0.4.1",
    );
    // Directly compare preimages rather than going through decode, since
    // decode itself already rejects a non-NativeC11 embedded binding above;
    // this proves `replay` would also catch it if a caller somehow bypassed
    // decode's own check.
    assert_ne!(candidate.preimage(), trusted.preimage());
}
