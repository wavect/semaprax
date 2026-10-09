use super::*;
use std::io::Write;
use std::process::{Command, Stdio};

fn stream_manifest() -> String {
    format!(
        "schema = \"{PROJECT_SCHEMA_V23}\"\nname = \"stream-check\"\nversion = \"0.1.0\"\nprofile = \"{PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V1}\"\nentry = \"stream.app\"\nsources = [\"a/app.spx\", \"b/input.spx\", \"c/tests.spx\"]\nweb_exports = [\"stream.command\"]\ncommand = \"stream.command\"\ninput = \"{PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1}\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"stream.tests\"]\n"
    )
}

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-stream-v23-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("a")).unwrap();
    std::fs::create_dir_all(root.join("b")).unwrap();
    std::fs::create_dir_all(root.join("c")).unwrap();
    std::fs::write(root.join(MANIFEST_FILE), stream_manifest()).unwrap();
    std::fs::write(
        root.join("a/app.spx"),
        canonical_source(
            "a/app.spx",
            r#"module stream.app;
use function @id("stream.read") from stream.input as read_stream;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("stream.command")
fn command() -> bool uses { process.stdin.read } { read_stream() }

@id("stream.app.main")
fn main() -> i64 { 0 }
"#,
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("b/input.spx"),
        canonical_source(
            "b/input.spx",
            r#"module stream.input;
permit { process.stdin.read }

@id("stream.read")
fn read_stream() -> bool uses { process.stdin.read } {
    let mut reader = stdin_stream_open();
    let mut saw_chunk = false;
    while !stdin_stream_eof(reader) {
        let chunk_size = { let chunk = stdin_stream_chunk(reader); byte_len(chunk) };
        if chunk_size > 0usize { saw_chunk = true; }
        reader = stdin_stream_next(reader);
        0
    }
    saw_chunk
}
"#,
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("c/tests.spx"),
        canonical_source(
            "c/tests.spx",
            r#"module stream.tests;
@id("stream.tests.main")
fn main() -> i64 { 0 }
"#,
        ),
    )
    .unwrap();
    root.canonicalize().unwrap()
}

#[test]
fn v23_manifest_is_canonical_and_v6_v7_cannot_select_stream_input() {
    let text = stream_manifest();
    let parsed = ProjectManifest::parse(&text).unwrap();
    assert_eq!(parsed.schema(), PROJECT_SCHEMA_V23);
    assert_eq!(
        parsed.project_profile(),
        ProjectProfile::StdinStreamCommandIoV1
    );
    assert_eq!(
        parsed.command_input(),
        Some(PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1)
    );
    assert!(parsed.is_v23());
    assert_eq!(parsed.to_canonical_toml(), text);

    for (schema, profile, input) in [
        (
            PROJECT_SCHEMA_V6,
            "language-command-io.v1",
            PROJECT_LANGUAGE_COMMAND_INPUT_V1,
        ),
        (
            PROJECT_SCHEMA_V7,
            "line-command-io.v1",
            PROJECT_LANGUAGE_COMMAND_INPUT_V1,
        ),
    ] {
        let old_manifest = text
            .replace(PROJECT_SCHEMA_V23, schema)
            .replace(PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V1, profile)
            .replace(PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1, input);
        assert!(ProjectManifest::parse(&old_manifest).is_ok());
        let forged_stream = old_manifest
            .replace(profile, PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V1)
            .replace(input, PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1);
        assert!(ProjectManifest::parse(&forged_stream).is_err());
    }
}

#[test]
fn table_layout_selects_the_explicit_v23_stream_profile() {
    let text = format!(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"stream-check\"\nversion = \"0.1.0\"\nprofile = \"{PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V1}\"\n\n[modules]\nentry = \"stream.app\"\nsources = [\"a/app.spx\", \"b/input.spx\", \"c/tests.spx\"]\ntests = [\"stream.tests\"]\n\n[exports]\nweb = [\"stream.command\"]\n\n[command]\nfunction = \"stream.command\"\ninput = \"{PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1}\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n"
    );
    let manifest = ProjectManifest::parse(&text).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V23);
    assert_eq!(
        manifest.project_profile(),
        ProjectProfile::StdinStreamCommandIoV1
    );
    assert_eq!(
        manifest.command_input(),
        Some(PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1)
    );
}

#[test]
fn stream_profile_refuses_web_and_npm_before_artifact_creation() {
    let root = fixture();
    let before = file_inventory(&root);
    let web_error = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_web_inline(MAX_PROJECT_WEB_BUILD_BYTES)
    })
    .unwrap_err();
    assert_eq!(web_error[0].code, "SPX-W120");
    assert!(web_error[0].message.contains("no WebAssembly bridge"));
    let npm_error = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)
    })
    .unwrap_err();
    assert_eq!(npm_error[0].code, "SPX-W120");
    assert!(npm_error[0].message.contains("no npm/WebAssembly bridge"));
    assert_eq!(file_inventory(&root), before);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn native_stream_profile_accepts_input_larger_than_the_snapshot_limit() {
    let root = fixture();
    let output = root.with_extension("stream-v23-native");
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_native(&output)
    })
    .unwrap();

    let mut child = Command::new(&output)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&vec![b' '; 70_000])
        .unwrap();
    assert!(child.wait().unwrap().success());

    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir_all(root);
}

fn exit_manifest() -> String {
    stream_manifest()
        .replace(PROJECT_SCHEMA_V23, PROJECT_SCHEMA_V24)
        .replace(
            PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V1,
            PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V2,
        )
}

fn exit_fixture() -> PathBuf {
    let root = fixture();
    std::fs::write(root.join(MANIFEST_FILE), exit_manifest()).unwrap();
    std::fs::write(root.join("a/app.spx"), canonical_source("a/app.spx", r#"module stream.app;
use function @id("stream.read") from stream.input as read_stream;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("stream.command")
fn command() -> i64 uses { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write } {
    let read = read_stream();
    let count = args_len();
    let written = if count == 2usize {
        let message = "request rejected\n";
        let view = string_as_str(message);
        let diagnostic = stderr_write(str_as_bytes(view));
        0
    } else {
        let message = "complete\n";
        let view = string_as_str(message);
        let output = stdout_write(str_as_bytes(view));
        0
    };
    if count == 0usize { 0 } else {
        if count == 1usize { 1 } else {
            if count == 2usize { 2 } else {
                if count == 3usize { 255 } else {
                    if count == 4usize { -1 } else { 256 }
                }
            }
        }
    }
}
@id("stream.app.main") fn main() -> i64 { 0 }
"#)).unwrap();
    root
}

#[test]
fn v24_stream_exit_manifest_and_table_selection_preserve_v23() {
    let text = exit_manifest();
    let parsed = ProjectManifest::parse(&text).unwrap();
    assert_eq!(parsed.schema(), PROJECT_SCHEMA_V24);
    assert_eq!(
        parsed.project_profile(),
        ProjectProfile::StdinStreamCommandIoV2
    );
    assert_eq!(parsed.to_canonical_toml(), text);
    assert!(!parsed.is_v23());
    let tables = format!("schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"stream-check\"\nversion = \"0.1.0\"\nprofile = \"{PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V2}\"\n\n[modules]\nentry = \"stream.app\"\nsources = [\"a/app.spx\", \"b/input.spx\", \"c/tests.spx\"]\ntests = [\"stream.tests\"]\n\n[exports]\nweb = [\"stream.command\"]\n\n[command]\nfunction = \"stream.command\"\ninput = \"{PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1}\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n");
    let table_manifest = ProjectManifest::parse(&tables).unwrap();
    assert_eq!(table_manifest.schema(), PROJECT_SCHEMA_V24);
    assert_eq!(table_manifest.project_profile(), parsed.project_profile());
    assert_eq!(table_manifest.command_input(), parsed.command_input());
    assert_eq!(table_manifest.command(), parsed.command());
    assert_eq!(table_manifest.entry(), parsed.entry());
    assert_eq!(table_manifest.sources(), parsed.sources());
    assert_eq!(table_manifest.web_exports(), parsed.web_exports());
    assert_eq!(table_manifest.capabilities(), parsed.capabilities());
    assert_eq!(table_manifest.test_module(), parsed.test_module());
    assert_eq!(table_manifest.to_canonical_toml(), tables);
    assert!(ProjectManifest::parse(&text.replace(PROJECT_SCHEMA_V24, PROJECT_SCHEMA_V23)).is_err());
    assert!(ProjectManifest::parse(
        &stream_manifest().replace(PROJECT_SCHEMA_V23, PROJECT_SCHEMA_V24)
    )
    .is_err());
    assert_eq!(
        ProjectManifest::parse(&stream_manifest())
            .unwrap()
            .to_canonical_toml(),
        stream_manifest()
    );
}

#[test]
fn v24_stream_exit_requires_i64_and_refuses_web_and_npm_without_artifacts() {
    let root = exit_fixture();
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        let error = crate::codegen::emit_hir_c_with_stdin_stream(
            snapshot.public_api_program(),
            "stream.command",
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-B103");
        assert!(error.message.contains("fn () -> bool"));
        let emitted = crate::codegen::emit_hir_c_with_stdin_stream_exit_status(
            snapshot.public_api_program(),
            "stream.command",
        )
        .unwrap();
        assert!(emitted.contains("spx_language_command_stream_run_v2"));
        assert!(!emitted.contains("int spx_language_command_stream_run_v1("));
        Ok(())
    })
    .unwrap();
    let before = file_inventory(&root);
    for web in [true, false] {
        let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
            if web {
                snapshot
                    .build_web_inline(MAX_PROJECT_WEB_BUILD_BYTES)
                    .map(|_| ())
            } else {
                snapshot
                    .build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)
                    .map(|_| ())
            }
        })
        .unwrap_err();
        assert_eq!(errors[0].code, "SPX-W120");
        assert!(errors[0]
            .message
            .contains(PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V2));
        assert_eq!(file_inventory(&root), before);
    }
    std::fs::write(root.join(MANIFEST_FILE), stream_manifest()).unwrap();
    let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(errors[0].code, "SPX-G172");
    assert!(errors[0].message.contains("fn() -> bool"));
    let _ = std::fs::remove_dir_all(root);
    let root = fixture();
    std::fs::write(root.join(MANIFEST_FILE), exit_manifest()).unwrap();
    let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(errors[0].code, "SPX-G172");
    assert!(errors[0].message.contains("fn() -> i64"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn native_v24_stream_exit_preserves_application_status_and_discards_invalid_results() {
    let root = exit_fixture();
    let output = root.with_extension("stream-v24-native");
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_native(&output)
    })
    .unwrap();
    for (count, status, stdout, stderr) in [
        (0, 0, "complete\n", ""),
        (1, 1, "complete\n", ""),
        (2, 2, "", "request rejected\n"),
        (3, 255, "complete\n", ""),
        (4, 2, "", "SEMAPRAX language command failed\n"),
        (5, 2, "", "SEMAPRAX language command failed\n"),
    ] {
        let mut child = Command::new(&output)
            .args(vec!["x"; count])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"stream input")
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert_eq!(result.status.code(), Some(status), "{count}");
        assert_eq!(result.stdout, stdout.as_bytes(), "{count}");
        assert_eq!(result.stderr, stderr.as_bytes(), "{count}");
    }
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir_all(root);
}

#[path = "stdin_stream_command/stream_data.rs"]
mod stream_data;
#[path = "stdin_stream_command/text.rs"]
mod text;

#[path = "stdin_stream_command/stream_records.rs"]
mod stream_records;
