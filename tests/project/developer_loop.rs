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
    assert_eq!(stderr(&test), "project tests failed with result 7\n");

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
