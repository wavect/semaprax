use semaprax::project::{
    ProjectManifest, ProjectProfile, PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2,
    PROJECT_LANGUAGE_COMMAND_INPUT_V1, PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1, PROJECT_SCHEMA_V6,
};

const V6: &str = "schema = \"semaprax.project.v6\"\nname = \"spxgrep-language-command\"\nversion = \"0.1.0\"\nprofile = \"language-command-io.v1\"\nentry = \"spxgrep_language.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"spxgrep-language.run\"]\ncommand = \"spxgrep-language.run\"\ninput = \"argv-utf8+stdin-bytes.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"spxgrep_language.tests\"]\n";

#[test]
fn v6_is_one_exact_canonical_language_command_manifest() {
    let manifest = ProjectManifest::parse(V6).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V6);
    assert_eq!(
        manifest.project_profile(),
        ProjectProfile::LanguageCommandIoV1
    );
    assert_eq!(
        manifest.profile(),
        Some(PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1)
    );
    assert_eq!(manifest.command(), Some("spxgrep-language.run"));
    assert_eq!(
        manifest.command_input(),
        Some(PROJECT_LANGUAGE_COMMAND_INPUT_V1)
    );
    assert!(manifest
        .capabilities()
        .iter()
        .map(String::as_str)
        .eq(PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2));
    assert_eq!(manifest.web_exports(), ["spxgrep-language.run"]);
    assert_eq!(manifest.to_canonical_toml(), V6);
}

#[test]
fn v6_rejects_profile_input_authority_and_command_confusion() {
    for hostile in [
        V6.replace("language-command-io.v1", "useful-data-command.v2"),
        V6.replace("argv-utf8+stdin-bytes.v1", "stdin-bytes+one-utf8-arg.v1"),
        V6.replace("input = \"argv-utf8+stdin-bytes.v1\"\n", ""),
        V6.replace(
            "[\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
            "[]",
        ),
        V6.replace(
            "[\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
            "[\"process.stderr.write\", \"process.args.read\", \"process.stdin.read\", \"process.stdout.write\"]",
        ),
        V6.replace(
            "[\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
            "[\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\", \"network\"]",
        ),
        V6.replace(
            "web_exports = [\"spxgrep-language.run\"]",
            "web_exports = [\"spxgrep-language.other\"]",
        ),
        V6.replace(
            "command = \"spxgrep-language.run\"",
            "command = \"spxgrep-language.other\"",
        ),
        V6.replace(
            "input = \"argv-utf8+stdin-bytes.v1\"\ncapabilities",
            "capabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ninput",
        ),
    ] {
        let diagnostics = ProjectManifest::parse(&hostile).unwrap_err();
        assert_eq!(diagnostics[0].code, "SPX-J100", "{hostile}");
    }
}

#[test]
fn v1_through_v5_manifest_bytes_remain_frozen() {
    for legacy in [
        "schema = \"semaprax.project.v1\"\nname = \"legacy\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v2\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-text-consumer.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v3\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v4\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data-command.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ncommand = \"legacy.value\"\ncapabilities = [\"process.stdout.write\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v5\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data-command.v2\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ncommand = \"legacy.value\"\ninput = \"stdin-bytes+one-utf8-arg.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"legacy.tests\"]\n",
    ] {
        assert_eq!(ProjectManifest::parse(legacy).unwrap().to_canonical_toml(), legacy);
    }
}
