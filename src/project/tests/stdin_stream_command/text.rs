//! Versioned selection and private String helper admission over two modules.
use super::*;
#[path = "text/collections.rs"]
mod collections;
fn manifest() -> String {
    exit_manifest()
        .replace(PROJECT_SCHEMA_V24, PROJECT_SCHEMA_V25)
        .replace(
            PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V2,
            PROJECT_PROFILE_STDIN_STREAM_TEXT_COMMAND_IO_V1,
        )
}
fn json_dependency_manifest() -> String {
    format!(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"stream-text-json\"\nversion = \"0.1.0\"\nprofile = \"{PROJECT_PROFILE_STDIN_STREAM_TEXT_COMMAND_IO_V1}\"\n\n[modules]\nentry = \"stream.app\"\nsources = [\"a/app.spx\", \"b/input.spx\", \"c/tests.spx\"]\ntests = [\"stream.tests\"]\n\n[exports]\nweb = [\"stream.command\"]\n\n[command]\nfunction = \"stream.command\"\ninput = \"{PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1}\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n\n[dependencies]\nstd.data.json.doc = \"=0.1.0\"\nstd.data.json.query = \"=0.1.0\"\n"
    )
}
fn text_fixture() -> PathBuf {
    let root = exit_fixture();
    std::fs::write(root.join(MANIFEST_FILE), json_dependency_manifest()).unwrap();
    std::fs::write(
        root.join("a/app.spx"),
        canonical_source(
            "a/app.spx",
            r#"module stream.app;
use function @id("text.cut") from stream.input as cut;
use function @id("text.json_probe") from stream.input as json_probe;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("stream.command") fn command() -> i64 uses { process.stdin.read, process.stdout.write } {
    let reader=stdin_stream_open();
    let empty=stdin_stream_eof(reader);
    let valid_json=json_probe();
    let text=cut("é\u{0}");
    let raw=string_as_str(text);
    let written=stdout_write(str_as_bytes(raw));
    if empty && valid_json { 0 } else { 1 }
}
@id("stream.app.main") fn main()->i64 { let text=cut("abc"); string_len(text)-3 }
"#,
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("b/input.spx"),
        canonical_source(
            "b/input.spx",
            r#"module stream.input;
use function @id("std.data.json.doc.is_document") from std.data.json.doc as is_document;
use function @id("std.data.json.query.decoded_token_eq") from std.data.json.query as decoded_token_eq;
@id("text.cut") fn cut(text:string)->string { string_slice(text,0,3) }
@id("text.json_probe") fn json_probe()->bool {
    let input=[91u8,34u8,110u8,97u8,92u8,117u8,48u8,48u8,54u8,100u8,101u8,34u8,44u8,34u8,110u8,97u8,109u8,101u8,34u8,93u8];
    let view=array_as_slice(input);
    is_document(view) && decoded_token_eq(view,1usize,13usize)
}
"#,
        ),
    )
    .unwrap();
    root
}
#[test]
fn v25_stream_text_exact_manifest_table_and_closed_old_profiles() {
    let text = manifest();
    let parsed = ProjectManifest::parse(&text).unwrap();
    assert_eq!(parsed.schema(), PROJECT_SCHEMA_V25);
    assert_eq!(
        parsed.project_profile(),
        ProjectProfile::StdinStreamTextCommandIoV1
    );
    assert_eq!(parsed.to_canonical_toml(), text);
    for schema in [PROJECT_SCHEMA_V23, PROJECT_SCHEMA_V24] {
        assert!(ProjectManifest::parse(&text.replace(PROJECT_SCHEMA_V25, schema)).is_err());
    }
    let tables=format!("schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"stream-check\"\nversion = \"0.1.0\"\nprofile = \"{PROJECT_PROFILE_STDIN_STREAM_TEXT_COMMAND_IO_V1}\"\n\n[modules]\nentry = \"stream.app\"\nsources = [\"a/app.spx\", \"b/input.spx\", \"c/tests.spx\"]\ntests = [\"stream.tests\"]\n\n[exports]\nweb = [\"stream.command\"]\n\n[command]\nfunction = \"stream.command\"\ninput = \"{PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1}\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n");
    let table = ProjectManifest::parse(&tables).unwrap();
    assert_eq!(table.schema(), PROJECT_SCHEMA_V25);
    assert_eq!(table.project_profile(), parsed.project_profile());
    for invalid in [
        text.replace("\"process.args.read\", ", ""),
        text.replace("stdin-stream.v1", "stdin-bytes.v1"),
    ] {
        assert!(ProjectManifest::parse(&invalid).is_err());
    }
}
#[test]
fn v25_stream_text_links_owned_string_helpers_and_refuses_web_npm() {
    let root = text_fixture();
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        let workspace = snapshot.workspace_manifest();
        assert!(workspace.contains("dependencies/std.data.json.query/0.1.0/query.spx"));
        assert!(workspace.contains("dependencies/std.data.json.doc/0.1.0/doc.spx"));
        assert!(workspace.contains("dependencies/std.data.json/0.1.0/json.spx"));
        Ok(())
    })
    .unwrap();
    let before = file_inventory(&root);
    assert_eq!(
        with_authenticated_project(&root.join(MANIFEST_FILE), |s| s
            .build_web_inline(MAX_PROJECT_WEB_BUILD_BYTES))
        .unwrap_err()[0]
            .code,
        "SPX-W120"
    );
    assert_eq!(
        with_authenticated_project(&root.join(MANIFEST_FILE), |s| s
            .build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES))
        .unwrap_err()[0]
            .code,
        "SPX-W120"
    );
    assert_eq!(file_inventory(&root), before);
    std::fs::write(root.join(MANIFEST_FILE), exit_manifest()).unwrap();
    let before = file_inventory(&root);
    let missing_dependency =
        with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(missing_dependency.len(), 1);
    assert_eq!(missing_dependency[0].code, "SPX-G172");
    assert_eq!(
        missing_dependency[0].message,
        "target module is missing or equals the caller module"
    );
    assert_eq!(file_inventory(&root), before);
    // Isolate the same owned String helper boundary from dependency selection:
    // v24 must still refuse text.cut after its missing JSON imports are removed.
    std::fs::write(
        root.join("a/app.spx"),
        canonical_source(
            "a/app.spx",
            r#"module stream.app;
use function @id("text.cut") from stream.input as cut;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("stream.command") fn command() -> i64 uses { process.stdin.read, process.stdout.write } {
    let reader=stdin_stream_open();
    let empty=stdin_stream_eof(reader);
    let text=cut("é\u{0}");
    let raw=string_as_str(text);
    let written=stdout_write(str_as_bytes(raw));
    if empty { 0 } else { 1 }
}
@id("stream.app.main") fn main()->i64 { let text=cut("abc"); string_len(text)-3 }
"#,
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("b/input.spx"),
        canonical_source(
            "b/input.spx",
            r#"module stream.input;
@id("text.cut") fn cut(text:string)->string { string_slice(text,0,3) }
"#,
        ),
    )
    .unwrap();
    let before = file_inventory(&root);
    let old_profile =
        with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(old_profile.len(), 1);
    assert_eq!(old_profile[0].code, "SPX-G174");
    assert_eq!(
        old_profile[0].message,
        "project function `text.cut` has a signature outside the selected profile"
    );
    assert_eq!(file_inventory(&root), before);
    let _ = std::fs::remove_dir_all(root);
}
#[test]
fn v25_stream_text_executes_entry_test_and_named_owned_string_helpers() {
    let root = text_fixture();
    std::fs::write(
        root.join("c/tests.spx"),
        canonical_source(
            "c/tests.spx",
            r#"module stream.tests;
use function @id("text.cut") from stream.input as cut;
@id("stream.tests.main") fn main()->i64 { let text=cut("abc"); string_len(text)-3 }
@id("stream.tests.owned") fn test_owned()->i64 { let text=cut("abc"); string_len(text)-3 }
"#,
        ),
    )
    .unwrap();
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        let options = ProjectExecutionOptions::default();
        assert!(snapshot.execute_entry(&options)?.command_succeeded());
        let test = snapshot.execute_test(&options)?;
        assert!(test.command_succeeded());
        assert_eq!(test.cases().len(), 1);
        let revision = snapshot.retain_revision();
        let cancelled = revision.execute_test_cancellable(&options, &ProjectExecutionCancellation::new())?;
        assert!(matches!(cancelled, super::super::super::execution::CancellableProjectExecution::Completed(test) if test.command_succeeded()));
        let prepared = revision.prepare_interpreter(PreparedProjectInterpreterOptions::default())?;
        let result = prepared.execute_test(&PreparedProjectExecutionOptions::default(), &ProjectExecutionCancellation::new())?;
        assert_eq!(result.outcome(), &ProjectPreparedExecutionOutcome::Returned(0));
        Ok(())
    }).unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn v25_stream_text_native_project_keeps_unicode_and_nul() {
    let root = text_fixture();
    let output = root.with_extension("stream-v25-native");
    with_authenticated_project(&root.join(MANIFEST_FILE), |s| s.build_native(&output)).unwrap();
    let result = Command::new(&output).stdin(Stdio::null()).output().unwrap();
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"\xc3\xa9\0");
    assert!(result.stderr.is_empty());
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir_all(root);
}
