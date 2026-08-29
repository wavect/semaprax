use super::*;

fn round_trip(manifest: &Path) {
    let fixture = Fixture::new("profile");
    let revision = crate::project::load_snapshot(manifest)
        .unwrap()
        .retain_revision();
    let locator = identify(&revision, revision.project_revision()).unwrap();
    let receipt = persist(&fixture.store, &revision, revision.project_revision()).unwrap();
    assert_eq!(receipt.entry_digest(), locator.entry_digest());
    let loaded = load(
        &fixture.store,
        locator.entry_digest(),
        locator.project_revision(),
    )
    .unwrap();
    assert_eq!(loaded.manifest().schema(), revision.manifest().schema());
    assert_eq!(loaded.project_revision(), revision.project_revision());
    assert_eq!(loaded.workspace_revision(), revision.workspace_revision());
    assert_eq!(
        loaded.semantic_graph_digest(),
        revision.semantic_graph_digest()
    );
}

fn authored_profile(label: &str, manifest: &str, sources: &[(&str, &str)]) -> (Fixture, PathBuf) {
    let fixture = Fixture::new(label);
    let root = fixture.directory.join("project");
    std::fs::create_dir(&root).unwrap();
    for (path, source) in sources {
        let destination = root.join(path);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let source = crate::format::canonical(&crate::parse(source, Path::new(path)).unwrap());
        std::fs::write(destination, source).unwrap();
    }
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
    let manifest = root.canonicalize().unwrap().join("semaprax.toml");
    (fixture, manifest)
}

#[test]
fn every_project_profile_in_v1_to_v10_round_trips() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "examples/calculator-project/semaprax.toml",
        "examples/config-validator-project/semaprax.toml",
        "examples/binary-frame-project/semaprax.toml",
        "examples/spxgrep-project/semaprax.toml",
        "examples/spxgrep-native-command-project/semaprax.toml",
        "examples/spxgrep-language-command-project/semaprax.toml",
        "examples/spxgrep-lines-project/semaprax.toml",
        "examples/frame-payload-project/semaprax.toml",
    ] {
        round_trip(&repository.join(relative));
    }

    let (_v10_fixture, v10) = authored_profile(
        "profile-v10",
        "schema = \"semaprax.project.v10\"\nname = \"utf8-api\"\nversion = \"1.0.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"utf8.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"utf8.greeting\"]\ntests = [\"utf8.tests\"]\n",
        &[
            (
                "src/app.spx",
                "module utf8.app;\n\n@id(\"utf8.greeting\")\nfn greeting() -> string\n{\n    \"hello\"\n}\n\n@id(\"utf8.main\")\nfn main() -> i64 { 0 }\n",
            ),
            (
                "src/tests.spx",
                "module utf8.tests;\n\n@id(\"utf8.tests.main\")\nfn main() -> i64 { 0 }\n",
            ),
        ],
    );
    round_trip(&v10);

    let (_v9_fixture, v9) = authored_profile(
        "profile-v9",
        "schema = \"semaprax.project.v9\"\nname = \"frame-info\"\nversion = \"0.1.0\"\nprofile = \"flat-owned-record-api.v1\"\nentry = \"frame.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"frame.info\"]\ntests = [\"frame.tests\"]\n",
        &[
            (
                "src/app.spx",
                "module frame.app;\n\n@id(\"frame.info.type\")\nrecord FrameInfo {\n    @id(\"frame.info.payload\") payload: Bytes,\n    @id(\"frame.info.kind\") kind: i64,\n}\n\n@id(\"frame.info\")\nfn info(value: borrow Slice<u8>) -> FrameInfo\n{\n    FrameInfo { payload: bytes_copy(value), kind: 7 }\n}\n\n@id(\"frame.main\")\nfn main() -> i64 { 0 }\n",
            ),
            (
                "src/tests.spx",
                "module frame.tests;\n\n@id(\"frame.tests.main\")\nfn main() -> i64 { 0 }\n",
            ),
        ],
    );
    round_trip(&v9);
}
