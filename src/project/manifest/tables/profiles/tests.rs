use super::*;

fn manifest(profile: &str) -> String {
    format!(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"collection-record\"\nversion = \"0.1.0\"\nprofile = \"{profile}\"\n\n[modules]\nentry = \"app.entry\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"app.tests\"]\n\n[exports]\nweb = [\"app.command\"]\n\n[command]\nfunction = \"app.command\"\ninput = \"argv-utf8+stdin-stream.v1\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n"
    )
}

#[test]
fn collection_record_profile_has_its_own_schema_and_v30_authority() {
    let token = "language-command-io.collection-record.v1";
    let selected = profile_by_name(token).unwrap();
    assert_eq!(selected, ProjectProfile::StdinStreamCollectionRecordCommandIoV1);
    assert_eq!(selected.name(), Some(token));
    assert!(selected.is_stdin_stream());

    let source = manifest(token);
    let parsed = super::super::super::ProjectManifest::parse(&source).unwrap();
    assert_eq!(parsed.schema(), "semaprax.project.v31");
    assert_eq!(parsed.project_profile(), selected);
    assert_eq!(parsed.to_canonical_toml(), source);

    let old = super::super::super::ProjectManifest::parse(&manifest(
        "language-command-io.owned-data.v1",
    ))
    .unwrap();
    assert_eq!(old.schema(), "semaprax.project.v30");
    assert_eq!(
        old.project_profile(),
        ProjectProfile::StdinStreamOwnedDataCommandIoV1
    );

    let changed_authority = source.replace("\"process.stdin.read\", ", "");
    assert!(super::super::super::ProjectManifest::parse(&changed_authority).is_err());
}
