use std::path::Path;

use super::*;

fn artifact() -> PrivateScalarAlgebraComponentArtifactV5 {
    let program = crate::parse(
        crate::wasm::SCALAR_ALGEBRA_COMPONENT_SOURCE_V5,
        Path::new("component-scalar-algebra-v5.spx"),
    )
    .unwrap();
    emit_private_scalar_algebra_component_v5(&program).unwrap()
}

#[test]
fn deterministic_v5_artifact_is_exactly_parsed_and_upstream_valid() {
    let first = artifact();
    assert_eq!(first.source_revision(), SOURCE_REVISION_KAT);
    assert_eq!(first.generated_core_digest(), GENERATED_CORE_KAT);
    assert_eq!(
        first.profile_digest(),
        [
            0xb4, 0x9d, 0x24, 0xae, 0x10, 0x0c, 0xf8, 0x3b, 0x49, 0xd8, 0xbb, 0x91, 0x46, 0x91,
            0x54, 0x35, 0x78, 0xa4, 0x29, 0x7e, 0x16, 0xb4, 0xfd, 0xd1, 0x97, 0xb8, 0xb7, 0x88,
            0xa6, 0x6c, 0x95, 0xf1,
        ]
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(first.bytes())),
        [
            0x6c, 0xeb, 0x9e, 0x30, 0x96, 0x94, 0xa5, 0xb9, 0x60, 0x94, 0x49, 0x58, 0xa4, 0xb0,
            0x52, 0x7e, 0x29, 0xef, 0xa6, 0xba, 0xe8, 0xf7, 0xfc, 0x27, 0xe9, 0x4a, 0xd0, 0x1a,
            0x84, 0x7b, 0xad, 0xca,
        ]
    );
    assert_eq!(
        first.digest(),
        [
            0x3f, 0x7c, 0xd7, 0x6b, 0xe5, 0x5f, 0x8f, 0x5f, 0x49, 0x88, 0x4b, 0xc0, 0x63, 0xb9,
            0xca, 0x1c, 0x7a, 0x97, 0xb1, 0xe2, 0xc3, 0x8e, 0x23, 0x5c, 0xf4, 0x02, 0x39, 0x53,
            0xca, 0x36, 0xbd, 0xcf,
        ]
    );
    assert_eq!(first, artifact());
    assert_eq!(first.wit(), WIT_V5);
    assert_ne!(first.layout_digests()[0], first.layout_digests()[1]);
    let validated = validate_private_scalar_algebra_component_v5(
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
        first.generated_core_digest()
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(first.bytes())
        .expect("pinned upstream validator rejected scalar-algebra component v5");
}

#[test]
fn every_byte_truncation_trailing_and_noncanonical_length_reject() {
    let artifact = artifact();
    for index in 0..artifact.bytes().len() {
        let mut hostile = artifact.bytes().to_vec();
        hostile[index] ^= 1;
        assert!(validate_private_scalar_algebra_component_v5(
            &hostile,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    for end in 0..artifact.bytes().len() {
        assert!(validate_private_scalar_algebra_component_v5(
            &artifact.bytes()[..end],
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    let mut trailing = artifact.bytes().to_vec();
    trailing.push(0);
    assert!(validate_private_scalar_algebra_component_v5(
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
    assert_eq!(offsets.len(), 1, "v5 core-instance anchor drifted");
    noncanonical.splice(offsets[0] + 1..offsets[0] + 2, [0x84, 0x00]);
    assert_eq!(
        validate_private_scalar_algebra_component_v5(
            &noncanonical,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        ),
        Err(PrivateComponentValidationError::Encoding)
    );
}

#[test]
fn same_signature_function_and_type_reindexing_rejects() {
    let artifact = artifact();
    let canonical = {
        let mut bytes = vec![0x06];
        for index in 0_u8..6 {
            bytes.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 14 + index]);
        }
        bytes
    };
    let canonical_at = artifact
        .bytes()
        .windows(canonical.len())
        .position(|window| window == canonical)
        .expect("canonical section anchor drifted");
    for (left, right) in [(1_usize, 5_usize), (3, 4)] {
        let mut hostile = artifact.bytes().to_vec();
        hostile.swap(canonical_at + 3 + left * 8, canonical_at + 3 + right * 8);
        assert!(validate_private_scalar_algebra_component_v5(
            &hostile,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());

        let mut hostile = artifact.bytes().to_vec();
        hostile.swap(canonical_at + 8 + left * 8, canonical_at + 8 + right * 8);
        assert!(validate_private_scalar_algebra_component_v5(
            &hostile,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
}

#[test]
fn v1_v2_v3_v4_v5_profiles_are_never_confused() {
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
    let v5 = artifact();
    for candidate in [v1.bytes(), v2.bytes(), v3.bytes(), v4.bytes()] {
        assert!(validate_private_scalar_algebra_component_v5(
            candidate,
            v5.source_revision(),
            v5.generated_core_digest(),
        )
        .is_err());
    }
    assert!(super::super::validate_private_component_v1(v5.bytes()).is_err());
    assert!(super::super::validate_private_checked_component_v2(
        v5.bytes(),
        v2.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_result_component_v3(
        v5.bytes(),
        v3.source_revision(),
        v3.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_source_result_component_v4(
        v5.bytes(),
        v4.source_revision(),
        v4.generated_core_digest(),
    )
    .is_err());
}
