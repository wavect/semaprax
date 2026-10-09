//! Explicit v30 routing, with the ordinary owning-collection corpus as a provider.
use super::*;
use crate::hir;

fn manifest(profile: &str) -> String {
    format!("schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"stream-owned\"\nversion = \"0.1.0\"\nprofile = \"{profile}\"\n\n[modules]\nentry = \"owned.app\"\nsources = [\"a/app.spx\", \"b/data.spx\", \"c/tests.spx\"]\ntests = [\"owned.tests\"]\n\n[exports]\nweb = [\"owned.command\"]\n\n[command]\nfunction = \"owned.command\"\ninput = \"{PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1}\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n")
}

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-owned-v30-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    for directory in ["a", "b", "c"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(
        root.join(MANIFEST_FILE),
        manifest(PROJECT_PROFILE_STDIN_STREAM_OWNED_DATA_COMMAND_IO_V1),
    )
    .unwrap();
    let provider = include_str!("../../../../tests/fixtures/owned-leaf-collections.spx")
        .replace("fn main()", "fn verify()");
    let app = r#"module owned.app;
use function @id("owned.leaf.main") from owned.leaf as verify;
permit {process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}
@id("owned.app.main") fn main()->i64 {verify()}
@id("owned.command") fn command()->i64 uses {process.stdin.read,process.stdout.write} {
 let mut reader=stdin_stream_open(); let mut size=0;
 while !stdin_stream_eof(reader) {
  let n={let chunk=stdin_stream_chunk(reader); byte_len(chunk)};
  size=size+i64_from_usize(n); reader=stdin_stream_next(reader); 0
 }
 let status=verify();
 if status==0 {
  let text=string_from_i64(size); let view=string_as_str(text);
  let written=stdout_write(str_as_bytes(view)); 0
 }else{status}
}
"#;
    let cases = r#"module owned.tests;
use function @id("owned.leaf.main") from owned.leaf as verify;
@id("owned.tests.main") fn main()->i64 {verify()}
@id("owned.tests.collections") fn test_collections()->i64 {verify()}
"#;
    for (path, source) in [
        ("a/app.spx", app),
        ("b/data.spx", provider.as_str()),
        ("c/tests.spx", cases),
    ] {
        std::fs::write(root.join(path), canonical_source(path, source)).unwrap();
    }
    root.canonicalize().unwrap()
}

fn example_fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-owned-leaf-example-v30-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/owned-leaf-command-project");
    std::fs::copy(example.join(MANIFEST_FILE), root.join(MANIFEST_FILE)).unwrap();
    for file in ["app.spx", "data.spx", "tests.spx"] {
        std::fs::copy(
            example.join("src").join(file),
            root.join("src").join(file),
        )
        .unwrap();
    }
    root.canonicalize().unwrap()
}

#[test]
fn v30_owned_collections_execute_project_tests_prepared_and_native_stream_command() {
    let root = fixture();
    let output = root.with_extension("native-v30");
    let graph = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert_eq!(snapshot.manifest().schema(), PROJECT_SCHEMA_V30);
        assert_eq!(
            snapshot.manifest().project_profile(),
            ProjectProfile::StdinStreamOwnedDataCommandIoV1
        );
        assert!(snapshot
            .manifest()
            .capabilities()
            .iter()
            .map(String::as_str)
            .eq(PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2));
        assert!(snapshot
            .execute_entry(&ProjectExecutionOptions::default())?
            .command_succeeded());
        let cases = snapshot.execute_test(&ProjectExecutionOptions::default())?;
        assert!(cases.command_succeeded());
        assert_eq!(cases.cases().len(), 1);
        let prepared = snapshot
            .retain_revision()
            .prepare_interpreter(PreparedProjectInterpreterOptions::default())?;
        assert_eq!(
            prepared
                .execute_test(
                    &PreparedProjectExecutionOptions::default(),
                    &ProjectExecutionCancellation::new()
                )?
                .outcome(),
            &ProjectPreparedExecutionOutcome::Returned(0)
        );
        let command = hir::DeclarationId::new("owned.command");
        hir::validate_stream_owned_program(snapshot.public_api_program(), Some(&command)).unwrap();
        assert!(
            hir::validate_stream_record_program(snapshot.public_api_program(), Some(&command))
                .is_err()
        );
        let mut forged = snapshot.public_api_program().clone();
        forged
            .functions
            .iter_mut()
            .find(|f| f.id == command)
            .unwrap()
            .return_type = hir::ResolvedType::Bool;
        assert!(hir::validate_stream_owned_program(&forged, Some(&command)).is_err());
        assert!(
            crate::codegen::emit_hir_c_with_stdin_stream_owned_data(&forged, "owned.command")
                .is_err()
        );
        assert_eq!(
            snapshot
                .build_web_inline(MAX_PROJECT_WEB_BUILD_BYTES)
                .unwrap_err()[0]
                .code,
            "SPX-W120"
        );
        assert_eq!(
            snapshot
                .build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)
                .unwrap_err()[0]
                .code,
            "SPX-W120"
        );
        snapshot.build_native(&output)?;
        Ok(snapshot.semantic_graph().to_owned())
    })
    .unwrap();
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert_eq!(snapshot.semantic_graph(), graph);
        Ok(())
    })
    .unwrap();
    let mut child = Command::new(&output)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"abcdefghi").unwrap();
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"9");
    assert!(result.stderr.is_empty());
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn v30_owned_leaf_command_example_executes_entry_tests_prepared_and_native_command() {
    let root = example_fixture();
    let output = root.with_extension("native-v30-example");
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert_eq!(snapshot.manifest().schema(), PROJECT_SCHEMA_V30);
        assert_eq!(
            snapshot.manifest().project_profile(),
            ProjectProfile::StdinStreamOwnedDataCommandIoV1
        );
        assert!(snapshot
            .execute_entry(&ProjectExecutionOptions::default())?
            .command_succeeded());
        let cases = snapshot.execute_test(&ProjectExecutionOptions::default())?;
        assert!(cases.command_succeeded());
        assert_eq!(cases.cases().len(), 1);
        let prepared = snapshot
            .retain_revision()
            .prepare_interpreter(PreparedProjectInterpreterOptions::default())?;
        assert_eq!(
            prepared
                .execute_test(
                    &PreparedProjectExecutionOptions::default(),
                    &ProjectExecutionCancellation::new()
                )?
                .outcome(),
            &ProjectPreparedExecutionOutcome::Returned(0)
        );
        snapshot.build_native(&output)?;
        Ok(())
    })
    .unwrap();
    let mut child = Command::new(&output)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"abcdefghi").unwrap();
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"9");
    assert!(result.stderr.is_empty());
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn v30_selection_does_not_widen_frozen_stream_profiles_or_root_abi() {
    let valid = manifest(PROJECT_PROFILE_STDIN_STREAM_OWNED_DATA_COMMAND_IO_V1);
    assert_eq!(
        ProjectManifest::parse(&valid).unwrap().schema(),
        PROJECT_SCHEMA_V30
    );
    for input in [
        valid.replace(PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1, "argv-utf8.v1"),
        valid.replace("process.stdin.read", "process.network.read"),
    ] {
        assert!(ProjectManifest::parse(&input).is_err());
    }
    let root = fixture();
    for profile in [
        PROJECT_PROFILE_STDIN_STREAM_DATA_COMMAND_IO_V1,
        PROJECT_PROFILE_STDIN_STREAM_DATA_COMMAND_IO_V2,
    ] {
        std::fs::write(root.join(MANIFEST_FILE), manifest(profile)).unwrap();
        let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.code == "SPX-G172" || error.code == "SPX-H006"));
    }
    std::fs::write(
        root.join(MANIFEST_FILE),
        manifest(PROJECT_PROFILE_STDIN_STREAM_OWNED_DATA_COMMAND_IO_V1),
    )
    .unwrap();
    let app_path = root.join("a/app.spx");
    let original = std::fs::read_to_string(&app_path).unwrap();
    let changed = original.replace("fn command() -> i64", "fn command(value: i64) -> i64");
    assert_ne!(changed, original);
    std::fs::write(&app_path, canonical_source("a/app.spx", &changed)).unwrap();
    let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-G172"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn frozen_stream_profile_rejects_owned_collection_uses_in_an_unused_scalar_helper() {
    let root = fixture();
    for (path, module) in [("a/app.spx", "owned.app"), ("c/tests.spx", "owned.tests")] {
        let mut source = format!("module {module};\n");
        if path == "a/app.spx" {
            source.push_str("permit {process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}\n@id(\"owned.command\") fn command()->i64 {0}\n");
        }
        source.push_str(&format!("@id(\"{module}.main\") fn main()->i64 {{0}}\n"));
        std::fs::write(root.join(path), canonical_source(path, &source)).unwrap();
    }
    std::fs::write(
        root.join(MANIFEST_FILE),
        manifest(PROJECT_PROFILE_STDIN_STREAM_DATA_COMMAND_IO_V2),
    )
    .unwrap();
    let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-G172"
        && error.message.contains("language-command-io.owned-data.v1")));
    let _ = std::fs::remove_dir_all(root);
}
