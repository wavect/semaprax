use std::path::Path;

use super::*;

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

fn artifact() -> PrivateOptionPropagationComponentArtifactV10 {
    let program = crate::parse(
        wasm::OPTION_PROPAGATION_SOURCE_V10,
        Path::new("component-option-propagation-v10.spx"),
    )
    .unwrap();
    emit_private_option_propagation_component_v10(&program).unwrap()
}

fn hex(value: [u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(64);
    for byte in value {
        write!(&mut encoded, "{byte:02x}").unwrap();
    }
    encoded
}

#[test]
fn deterministic_v10_component_is_upstream_valid_and_all_roots_bound() {
    let first = artifact();
    assert_eq!(first, artifact());
    assert_eq!(first.wit(), WIT_V10);
    assert_eq!(
        first.source_revision(),
        "sha256:98b8fc892c183499153142d5bbdb4162e31bda95ef145d34dbb1ff57c9b8fc72"
    );
    assert_eq!(
        hex(first.graph_digest()),
        "96083f90fab18c919a96cee48109e606e089159e109869a42bdf48831743d45d"
    );
    assert_eq!(
        hex(first.prelude_digest()),
        "d37bad7e3911669bbf2c66b25c8b31d5c2e36eb181cc54fdc86c3a49a8fb9c5e"
    );
    assert_eq!(
        hex(first.option_i64_layout_digest()),
        "79194fc88011ac060877e60293d0a4272429dd9e2d720674d0d54e804562deda"
    );
    assert_eq!(
        hex(first.option_bool_layout_digest()),
        "dec126293ece7ec0e48d3d85ccdb494f7c7cfe4c3d4a9b1a61b50f6f862ff038"
    );
    assert_eq!(
        hex(first.plan_digest()),
        "d07fa51fc6f192a43318140264fa0e5964933ed90bc065cc8c74708e258ff92f"
    );
    assert_eq!(
        hex(first.generated_core_digest()),
        "16d1d34024e3fad920d8d00a61d7cb3bd010335ca382f23615b3b3da4143aaec"
    );
    assert_eq!(
        hex(first.profile_digest()),
        "f53a0c21638b5a360faa19ad4fdef68f6d861a5baffe39422847128686e82bef"
    );
    assert_eq!(
        hex(Sha256::digest(first.bytes()).into()),
        "f5770bdfdbc862ea39640b2c706c1d9ea171164c220d18366e25b3219443ad0d"
    );
    assert_eq!(
        hex(first.digest()),
        "90ab80260c84abfe85d1edc666ab3750b81388e6e4cffd7ca21c301b9d0ee589"
    );
    let validated = validate_private_option_propagation_component_v10(
        first.bytes(),
        first.source_revision(),
        first.generated_core_digest(),
    )
    .unwrap();
    assert_eq!(validated.interface_export_name(), INTERFACE_EXPORT);
    assert_eq!(validated.function_export_name(), FUNCTION_EXPORT);
    assert_eq!(validated.source_option_export_name(), SOURCE_OPTION_EXPORT);
    assert_eq!(validated.target_option_export_name(), TARGET_OPTION_EXPORT);
    assert_eq!(validated.source_revision(), first.source_revision());
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(validated.generated_core())),
        first.generated_core_digest()
    );
    assert_eq!(
        validate_private_option_propagation_component_v10(
            first.bytes(),
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            first.generated_core_digest(),
        ),
        Err(PrivateComponentValidationError::Profile)
    );
    let mut wrong_core = first.generated_core_digest();
    wrong_core[0] ^= 1;
    assert_eq!(
        validate_private_option_propagation_component_v10(
            first.bytes(),
            first.source_revision(),
            wrong_core,
        ),
        Err(PrivateComponentValidationError::Profile)
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(first.bytes())
        .expect("upstream validator rejected option-propagation component v10");
}

#[test]
fn every_byte_truncation_trailing_and_noncanonical_length_reject() {
    let artifact = artifact();
    for index in 0..artifact.bytes().len() {
        let mut hostile = artifact.bytes().to_vec();
        hostile[index] ^= 1;
        assert!(
            validate_private_option_propagation_component_v10(
                &hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err(),
            "v10 component byte {index} escaped authentication"
        );
    }
    for end in 0..artifact.bytes().len() {
        assert!(validate_private_option_propagation_component_v10(
            &artifact.bytes()[..end],
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    let mut trailing = artifact.bytes().to_vec();
    trailing.push(0);
    assert!(validate_private_option_propagation_component_v10(
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
    assert_eq!(offsets.len(), 1, "v10 core-instance anchor drifted");
    noncanonical.splice(offsets[0] + 1..offsets[0] + 2, [0x84, 0x00]);
    assert!(validate_private_option_propagation_component_v10(
        &noncanonical,
        artifact.source_revision(),
        artifact.generated_core_digest(),
    )
    .is_err());

    let mut retryable_confusion = artifact.bytes().to_vec();
    let anchor = b"retryable\x01";
    let offsets = retryable_confusion
        .windows(anchor.len())
        .enumerate()
        .filter_map(|(index, window)| (window == anchor).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "retryable type anchor drifted");
    retryable_confusion[offsets[0] + anchor.len() - 1] = 0;
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&retryable_confusion)
        .expect("retryable source-option confusion should remain structurally valid");
    assert!(validate_private_option_propagation_component_v10(
        &retryable_confusion,
        artifact.source_revision(),
        artifact.generated_core_digest(),
    )
    .is_err());
}

#[test]
fn independent_source_profile_mutations_reject() {
    for hostile in [
        wasm::OPTION_PROPAGATION_SOURCE_V10.replacen(
            "component.option-propagation.evaluate",
            "component.evaluate",
            1,
        ),
        wasm::OPTION_PROPAGATION_SOURCE_V10.replacen(
            "let checked = input?;",
            "let other = input?;",
            1,
        ),
        wasm::OPTION_PROPAGATION_SOURCE_V10.replacen("checked + 1", "checked + 2", 1),
        wasm::OPTION_PROPAGATION_SOURCE_V10.replacen("divisor != 13", "divisor != 12", 1),
    ] {
        match crate::parse(&hostile, Path::new("hostile-option-propagation-v10.spx")) {
            Ok(program) => {
                assert!(emit_private_option_propagation_component_v10(&program).is_err())
            }
            Err(error) => assert!(!error.code.is_empty()),
        }
    }
}

#[test]
fn v1_through_v10_profiles_are_never_confused() {
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
    let v9_program = crate::parse(
        wasm::GENERIC_FUNCTION_COMPONENT_SOURCE_V9,
        Path::new("v9.spx"),
    )
    .unwrap();
    let v9 = super::super::emit_private_generic_function_component_v9(&v9_program).unwrap();
    let v10 = artifact();

    for candidate in [
        v1.bytes(),
        v2.bytes(),
        v3.bytes(),
        v4.bytes(),
        v5.bytes(),
        v6.bytes(),
        v7.bytes(),
        v8.bytes(),
        v9.bytes(),
    ] {
        assert!(validate_private_option_propagation_component_v10(
            candidate,
            v10.source_revision(),
            v10.generated_core_digest(),
        )
        .is_err());
    }
    assert!(super::super::validate_private_component_v1(v10.bytes()).is_err());
    assert!(super::super::validate_private_checked_component_v2(
        v10.bytes(),
        v2.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_result_component_v3(
        v10.bytes(),
        v3.source_revision(),
        v3.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_source_result_component_v4(
        v10.bytes(),
        v4.source_revision(),
        v4.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_scalar_algebra_component_v5(
        v10.bytes(),
        v5.source_revision(),
        v5.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_nested_record_component_v6(
        v10.bytes(),
        v6.source_revision(),
        v6.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_generic_record_component_v7(
        v10.bytes(),
        v7.source_revision(),
        v7.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_record_pattern_component_v8(
        v10.bytes(),
        v8.source_revision(),
        v8.generated_core_digest(),
    )
    .is_err());
    assert!(
        super::super::validate_private_generic_function_component_v9(
            v10.bytes(),
            v9.source_revision(),
            v9.generated_core_digest(),
        )
        .is_err()
    );
}
