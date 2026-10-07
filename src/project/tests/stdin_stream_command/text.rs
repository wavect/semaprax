//! Versioned selection and private String helper admission over two modules.
use super::*;
fn manifest() -> String {
    exit_manifest()
        .replace(PROJECT_SCHEMA_V24, PROJECT_SCHEMA_V25)
        .replace(
            PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V2,
            PROJECT_PROFILE_STDIN_STREAM_TEXT_COMMAND_IO_V1,
        )
}
fn text_fixture() -> PathBuf {
    let root = exit_fixture();
    std::fs::write(root.join(MANIFEST_FILE), manifest()).unwrap();
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
    with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap();
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
    assert!(
        with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(()))
            .unwrap_err()
            .iter()
            .any(|d| d.code == "SPX-G174")
    );
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
