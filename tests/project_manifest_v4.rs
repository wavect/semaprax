use semaprax::project::{
    ProjectManifest, ProjectProfile, PROJECT_COMMAND_STDOUT_CAPABILITY,
    PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1, PROJECT_SCHEMA_V4,
};

const V4: &str = "schema = \"semaprax.project.v4\"\nname = \"spxgrep\"\nversion = \"0.1.0\"\nprofile = \"useful-data-command.v1\"\nentry = \"spxgrep.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"spxgrep.contains\"]\ncommand = \"spxgrep.contains\"\ncapabilities = [\"process.stdout.write\"]\ntests = [\"spxgrep.tests\"]\n";

#[test]
fn v4_is_one_exact_canonical_command_manifest() {
    let manifest = ProjectManifest::parse(V4).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V4);
    assert_eq!(
        manifest.project_profile(),
        ProjectProfile::UsefulDataCommandV1
    );
    assert_eq!(
        manifest.profile(),
        Some(PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1)
    );
    assert_eq!(manifest.command(), Some("spxgrep.contains"));
    assert_eq!(manifest.capabilities(), [PROJECT_COMMAND_STDOUT_CAPABILITY]);
    assert_eq!(manifest.web_exports(), ["spxgrep.contains"]);
    assert_eq!(manifest.to_canonical_toml(), V4);
}

#[test]
fn v4_rejects_capability_and_command_inventory_widening() {
    for hostile in [
        V4.replace(
            "capabilities = [\"process.stdout.write\"]",
            "capabilities = []",
        ),
        V4.replace(
            "capabilities = [\"process.stdout.write\"]",
            "capabilities = [\"process.stdout.write\", \"network\"]",
        ),
        V4.replace(
            "web_exports = [\"spxgrep.contains\"]",
            "web_exports = [\"spxgrep.contains\", \"spxgrep.other\"]",
        ),
        V4.replace(
            "command = \"spxgrep.contains\"",
            "command = \"spxgrep.other\"",
        ),
    ] {
        let diagnostics = ProjectManifest::parse(&hostile).unwrap_err();
        assert_eq!(diagnostics[0].code, "SPX-J100");
    }
}

#[test]
fn legacy_manifest_canonical_bytes_are_unchanged() {
    for legacy in [
        "schema = \"semaprax.project.v1\"\nname = \"legacy\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v2\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-text-consumer.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v3\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
    ] {
        assert_eq!(ProjectManifest::parse(legacy).unwrap().to_canonical_toml(), legacy);
    }
}
