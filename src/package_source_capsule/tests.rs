use super::*;

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
