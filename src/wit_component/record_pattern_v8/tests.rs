use std::path::Path;

use super::*;

fn artifact() -> PrivateRecordPatternComponentArtifactV8 {
    let program = crate::parse(
        wasm::RECORD_PATTERN_COMPONENT_SOURCE_V8,
        Path::new("component-record-pattern-v8.spx"),
    )
    .unwrap();
    emit_private_record_pattern_component_v8(&program).unwrap()
}

#[test]
fn deterministic_v8_component_is_upstream_valid() {
    let first = artifact();
    assert_eq!(first.source_revision(), SOURCE_REVISION_KAT);
    assert_eq!(first.generated_core_digest(), GENERATED_CORE_KAT);
    assert_eq!(
        first.profile_digest(),
        [
            0x79, 0xd4, 0xba, 0xde, 0x38, 0xdd, 0x3f, 0xff, 0x9c, 0x71, 0x45, 0xb4, 0x06, 0xbb,
            0x0b, 0xb2, 0x65, 0xff, 0x3e, 0xf7, 0xcf, 0x08, 0x4e, 0xda, 0xc8, 0x33, 0x84, 0xc8,
            0x46, 0x10, 0xbc, 0xe2,
        ]
    );
    assert_eq!(
        first.graph_digest(),
        [
            0xc5, 0x87, 0x41, 0x58, 0x19, 0x39, 0x5e, 0x3d, 0x61, 0x8b, 0x1e, 0x72, 0x4d, 0x63,
            0x9d, 0x65, 0x0e, 0x7c, 0x55, 0xb0, 0x46, 0xf4, 0xb7, 0x7b, 0x8b, 0xcb, 0x5d, 0xe4,
            0xff, 0x95, 0x68, 0x2b,
        ]
    );
    assert_eq!(
        first.plan_digest(),
        [
            0xc7, 0x7c, 0x40, 0x60, 0xfb, 0x0b, 0x00, 0x51, 0xaf, 0x12, 0x5f, 0x4c, 0xa3, 0x53,
            0xdf, 0x3a, 0x6f, 0x5d, 0xbd, 0x36, 0x7c, 0xdc, 0x5f, 0xfd, 0x61, 0x34, 0x7a, 0x7c,
            0x22, 0x84, 0x70, 0x59,
        ]
    );
    assert_eq!(
        first.layout_digests(),
        [
            [
                0xd2, 0xff, 0x60, 0x84, 0xbc, 0xfc, 0x95, 0x70, 0x1b, 0x1d, 0xd5, 0x98, 0x35, 0xd0,
                0xac, 0x3a, 0xf9, 0x63, 0x62, 0xe0, 0x5e, 0x56, 0xdc, 0xad, 0xcb, 0xd4, 0xb8, 0xe5,
                0xdc, 0x7d, 0x9d, 0x80,
            ],
            [
                0x3e, 0x09, 0xce, 0xfc, 0x7d, 0x1a, 0xe9, 0xbc, 0x52, 0xec, 0x82, 0x7d, 0xeb, 0xdb,
                0xcd, 0x07, 0x53, 0xd6, 0x3b, 0xcc, 0xa9, 0x94, 0xef, 0x77, 0x6e, 0xad, 0xb6, 0x6b,
                0xa2, 0x54, 0xe6, 0x7a,
            ],
        ]
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(first.bytes())),
        [
            0xd8, 0x85, 0x90, 0x75, 0x2e, 0xd7, 0xb0, 0x8b, 0x0f, 0x0a, 0x32, 0x01, 0x9b, 0xa8,
            0xb4, 0xc5, 0xfc, 0x48, 0x9d, 0x59, 0xf0, 0x6b, 0x96, 0x98, 0x6d, 0x7a, 0xd6, 0x9e,
            0x25, 0x54, 0xa1, 0x0e,
        ]
    );
    assert_eq!(
        first.digest(),
        [
            0xe3, 0x2f, 0xe0, 0xa1, 0x5a, 0x34, 0x58, 0xf1, 0x6a, 0xa4, 0xda, 0x59, 0xd8, 0x76,
            0x83, 0x01, 0x3d, 0xbe, 0xba, 0x03, 0x75, 0x49, 0x66, 0xf3, 0x5e, 0x0c, 0xb6, 0x36,
            0x00, 0xe6, 0x13, 0xa3,
        ]
    );
    assert_eq!(first, artifact());
    assert_eq!(first.wit(), WIT_V8);
    assert_ne!(first.layout_digests()[0], first.layout_digests()[1]);
    let validated = validate_private_record_pattern_component_v8(
        first.bytes(),
        first.source_revision(),
        first.generated_core_digest(),
    )
    .unwrap();
    assert_eq!(validated.interface_export_name(), INTERFACE_EXPORT);
    assert_eq!(validated.function_export_names(), FUNCTION_EXPORTS);
    assert_eq!(validated.type_export_names(), TYPE_EXPORTS);
    assert_eq!(validated.source_revision(), SOURCE_REVISION_KAT);
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(validated.generated_core())),
        GENERATED_CORE_KAT
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(first.bytes())
        .expect("upstream validator rejected record-pattern component v8");
}

#[test]
fn every_byte_truncation_trailing_and_noncanonical_length_reject() {
    let artifact = artifact();
    for index in 0..artifact.bytes().len() {
        let mut hostile = artifact.bytes().to_vec();
        hostile[index] ^= 1;
        assert!(validate_private_record_pattern_component_v8(
            &hostile,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    for end in 0..artifact.bytes().len() {
        assert!(validate_private_record_pattern_component_v8(
            &artifact.bytes()[..end],
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    let mut trailing = artifact.bytes().to_vec();
    trailing.push(0);
    assert!(validate_private_record_pattern_component_v8(
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
    assert_eq!(positions.len(), 1, "v8 core-instance anchor drifted");
    noncanonical.splice(positions[0] + 1..positions[0] + 2, [0x84, 0x00]);
    assert_eq!(
        validate_private_record_pattern_component_v8(
            &noncanonical,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        ),
        Err(PrivateComponentValidationError::Encoding)
    );
}

#[test]
fn all_equal_signature_identity_type_and_lift_swaps_reject() {
    let artifact = artifact();
    let rejects = |hostile: &[u8]| {
        assert!(validate_private_record_pattern_component_v8(
            hostile,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    };

    let mut canonical_anchor = Vec::new();
    for (index, ty) in [5_u8, 5, 6, 6].into_iter().enumerate() {
        canonical_anchor.extend([0x00, 0x00, index as u8, 0x02, 0x00, 0x03, 0x00, ty]);
    }
    let canonical_at = artifact
        .bytes()
        .windows(canonical_anchor.len())
        .position(|window| window == canonical_anchor)
        .expect("v8 canonical lift anchor drifted");

    // Every pair has the same flattened core signature. All six valid
    // reindexings are rejected by identity, never admitted by layout.
    for left in 0..4 {
        for right in left + 1..4 {
            let mut hostile = artifact.bytes().to_vec();
            hostile.swap(canonical_at + 2 + left * 8, canonical_at + 2 + right * 8);
            wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
                .validate_all(&hostile)
                .expect("same-signature v8 core-index swap should be structurally valid");
            rejects(&hostile);
        }
    }

    // Crossing concrete Phantom function types changes named instance
    // identity despite an equal physical record layout.
    let mut hostile = artifact.bytes().to_vec();
    hostile.swap(canonical_at + 7, canonical_at + 23);
    rejects(&hostile);

    let mut interface_anchor = vec![0x01, 0x07];
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
        .expect("v8 interface anchor drifted");
    let function_ref = |name: &str| {
        interface_anchor
            .windows(name.len())
            .position(|window| window == name.as_bytes())
            .expect("v8 function interface anchor drifted")
            + name.len()
            + 1
    };
    let mut hostile = artifact.bytes().to_vec();
    hostile.swap(
        interface_at + function_ref(FUNCTION_EXPORTS[0]),
        interface_at + function_ref(FUNCTION_EXPORTS[3]),
    );
    rejects(&hostile);

    for needle in [
        b"phantom-i64".as_slice(),
        b"phantom-bool".as_slice(),
        b"marker".as_slice(),
    ] {
        let at = artifact
            .bytes()
            .windows(needle.len())
            .rposition(|window| window == needle)
            .expect("v8 named type anchor drifted");
        let mut hostile = artifact.bytes().to_vec();
        hostile[at] ^= 1;
        rejects(&hostile);
    }

    let program = crate::parse(
        wasm::RECORD_PATTERN_COMPONENT_SOURCE_V8,
        Path::new("rehashed-core-v8.spx"),
    )
    .unwrap();
    let mut hostile_core = wasm::emit_private_record_pattern_core_v8(&program)
        .unwrap()
        .bytes;
    let last = hostile_core.len() - 1;
    hostile_core[last] ^= 1;
    let rehashed: [u8; 32] = Sha256::digest(&hostile_core).into();
    let hostile_component = compose(&hostile_core);
    assert!(validate_private_record_pattern_component_v8(
        &hostile_component,
        artifact.source_revision(),
        rehashed,
    )
    .is_err());
}

#[test]
fn v1_through_v8_profiles_are_never_confused() {
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
    let v7_program = crate::parse(
        wasm::GENERIC_RECORD_COMPONENT_SOURCE_V7,
        Path::new("v7.spx"),
    )
    .unwrap();
    let v7 = super::super::emit_private_generic_record_component_v7(&v7_program).unwrap();
    let v8 = artifact();
    for candidate in [
        v1.bytes(),
        v2.bytes(),
        v3.bytes(),
        v4.bytes(),
        v5.bytes(),
        v6.bytes(),
        v7.bytes(),
    ] {
        assert!(validate_private_record_pattern_component_v8(
            candidate,
            v8.source_revision(),
            v8.generated_core_digest(),
        )
        .is_err());
    }
    assert!(super::super::validate_private_component_v1(v8.bytes()).is_err());
    assert!(super::super::validate_private_checked_component_v2(
        v8.bytes(),
        v2.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_result_component_v3(
        v8.bytes(),
        v3.source_revision(),
        v3.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_source_result_component_v4(
        v8.bytes(),
        v4.source_revision(),
        v4.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_scalar_algebra_component_v5(
        v8.bytes(),
        v5.source_revision(),
        v5.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_nested_record_component_v6(
        v8.bytes(),
        v6.source_revision(),
        v6.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_generic_record_component_v7(
        v8.bytes(),
        v7.source_revision(),
        v7.generated_core_digest(),
    )
    .is_err());
}
