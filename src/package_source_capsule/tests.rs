use super::*;

#[test]
fn verify_rejects_invalid_options_before_scanning_submitted_wire() {
    let options = SourceCapsuleOptions {
        root_package: "app.main".to_owned(),
        max_bytes: 0,
    };
    let input = ResolutionInput {
        requirements: Vec::new(),
        subjects: Vec::new(),
        target: "wasm32".to_owned(),
        allowed_capabilities: Vec::new(),
    };
    assert_eq!(
        verify(
            "not canonical JSON",
            &[],
            "not resolver evidence",
            &input,
            &ResolutionOptions::default(),
            &options,
        )
        .unwrap_err()
        .code,
        "SPX-PS501"
    );
}

#[test]
fn options_bind_an_explicit_canonical_root_and_output_bound() {
    assert!(SourceCapsuleOptions::new("app.main".to_owned(), 4 * 1024).is_ok());
    assert_eq!(
        SourceCapsuleOptions::new("app-main".to_owned(), 4 * 1024)
            .unwrap_err()
            .code,
        "SPX-PS501"
    );
    assert_eq!(
        SourceCapsuleOptions::new("app.main".to_owned(), 4 * 1024 - 1)
            .unwrap_err()
            .code,
        "SPX-PS501"
    );
}

#[test]
fn submitted_wire_rejects_duplicate_keys_before_deserialization() {
    let duplicate = concat!(
        "{\"schema\":\"semaprax.offline-multi-package-source-capsule.v1\",",
        "\"schema\":\"semaprax.offline-multi-package-source-capsule.v1\",",
        "\"digest\":\"sha256:00\",\"bytes\":2,\"payload\":{}}"
    );
    assert_eq!(
        wire::validate_submitted(duplicate, MAX_OUTPUT_BYTES)
            .unwrap_err()
            .code,
        "SPX-PS506"
    );
}
