//! Bounded CLI execution of the manifest-declared SourceCommand test closure.
use super::*;

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

struct NativeFixture {
    root: PathBuf,
    scratch: PathBuf,
}

impl NativeFixture {
    fn new(label: &str, tests: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-native-test-cli-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let scratch = root.join("scratch");
        std::fs::create_dir(&scratch).unwrap();
        std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
        for (name, source) in [("app.spx", APP), ("tests.spx", tests)] {
            let path = root.join(name);
            let program = semaprax::parse(source, &path).unwrap();
            std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
        }
        Self { root, scratch }
    }

    fn cli(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(arguments)
            .current_dir(&self.root)
            .env("TMPDIR", &self.scratch)
            .output()
            .unwrap()
    }

    fn scratch_empty(&self) {
        let native = std::fs::read_dir(&self.scratch)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().starts_with(".semaprax-native-"))
            .collect::<Vec<_>>();
        assert!(native.is_empty(), "native test scratch leaked: {native:?}");
    }
}

impl Drop for NativeFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON: {error}; stdout={} stderr={}",
            stdout(output),
            stderr(output)
        )
    })
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn native_cli_selects_main_and_named_cases_and_reports_untruncated_result() {
    let tests = r#"module decimal.tests;
@id("decimal.tests.main") fn main() -> i64 { 0 }
@id("decimal.tests.test_pass") fn test_pass() -> i64 { 0 }
@id("decimal.tests.test_fail") fn test_fail() -> i64 { 256 }
@id("decimal.tests.test_helper") fn test_helper(value: i64) -> i64 { value }
"#;
    let fixture = NativeFixture::new("cases", tests);
    let default = fixture.cli(&["test", "--json"]);
    assert_eq!(default.status.code(), Some(1));
    assert!(
        stdout(&default).contains("SPX-F102"),
        "{}",
        stdout(&default)
    );
    let output = fixture.cli(&["test", "--target", "native", "--json"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let envelope = json(&output);
    assert_eq!(envelope["schema"], "semaprax.native-test.v1");
    assert_eq!(envelope["target"], "native");
    assert_eq!(envelope["passed"], false);
    let cases = envelope["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 3);
    assert_eq!(
        cases
            .iter()
            .map(|case| case["stable_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "decimal.tests.main",
            "decimal.tests.test_pass",
            "decimal.tests.test_fail"
        ]
    );
    assert_eq!(
        cases
            .iter()
            .map(|case| case["passed"].as_bool().unwrap())
            .collect::<Vec<_>>(),
        [true, true, false]
    );
    assert_eq!(cases[2]["result"], 256);
    fixture.scratch_empty();
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn native_cli_runs_with_zero_arguments_and_project_relative_file_access() {
    let tests = r#"module decimal.tests;
permit { fs.read, process.args.read }
@id("decimal.tests.main")
fn main() -> i64 uses { process.args.read } { if args_len() == 0usize { 0 } else { 1 } }
@id("decimal.tests.test_file")
fn test_file() -> i64 uses { fs.read }
{
    let path = "digits";
    let view = string_as_str(path);
    let contents = file_read_text(view);
    if string_len(contents) == 4i64 { 0 } else { 1 }
}
"#;
    let fixture = NativeFixture::new("io", tests);
    std::fs::write(fixture.root.join("digits"), "1234").unwrap();
    let output = fixture.cli(&["test", "--target", "native", "--json"]);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        stdout(&output),
        stderr(&output)
    );
    let envelope = json(&output);
    assert_eq!(envelope["passed"], true);
    assert_eq!(envelope["cases"].as_array().unwrap().len(), 2);
    assert!(envelope["cases"]
        .as_array()
        .unwrap()
        .iter()
        .all(|case| case["exit_code"] == 0));
    fixture.scratch_empty();
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn native_cli_times_out_and_reaps_infinite_test() {
    let tests = r#"module decimal.tests;
@id("decimal.tests.main")
fn main() -> i64 { while true { 0 } 0 }
"#;
    let fixture = NativeFixture::new("timeout", tests);
    let output = fixture.cli(&[
        "test",
        "--target",
        "native",
        "--native-timeout-ms",
        "25",
        "--json",
    ]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout={} stderr={}",
        stdout(&output),
        stderr(&output)
    );
    let envelope = json(&output);
    assert_eq!(envelope["passed"], false);
    assert!(envelope["cases"][0]["outcome"]
        .as_str()
        .unwrap()
        .contains("exceeded 25 ms"));
    fixture.scratch_empty();
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn native_cli_rejects_combined_output_over_limit_and_removes_binary() {
    let tests = r#"module decimal.tests;
permit { process.stdout.write, process.stderr.write }
@id("decimal.tests.main")
fn main() -> i64 uses { process.stdout.write, process.stderr.write }
{
    let text = "abcdefgh";
    let view = string_as_str(text);
    let out = stdout_write(str_as_bytes(view));
    let err = stderr_write(str_as_bytes(view));
    0
}
"#;
    let fixture = NativeFixture::new("output", tests);
    let output = fixture.cli(&[
        "test",
        "--target",
        "native",
        "--native-max-output-bytes",
        "12",
        "--json",
    ]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout={} stderr={}",
        stdout(&output),
        stderr(&output)
    );
    let envelope = json(&output);
    assert!(envelope["cases"][0]["outcome"]
        .as_str()
        .unwrap()
        .contains("exceeded 12 output bytes"));
    fixture.scratch_empty();
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn native_cli_stops_large_output_before_timeout() {
    let tests = r#"module decimal.tests;
permit { process.stderr.write, process.stdout.write }
@id("decimal.tests.main")
fn main() -> i64 uses { process.stdout.write }
{
    let text = "abcdefgh";
    let view = string_as_str(text);
    let bytes = str_as_bytes(view);
    let mut index = 0usize;
    while index < 20000usize {
        let written = stdout_append(bytes);
        index = index + 1usize;
        index < 20000usize
    }
    0
}
"#;
    let fixture = NativeFixture::new("large-output", tests);
    std::fs::write(
        fixture.root.join("semaprax.toml"),
        MANIFEST.replace("source-command.v1", "source-command.resource-output.v1"),
    )
    .unwrap();
    let output = fixture.cli(&[
        "test",
        "--target",
        "native",
        "--native-timeout-ms",
        "2000",
        "--native-max-output-bytes",
        "1024",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let envelope = json(&output);
    assert!(envelope["cases"][0]["outcome"]
        .as_str()
        .unwrap()
        .contains("exceeded 1024 output bytes"));
    fixture.scratch_empty();
}
