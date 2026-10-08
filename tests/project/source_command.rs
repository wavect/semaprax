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
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "semaprax-source-command-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
        for (path, source) in [
            ("app.spx", app),
            (
                "tests.spx",
                "module decimal.tests;\n@id(\"decimal.tests.main\")\nfn main() -> i64 { 0 }\n",
            ),
        ] {
            let path = root.join(path);
            let program = semaprax::parse(source, &path).unwrap();
            std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
        }
        Self(root)
    }
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
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn source_command_bundled_decimal_native_and_closed_runtime_failures() {
    let fixture = Fixture::new(APP, MANIFEST);
    let binary = fixture.0.join("command");
    project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let graph: serde_json::Value = serde_json::from_str(snapshot.semantic_graph()).unwrap();
        assert_eq!(graph["project_schema"], "semaprax.project.v26");
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
