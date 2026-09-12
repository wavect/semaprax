//! Issue #173: hostile replay for the Public Generic Descriptor v1 and
//! Carrier v1 *reference codecs* themselves — `descriptor.rs`, `carrier.rs`,
//! `carrier/frame.rs`, `native/binding.rs`, and `wasm/binding.rs` — rather
//! than the generated calling consumers (`tests/public_generic_native_adapter_v1`,
//! `tests/public_generic_wasm_adapter_v1`) or the metadata-consumer grammar
//! corpus (`public_generic_consumers.rs`, PG-5's separate "grammar half").
//!
//! Audited state at issue #173's baseline (`ae25c6a4`, then commits
//! `8daf13f5`/`088b4130`): the reference decoders for all five codecs above
//! already carry substantial hostile coverage in their own `#[cfg(test)]`
//! modules (`src/public_generic_abi/descriptor/tests.rs`,
//! `src/public_generic_abi/carrier/tests.rs`,
//! `src/public_generic_abi/carrier/frame/tests.rs`) — truncation, trailing
//! bytes, an unknown schema literal, reordered/duplicate/extra fields (via
//! the shared "any byte past the fixed field count is trailing bytes"
//! mechanism), an oversized length claim, a stale
//! `boundary_profile`/`type_grammar_schema` version, cross-paired staleness
//! on every bound field, a duplicate leaf path, a missing/extra/reordered
//! leaf, a wrong direction/descriptor/endpoint binding, and a tampered
//! self-digest. Two closed refusal classes named explicitly in the issue's
//! scope ("invalid UTF-8", "variant tag") were NOT exercised anywhere in
//! that coverage, confirmed by `rg -n "utf8|UTF-8" src/public_generic_abi`
//! and `rg -n "unknown leaf kind"` turning up the production branch in five
//! codecs and the `LeafKind::from_tag` rejection branch in
//! `carrier/frame.rs`, each with zero asserting test:
//!
//! - every one of `descriptor.rs`, `carrier.rs`, `carrier/frame.rs`,
//!   `native/binding.rs`, and `wasm/binding.rs` decodes a string field with
//!   `String::from_utf8`/`str::from_utf8` and maps a failure to that codec's
//!   own malformed-input code — never exercised by an invalid-UTF-8 fixture
//!   anywhere in the repository;
//! - `LeafKind` (`carrier/frame.rs`) is a closed one-variant enum this round
//!   (`Bytes` only, tag `0`), so no *second legal* variant exists to
//!   substitute — but the decoder's `LeafKind::from_tag` rejection of an
//!   unrecognized tag byte (a real, reachable "variant tag" hostile case,
//!   forging a tag no public constructor can produce) was unexercised.
//!
//! This module closes both gaps against the real public codec API only
//! (`decode`/`parse_bounded`/`decode_binding`/`decode_native_provider_binding`/
//! `decode_wasm_provider_binding`), never a private helper, so it is exactly
//! what an external, independent replayer sees. Every case: encode a
//! well-formed fixture (the positive control, asserted ACCEPTED first),
//! locate one field's own plaintext content verbatim and uniquely inside
//! the encoded bytes, overwrite only that content in place (preserving
//! every length prefix, so the SPECIFIC branch under test — content
//! validity, not framing or length — is what actually rejects it), and
//! assert the one exact refusal code (and, where the codec's message is
//! specific enough to distinguish it from every other malformed-input
//! reason that code also covers, the exact message substring).
//!
//! Deliberately out of scope here, and left to the existing coverage cited
//! above or to the calling-consumer/carrier-frame corpora that already
//! cover them: reorder/duplicate/extra-field cases (already the same
//! "trailing bytes" mechanism under every codec above — see each codec's own
//! `tests.rs` — so a byte-identical repetition here would assert the same
//! branch under a different name, exactly the "proves less than its name
//! claims" pattern this issue's audit warns against), stale
//! Project/ProgramRoot/artifact/target associations (`descriptor/tests.rs`'s
//! cross-paired-field test and this issue's own
//! `binding_wrong_target_profile`/`binding_valid_for_different_artifact`
//! cases in the shared calling-consumer corpus), and cleanup-plan/release
//! substitution (`carrier/tests.rs`'s release-order tests, already exercised
//! through the real cross-engine settlement corpus, issue #162).

use semaprax::public_generic_abi::carrier::frame::{
    parse_bounded, CarrierLeaf, LeafKind, LogicalCarrierFrame,
};
use semaprax::public_generic_abi::carrier::trace::Direction;
use semaprax::public_generic_abi::carrier::{
    decode_binding, CarrierBindingV1, TargetProfile, MALFORMED_CARRIER,
};
use semaprax::public_generic_abi::descriptor::{
    decode as decode_descriptor, DescriptorV1, InstanceBinding, MALFORMED_DESCRIPTOR,
};
use semaprax::public_generic_abi::native::binding::{
    decode_native_provider_binding, NativeProviderBindingV1, MALFORMED_NATIVE_BINDING,
};
use semaprax::public_generic_abi::wasm::binding::{
    decode_wasm_provider_binding, WasmProviderBindingV1, MALFORMED_WASM_BINDING,
};

/// Locate `needle`'s one and only literal occurrence in `haystack`, panicking
/// (rather than silently mutating the wrong bytes, or a repeated one) if it
/// is absent or not unique. Every fixture field value in this module is
/// deliberately long and distinctive so a real collision never happens; this
/// assertion is the safety net if that ever stops being true.
fn locate_unique(haystack: &[u8], needle: &[u8]) -> usize {
    assert!(
        !needle.is_empty(),
        "a substitution target must not be empty"
    );
    let mut matches = haystack
        .windows(needle.len())
        .enumerate()
        .filter(|(_, window)| *window == needle)
        .map(|(position, _)| position);
    let first = matches
        .next()
        .unwrap_or_else(|| panic!("fixture field content not found verbatim in the encoded bytes"));
    assert!(
        matches.next().is_none(),
        "fixture field content is not unique in the encoded bytes; use a more distinctive value"
    );
    first
}

/// Replace the one unique literal occurrence of `original` inside `bytes`
/// with `replacement` of the SAME length, so every length prefix in the
/// buffer stays correct and only the field's own content changes -- the
/// specific branch this module targets (content validity) is what rejects
/// the mutated bytes, not framing or a length mismatch.
fn substitute_field_content(bytes: &[u8], original: &[u8], replacement: &[u8]) -> Vec<u8> {
    assert_eq!(
        original.len(),
        replacement.len(),
        "a content substitution must preserve the field's wire length"
    );
    let at = locate_unique(bytes, original);
    let mut mutated = bytes.to_vec();
    mutated[at..at + original.len()].copy_from_slice(replacement);
    mutated
}

/// Bytes that are never valid UTF-8 in any position (lead or continuation),
/// repeated to the requested length -- an unambiguous "invalid UTF-8"
/// fixture regardless of what content it replaces.
fn invalid_utf8_of_len(len: usize) -> Vec<u8> {
    vec![0xFFu8; len]
}

// ---------------------------------------------------------------------
// Public Generic Descriptor v1 (`descriptor.rs`)
// ---------------------------------------------------------------------

fn sample_descriptor() -> DescriptorV1 {
    DescriptorV1::new(
        "issue173.hostile.descriptor.export-id.fixture",
        "issue173_hostile_descriptor_export_name_fixture",
        "sha256:issue173-hostile-descriptor-program-root-fixture",
        "sha256:issue173-hostile-descriptor-source-projection-fixture",
        "sha256:issue173-hostile-descriptor-public-surface-fixture",
        InstanceBinding {
            term: "@11:issue173.hostile.descriptor.pair<bytes,bool>".to_owned(),
            instance_digest: "sha256:issue173-hostile-descriptor-input-digest-fixture".to_owned(),
        },
        InstanceBinding {
            term: "@11:issue173.hostile.descriptor.pair<bytes,i64>".to_owned(),
            instance_digest: "sha256:issue173-hostile-descriptor-result-digest-fixture".to_owned(),
        },
    )
}

#[test]
fn descriptor_decode_rejects_invalid_utf8_in_the_export_id_field() {
    let trusted = sample_descriptor();
    let valid_bytes = trusted.encode();

    // Positive control: the unmutated, well-formed fixture decodes.
    decode_descriptor(&valid_bytes).expect("the well-formed descriptor fixture must decode");

    let export_id = trusted.export_id().as_bytes();
    let mutated = substitute_field_content(
        &valid_bytes,
        export_id,
        &invalid_utf8_of_len(export_id.len()),
    );
    let error = decode_descriptor(&mutated)
        .expect_err("an export_id field with invalid UTF-8 bytes must not decode");
    assert_eq!(error.code, MALFORMED_DESCRIPTOR);
    assert!(
        error.message.contains("is not UTF-8"),
        "expected the specific invalid-UTF-8 refusal reason, got: {}",
        error.message
    );
}

// ---------------------------------------------------------------------
// Public Generic Carrier v1 binding codec (`carrier.rs`)
// ---------------------------------------------------------------------

fn sample_carrier_binding() -> CarrierBindingV1 {
    CarrierBindingV1::new(
        "sha256:issue173-hostile-carrier-binding-descriptor-identity-fixture",
        TargetProfile::NativeC11,
        "runtime:issue173-hostile-carrier-binding-runtime-identity-fixture",
    )
}

#[test]
fn carrier_binding_decode_rejects_invalid_utf8_in_the_runtime_identity_field() {
    let trusted = sample_carrier_binding();
    let valid_bytes = trusted.encode();

    decode_binding(&valid_bytes).expect("the well-formed carrier binding fixture must decode");

    let runtime_identity = trusted.runtime_identity().as_bytes();
    let mutated = substitute_field_content(
        &valid_bytes,
        runtime_identity,
        &invalid_utf8_of_len(runtime_identity.len()),
    );
    let error = decode_binding(&mutated)
        .expect_err("a runtime_identity field with invalid UTF-8 bytes must not decode");
    assert_eq!(error.code, MALFORMED_CARRIER);
    assert!(
        error.message.contains("is not UTF-8"),
        "expected the specific invalid-UTF-8 refusal reason, got: {}",
        error.message
    );
}

// ---------------------------------------------------------------------
// Native provider binding codec (`native/binding.rs`)
// ---------------------------------------------------------------------

fn sample_native_provider_binding() -> NativeProviderBindingV1 {
    NativeProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:issue173-hostile-native-binding-descriptor-identity-fixture",
            TargetProfile::NativeC11,
            "runtime:issue173-hostile-native-binding-runtime-identity-fixture",
        ),
        "sha256:issue173-hostile-native-binding-provider-artifact-digest-fixture",
        "spx_pg_endpoint_issue173_hostile_native_binding_fixture",
        "semaprax-0.4.1",
    )
}

#[test]
fn native_provider_binding_decode_rejects_invalid_utf8_in_the_exported_endpoint_symbol_field() {
    let trusted = sample_native_provider_binding();
    let valid_bytes = trusted.encode();

    decode_native_provider_binding(&valid_bytes)
        .expect("the well-formed native provider binding fixture must decode");

    let symbol = trusted.exported_endpoint_symbol().as_bytes();
    let mutated =
        substitute_field_content(&valid_bytes, symbol, &invalid_utf8_of_len(symbol.len()));
    let error = decode_native_provider_binding(&mutated)
        .expect_err("an exported_endpoint_symbol field with invalid UTF-8 bytes must not decode");
    assert_eq!(error.code, MALFORMED_NATIVE_BINDING);
    assert!(
        error.message.contains("is not UTF-8"),
        "expected the specific invalid-UTF-8 refusal reason, got: {}",
        error.message
    );
}

// ---------------------------------------------------------------------
// Wasm provider binding codec (`wasm/binding.rs`)
// ---------------------------------------------------------------------

fn sample_wasm_provider_binding() -> WasmProviderBindingV1 {
    WasmProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:issue173-hostile-wasm-binding-descriptor-identity-fixture",
            TargetProfile::CoreWasm,
            "runtime:issue173-hostile-wasm-binding-runtime-identity-fixture",
        ),
        "sha256:issue173-hostile-wasm-binding-provider-artifact-digest-fixture",
        "spx_pg_endpoint_issue173_hostile_wasm_binding_fixture",
        "semaprax-0.4.1",
    )
}

#[test]
fn wasm_provider_binding_decode_rejects_invalid_utf8_in_the_exported_endpoint_export_name_field() {
    let trusted = sample_wasm_provider_binding();
    let valid_bytes = trusted.encode();

    decode_wasm_provider_binding(&valid_bytes)
        .expect("the well-formed wasm provider binding fixture must decode");

    let export_name = trusted.exported_endpoint_export_name().as_bytes();
    let mutated = substitute_field_content(
        &valid_bytes,
        export_name,
        &invalid_utf8_of_len(export_name.len()),
    );
    let error = decode_wasm_provider_binding(&mutated).expect_err(
        "an exported_endpoint_export_name field with invalid UTF-8 bytes must not decode",
    );
    assert_eq!(error.code, MALFORMED_WASM_BINDING);
    assert!(
        error.message.contains("is not UTF-8"),
        "expected the specific invalid-UTF-8 refusal reason, got: {}",
        error.message
    );
}

// ---------------------------------------------------------------------
// Logical carrier frame codec (`carrier/frame.rs`)
// ---------------------------------------------------------------------

const FRAME_LEAF_PATH: &str = "issue173.hostile.carrier-frame.leaf-path.fixture";

fn sample_carrier_frame() -> LogicalCarrierFrame {
    LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:issue173-hostile-carrier-frame-descriptor-digest-fixture",
        "sha256:issue173-hostile-carrier-frame-endpoint-identity-digest-fixture",
        "sha256:issue173-hostile-carrier-frame-instance-identity-digest-fixture",
        "sha256:issue173-hostile-carrier-frame-leaf-inventory-digest-fixture",
        vec![CarrierLeaf::new(
            FRAME_LEAF_PATH,
            LeafKind::Bytes,
            b"issue173-fixture-payload".to_vec(),
        )],
    )
}

#[test]
fn carrier_frame_parse_bounded_rejects_invalid_utf8_in_a_leaf_path() {
    let trusted = sample_carrier_frame();
    let valid_bytes = trusted.encode();

    parse_bounded(&valid_bytes).expect("the well-formed carrier frame fixture must decode");

    let path = FRAME_LEAF_PATH.as_bytes();
    let mutated = substitute_field_content(&valid_bytes, path, &invalid_utf8_of_len(path.len()));
    let error =
        parse_bounded(&mutated).expect_err("a leaf path with invalid UTF-8 bytes must not decode");
    assert_eq!(error.code, MALFORMED_CARRIER);
    assert!(
        error.message.contains("is not UTF-8"),
        "expected the specific invalid-UTF-8 refusal reason, got: {}",
        error.message
    );
}

/// A "variant tag" hostile case in the sense issue #173 asks for: `LeafKind`
/// is a closed one-variant enum today (`Bytes`, wire tag `0`) and no public
/// constructor can produce a second variant, so this forges the ONE thing a
/// real attacker controls that no legitimate encoder ever would -- an
/// unrecognized tag byte -- directly in the wire bytes, immediately after
/// the leaf path whose exact end this test locates first (never assumed).
#[test]
fn carrier_frame_parse_bounded_rejects_an_unrecognized_leaf_kind_variant_tag() {
    let trusted = sample_carrier_frame();
    let valid_bytes = trusted.encode();

    parse_bounded(&valid_bytes).expect("the well-formed carrier frame fixture must decode");

    let path = FRAME_LEAF_PATH.as_bytes();
    let path_at = locate_unique(&valid_bytes, path);
    let tag_at = path_at + path.len();
    assert_eq!(
        valid_bytes[tag_at], 0,
        "sanity check: the byte immediately after the leaf path must be the LeafKind::Bytes wire \
         tag (0) -- if this fails, the offset arithmetic above no longer matches the real wire \
         layout and must be re-derived, not patched around"
    );

    let mut mutated = valid_bytes.clone();
    // No `LeafKind` variant is tagged `1` today (`Bytes` is the only variant,
    // tagged `0`); this is exactly the byte `LeafKind::from_tag` must refuse.
    mutated[tag_at] = 1;
    let error =
        parse_bounded(&mutated).expect_err("an unrecognized leaf-kind wire tag must not decode");
    assert_eq!(error.code, MALFORMED_CARRIER);
    assert!(
        error.message.contains("unknown leaf kind"),
        "expected the specific unknown-leaf-kind refusal reason, got: {}",
        error.message
    );
}
