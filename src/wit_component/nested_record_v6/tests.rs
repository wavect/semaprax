use std::path::Path;

use super::*;

fn artifact() -> PrivateNestedRecordComponentArtifactV6 {
    let program = crate::parse(
        wasm::NESTED_RECORD_COMPONENT_SOURCE_V6,
        Path::new("component-nested-record-v6.spx"),
    )
    .unwrap();
    emit_private_nested_record_component_v6(&program).unwrap()
}

#[test]
fn deterministic_v6_artifact_is_exactly_parsed_and_upstream_valid() {
    let first = artifact();
    assert_eq!(first.source_revision(), SOURCE_REVISION_KAT);
    assert_eq!(first.generated_core_digest(), GENERATED_CORE_KAT);
    assert_eq!(
        first.profile_digest(),
        [
            0x9e, 0xd5, 0x06, 0xe7, 0x81, 0x34, 0xb7, 0xde, 0x29, 0xed, 0x69, 0x30, 0x84, 0xad,
            0x68, 0x50, 0x68, 0x79, 0x2b, 0x80, 0x32, 0x1e, 0x29, 0xb8, 0x19, 0xcb, 0xeb, 0x8c,
            0xf9, 0x6f, 0x17, 0xa3,
        ]
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(first.bytes())),
        [
            0xad, 0x40, 0x8a, 0x7a, 0x6a, 0x35, 0x96, 0xa0, 0x26, 0xeb, 0x73, 0xbc, 0x42, 0x3e,
            0x59, 0xf3, 0x03, 0x50, 0xc0, 0xe4, 0xf7, 0xcb, 0xc5, 0x07, 0xce, 0x60, 0x51, 0x0e,
            0xff, 0x2b, 0x53, 0x0f,
        ]
    );
    assert_eq!(
        first.digest(),
        [
            0xca, 0x08, 0x56, 0xfe, 0xd4, 0xee, 0xf6, 0xac, 0x7d, 0x3a, 0xb7, 0xed, 0x46, 0x60,
            0x75, 0xc6, 0x0d, 0x7f, 0xf4, 0xec, 0x03, 0x72, 0xa8, 0x91, 0xdd, 0xf4, 0x83, 0xd1,
            0x99, 0x94, 0x1a, 0x3f,
        ]
    );
    assert_eq!(first, artifact());
    assert_eq!(first.wit(), WIT_V6);
    assert_ne!(first.layout_digests()[0], first.layout_digests()[1]);
    let validated = validate_private_nested_record_component_v6(
        first.bytes(),
        first.source_revision(),
        first.generated_core_digest(),
    )
    .unwrap();
    assert_eq!(validated.interface_export_name(), INTERFACE_EXPORT);
    assert_eq!(validated.function_export_name(), FUNCTION_EXPORT);
    assert_eq!(validated.type_export_names(), TYPE_EXPORTS);
    assert_eq!(validated.source_revision(), first.source_revision());
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(validated.generated_core())),
        GENERATED_CORE_KAT
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(first.bytes())
        .expect("pinned upstream validator rejected nested-record component v6");
}

#[test]
fn every_byte_truncation_trailing_and_noncanonical_length_reject() {
    let artifact = artifact();
    for index in 0..artifact.bytes().len() {
        let mut hostile = artifact.bytes().to_vec();
        hostile[index] ^= 1;
        assert!(validate_private_nested_record_component_v6(
            &hostile,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    for end in 0..artifact.bytes().len() {
        assert!(validate_private_nested_record_component_v6(
            &artifact.bytes()[..end],
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    let mut trailing = artifact.bytes().to_vec();
    trailing.push(0);
    assert!(validate_private_nested_record_component_v6(
        &trailing,
        artifact.source_revision(),
        artifact.generated_core_digest(),
    )
    .is_err());
    let mut noncanonical = artifact.bytes().to_vec();
    let needle = [0x02, 0x04, 0x01, 0x00, 0x00, 0x00];
    let offsets = noncanonical
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "v6 core-instance anchor drifted");
    noncanonical.splice(offsets[0] + 1..offsets[0] + 2, [0x84, 0x00]);
    assert_eq!(
        validate_private_nested_record_component_v6(
            &noncanonical,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        ),
        Err(PrivateComponentValidationError::Encoding)
    );
}

#[test]
fn type_field_lift_and_rehashed_core_mutations_reject() {
    let artifact = artifact();
    for needle in [
        b"value".as_slice(),
        b"other".as_slice(),
        &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x05],
    ] {
        let offset = artifact
            .bytes()
            .windows(needle.len())
            .rposition(|window| window == needle)
            .expect("v6 hostile anchor drifted");
        let mut hostile = artifact.bytes().to_vec();
        hostile[offset] ^= 1;
        assert!(validate_private_nested_record_component_v6(
            &hostile,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }

    let inner_shape = {
        let mut bytes = vec![0x72, 0x02];
        push_name(&mut bytes, "value");
        bytes.push(0x78);
        push_name(&mut bytes, "flag");
        bytes.push(0x7f);
        bytes
    };
    let outer_shape = {
        let mut bytes = vec![0x72, 0x02];
        push_name(&mut bytes, "inner");
        bytes.push(0x02);
        push_name(&mut bytes, "other");
        bytes.push(0x78);
        bytes
    };
    let inner_at = artifact
        .bytes()
        .windows(inner_shape.len())
        .position(|window| window == inner_shape)
        .expect("inner component type anchor drifted");
    let outer_at = artifact
        .bytes()
        .windows(outer_shape.len())
        .position(|window| window == outer_shape)
        .expect("outer component type anchor drifted");

    // Same-length semantic i64 field names swapped across the two records.
    let mut hostile = artifact.bytes().to_vec();
    for offset in 0..5 {
        hostile.swap(inner_at + 3 + offset, outer_at + 10 + offset);
    }
    assert!(validate_private_nested_record_component_v6(
        &hostile,
        artifact.source_revision(),
        artifact.generated_core_digest(),
    )
    .is_err());

    // Valid record encoding with Inner's field order and types swapped.
    let mut hostile = artifact.bytes().to_vec();
    let mut swapped_inner = vec![0x72, 0x02];
    push_name(&mut swapped_inner, "flag");
    swapped_inner.push(0x7f);
    push_name(&mut swapped_inner, "value");
    swapped_inner.push(0x78);
    hostile.splice(inner_at..inner_at + inner_shape.len(), swapped_inner);
    assert!(validate_private_nested_record_component_v6(
        &hostile,
        artifact.source_revision(),
        artifact.generated_core_digest(),
    )
    .is_err());

    // Valid interface exports whose inner/outer type identities are crossed.
    let mut interface_anchor = vec![0x01, 0x04, 0x00];
    push_name(&mut interface_anchor, "status");
    interface_anchor.extend([0x03, 0x01, 0x00]);
    push_name(&mut interface_anchor, "inner");
    interface_anchor.extend([0x03, 0x02, 0x00]);
    push_name(&mut interface_anchor, "outer");
    interface_anchor.extend([0x03, 0x03]);
    let interface_at = artifact
        .bytes()
        .windows(interface_anchor.len())
        .position(|window| window == interface_anchor)
        .expect("interface type anchor drifted");
    let mut hostile = artifact.bytes().to_vec();
    let inner_ref = interface_at + 20;
    let outer_ref = interface_at + 29;
    hostile.swap(inner_ref, outer_ref);
    assert!(validate_private_nested_record_component_v6(
        &hostile,
        artifact.source_revision(),
        artifact.generated_core_digest(),
    )
    .is_err());

    let program = crate::parse(
        wasm::NESTED_RECORD_COMPONENT_SOURCE_V6,
        Path::new("rehashed-core-v6.spx"),
    )
    .unwrap();
    let mut hostile_core = wasm::emit_private_nested_record_core_v6(&program)
        .unwrap()
        .bytes;
    let last = hostile_core.len() - 1;
    hostile_core[last] ^= 1;
    let rehashed: [u8; 32] = Sha256::digest(&hostile_core).into();
    let hostile_component = compose(&hostile_core);
    assert!(validate_private_nested_record_component_v6(
        &hostile_component,
        artifact.source_revision(),
        rehashed,
    )
    .is_err());
}

#[test]
fn v1_v2_v3_v4_v5_v6_profiles_are_never_confused() {
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
    let v6 = artifact();
    for candidate in [v1.bytes(), v2.bytes(), v3.bytes(), v4.bytes(), v5.bytes()] {
        assert!(validate_private_nested_record_component_v6(
            candidate,
            v6.source_revision(),
            v6.generated_core_digest(),
        )
        .is_err());
    }
    assert!(super::super::validate_private_component_v1(v6.bytes()).is_err());
    assert!(super::super::validate_private_checked_component_v2(
        v6.bytes(),
        v2.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_result_component_v3(
        v6.bytes(),
        v3.source_revision(),
        v3.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_source_result_component_v4(
        v6.bytes(),
        v4.source_revision(),
        v4.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_scalar_algebra_component_v5(
        v6.bytes(),
        v5.source_revision(),
        v5.generated_core_digest(),
    )
    .is_err());
}
