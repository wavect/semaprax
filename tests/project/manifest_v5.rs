use semaprax::project::{
    ProjectManifest, ProjectProfile, PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2,
    PROJECT_COMMAND_INPUT_V1, PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2, PROJECT_SCHEMA_V5,
};

const V5: &str = "schema = \"semaprax.project.v5\"\nname = \"spxgrep\"\nversion = \"0.1.0\"\nprofile = \"useful-data-command.v2\"\nentry = \"spxgrep.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"spxgrep.contains\"]\ncommand = \"spxgrep.contains\"\ninput = \"stdin-bytes+one-utf8-arg.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"spxgrep.tests\"]\n";

#[test]
fn v5_is_one_exact_canonical_fixed_adapter_manifest() {
    let manifest = ProjectManifest::parse(V5).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V5);
    assert_eq!(
        manifest.project_profile(),
        ProjectProfile::UsefulDataCommandV2
    );
    assert_eq!(
        manifest.profile(),
        Some(PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2)
    );
    assert_eq!(manifest.command(), Some("spxgrep.contains"));
    assert_eq!(manifest.command_input(), Some(PROJECT_COMMAND_INPUT_V1));
    assert!(manifest
        .capabilities()
        .iter()
        .map(String::as_str)
        .eq(PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2));
    assert_eq!(manifest.web_exports(), ["spxgrep.contains"]);
    assert_eq!(manifest.to_canonical_toml(), V5);
}

#[test]
fn v5_rejects_every_adapter_authority_widening_or_confusion() {
    for hostile in [
        V5.replace(
            "profile = \"useful-data-command.v2\"",
            "profile = \"useful-data-command.v1\"",
        ),
        V5.replace(
            "input = \"stdin-bytes+one-utf8-arg.v1\"",
            "input = \"stdin-bytes.v1\"",
        ),
        V5.replace(
            "input = \"stdin-bytes+one-utf8-arg.v1\"\n",
            "",
        ),
        V5.replace(
            "[\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
            "[]",
        ),
        V5.replace(
            "[\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
            "[\"process.stderr.write\", \"process.args.read\", \"process.stdin.read\", \"process.stdout.write\"]",
        ),
        V5.replace(
            "[\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
            "[\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\", \"network\"]",
        ),
        V5.replace(
            "web_exports = [\"spxgrep.contains\"]",
            "web_exports = [\"spxgrep.contains\", \"spxgrep.other\"]",
        ),
        V5.replace(
            "command = \"spxgrep.contains\"",
            "command = \"spxgrep.other\"",
        ),
        V5.replace(
            "input = \"stdin-bytes+one-utf8-arg.v1\"\ncapabilities",
            "capabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ninput",
        ),
    ] {
        let diagnostics = ProjectManifest::parse(&hostile).unwrap_err();
        assert_eq!(diagnostics[0].code, "SPX-J100", "{hostile}");
    }
}

#[test]
fn v1_through_v4_manifest_bytes_remain_frozen() {
    for legacy in [
        "schema = \"semaprax.project.v1\"\nname = \"legacy\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v2\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-text-consumer.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v3\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v4\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data-command.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ncommand = \"legacy.value\"\ncapabilities = [\"process.stdout.write\"]\ntests = [\"legacy.tests\"]\n",
    ] {
        assert_eq!(
            ProjectManifest::parse(legacy).unwrap().to_canonical_toml(),
            legacy
        );
    }
}
