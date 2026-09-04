use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const PROJECT_FILES: &[&str] = &[
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];

struct Fixture {
    root: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture(label: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-developer-loop-v1-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
    for file in PROJECT_FILES {
        std::fs::copy(source.join(file), root.join(file)).unwrap();
    }
    Fixture {
        root: root.canonicalize().unwrap(),
    }
}

fn cli(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .current_dir(root)
        .output()
        .unwrap()
}

fn inventory(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, output: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if entry.file_type().unwrap().is_dir() {
                visit(root, &entry.path(), output);
            } else {
                output.insert(
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    std::fs::read(entry.path()).unwrap(),
                );
            }
        }
    }

    let mut output = BTreeMap::new();
    visit(root, root, &mut output);
    output
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn replace_exactly_once(source: &str, needle: &str, replacement: &str) -> String {
    assert_eq!(
        source.matches(needle).count(),
        1,
        "fixture mutation needle must match exactly once: {needle}"
    );
    source.replacen(needle, replacement, 1)
}

fn replay_payload_digest(envelope: &str) -> String {
    let marker = ",\"payload_digest\":";
    let offset = envelope.rfind(marker).unwrap();
    let payload = format!("{}}}", &envelope[..offset]);
    let mut digest = Sha256::new();
    digest.update(b"semaprax.project-execution.payload.v1\0");
    digest.update((payload.len() as u64).to_le_bytes());
    digest.update(payload.as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(digest.finalize())
    )
}

#[test]
fn project_run_and_test_use_the_authenticated_in_process_closures_without_writes() {
    let fixture = fixture("success");
    let before = inventory(&fixture.root);

    let run = cli(&fixture.root, &["run"]);
    assert!(run.status.success(), "run failed: {}", stderr(&run));
    assert_eq!(stdout(&run), "42\n");
    assert!(run.stderr.is_empty());

    let manifest = fixture.root.join("semaprax.toml");
    let explicit = cli(
        &fixture.root,
        &["run", "--manifest-path", manifest.to_str().unwrap()],
    );
    assert!(
        explicit.status.success(),
        "explicit run failed: {}",
        stderr(&explicit)
    );
    assert_eq!(stdout(&explicit), "42\n");

    let test = cli(&fixture.root, &["test", "semaprax.toml"]);
    assert!(test.status.success(), "test failed: {}", stderr(&test));
    assert_eq!(stdout(&test), "project tests passed\n");
    assert!(test.stderr.is_empty());

    assert_eq!(inventory(&fixture.root), before);
}

#[test]
fn manifest_test_result_controls_test_status_without_affecting_entry_run() {
    let fixture = fixture("failure");
    let tests = fixture.root.join("src/tests.spx");
    let source = std::fs::read_to_string(&tests).unwrap();
    std::fs::write(
        &tests,
        replace_exactly_once(
            &source,
            "if add(19, 23) == 42 && subtract(23, 19) == 4 && multiply(6, 7) == 42 && divide(84, 2) == 42 && is_negative(-1) && not(false) { 0 } else { 1 }",
            "7",
        ),
    )
    .unwrap();
    let before = inventory(&fixture.root);

    let test = cli(&fixture.root, &["test"]);
    assert_eq!(test.status.code(), Some(1));
    assert!(stdout(&test).is_empty());
    assert_eq!(
        stderr(&test),
        "failed calculator.tests.main: returned 7\n\
         project tests failed: 1 of 1 in calculator.tests\n\
         \x20 help: a test passes by returning 0; a nonzero return is the failing check's code or count, \
         so give each check its own `fn test_<name>() -> i64` in the test module to have it reported by name\n"
    );

    let run = cli(&fixture.root, &["run"]);
    assert!(run.status.success(), "entry run failed: {}", stderr(&run));
    assert_eq!(stdout(&run), "42\n");
    assert_eq!(inventory(&fixture.root), before);
}

#[test]
fn canonical_json_binds_the_authenticated_subject_and_distinct_outcomes() {
    let fixture = fixture("json");
    let first = cli(&fixture.root, &["run", "--json"]);
    let second = cli(&fixture.root, &["run", "--json"]);
    assert!(
        first.status.success(),
        "JSON run failed: {}",
        stderr(&first)
    );
    assert_eq!(first.stdout, second.stdout);
    let run: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(run["schema"], "semaprax.project-execution.v1");
    assert_eq!(run["project_schema"], "semaprax.project.v1");
    assert_eq!(run["project"], "calculator");
    assert_eq!(run["role"], "entry");
    assert_eq!(run["module"], "calculator.app");
    assert_eq!(run["stable_id"], "calculator.app.main");
    assert_eq!(run["outcome"]["kind"], "returned");
    assert_eq!(run["outcome"]["value"], "42");
    assert!(run["project_revision"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert!(run["workspace_revision"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert!(run["payload_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(
        run["payload_digest"],
        replay_payload_digest(stdout(&first).trim_end())
    );

    let exhausted = cli(&fixture.root, &["test", "--json", "--max-steps", "1"]);
    assert_eq!(exhausted.status.code(), Some(1));
    assert!(exhausted.stderr.is_empty());
    let exhausted: serde_json::Value = serde_json::from_slice(&exhausted.stdout).unwrap();
    assert_eq!(exhausted["role"], "test");
    assert_eq!(exhausted["module"], "calculator.tests");
    assert_eq!(exhausted["stable_id"], "calculator.tests.main");
    assert_eq!(exhausted["outcome"]["kind"], "fuel_exhausted");
    assert_eq!(exhausted["fuel"]["max_steps"], 1);
}

#[test]
fn language_failure_is_not_misreported_as_fuel_or_test_assertion_failure() {
    let fixture = fixture("language-failure");
    let app = fixture.root.join("src/app.spx");
    let source = std::fs::read_to_string(&app).unwrap();
    std::fs::write(
        &app,
        replace_exactly_once(
            &source,
            "add(multiply(6, 7), subtract(divide(4, 2), 2))",
            "divide(1, 0)",
        ),
    )
    .unwrap();

    let output = cli(&fixture.root, &["run", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let execution: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(execution["outcome"]["kind"], "language_failure");
    assert_eq!(
        execution["outcome"]["status"]["domain_id"],
        "semaprax.contract.v1"
    );
    assert_ne!(execution["outcome"]["kind"], "fuel_exhausted");
}

const NAMED_CASES: &str = "
@id(\"calculator.tests.test_add\")
fn test_add() -> i64
{
    if add(1, 2) == 3 { 0 } else { 1 }
}

@id(\"calculator.tests.test_subtract\")
fn test_subtract() -> i64
{
    if subtract(1, 2) == 0 { 0 } else { 2 }
}

@id(\"calculator.tests.test_divide_by_zero\")
fn test_divide_by_zero() -> i64
{
    divide(7, 0)
}

@id(\"calculator.tests.test_helper\")
fn test_helper(value: i64) -> i64
{
    value
}
";

/// Every zero-parameter `i64` function named `test_*` in the declared test
/// module runs on its own; `main` keeps deciding exactly as before. The human
/// report names each failing case with its outcome, and the JSON envelope gains
/// an additive `cases` array that replays under the same payload digest and
/// verifies through the public route.
#[test]
fn named_test_cases_are_executed_and_reported_individually() {
    let fixture = fixture("named-cases");
    let tests = fixture.root.join("src/tests.spx");
    let source = std::fs::read_to_string(&tests).unwrap();
    std::fs::write(&tests, format!("{source}{NAMED_CASES}")).unwrap();
    let before = inventory(&fixture.root);

    let test = cli(&fixture.root, &["test"]);
    assert_eq!(test.status.code(), Some(1));
    assert!(stdout(&test).is_empty());
    assert_eq!(
        stderr(&test),
        "failed calculator.tests.test_divide_by_zero: language status {\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":1,\"class\":\"contract\",\"retryable\":false}\n\
         \x20 contract: requires right != 0 in calculator.divide\n\
         \x20 arguments: left = 7, right = 0\n\
         failed calculator.tests.test_subtract: returned 2\n\
         project tests failed: 2 of 4 in calculator.tests\n\
         \x20 help: a test passes by returning 0; a nonzero return is the failing check's code or count\n"
    );

    let json = cli(&fixture.root, &["test", "--json"]);
    assert_eq!(json.status.code(), Some(1));
    assert!(json.stderr.is_empty());
    let envelope = stdout(&json);
    semaprax::project::verify_execution_envelope(envelope.trim_end()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    assert_eq!(value["schema"], "semaprax.project-execution.v1");
    assert_eq!(value["stable_id"], "calculator.tests.main");
    assert_eq!(value["outcome"]["value"], "0");
    assert_eq!(
        value["payload_digest"],
        replay_payload_digest(envelope.trim_end())
    );
    let cases = value["cases"].as_array().unwrap();
    assert_eq!(
        cases
            .iter()
            .map(|case| case["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["test_add", "test_divide_by_zero", "test_subtract"],
        "cases follow stable-identity order and `test_helper(value)` is not a case"
    );
    assert_eq!(cases[0]["stable_id"], "calculator.tests.test_add");
    assert_eq!(cases[0]["outcome"]["value"], "0");
    assert_eq!(cases[0]["fuel"]["max_steps"], value["limits"]["max_steps"]);
    assert_eq!(cases[1]["outcome"]["kind"], "language_failure");
    assert_eq!(
        cases[1]["outcome"]["failure"]["function"],
        "calculator.divide"
    );
    assert_eq!(cases[1]["outcome"]["failure"]["phase"], "requires");
    assert_eq!(cases[1]["outcome"]["failure"]["clause"], "right != 0");
    assert_eq!(
        cases[1]["outcome"]["failure"]["arguments"],
        serde_json::json!([
            {"name": "left", "type": "i64", "value": "7"},
            {"name": "right", "type": "i64", "value": "0"},
        ])
    );
    assert_eq!(cases[2]["outcome"]["value"], "2");

    assert_eq!(inventory(&fixture.root), before, "test runs write nothing");

    // Fixing the two cases passes the command and reports the case count.
    let source = std::fs::read_to_string(&tests).unwrap();
    let source = replace_exactly_once(&source, "divide(7, 0)", "divide(7, 7) - 1");
    let source = replace_exactly_once(&source, "subtract(1, 2) == 0", "subtract(1, 2) == -1");
    std::fs::write(&tests, source).unwrap();
    let before = inventory(&fixture.root);
    let passed = cli(&fixture.root, &["test"]);
    assert!(passed.status.success(), "{}", stderr(&passed));
    assert_eq!(stdout(&passed), "project tests passed (3 named cases)\n");
    let passed_json = cli(&fixture.root, &["test", "--json"]);
    assert!(passed_json.status.success());
    let value: serde_json::Value = serde_json::from_slice(&passed_json.stdout).unwrap();
    assert_eq!(value["cases"].as_array().unwrap().len(), 3);

    // A test module without named cases keeps the exact legacy envelope shape
    // apart from the always-present, empty `cases` array.
    let run = cli(&fixture.root, &["run", "--json"]);
    let run: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    assert!(run.get("cases").is_none(), "entry envelopes carry no cases");
    assert_eq!(inventory(&fixture.root), before);
}

/// A violated `requires` inside `semaprax run` names the function, the clause,
/// and the call's arguments in both projections while the status object stays
/// exactly what every backend reports.
#[test]
fn contract_failure_names_the_function_clause_and_arguments() {
    let fixture = fixture("contract-detail");
    let app = fixture.root.join("src/app.spx");
    let source = std::fs::read_to_string(&app).unwrap();
    std::fs::write(
        &app,
        replace_exactly_once(
            &source,
            "add(multiply(6, 7), subtract(divide(4, 2), 2))",
            "divide(1, 0)",
        ),
    )
    .unwrap();

    let run = cli(&fixture.root, &["run"]);
    assert_eq!(run.status.code(), Some(1));
    assert!(stdout(&run).is_empty());
    assert_eq!(
        stderr(&run),
        "project execution failed with language status {\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":1,\"class\":\"contract\",\"retryable\":false}\n\
         \x20 contract: requires right != 0 in calculator.divide\n\
         \x20 arguments: left = 1, right = 0\n"
    );

    let json = cli(&fixture.root, &["run", "--json"]);
    assert_eq!(json.status.code(), Some(1));
    assert!(json.stderr.is_empty());
    let envelope = stdout(&json);
    semaprax::project::verify_execution_envelope(envelope.trim_end()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    assert_eq!(value["outcome"]["kind"], "language_failure");
    assert_eq!(
        value["outcome"]["status"],
        serde_json::json!({
            "schema": "semaprax.status.v1",
            "domain_id": "semaprax.contract.v1",
            "code": 1,
            "class": "contract",
            "retryable": false,
        })
    );
    assert_eq!(
        value["outcome"]["failure"],
        serde_json::json!({
            "function": "calculator.divide",
            "phase": "requires",
            "clause": "right != 0",
            "arguments": [
                {"name": "left", "type": "i64", "value": "1"},
                {"name": "right", "type": "i64", "value": "0"},
            ],
        })
    );
    assert_eq!(
        value["payload_digest"],
        replay_payload_digest(envelope.trim_end())
    );
}
