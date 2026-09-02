use semaprax::project::{
    ProjectManifest, ProjectProfile, PROJECT_PROFILE_OWNED_DATA_API_V1,
    PROJECT_PROFILE_OWNED_UTF8_API_V1, PROJECT_SCHEMA_V10, PROJECT_SCHEMA_V8,
};

const MANIFEST: &str = "schema = \"semaprax.project.v10\"\nname = \"utf8-api\"\nversion = \"1.0.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"utf8.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"utf8.greeting\"]\ntests = [\"utf8.tests\"]\n";

#[test]
fn canonical_v10_manifest_is_exact_and_schema_bound() {
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V10);
    assert_eq!(manifest.project_profile(), ProjectProfile::OwnedUtf8ApiV1);
    assert_eq!(manifest.profile(), Some(PROJECT_PROFILE_OWNED_UTF8_API_V1));
    assert_eq!(manifest.to_canonical_toml(), MANIFEST);

    assert!(ProjectManifest::parse(&MANIFEST.replace(
        PROJECT_PROFILE_OWNED_UTF8_API_V1,
        PROJECT_PROFILE_OWNED_DATA_API_V1
    ))
    .is_err());
    assert!(ProjectManifest::parse(
        &MANIFEST
            .replace(PROJECT_SCHEMA_V10, PROJECT_SCHEMA_V8)
            .replace(
                PROJECT_PROFILE_OWNED_UTF8_API_V1,
                PROJECT_PROFILE_OWNED_DATA_API_V1
            )
    )
    .is_ok());
}

#[test]
fn v10_rejects_noncanonical_manifest_bytes_without_changing_v8() {
    let lines = MANIFEST.lines().collect::<Vec<_>>();
    for missing in 0..lines.len() {
        let mut candidate = lines.clone();
        candidate.remove(missing);
        assert!(ProjectManifest::parse(&(candidate.join("\n") + "\n")).is_err());
    }
    for left in 0..lines.len() - 1 {
        let mut candidate = lines.clone();
        candidate.swap(left, left + 1);
        assert!(ProjectManifest::parse(&(candidate.join("\n") + "\n")).is_err());
    }
    assert!(ProjectManifest::parse(MANIFEST.trim_end()).is_err());
    assert!(ProjectManifest::parse(&MANIFEST.replace('\n', "\r\n")).is_err());

    let v8 = MANIFEST
        .replace(PROJECT_SCHEMA_V10, PROJECT_SCHEMA_V8)
        .replace(
            PROJECT_PROFILE_OWNED_UTF8_API_V1,
            PROJECT_PROFILE_OWNED_DATA_API_V1,
        );
    assert_eq!(ProjectManifest::parse(&v8).unwrap().to_canonical_toml(), v8);
}
