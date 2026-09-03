use std::path::Path;

use super::*;

const SOURCE: &str = r#"module test.component_source_result_v4;

@id("component.source")
fn source(value: i64, reject: bool) -> Result<i64, bool> {
    if reject {
        Result<i64, bool>::Err { error: value > 0 }
    } else {
        Result<i64, bool>::Ok { value: value }
    }
}

@id("component.evaluate")
fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<bool, bool>
    requires value != -99
    ensures divisor != 13
{
    let checked = source(value, reject)?;
    Result<bool, bool>::Ok { value: (checked + 1) / divisor > 0 }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn artifact() -> PrivateSourceResultComponentArtifactV4 {
    let program = crate::parse(SOURCE, Path::new("component-source-result-v4.spx")).unwrap();
    emit_private_source_result_component_v4(&program).unwrap()
}

#[test]
fn deterministic_v4_artifact_is_exactly_parsed_and_upstream_valid() {
    let first = artifact();
    assert_eq!(
        first.generated_core_digest(),
        [
            0x54, 0xfa, 0x28, 0x22, 0xc5, 0x1a, 0x71, 0xce, 0xbf, 0xd8, 0x8d, 0x37, 0x9b, 0x45,
            0xc3, 0x7f, 0xfd, 0x3d, 0x0f, 0x0b, 0x28, 0x93, 0xcb, 0x4f, 0x29, 0x66, 0xf9, 0xe2,
            0xdb, 0x6d, 0x5e, 0x5f,
        ],
        "generated-core KAT changed"
    );
    assert_eq!(
        first.profile_digest(),
        [
            0xfa, 0x1f, 0x0b, 0x5e, 0xca, 0x07, 0xb4, 0xb3, 0xcb, 0xa2, 0xc3, 0xd9, 0xc5, 0xfd,
            0xd0, 0x07, 0x27, 0x6d, 0x7f, 0xa6, 0x72, 0xa3, 0xe4, 0xa4, 0x9e, 0x9f, 0xfd, 0x20,
            0xd3, 0xdc, 0xe0, 0x6c,
        ],
        "profile KAT changed"
    );
    assert_eq!(
        first.digest(),
        [
            0xf5, 0xfa, 0x5a, 0xe3, 0x90, 0x5d, 0x30, 0xc9, 0x98, 0xf7, 0x83, 0xe9, 0xb7, 0x78,
            0x67, 0x98, 0x68, 0x13, 0xb0, 0xe8, 0xb4, 0x41, 0x2f, 0xa4, 0xaf, 0xa9, 0x8e, 0x93,
            0x2e, 0xda, 0x4d, 0x40,
        ],
        "component DAG KAT changed"
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(first.bytes())),
        [
            0x3e, 0x7b, 0x9c, 0x2d, 0xdc, 0x8c, 0xa6, 0xfd, 0xfa, 0x80, 0x1e, 0xb5, 0x0a, 0xe3,
            0xa2, 0x15, 0x31, 0xfc, 0xe4, 0x46, 0x77, 0x34, 0x5d, 0xde, 0xa6, 0x8d, 0x20, 0x58,
            0x1c, 0x79, 0xb2, 0x3b,
        ],
        "exact component-byte SHA-256 KAT changed"
    );
    assert_eq!(
        first.prelude_digest(),
        [
            0xd3, 0x7b, 0xad, 0x7e, 0x39, 0x11, 0x66, 0x9b, 0xbf, 0x2c, 0x66, 0xb2, 0x5c, 0x8b,
            0x31, 0xd5, 0xc2, 0xe3, 0x6e, 0xb1, 0x81, 0xcc, 0x54, 0xfd, 0xc8, 0x6c, 0x3a, 0x49,
            0xa8, 0xfb, 0x9c, 0x5e,
        ],
        "prelude KAT changed"
    );
    assert_eq!(
        first.result_i64_bool_layout_digest(),
        [
            0xc0, 0x11, 0x12, 0xf9, 0x09, 0xa0, 0x74, 0x34, 0x3a, 0xe4, 0xeb, 0x3a, 0xbd, 0xe6,
            0xad, 0x70, 0x93, 0x02, 0x80, 0xe4, 0xa8, 0x01, 0x6c, 0x16, 0x5e, 0x05, 0xf3, 0x17,
            0xbe, 0xd9, 0xf1, 0x99,
        ],
        "Result<i64, bool> layout-v2 KAT changed"
    );
    assert_eq!(
        first.result_bool_bool_layout_digest(),
        [
            0x39, 0xaf, 0x02, 0x08, 0x45, 0x88, 0x12, 0x6c, 0x5f, 0x6d, 0x20, 0xab, 0x8f, 0x3e,
            0xf1, 0xf8, 0x24, 0x9b, 0x8c, 0xa1, 0x9e, 0x52, 0x15, 0x33, 0x98, 0xa5, 0x21, 0xc2,
            0xc4, 0x9a, 0x55, 0x8d,
        ],
        "Result<bool, bool> layout-v2 KAT changed"
    );
    assert_eq!(first, artifact());
    assert_eq!(first.wit(), WIT_V4);
    assert_eq!(
        first.source_revision(),
        "sha256:4391bc27b5db547f2b162c2b5467c2b75797e8a5ef64e4ffe4abef15678c6254",
        "source revision KAT changed"
    );
    let validated = validate_private_source_result_component_v4(
        first.bytes(),
        first.source_revision(),
        first.generated_core_digest(),
    )
    .unwrap();
    assert_eq!(validated.source_revision(), first.source_revision());
    assert_eq!(validated.interface_export_name(), INTERFACE_EXPORT);
    assert_eq!(validated.function_export_name(), FUNCTION_EXPORT);
    assert_eq!(
        validated.language_result_export_name(),
        LANGUAGE_RESULT_EXPORT
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(validated.generated_core())),
        first.generated_core_digest()
    );
    assert_eq!(first.prelude_digest(), prelude::digest_v1());
    assert_ne!(
        first.result_i64_bool_layout_digest(),
        first.result_bool_bool_layout_digest()
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(first.bytes())
        .expect("pinned upstream validator rejected source-result component v4");
}

#[test]
fn every_byte_truncation_trailing_and_noncanonical_length_reject() {
    let artifact = artifact();
    for index in 0..artifact.bytes().len() {
        let mut hostile = artifact.bytes().to_vec();
        hostile[index] ^= 1;
        assert!(
            validate_private_source_result_component_v4(
                &hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err(),
            "source-result component byte {index} escaped authentication"
        );
    }
    for end in 0..artifact.bytes().len() {
        assert!(validate_private_source_result_component_v4(
            &artifact.bytes()[..end],
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
    let mut trailing = artifact.bytes().to_vec();
    trailing.push(0);
    assert!(validate_private_source_result_component_v4(
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
    assert_eq!(offsets.len(), 1, "core-instance section anchor drifted");
    noncanonical.splice(offsets[0] + 1..offsets[0] + 2, [0x84, 0x00]);
    assert_eq!(
        validate_private_source_result_component_v4(
            &noncanonical,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        ),
        Err(PrivateComponentValidationError::Encoding)
    );
}

#[test]
fn v1_v2_v3_v4_profiles_are_never_confused() {
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
    let v4 = artifact();
    for candidate in [v1.bytes(), v2.bytes(), v3.bytes()] {
        assert!(validate_private_source_result_component_v4(
            candidate,
            v4.source_revision(),
            v4.generated_core_digest(),
        )
        .is_err());
    }
    assert!(super::super::validate_private_component_v1(v4.bytes()).is_err());
    assert!(super::super::validate_private_checked_component_v2(
        v4.bytes(),
        v2.generated_core_digest(),
    )
    .is_err());
    assert!(super::super::validate_private_result_component_v3(
        v4.bytes(),
        v3.source_revision(),
        v3.generated_core_digest(),
    )
    .is_err());
}

#[test]
fn rehashed_flattened_named_type_lift_and_export_hostiles_reject() {
    let artifact = artifact();
    let hostiles = [
        (
            &[0x6a, 0x01, 0x7f, 0x01, 0x7f][..],
            4,
            0x78,
            "inner-error-type",
        ),
        (
            &[0x6a, 0x01, 0x02, 0x01, 0x01][..],
            2,
            0x7f,
            "flattened-outer-ok",
        ),
        (
            &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04][..],
            7,
            0x03,
            "lift-type",
        ),
        (&[0x05, 0x00, 0x00][..], 0, 0x01, "interface-kind"),
    ];
    for (needle, relative, replacement, name) in hostiles {
        let mut hostile = artifact.bytes().to_vec();
        let offsets = hostile
            .windows(needle.len())
            .enumerate()
            .filter_map(|(index, window)| (window == needle).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(offsets.len(), 1, "hostile anchor {name} must be unique");
        hostile[offsets[0] + relative] = replacement;
        assert!(validate_private_source_result_component_v4(
            &hostile,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
    }
}

#[test]
fn excluded_authority_type_and_signature_profiles_fail_closed() {
    for source in [
            SOURCE.replace(
                "fn evaluate(value: i64, reject: bool, divisor: i64)",
                "fn evaluate(value: i64, reject: bool, divisor: i64, extra: bool)",
            ),
            SOURCE
                .replace(
                    "fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<bool, bool>",
                    "fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<i64, bool>",
                )
                .replace(
                    "Result<bool, bool>::Ok { value: (checked + 1) / divisor > 0 }",
                    "Result<i64, bool>::Ok { value: checked }",
                ),
            SOURCE.replace(
                "module test.component_source_result_v4;",
                "module test.component_source_result_v4;\npermit { clock.read }",
            ).replace(
                "fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<bool, bool>",
                "fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<bool, bool> uses { clock.read }",
            ),
        ] {
            let program = crate::parse(
                &source,
                Path::new("excluded-component-source-result-v4.spx"),
            )
            .unwrap();
            assert_eq!(
                emit_private_source_result_component_v4(&program)
                    .unwrap_err()
                    .code,
                "SPX-WIT108"
            );
        }
}
