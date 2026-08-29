use super::*;

fn options() -> LinkedOfflinePackageBuildOptions {
    LinkedOfflinePackageBuildOptions {
        root_package: "app".to_owned(),
        exports: vec!["app.main".to_owned()],
        max_artifact_bytes: 4 * 1024,
        max_evidence_bytes: 4 * 1024,
    }
}

#[test]
fn options_reject_unsorted_exports_in_the_distinct_v2_family() {
    let mut value = options();
    value.exports = vec!["z".to_owned(), "a".to_owned()];
    assert_eq!(
        admission::validate_options(&value).unwrap_err().code,
        "SPX-PB601"
    );
}

#[test]
fn hostile_submitted_wire_never_reaches_exact_replay() {
    assert_eq!(
        wire::validate_submitted_manifest("{\n}", 4096)
            .unwrap_err()
            .code,
        "SPX-PB606"
    );
    assert_eq!(
        wire::validate_submitted_evidence("[]", 4096)
            .unwrap_err()
            .code,
        "SPX-PB606"
    );
}

#[test]
fn v1_and_v2_wire_identities_are_disjoint() {
    assert_ne!(MANIFEST_SCHEMA, crate::package_build::MANIFEST_SCHEMA);
    assert_ne!(EVIDENCE_SCHEMA, crate::package_build::EVIDENCE_SCHEMA);
    assert_ne!(PROFILE, crate::package_build::PROFILE);
}

#[test]
fn provider_export_cannot_be_selected_as_a_root_export() {
    let value = options();
    let root_exports = vec!["app.other".to_owned()];
    assert_eq!(
        admission::validate_root_exports(&value, &root_exports)
            .unwrap_err()
            .code,
        "SPX-PB604"
    );
}
