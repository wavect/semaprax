use semaprax::package_build::{
    OfflinePackageBuildOptions, EVIDENCE_SCHEMA, MANIFEST_SCHEMA, MAX_ARTIFACT_BYTES,
    MAX_EVIDENCE_BYTES, PROFILE,
};

#[test]
fn public_option_and_schema_boundary_is_closed() {
    let minimum = OfflinePackageBuildOptions::new(
        "pkg.root".to_owned(),
        vec!["pkg.main".to_owned()],
        4 * 1024,
        4 * 1024,
    )
    .unwrap();
    assert_eq!(minimum.max_artifact_bytes, 4 * 1024);
    assert!(OfflinePackageBuildOptions::new(
        "pkg.root".to_owned(),
        vec!["pkg.main".to_owned()],
        4 * 1024 - 1,
        MAX_EVIDENCE_BYTES,
    )
    .is_err());
    assert!(OfflinePackageBuildOptions::new(
        "pkg.root".to_owned(),
        vec!["pkg.main".to_owned()],
        MAX_ARTIFACT_BYTES,
        MAX_EVIDENCE_BYTES + 1,
    )
    .is_err());
    assert_eq!(
        MANIFEST_SCHEMA,
        "semaprax.offline-effect-free-wasm-package-build.v1"
    );
    assert_eq!(
        EVIDENCE_SCHEMA,
        "semaprax.offline-effect-free-wasm-package-build-evidence.v1"
    );
    assert_eq!(PROFILE, "effect-free-core-wasm-scalar.v1");
}
