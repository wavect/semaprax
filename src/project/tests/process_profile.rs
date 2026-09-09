use super::super::{
    ProjectManifest, ProjectProfile, PROJECT_PROFILE_ENVIRONMENT_IO_V1,
    PROJECT_PROFILE_PROCESS_IO_V1, PROJECT_SCHEMA_V17, PROJECT_SCHEMA_V18,
};

fn frozen(schema: &str, profile: &str, capabilities: &str, exports: &str) -> String {
    format!(
        "schema = \"{schema}\"\nname = \"process-fixture\"\nversion = \"1.0.0\"\nprofile = \"{profile}\"\nentry = \"process.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = {exports}\ncommand = \"process.run\"\ncapabilities = {capabilities}\ntests = [\"process.tests\"]\n"
    )
}

#[test]
fn process_v18_accepts_sorted_execute_subset_without_exports() {
    let source = frozen(
        PROJECT_SCHEMA_V18,
        PROJECT_PROFILE_PROCESS_IO_V1,
        "[\"process.execute\"]",
        "[]",
    );
    let manifest = ProjectManifest::parse(&source).unwrap();
    assert_eq!(manifest.project_profile(), ProjectProfile::ProcessIoV1);
    assert_eq!(manifest.to_canonical_toml(), source);
}

#[test]
fn process_v18_requires_execute_and_rejects_unrelated_effects_or_exports() {
    for capabilities in [
        "[\"process.args.read\"]",
        "[\"fs.read\", \"process.execute\"]",
        "[\"network.connect\", \"process.execute\"]",
        "[\"process.execute\", \"process.write\"]",
    ] {
        assert!(ProjectManifest::parse(&frozen(
            PROJECT_SCHEMA_V18,
            PROJECT_PROFILE_PROCESS_IO_V1,
            capabilities,
            "[]",
        ))
        .is_err());
    }
    assert!(ProjectManifest::parse(&frozen(
        PROJECT_SCHEMA_V18,
        PROJECT_PROFILE_PROCESS_IO_V1,
        "[\"process.execute\"]",
        "[\"process.run\"]",
    ))
    .is_err());
}

#[test]
fn environment_v17_frozen_bytes_remain_canonical() {
    let source = frozen(
        PROJECT_SCHEMA_V17,
        PROJECT_PROFILE_ENVIRONMENT_IO_V1,
        "[\"process.environment.read\"]",
        "[]",
    );
    let manifest = ProjectManifest::parse(&source).unwrap();
    assert_eq!(manifest.project_profile(), ProjectProfile::EnvironmentIoV1);
    assert_eq!(manifest.to_canonical_toml(), source);
}
