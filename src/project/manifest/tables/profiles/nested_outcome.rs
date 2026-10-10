use super::*;

fn manifest(profile: &str) -> String {
    format!(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"nested-outcome\"\nversion = \"0.1.0\"\nprofile = \"{profile}\"\n\n[modules]\nentry = \"app.entry\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"app.tests\"]\n\n[exports]\nweb = [\"app.command\"]\n\n[command]\nfunction = \"app.command\"\ninput = \"argv-utf8+stdin-stream.v1\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n"
    )
}

#[test]
fn nested_outcome_profile_has_its_own_v32_route() {
    let token = PROJECT_PROFILE_STDIN_STREAM_NESTED_OUTCOME_COMMAND_IO_V1;
    let selected = profile_by_name(token).unwrap();
    assert_eq!(selected, ProjectProfile::StdinStreamNestedOutcomeCommandIoV1);
    assert_eq!(selected.name(), Some(token));
    assert!(selected.is_stdin_stream());

    let source = manifest(token);
    let parsed = super::super::super::ProjectManifest::parse(&source).unwrap();
    assert_eq!(parsed.schema(), PROJECT_SCHEMA_V32);
    assert_eq!(parsed.project_profile(), selected);
    assert_eq!(parsed.to_canonical_toml(), source);

    let old = super::super::super::ProjectManifest::parse(&manifest(
        PROJECT_PROFILE_STDIN_STREAM_COLLECTION_RECORD_COMMAND_IO_V1,
    ))
    .unwrap();
    assert_eq!(old.schema(), PROJECT_SCHEMA_V31);
    assert_eq!(
        old.project_profile(),
        ProjectProfile::StdinStreamCollectionRecordCommandIoV1
    );

    let changed_authority = source.replace("\"process.stdin.read\", ", "");
    assert!(super::super::super::ProjectManifest::parse(&changed_authority).is_err());
}
