use std::path::Path;

use super::*;

fn artifact() -> PrivateGenericFunctionComponentArtifactV9 {
    let program = crate::parse(
        wasm::GENERIC_FUNCTION_COMPONENT_SOURCE_V9,
        Path::new("component-generic-function-v9.spx"),
    )
    .unwrap();
    emit_private_generic_function_component_v9(&program).unwrap()
}

#[test]
fn deterministic_v9_component_is_upstream_valid_and_kat_bound() {
    let first = artifact();
    assert_eq!(first, artifact());
    assert_eq!(first.source_revision(), SOURCE_REVISION_KAT);
    assert_eq!(first.wit(), WIT_V9);
    assert_eq!(
        first.graph_digest(),
        hex32("62907c4b95495bb573b2b37de9f0b08c7a82218934154521e8c0c8396158cc6e")
    );
    assert_eq!(first.generated_core_digest(), GENERATED_CORE_KAT);
    assert_eq!(
        first.profile_digest(),
        hex32("365897ddb2770cc25a11690dddbfef5d232244ec5d328c79a24a1410e684615e")
    );
    assert_eq!(
        first.plan_digest(),
        hex32("edd11c98bbc902d9dbc9c942375477fcf1e6c3f1befbe3c4a9f260107104485e")
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(first.bytes())),
        hex32("3cf6c7d7d02e838fb374478a2b5b25077c7c612ad36e30deaffd15311a25a688")
    );
    assert_eq!(
        first.digest(),
        hex32("2623ff9a7eda5526616a15befd4951de86874a59911dcba2a7d3bcc2d178a474")
    );
    let validated = validate_private_generic_function_component_v9(
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
        .expect("upstream validator rejected generic-function component v9");
}

#[test]
fn every_byte_truncation_trailing_and_all_fifteen_swaps_reject() {
    let artifact = artifact();
    for index in 0..artifact.bytes().len() {
        let mut hostile = artifact.bytes().to_vec();
        hostile[index] ^= 1;
        assert!(validate_private_generic_function_component_v9(
            &hostile,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    for end in 0..artifact.bytes().len() {
        assert!(validate_private_generic_function_component_v9(
            &artifact.bytes()[..end],
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    let mut trailing = artifact.bytes().to_vec();
    trailing.push(0);
    assert!(validate_private_generic_function_component_v9(
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
    assert_eq!(positions.len(), 1, "v9 core-instance anchor drifted");
    noncanonical.splice(positions[0] + 1..positions[0] + 2, [0x84, 0x00]);
    assert_eq!(
        validate_private_generic_function_component_v9(
            &noncanonical,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        ),
        Err(PrivateComponentValidationError::Encoding)
    );

    let mut canonical_anchor = Vec::new();
    for index in 0_u8..6 {
        canonical_anchor.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 0x03]);
    }
    let canonical_at = artifact
        .bytes()
        .windows(canonical_anchor.len())
        .position(|window| window == canonical_anchor)
        .expect("v9 canonical lift anchor drifted");
    let mut swaps = 0;
    for left in 0..6 {
        for right in left + 1..6 {
            let mut hostile = artifact.bytes().to_vec();
            hostile.swap(canonical_at + 2 + left * 8, canonical_at + 2 + right * 8);
            wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
                .validate_all(&hostile)
                .expect("same-signature v9 core-index swap should be structurally valid");
            assert!(validate_private_generic_function_component_v9(
                &hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
            swaps += 1;
        }
    }
    assert_eq!(swaps, 15);
}

#[test]
fn independent_profile_mutations_reject() {
    for hostile in [
        wasm::GENERIC_FUNCTION_COMPONENT_SOURCE_V9.replacen("preserve<i64>", "preserve<bool>", 1),
        wasm::GENERIC_FUNCTION_COMPONENT_SOURCE_V9.replacen(
            "ordered<i64, bool>",
            "ordered<bool, i64>",
            1,
        ),
        wasm::GENERIC_FUNCTION_COMPONENT_SOURCE_V9.replacen(
            "fn ordered<T, U>",
            "fn ordered<U, T>",
            1,
        ),
    ] {
        let parsed = crate::parse(&hostile, Path::new("hostile-v9-profile.spx"));
        match parsed {
            Ok(program) => {
                assert!(emit_private_generic_function_component_v9(&program).is_err())
            }
            Err(error) => assert!(!error.code.is_empty()),
        }
    }
}

#[test]
fn v1_through_v9_profiles_are_never_confused() {
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
    let v8_program = crate::parse(
        wasm::RECORD_PATTERN_COMPONENT_SOURCE_V8,
        Path::new("v8.spx"),
    )
    .unwrap();
    let v8 = super::super::emit_private_record_pattern_component_v8(&v8_program).unwrap();
    let v9 = artifact();
    for candidate in [
        v1.bytes(),
        v2.bytes(),
        v3.bytes(),
        v4.bytes(),
        v5.bytes(),
        v6.bytes(),
        v7.bytes(),
        v8.bytes(),
    ] {
        assert!(validate_private_generic_function_component_v9(
            candidate,
            v9.source_revision(),
            v9.generated_core_digest(),
        )
        .is_err());
    }
    assert!(super::super::validate_private_component_v1(v9.bytes()).is_err());
    assert!(super::super::validate_private_checked_component_v2(
        v9.bytes(),
        v2.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_result_component_v3(
        v9.bytes(),
        v3.source_revision(),
        v3.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_source_result_component_v4(
        v9.bytes(),
        v4.source_revision(),
        v4.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_scalar_algebra_component_v5(
        v9.bytes(),
        v5.source_revision(),
        v5.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_nested_record_component_v6(
        v9.bytes(),
        v6.source_revision(),
        v6.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_generic_record_component_v7(
        v9.bytes(),
        v7.source_revision(),
        v7.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_record_pattern_component_v8(
        v9.bytes(),
        v8.source_revision(),
        v8.generated_core_digest(),
    )
    .is_err());
}

fn hex32(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    let mut bytes = [0_u8; 32];
    for (index, chunk) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let nibble = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("non-lowercase-hex KAT"),
        };
        bytes[index] = (nibble(chunk[0]) << 4) | nibble(chunk[1]);
    }
    bytes
}
