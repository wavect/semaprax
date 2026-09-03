use std::path::Path;

use super::*;

fn artifact() -> PrivateGenericRecordComponentArtifactV7 {
    let program = crate::parse(
        wasm::GENERIC_RECORD_COMPONENT_SOURCE_V7,
        Path::new("component-generic-record-v7.spx"),
    )
    .unwrap();
    emit_private_generic_record_component_v7(&program).unwrap()
}

#[test]
fn deterministic_v7_component_is_upstream_valid() {
    let first = artifact();
    assert_eq!(first.source_revision(), SOURCE_REVISION_KAT);
    assert_eq!(first.generated_core_digest(), GENERATED_CORE_KAT);
    assert_eq!(
        first.profile_digest(),
        [
            0x7b, 0x19, 0xf7, 0x4a, 0xb1, 0x85, 0xda, 0x90, 0x44, 0x5a, 0x04, 0x2d, 0xbd, 0x04,
            0xb6, 0xf3, 0x9f, 0x7f, 0x9e, 0xff, 0x3f, 0xff, 0xf3, 0x4f, 0xc5, 0xf0, 0xa3, 0xbd,
            0xfd, 0x4a, 0x9b, 0xbf,
        ]
    );
    assert_eq!(
        first.graph_digest(),
        [
            0xcc, 0x0e, 0xab, 0x96, 0x9a, 0x90, 0x77, 0x87, 0x8c, 0x78, 0x84, 0x68, 0xe4, 0xe7,
            0xdd, 0xfa, 0x90, 0xb1, 0xd0, 0x04, 0x63, 0x78, 0x5e, 0x0b, 0xe2, 0x95, 0xa9, 0xbc,
            0xaa, 0xef, 0xe4, 0x2e,
        ]
    );
    assert_eq!(
        first.plan_digest(),
        [
            0x40, 0x95, 0x4a, 0xca, 0x3c, 0x3a, 0xc6, 0x7e, 0x23, 0x09, 0x6f, 0x19, 0x97, 0x5f,
            0x76, 0xf4, 0x26, 0x97, 0x6e, 0xf8, 0xcd, 0x68, 0x93, 0xed, 0x45, 0x42, 0x3d, 0x7b,
            0xc2, 0x11, 0xaf, 0x02,
        ]
    );
    assert_eq!(
        first.layout_digests(),
        [
            [
                0x35, 0x5b, 0x17, 0x18, 0xb6, 0x50, 0x5d, 0xa3, 0x5e, 0x2f, 0xdd, 0x0f, 0xb1, 0x61,
                0x1f, 0xe4, 0x35, 0x2f, 0x25, 0xe7, 0x17, 0x76, 0xaa, 0xc8, 0x41, 0x6b, 0xcc, 0x47,
                0x48, 0xbc, 0x62, 0xc0,
            ],
            [
                0x23, 0x34, 0x89, 0x5b, 0xca, 0xd1, 0xa0, 0x78, 0x8f, 0xcf, 0xcd, 0x8b, 0xbf, 0xa8,
                0xb6, 0x74, 0x37, 0xc8, 0x8a, 0x93, 0x7e, 0x21, 0xf0, 0x11, 0x74, 0xad, 0x40, 0x14,
                0xcc, 0xcb, 0x65, 0x23,
            ],
            [
                0x33, 0x4f, 0xa6, 0xbc, 0xb6, 0x4f, 0x4f, 0x55, 0x1a, 0x98, 0xf9, 0x46, 0x2a, 0x5f,
                0xdb, 0xe2, 0xd5, 0x1a, 0x9f, 0xc8, 0x38, 0x99, 0x2e, 0xb0, 0x83, 0xe2, 0xf3, 0x22,
                0xbc, 0x5f, 0xaa, 0xf6,
            ],
            [
                0xe3, 0x9e, 0x1d, 0xfa, 0x20, 0x60, 0xed, 0xd4, 0xb8, 0xcf, 0xca, 0xc4, 0xbb, 0xc6,
                0x7e, 0x4e, 0x71, 0x95, 0xce, 0x99, 0x0e, 0x6d, 0xe7, 0xbe, 0x78, 0x1e, 0xac, 0x09,
                0x8f, 0xe2, 0x0a, 0xfe,
            ],
        ]
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(first.bytes())),
        [
            0x78, 0x0a, 0x0c, 0xcf, 0xc3, 0x5c, 0x7f, 0xf6, 0xd9, 0x33, 0x48, 0x37, 0x11, 0xe9,
            0x58, 0xd2, 0x9c, 0xfd, 0x44, 0xc2, 0x90, 0x76, 0x2b, 0x05, 0xcd, 0x51, 0x83, 0xe6,
            0xbf, 0x04, 0xb5, 0xb0,
        ]
    );
    assert_eq!(
        first.digest(),
        [
            0xc3, 0xd1, 0xfd, 0x10, 0x50, 0x1b, 0xfe, 0x8d, 0xcd, 0x4b, 0x5c, 0x8f, 0x24, 0x18,
            0x4d, 0x12, 0x7e, 0x46, 0x2b, 0x9c, 0xa4, 0xbc, 0x6b, 0x1f, 0x94, 0x22, 0xad, 0x8f,
            0xbc, 0xc0, 0xb2, 0x6e,
        ]
    );
    assert_eq!(first, artifact());
    assert_eq!(first.wit(), WIT_V7);
    assert_ne!(first.layout_digests()[2], first.layout_digests()[3]);
    let validated = validate_private_generic_record_component_v7(
        first.bytes(),
        first.source_revision(),
        first.generated_core_digest(),
    )
    .unwrap();
    assert_eq!(validated.interface_export_name(), INTERFACE_EXPORT);
    assert_eq!(validated.function_export_names(), FUNCTION_EXPORTS);
    assert_eq!(validated.type_export_names(), TYPE_EXPORTS);
    assert_eq!(validated.source_revision(), first.source_revision());
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(validated.generated_core())),
        GENERATED_CORE_KAT
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(first.bytes())
        .expect("upstream validator rejected generic-record component v7");
}

#[test]
fn every_byte_truncation_trailing_and_noncanonical_length_reject() {
    let artifact = artifact();
    for index in 0..artifact.bytes().len() {
        let mut hostile = artifact.bytes().to_vec();
        hostile[index] ^= 1;
        assert!(validate_private_generic_record_component_v7(
            &hostile,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    for end in 0..artifact.bytes().len() {
        assert!(validate_private_generic_record_component_v7(
            &artifact.bytes()[..end],
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    let mut trailing = artifact.bytes().to_vec();
    trailing.push(0);
    assert!(validate_private_generic_record_component_v7(
        &trailing,
        artifact.source_revision(),
        artifact.generated_core_digest(),
    )
    .is_err());
    let mut noncanonical = artifact.bytes().to_vec();
    let anchor = [0x02, 0x04, 0x01, 0x00, 0x00, 0x00];
    let positions = noncanonical
        .windows(anchor.len())
        .enumerate()
        .filter_map(|(index, window)| (window == anchor).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1, "v7 core-instance anchor drifted");
    noncanonical.splice(positions[0] + 1..positions[0] + 2, [0x84, 0x00]);
    assert_eq!(
        validate_private_generic_record_component_v7(
            &noncanonical,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        ),
        Err(PrivateComponentValidationError::Encoding)
    );
}

#[test]
fn exact_field_type_lift_and_instance_mappings_reject() {
    let artifact = artifact();
    let rejects = |hostile: &[u8]| {
        assert!(validate_private_generic_record_component_v7(
            hostile,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    };
    for needle in [
        b"duo-i64-bool".as_slice(),
        b"duo-bool-i64".as_slice(),
        b"phantom-i64".as_slice(),
        b"phantom-bool".as_slice(),
        b"left".as_slice(),
        b"right".as_slice(),
    ] {
        let offset = artifact
            .bytes()
            .windows(needle.len())
            .rposition(|window| window == needle)
            .expect("v7 semantic anchor drifted");
        let mut hostile = artifact.bytes().to_vec();
        hostile[offset] ^= 1;
        rejects(&hostile);
    }

    let duo_i64_bool = {
        let mut bytes = vec![0x72, 0x02];
        push_name(&mut bytes, "left");
        bytes.push(0x78);
        push_name(&mut bytes, "right");
        bytes.push(0x7f);
        bytes
    };
    let duo_at = artifact
        .bytes()
        .windows(duo_i64_bool.len())
        .position(|window| window == duo_i64_bool)
        .expect("v7 Duo<i64,bool> type anchor drifted");
    let mut hostile = artifact.bytes().to_vec();
    let mut swapped = vec![0x72, 0x02];
    push_name(&mut swapped, "right");
    swapped.push(0x7f);
    push_name(&mut swapped, "left");
    swapped.push(0x78);
    hostile.splice(duo_at..duo_at + duo_i64_bool.len(), swapped);
    rejects(&hostile);

    let mut canonical_anchor = Vec::new();
    for index in 0_u8..4 {
        canonical_anchor.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 10 + index]);
    }
    let canonical_at = artifact
        .bytes()
        .windows(canonical_anchor.len())
        .position(|window| window == canonical_anchor)
        .expect("v7 canonical lift anchor drifted");

    // The two Phantom core functions have the same physical signature. A valid
    // Component can cross their core indices, but the exact v7 map must reject it.
    let mut hostile = artifact.bytes().to_vec();
    hostile.swap(canonical_at + 18, canonical_at + 26);
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&hostile)
        .expect("same-signature Phantom core-index swap should remain structurally valid");
    rejects(&hostile);

    // Crossing the distinct named Phantom result types is also exact-profile hostile.
    let mut hostile = artifact.bytes().to_vec();
    hostile.swap(canonical_at + 23, canonical_at + 31);
    rejects(&hostile);

    let mut interface_anchor = vec![0x01, 0x09];
    for (index, name) in TYPE_EXPORTS.into_iter().enumerate() {
        interface_anchor.push(0x00);
        push_name(&mut interface_anchor, name);
        interface_anchor.extend([0x03, 0x01 + index as u8]);
    }
    for (index, name) in FUNCTION_EXPORTS.into_iter().enumerate() {
        interface_anchor.push(0x00);
        push_name(&mut interface_anchor, name);
        interface_anchor.extend([0x01, index as u8]);
    }
    let interface_at = artifact
        .bytes()
        .windows(interface_anchor.len())
        .position(|window| window == interface_anchor)
        .expect("v7 interface anchor drifted");
    let mut hostile = artifact.bytes().to_vec();
    let preserve_ref = interface_anchor
        .windows(b"preserve-phantom-i64".len())
        .position(|window| window == b"preserve-phantom-i64")
        .unwrap()
        + b"preserve-phantom-i64".len()
        + 1;
    let invert_ref = interface_anchor
        .windows(b"invert-phantom-bool".len())
        .position(|window| window == b"invert-phantom-bool")
        .unwrap()
        + b"invert-phantom-bool".len()
        + 1;
    hostile.swap(interface_at + preserve_ref, interface_at + invert_ref);
    rejects(&hostile);

    let program = crate::parse(
        wasm::GENERIC_RECORD_COMPONENT_SOURCE_V7,
        Path::new("rehashed-core-v7.spx"),
    )
    .unwrap();
    let mut hostile_core = wasm::emit_private_generic_record_core_v7(&program)
        .unwrap()
        .bytes;
    let last = hostile_core.len() - 1;
    hostile_core[last] ^= 1;
    let rehashed: [u8; 32] = Sha256::digest(&hostile_core).into();
    let hostile_component = compose(&hostile_core);
    assert!(validate_private_generic_record_component_v7(
        &hostile_component,
        artifact.source_revision(),
        rehashed,
    )
    .is_err());
}

#[test]
fn v1_through_v7_profiles_are_never_confused() {
    const V4_SOURCE: &str = r#"module v4;
@id("component.source")
fn source(value:i64,reject:bool)->Result<i64,bool> { if reject { Result<i64,bool>::Err { error: value > 0 } } else { Result<i64,bool>::Ok { value: value } } }
@id("component.evaluate")
fn evaluate(value:i64,reject:bool,divisor:i64)->Result<bool,bool>
requires value != -99
ensures divisor != 13
{ let checked = source(value,reject)?; Result<bool,bool>::Ok { value: (checked + 1) / divisor > 0 } }
@id("app.main") fn main()->i64 { 0 }
"#;
    let v1 = super::super::emit_private_component_v1();
    let v2_program = crate::parse(
        "module v2; @id(\"app.main\") fn main() -> i64 { 42 }",
        Path::new("v2.spx"),
    )
    .unwrap();
    let v2 = super::super::emit_private_checked_component_v2(&v2_program).unwrap();
    let v3_program = crate::parse(
            "module v3; @id(\"component.evaluate\") fn evaluate(left:i64,right:i64)->i64 { left + right } @id(\"app.main\") fn main()->i64 { 0 }",
            Path::new("v3.spx"),
        )
        .unwrap();
    let v3 = super::super::emit_private_result_component_v3(&v3_program).unwrap();
    let v4_program = crate::parse(V4_SOURCE, Path::new("v4.spx")).unwrap();
    let v4 = super::super::emit_private_source_result_component_v4(&v4_program).unwrap();
    let v5_program = crate::parse(
        wasm::SCALAR_ALGEBRA_COMPONENT_SOURCE_V5,
        Path::new("v5.spx"),
    )
    .unwrap();
    let v5 = super::super::emit_private_scalar_algebra_component_v5(&v5_program).unwrap();
    let v6_program =
        crate::parse(wasm::NESTED_RECORD_COMPONENT_SOURCE_V6, Path::new("v6.spx")).unwrap();
    let v6 = super::super::emit_private_nested_record_component_v6(&v6_program).unwrap();
    let v7 = artifact();
    for candidate in [
        v1.bytes(),
        v2.bytes(),
        v3.bytes(),
        v4.bytes(),
        v5.bytes(),
        v6.bytes(),
    ] {
        assert!(validate_private_generic_record_component_v7(
            candidate,
            v7.source_revision(),
            v7.generated_core_digest(),
        )
        .is_err());
    }
    assert!(super::super::validate_private_component_v1(v7.bytes()).is_err());
    assert!(super::super::validate_private_checked_component_v2(
        v7.bytes(),
        v2.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_result_component_v3(
        v7.bytes(),
        v3.source_revision(),
        v3.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_source_result_component_v4(
        v7.bytes(),
        v4.source_revision(),
        v4.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_scalar_algebra_component_v5(
        v7.bytes(),
        v5.source_revision(),
        v5.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_nested_record_component_v6(
        v7.bytes(),
        v6.source_revision(),
        v6.generated_core_digest(),
    )
    .is_err());
}
