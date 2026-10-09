//! Project v26: ordinary bundled imports with the exact SourceCommand runtime.
use semaprax::project::{self, ProjectExecutionOptions, ProjectManifest, ProjectProfile};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const MANIFEST: &str = r#"schema = "semaprax.manifest.v1"

[package]
name = "decimal-command"
version = "0.1.0"
profile = "source-command.v1"

[modules]
entry = "decimal.command"
sources = ["app.spx", "tests.spx"]
tests = ["decimal.tests"]

[exports]
web = []

[command]
function = "decimal.command.main"
input = "argv-utf8+file-text.v1"

[capabilities]
required = ["fs.read", "process.args.read", "process.stderr.write", "process.stdout.write"]

[dependencies]
std.int.decimal = "=0.1.0"

[targets]
matrix = ["native64"]
"#;

const RESOURCE_MANIFEST: &str = r#"schema = "semaprax.manifest.v1"

[package]
name = "decimal-command"
version = "0.1.0"
profile = "source-command.resource-output.v1"

[modules]
entry = "decimal.command"
sources = ["app.spx", "tests.spx"]
tests = ["decimal.tests"]

[exports]
web = []

[command]
function = "decimal.command.main"
input = "argv-utf8+file-text.v1"

[capabilities]
required = ["fs.read", "process.args.read", "process.stderr.write", "process.stdout.write"]

[targets]
matrix = ["native64"]
"#;

fn resource_app(repeats: usize, result: i64) -> String {
    format!(
        "module decimal.command;\npermit {{ fs.read, process.args.read, process.stderr.write, process.stdout.write }}\n\n@id(\"decimal.command.main\")\nfn main() -> i64 uses {{ fs.read, process.args.read, process.stderr.write, process.stdout.write }}\n{{\n    if args_len() != 1usize {{\n        let usage = \"usage\\n\";\n        let view = string_as_str(usage);\n        let ignored = stderr_append(str_as_bytes(view));\n        2\n    }} else {{\n        let path = arg_utf8(0usize);\n        let left = file_read_text(path);\n        let right = file_read_text(path);\n        let joined = string_concat(left, right);\n        let joined_view = string_as_str(joined);\n        let joined_bytes = str_as_bytes(joined_view);\n        let mut index = 0usize;\n        while index < {repeats}usize {{\n            let written = stdout_append(joined_bytes);\n            index = index + 1usize;\n            index < {repeats}usize\n        }}\n        {result}\n    }}\n}}\n"
    )
}

fn resource_direct_app() -> &'static str {
    r#"module decimal.command;
permit { fs.read, process.args.read, process.stdout.write }

@id("decimal.command.main")
fn main() -> i64 uses { fs.read, process.args.read, process.stdout.write }
{
    if args_len() != 1usize {
        2
    } else {
        let path = arg_utf8(0usize);
        let left = file_read_text(path);
        let right = file_read_text(path);
        let joined = string_concat(left, right);
        let joined_view = string_as_str(joined);
        let joined_bytes = str_as_bytes(joined_view);
        let written = stdout_write(joined_bytes);
        if written == 131072usize { 0 } else { 1 }
    }
}
"#
}

const APP: &str = r#"module decimal.command;
use function @id("std.int.decimal.canonicalize") from std.int.decimal as canonicalize;
use function @id("std.int.decimal.add") from std.int.decimal as add;
use function @id("std.int.decimal.divide") from std.int.decimal as divide;
permit { fs.read, process.args.read, process.stderr.write, process.stdout.write }

@id("decimal.command.main")
fn main() -> i64 uses { fs.read, process.args.read, process.stderr.write, process.stdout.write }
{
    if args_len() != 1usize {
        let usage = "usage\n";
        let view = string_as_str(usage);
        let ignored = stderr_write(str_as_bytes(view));
        2
    } else {
        let path = arg_utf8(0usize);
        let input = file_read_text(path);
        let value = canonicalize(input);
        let sum = add(value, "1");
        let divisor = "3";
        let output = divide(sum, string_as_str(divisor));
        let view = string_as_str(output);
        let ignored = stdout_write(str_as_bytes(view));
        0
    }
}
"#;

struct Fixture(PathBuf);
impl Fixture {
    fn new(app: &str, manifest: &str) -> Self {
        Self::with_tests(
            app,
            manifest,
            "module decimal.tests;\n@id(\"decimal.tests.main\")\nfn main() -> i64 { 0 }\n",
        )
    }

    fn with_tests(app: &str, manifest: &str, tests: &str) -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "semaprax-source-command-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
        for (path, source) in [("app.spx", app), ("tests.spx", tests)] {
            let path = root.join(path);
            let program = semaprax::parse(source, &path).unwrap();
            std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
        }
        Self(root)
    }
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn source_command_native_tests_compile_only_declared_test_roots() {
    let tests = r#"module decimal.tests;
@id("decimal.tests.main")
fn main() -> i64 { 0 }

@id("decimal.tests.test_pass")
fn test_pass() -> i64 { 0 }

@id("decimal.tests.test_fail")
fn test_fail() -> i64 { 7 }

@id("decimal.tests.test_helper")
fn test_helper(value: i64) -> i64 { value }
"#;
    let resource_app = resource_app(1, 0);
    for (app, manifest) in [(APP, MANIFEST), (resource_app.as_str(), RESOURCE_MANIFEST)] {
        let fixture = Fixture::with_tests(app, manifest, tests);
        let manifest_path = fixture.0.join("semaprax.toml");
        project::with_authenticated_project(&manifest_path, |snapshot| {
            let roots = snapshot.native_test_roots()?;
            assert_eq!(
                roots
                    .iter()
                    .map(|root| (root.stable_id(), root.is_main()))
                    .collect::<Vec<_>>(),
                [
                    ("decimal.tests.main", true),
                    ("decimal.tests.test_fail", false),
                    ("decimal.tests.test_pass", false),
                ]
            );
            assert_eq!(
                snapshot
                    .build_native_test("decimal.tests.test_helper", &fixture.0.join("excluded"))
                    .unwrap_err()[0]
                    .code,
                "SPX-G172"
            );
            assert!(!fixture.0.join("excluded").exists());
            for (index, root) in roots.iter().enumerate() {
                let binary = fixture.0.join(format!("native-test-{index}"));
                snapshot.build_native_test(root.stable_id(), &binary)?;
                let result = Command::new(&binary).output().unwrap();
                assert!(result.status.success(), "{}", root.stable_id());
                let expected = if root.stable_id() == "decimal.tests.test_fail" {
                    b"7\n"
                } else {
                    b"0\n"
                };
                assert_eq!(result.stdout, expected, "{}", root.stable_id());
                assert!(result.stderr.is_empty());
                std::fs::remove_file(binary).unwrap();
            }
            assert_eq!(
                snapshot
                    .execute_test(&ProjectExecutionOptions::default())
                    .unwrap_err()[0]
                    .code,
                "SPX-F102"
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn source_command_native_tests_use_manifest_bounded_argv_and_file_provider() {
    let tests = r#"module decimal.tests;
permit { fs.read, process.args.read }

@id("decimal.tests.main")
fn main() -> i64 uses { process.args.read }
{
    if args_len() == 0usize { 0 } else { 1 }
}

@id("decimal.tests.test_file")
fn test_file() -> i64 uses { fs.read }
{
    let path = "digits";
    let view = string_as_str(path);
    let contents = file_read_text(view);
    if string_len(contents) == 4 { 0 } else { 1 }
}
"#;
    let fixture = Fixture::with_tests(APP, MANIFEST, tests);
    std::fs::write(fixture.0.join("digits"), "1234").unwrap();
    project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let roots = snapshot.native_test_roots()?;
        assert_eq!(roots.len(), 2);
        assert!(roots.iter().all(|root| root.result_is_exit_status()));
        for (index, root) in roots.iter().enumerate() {
            let output = fixture.0.join(format!("effectful-native-test-{index}"));
            snapshot.build_native_test(root.stable_id(), &output)?;
            let result = Command::new(&output)
                .current_dir(&fixture.0)
                .output()
                .unwrap();
            assert_eq!(result.status.code(), Some(0), "{}", root.stable_id());
            assert!(result.stdout.is_empty());
            assert!(result.stderr.is_empty());
            std::fs::remove_file(output).unwrap();
        }
        Ok(())
    })
    .unwrap();
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn source_command_manifest_projects_exact_profile_and_refuses_other_targets() {
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    assert_eq!(manifest.schema(), "semaprax.project.v26");
    assert_eq!(manifest.project_profile(), ProjectProfile::SourceCommandV1);
    assert_eq!(manifest.to_canonical_toml(), MANIFEST);
    for invalid in [
        MANIFEST.replace("argv-utf8+file-text.v1", "argv-utf8+stdin-bytes.v1"),
        MANIFEST.replace("matrix = [\"native64\"]", "matrix = [\"native64\", \"wasm32\"]"),
        MANIFEST.replace("\"fs.read\", \"process.args.read\", \"process.stderr.write\", \"process.stdout.write\"", "\"process.stdout.write\""),
        MANIFEST.replace("\"fs.read\",", "\"fs.write\","),
        MANIFEST.replace("web = []", "web = [\"decimal.command.main\"]"),
    ] {
        assert!(ProjectManifest::parse(&invalid).is_err(), "{invalid}");
    }
}

#[test]
fn resource_output_manifest_projects_additive_v28_without_changing_v26() {
    let manifest = ProjectManifest::parse(RESOURCE_MANIFEST).unwrap();
    assert_eq!(manifest.schema(), "semaprax.project.v28");
    assert_eq!(
        manifest.project_profile(),
        ProjectProfile::SourceCommandResourceOutputV1
    );
    assert_eq!(manifest.to_canonical_toml(), RESOURCE_MANIFEST);
    assert_eq!(
        ProjectManifest::parse(MANIFEST).unwrap().schema(),
        "semaprax.project.v26"
    );
    for invalid in [
        RESOURCE_MANIFEST.replace("argv-utf8+file-text.v1", "argv-utf8+stdin-bytes.v1"),
        RESOURCE_MANIFEST.replace("matrix = [\"native64\"]", "matrix = [\"wasm32\"]"),
        RESOURCE_MANIFEST.replace("\"fs.read\", ", "\"process.stdin.read\", "),
        RESOURCE_MANIFEST.replace("web = []", "web = [\"decimal.command.main\"]"),
    ] {
        assert!(ProjectManifest::parse(&invalid).is_err(), "{invalid}");
    }
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn resource_output_stages_large_appends_and_discards_every_failed_attempt() {
    let fixture = Fixture::new(&resource_app(8, 0), RESOURCE_MANIFEST);
    let binary = fixture.0.join("resource-command");
    let (semantic_graph, semantic_graph_digest) =
        project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            snapshot.check()?;
            let graph: serde_json::Value = serde_json::from_str(snapshot.semantic_graph()).unwrap();
            assert_eq!(graph["project_schema"], "semaprax.project.v28");
            assert_eq!(
                graph["source_command_resource_output"],
                serde_json::json!({
                    "schema": "semaprax.source-command-resource-output.v1",
                    "profile": "source-command.resource-output.v1",
                    "portable_capacity_summaries_role": "pre-hir-source-site-admission-only",
                    "legacy_direct_output_max_bytes": 65_536,
                    "multiple_unknown_direct_roots": "refused",
                    "owned_string_max_bytes": 1_048_576,
                    "authenticated_borrowed_str_max_bytes": 1_048_576,
                    "ordinary_slice_max_bytes": 65_536,
                    "owned_bytes_max_bytes": 131_072,
                    "combined_staged_output_max_bytes": 1_048_576,
                    "publication": "terminal-success-only",
                    "failure": "discard-wipe-free",
                })
            );
            let graph_digest = graph["graph_digest"].as_str().unwrap().to_owned();
            let lock: serde_json::Value =
                serde_json::from_str(&project::render_project_lock(snapshot)?).unwrap();
            assert_eq!(
                lock["payload"]["package"]["profile"],
                "source-command.resource-output.v1"
            );
            assert_eq!(
                lock["payload"]["package"]["contract"],
                "semaprax.project.v28"
            );
            assert_eq!(
                lock["payload"]["interface"]["kind"],
                "source-command.resource-output.v1"
            );
            assert_eq!(
                lock["payload"]["interface"]["digest"],
                serde_json::Value::Null
            );
            assert_eq!(
                snapshot
                    .execute_entry(&ProjectExecutionOptions::default())
                    .unwrap_err()[0]
                    .code,
                "SPX-F102"
            );
            assert_eq!(snapshot.test_wasm_module().unwrap_err()[0].code, "SPX-W120");
            assert_eq!(
                snapshot.build_npm_inline(1_000_000).unwrap_err()[0].code,
                "SPX-W120"
            );
            snapshot.build_native(&binary)?;
            Ok((snapshot.semantic_graph().to_owned(), graph_digest))
        })
        .unwrap();
    project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        assert_eq!(snapshot.semantic_graph(), semantic_graph);
        let graph: serde_json::Value = serde_json::from_str(snapshot.semantic_graph()).unwrap();
        assert_eq!(graph["project_schema"], "semaprax.project.v28");
        assert_eq!(
            graph["graph_digest"].as_str(),
            Some(semantic_graph_digest.as_str())
        );
        Ok(())
    })
    .unwrap();
    let payload = vec![b'7'; 65_536];
    std::fs::write(fixture.0.join("full"), &payload).unwrap();
    let output = Command::new(&binary)
        .current_dir(&fixture.0)
        .arg("full")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.len(), 1_048_576);
    assert!(output.stdout.iter().all(|byte| *byte == b'7'));

    let over = resource_app(8, 0).replacen(
        "        0\n    }\n}",
        "        let extra = \"x\";\n        let extra_view = string_as_str(extra);\n        let extra_written = stderr_append(str_as_bytes(extra_view));\n        0\n    }\n}",
        1,
    );
    for (name, source) in [("over", over), ("late-failure", resource_app(3, 256))] {
        let failed = Fixture::new(&source, RESOURCE_MANIFEST);
        let failed_binary = failed.0.join(name);
        project::with_authenticated_project(&failed.0.join("semaprax.toml"), |snapshot| {
            snapshot.build_native(&failed_binary)
        })
        .unwrap();
        std::fs::write(failed.0.join("full"), &payload).unwrap();
        let output = Command::new(&failed_binary)
            .current_dir(&failed.0)
            .arg("full")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }

    let stale = Fixture::new(&resource_app(3, 0), RESOURCE_MANIFEST);
    let source_path = stale.0.join("app.spx");
    let stale_output = stale.0.join("stale-resource-command");
    let result = project::with_authenticated_project(&stale.0.join("semaprax.toml"), |snapshot| {
        let changed = resource_app(4, 0);
        let parsed = semaprax::parse(&changed, &source_path).unwrap();
        std::fs::write(&source_path, semaprax::format::canonical(&parsed)).unwrap();
        assert!(snapshot.build_native(&stale_output).is_err());
        assert!(!stale_output.exists());
        Ok(())
    });
    assert!(result.is_err());
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn resource_output_wide_direct_borrowed_text_uses_checked_v28_route() {
    let manifest = RESOURCE_MANIFEST.replace(
        "[\"fs.read\", \"process.args.read\", \"process.stderr.write\", \"process.stdout.write\"]",
        "[\"fs.read\", \"process.args.read\", \"process.stdout.write\"]",
    );
    let fixture = Fixture::new(resource_direct_app(), &manifest);
    let binary = fixture.0.join("wide-direct-command");
    project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        snapshot.build_native(&binary)
    })
    .unwrap();
    std::fs::write(fixture.0.join("full"), vec![b'8'; 65_536]).unwrap();
    let output = Command::new(&binary)
        .current_dir(&fixture.0)
        .arg("full")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.len(), 131_072);
    assert!(output.stdout.iter().all(|byte| *byte == b'8'));
}

#[test]
fn source_command_profiles_retain_pre_hir_multiple_unknown_write_refusal() {
    let source = r#"module decimal.command;
permit { fs.read, process.args.read, process.stderr.write, process.stdout.write }

@id("decimal.command.main")
fn main() -> i64 uses { fs.read, process.args.read, process.stderr.write, process.stdout.write }
{
    let path = arg_utf8(0usize);
    let stdout_text = file_read_text(path);
    let stdout_view = string_as_str(stdout_text);
    let stdout_written = stdout_write(str_as_bytes(stdout_view));
    let stderr_text = file_read_text(path);
    let stderr_view = string_as_str(stderr_text);
    let stderr_written = stderr_write(str_as_bytes(stderr_view));
    0
}
"#;
    for manifest in [
        RESOURCE_MANIFEST.to_owned(),
        RESOURCE_MANIFEST.replace("source-command.resource-output.v1", "source-command.v1"),
    ] {
        let fixture = Fixture::new(source, &manifest);
        let errors =
            project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
                snapshot.check()
            })
            .unwrap_err();
        assert_eq!(errors[0].code, "SPX-T269", "{manifest}");
    }
}

#[test]
fn resource_output_refuses_legacy_append_mixtures_at_source_boundary() {
    let mixed = r#"module decimal.command;
permit { process.stderr.write, process.stdout.write }
@id("decimal.command.main")
fn main() -> i64 uses { process.stderr.write, process.stdout.write }
{
    let first = "first";
    let first_view = string_as_str(first);
    let wrote = stdout_write(str_as_bytes(first_view));
    let second = "second";
    let second_view = string_as_str(second);
    let appended = stderr_append(str_as_bytes(second_view));
    0
}
"#;
    let manifest = RESOURCE_MANIFEST.replace(
        "[\"fs.read\", \"process.args.read\", \"process.stderr.write\", \"process.stdout.write\"]",
        "[\"process.stderr.write\", \"process.stdout.write\"]",
    );
    let fixture = Fixture::new(mixed, &manifest);
    let errors =
        project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            snapshot.check()
        })
        .unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, "SPX-T269");
    assert_eq!(
        errors[0].message,
        "legacy transcript writes and runtime-bounded appends cannot share an executable path"
    );
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn source_command_bundled_decimal_native_and_closed_runtime_failures() {
    let fixture = Fixture::new(APP, MANIFEST);
    let binary = fixture.0.join("command");
    project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let graph: serde_json::Value = serde_json::from_str(snapshot.semantic_graph()).unwrap();
        assert_eq!(graph["project_schema"], "semaprax.project.v26");
        assert!(graph.get("source_command_resource_output").is_none());
        let lock: serde_json::Value =
            serde_json::from_str(&project::render_project_lock(snapshot)?).unwrap();
        assert_eq!(lock["payload"]["package"]["profile"], "source-command.v1");
        assert_eq!(
            lock["payload"]["package"]["contract"],
            "semaprax.project.v26"
        );
        assert_eq!(lock["payload"]["interface"]["kind"], "source-command.v1");
        assert_eq!(
            lock["payload"]["interface"]["digest"],
            serde_json::Value::Null
        );
        assert!(snapshot
            .entry_program()
            .functions
            .iter()
            .any(|f| f.id.as_str() == "std.int.decimal.add"));
        assert_eq!(
            snapshot
                .execute_entry(&ProjectExecutionOptions::default())
                .unwrap_err()[0]
                .code,
            "SPX-F102"
        );
        assert_eq!(snapshot.test_wasm_module().unwrap_err()[0].code, "SPX-W120");
        assert_eq!(
            snapshot.build_npm_inline(1_000_000).unwrap_err()[0].code,
            "SPX-W120"
        );
        snapshot.build_native(&binary)
    })
    .unwrap();
    // A standalone source control exercises the same argv/file/status adapter,
    // without copying any library implementation into either application.
    let control_source = APP.lines().filter(|line| !line.starts_with("use function")).collect::<Vec<_>>().join("\n")
        .replace("        let value = canonicalize(input);\n        let sum = add(value, \"1\");\n        let divisor = \"3\";\n        let output = divide(sum, string_as_str(divisor));", "        let output = input;");
    let control = semaprax::parse(&control_source, fixture.0.join("control.spx")).unwrap();
    let c = semaprax::codegen::emit_c_with_source_command(&control).unwrap();
    let control_binary = fixture.0.join("control");
    semaprax::codegen::compile_native_executable(&c, &control_binary).unwrap();
    std::fs::write(fixture.0.join("digits"), format!("000{}", "9".repeat(24))).unwrap();
    let output = Command::new(&binary)
        .current_dir(&fixture.0)
        .arg("digits")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"333333333333333333333333");
    assert!(output.stderr.is_empty());
    let usage = Command::new(&binary)
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stdout.is_empty());
    assert_eq!(usage.stderr, b"usage\n");
    let control_usage = Command::new(&control_binary)
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert_eq!(usage.status.code(), control_usage.status.code());
    assert_eq!(usage.stdout, control_usage.stdout);
    assert_eq!(usage.stderr, control_usage.stderr);
    for path in ["../digits", "/etc/passwd", "missing"] {
        let output = Command::new(&binary)
            .current_dir(&fixture.0)
            .arg(path)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        let control = Command::new(&control_binary)
            .current_dir(&fixture.0)
            .arg(path)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), control.status.code());
        assert_eq!(output.stdout, control.stdout);
        assert_eq!(output.stderr, control.stderr);
    }
    std::fs::write(fixture.0.join("bad"), "1x").unwrap();
    let output = Command::new(&binary)
        .current_dir(&fixture.0)
        .arg("bad")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostic.starts_with("SEMAPRAX contract failure\n  contract: requires "));
    assert!(diagnostic.contains(" in std.int.decimal.canonicalize\n"));
    assert!(diagnostic.ends_with("  arguments: text = <data>\n"));
    assert_eq!(diagnostic.lines().count(), 3);
}

#[test]
fn source_command_invalid_entry_authority_and_drift_leave_source_unchanged() {
    let wrong_entry = Fixture::new(
        APP,
        &MANIFEST.replace(
            "function = \"decimal.command.main\"",
            "function = \"std.int.decimal.add\"",
        ),
    );
    let result =
        project::with_authenticated_project(&wrong_entry.0.join("semaprax.toml"), |snapshot| {
            snapshot.check()
        });
    assert!(result.unwrap_err().iter().any(|e| e.code == "SPX-J130"));
    let authority = Fixture::new(APP, &MANIFEST.replace("\"fs.read\", ", ""));
    let result =
        project::with_authenticated_project(&authority.0.join("semaprax.toml"), |snapshot| {
            snapshot.check()
        });
    assert!(result.unwrap_err().iter().any(|e| e.code == "SPX-J131"));
    let stale = Fixture::new(APP, MANIFEST);
    let path = stale.0.join("app.spx");
    let original = std::fs::read(&path).unwrap();
    let result = project::with_authenticated_project(&stale.0.join("semaprax.toml"), |snapshot| {
        let changed = APP.replace("        2\n", "        3\n");
        let program = semaprax::parse(&changed, &path).unwrap();
        let changed = semaprax::format::canonical(&program);
        std::fs::write(&path, &changed).unwrap();
        assert!(snapshot
            .build_native(&stale.0.join("stale-command"))
            .is_err());
        assert!(!stale.0.join("stale-command").exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), changed);
        Ok(())
    });
    assert!(
        result.is_err(),
        "held-input drift must fail the session recheck"
    );
    assert_ne!(std::fs::read(path).unwrap(), original);
}
