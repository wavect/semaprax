use semaprax::project::{
    ProjectManifest, ProjectProfile, PROJECT_PROFILE_NESTED_OWNED_RECORD_API_V1, PROJECT_SCHEMA_V11,
};

const MANIFEST: &str = "schema = \"semaprax.project.v11\"\nname = \"nested-api\"\nversion = \"1.0.0\"\nprofile = \"nested-owned-record-api.v1\"\nentry = \"nested.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"nested.build\"]\ntests = [\"nested.tests\"]\n";

#[test]
fn canonical_v11_manifest_is_additive_and_exact() {
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V11);
    assert_eq!(
        manifest.project_profile(),
        ProjectProfile::NestedOwnedRecordApiV1
    );
    assert_eq!(
        manifest.profile(),
        Some(PROJECT_PROFILE_NESTED_OWNED_RECORD_API_V1)
    );
    assert!(manifest.is_v11());
    assert_eq!(manifest.to_canonical_toml(), MANIFEST);
    for hostile in [
        MANIFEST.replace("nested-owned-record-api.v1", "flat-owned-record-api.v1"),
        MANIFEST.replace("semaprax.project.v11", "semaprax.project.v10"),
        MANIFEST.trim_end().to_owned(),
        MANIFEST.replace('\n', "\r\n"),
    ] {
        assert!(ProjectManifest::parse(&hostile).is_err(), "{hostile}");
    }
}
