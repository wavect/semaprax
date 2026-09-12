//! The cross-language Agent benchmark laboratory (issue #211).
//!
//! `benchmarks/cross-language-v1/` owns `run.py`, `tasks.json`,
//! `adapters.json`, and per-task `public/`/`hidden/` source trees. This
//! module pins the harness's own behavior with deterministic, synthetic
//! adapters (never a real language toolchain, so these cases run anywhere
//! `python3` runs) and separately pins the committed pilot task's inventory
//! shape. It does not exercise a real language toolchain end to end, and it
//! never measures or asserts a wall-clock time: `benchmark.cross_language.v1`
//! has no timing field to assert about, by design (see
//! `benchmarks/cross-language-v1/docs/METHODOLOGY.md`).
//!
//! `python3` is assumed present, exactly as
//! `tests/documentation/performance_benchmark_suite.rs` assumes it.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const SUITE: &str = "benchmarks/cross-language-v1";
static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "spx-cross-language-benchmark-suite-{}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::SeqCst),
        name
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    // Project loading elsewhere in this repository rejects a symlinked
    // ancestor, and the platform temporary directory is one on macOS; the
    // harness itself does not load a project here, but canonicalizing keeps
    // every path comparison in these cases exact.
    std::fs::canonicalize(&directory).unwrap()
}

fn runner() -> Command {
    let mut command = Command::new("python3");
    command.arg(root().join(SUITE).join("run.py"));
    command
}

fn document(output: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(output).unwrap()).unwrap()
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_string(value).unwrap()).unwrap();
}

fn result_for<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no result for {id}: {document}"))
}

// ---------------------------------------------------------------------------
// Deterministic mock adapters: a "mock agent" here is a trivial Python
// script the harness runs as if it were a language's official toolchain, so
// these cases pin the harness's own logic without needing a real rustc,
// tsc, or semaprax build.
// ---------------------------------------------------------------------------

/// One mock language: `exit_code` is what its `prog.py` (used for both the
/// public and hidden phase, unless `hidden_exit_code` overrides it) returns.
struct MockLanguage {
    id: &'static str,
    exit_code: i32,
    hidden_exit_code: Option<i32>,
}

fn write_mock_language(task_dir: &Path, language: &MockLanguage) {
    let public = task_dir.join("public").join(language.id);
    let hidden = task_dir.join("hidden").join(language.id);
    std::fs::create_dir_all(&public).unwrap();
    std::fs::create_dir_all(&hidden).unwrap();
    std::fs::write(
        public.join("prog.py"),
        format!("import sys\nsys.exit({})\n", language.exit_code),
    )
    .unwrap();
    std::fs::write(
        hidden.join("prog.py"),
        format!(
            "import sys\nsys.exit({})\n",
            language.hidden_exit_code.unwrap_or(language.exit_code)
        ),
    )
    .unwrap();
}

fn mock_adapters_document(languages: &[&str]) -> Value {
    let adapters: Vec<Value> = languages
        .iter()
        .map(|id| {
            serde_json::json!({
                "id": id,
                "language": id,
                "implemented": true,
                "version_command": ["python3", "--version"],
                "run_command": ["python3", "prog.py"],
                "success": {"kind": "exit_code_zero"}
            })
        })
        .collect();
    serde_json::json!({"schema": "benchmark.cross_language.adapters.v1", "adapters": adapters})
}

fn mock_tasks_document(task_id: &str, languages: &[&str]) -> Value {
    let mut map = serde_json::Map::new();
    for id in languages {
        map.insert(
            (*id).to_owned(),
            serde_json::json!({
                "public": format!("task/public/{id}"),
                "hidden": format!("task/hidden/{id}"),
            }),
        );
    }
    serde_json::json!({
        "schema": "benchmark.cross_language.tasks.v1",
        "tasks": [{
            "id": task_id,
            "category": "greenfield",
            "summary": "deterministic mock adapters for the harness self-test",
            "languages": map,
        }]
    })
}

#[test]
fn deterministic_mock_adapters_produce_known_ok_failed_and_blocked_artifacts() {
    let directory = scratch("mock-ok-fail-blocked");
    let task_dir = directory.join("task");
    write_mock_language(
        &task_dir,
        &MockLanguage {
            id: "mockok",
            exit_code: 0,
            hidden_exit_code: None,
        },
    );
    write_mock_language(
        &task_dir,
        &MockLanguage {
            id: "mockfail",
            exit_code: 0,
            hidden_exit_code: Some(1),
        },
    );

    let mut adapters = mock_adapters_document(&["mockok", "mockfail"]);
    adapters["adapters"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "mockblocked",
            "language": "MockBlocked",
            "implemented": false,
            "blocked_reason": "deliberately unwired for this self-test"
        }));
    let tasks = mock_tasks_document("mock-task", &["mockok", "mockfail"]);

    let adapters_path = directory.join("adapters.json");
    let tasks_path = directory.join("tasks.json");
    write_json(&adapters_path, &adapters);
    write_json(&tasks_path, &tasks);

    let output = directory.join("result.json");
    let result = runner()
        .arg("--root")
        .arg(&directory)
        .arg("--tasks")
        .arg(&tasks_path)
        .arg("--adapters")
        .arg(&adapters_path)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(1),
        "a failing pair must fail the run: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let document = document(&output);

    let ok = result_for(&document, "mock-task::mockok");
    assert_eq!(ok["status"], "ok");
    assert_eq!(ok["leak_check"], "ok");
    assert!(ok["provenance"]["digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    let failed = result_for(&document, "mock-task::mockfail");
    assert_eq!(failed["status"], "failed");
    assert!(failed["reason"].as_str().unwrap().contains("hidden"));
    // A failure that fails at the hidden phase still passed its public phase;
    // both are recorded so a reader can see exactly where it broke.
    assert_eq!(failed["public"]["passed"], true);
    assert_eq!(failed["hidden"]["passed"], false);

    let blocked = result_for(&document, "mock-task::mockblocked");
    assert_eq!(blocked["status"], "blocked");
    assert_eq!(
        blocked["reason"], "deliberately unwired for this self-test",
        "a blocked adapter's declared reason must be reported verbatim"
    );
    assert!(
        blocked.get("public").is_none() && blocked.get("hidden").is_none(),
        "a blocked pair must never carry a pass/fail artifact: {blocked}"
    );

    let summary = &document["summary"];
    assert_eq!(summary["ok"], 1);
    assert_eq!(summary["failed"], 1);
    assert_eq!(summary["blocked"], 1);
    assert_eq!(summary["drifted"], 0);
}

#[test]
fn a_failing_pair_can_never_be_scored_as_an_improvement() {
    let directory = scratch("compare-incomparable");
    let task_dir = directory.join("task");
    write_mock_language(
        &task_dir,
        &MockLanguage {
            id: "mockok",
            exit_code: 0,
            hidden_exit_code: None,
        },
    );
    let adapters = mock_adapters_document(&["mockok"]);
    let tasks = mock_tasks_document("mock-task", &["mockok"]);
    let adapters_path = directory.join("adapters.json");
    let tasks_path = directory.join("tasks.json");
    write_json(&adapters_path, &adapters);
    write_json(&tasks_path, &tasks);

    let baseline_output = directory.join("baseline.json");
    let baseline_run = runner()
        .arg("--root")
        .arg(&directory)
        .arg("--tasks")
        .arg(&tasks_path)
        .arg("--adapters")
        .arg(&adapters_path)
        .arg("--output")
        .arg(&baseline_output)
        .output()
        .unwrap();
    assert!(baseline_run.status.success());
    assert_eq!(
        result_for(&document(&baseline_output), "mock-task::mockok")["status"],
        "ok"
    );

    // Now the same pair regresses: its hidden phase starts failing.
    std::fs::write(
        task_dir.join("hidden/mockok/prog.py"),
        "import sys\nsys.exit(1)\n",
    )
    .unwrap();
    let local_output = directory.join("local.json");
    let local_run = runner()
        .arg("--root")
        .arg(&directory)
        .arg("--tasks")
        .arg(&tasks_path)
        .arg("--adapters")
        .arg(&adapters_path)
        .arg("--output")
        .arg(&local_output)
        .arg("--compare")
        .arg(&baseline_output)
        .output()
        .unwrap();
    assert_eq!(local_run.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&local_run.stdout);
    let line = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("mock-task::mockok:"))
        .unwrap_or_else(|| panic!("no comparison line for mock-task::mockok:\n{stdout}"));
    assert!(
        line.contains("incomparable"),
        "a regression to failure must not be scored as one: {line}"
    );
    assert!(
        !line.contains("regression") && !line.contains("improvement"),
        "a failing pair carries no verdict beyond incomparable: {line}"
    );
}

#[test]
fn a_mismatched_expected_digest_fails_closed_without_running_anything() {
    let directory = scratch("drift");
    let task_dir = directory.join("task");
    write_mock_language(
        &task_dir,
        &MockLanguage {
            id: "mockok",
            exit_code: 0,
            hidden_exit_code: None,
        },
    );
    let adapters = mock_adapters_document(&["mockok"]);
    let mut tasks = mock_tasks_document("mock-task", &["mockok"]);
    tasks["tasks"][0]["languages"]["mockok"]["expected_digest"] =
        Value::from("sha256:0000000000000000000000000000000000000000000000000000000000000000");
    let adapters_path = directory.join("adapters.json");
    let tasks_path = directory.join("tasks.json");
    write_json(&adapters_path, &adapters);
    write_json(&tasks_path, &tasks);

    let output = directory.join("result.json");
    let result = runner()
        .arg("--root")
        .arg(&directory)
        .arg("--tasks")
        .arg(&tasks_path)
        .arg("--adapters")
        .arg(&adapters_path)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    let output_document = document(&output);
    let record = result_for(&output_document, "mock-task::mockok");
    assert_eq!(record["status"], "drifted");
    assert!(
        record.get("public").is_none() && record.get("hidden").is_none(),
        "a drifted pair must never run a build or test step: {record}"
    );
}

#[test]
fn a_hidden_only_file_is_absent_from_the_public_build_tree() {
    // The strongest available proof that "hidden" means hidden: a public
    // program that tries to read a file which exists only in `hidden/` must
    // fail to find it, because the harness never copies `hidden/` into the
    // scratch directory the public phase runs in.
    let directory = scratch("hidden-isolation");
    let task_dir = directory.join("task");
    let public = task_dir.join("public/mockreader");
    let hidden = task_dir.join("hidden/mockreader");
    std::fs::create_dir_all(&public).unwrap();
    std::fs::create_dir_all(&hidden).unwrap();
    std::fs::write(
        public.join("prog.py"),
        "import os, sys\nsys.exit(0 if os.path.exists('secret.txt') else 7)\n",
    )
    .unwrap();
    std::fs::write(
        hidden.join("prog.py"),
        "import os, sys\nsys.exit(0 if os.path.exists('secret.txt') else 7)\n",
    )
    .unwrap();
    std::fs::write(
        hidden.join("secret.txt"),
        "only the hidden phase may see this\n",
    )
    .unwrap();

    let adapters = mock_adapters_document(&["mockreader"]);
    let tasks = mock_tasks_document("mock-task", &["mockreader"]);
    let adapters_path = directory.join("adapters.json");
    let tasks_path = directory.join("tasks.json");
    write_json(&adapters_path, &adapters);
    write_json(&tasks_path, &tasks);

    let output = directory.join("result.json");
    let result = runner()
        .arg("--root")
        .arg(&directory)
        .arg("--tasks")
        .arg(&tasks_path)
        .arg("--adapters")
        .arg(&adapters_path)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    let document = document(&output);
    let record = result_for(&document, "mock-task::mockreader");
    // The public phase cannot see `secret.txt` and exits 7; the hidden phase
    // (which overlays `hidden/` on top) does see it and exits 0.
    assert_eq!(record["public"]["passed"], false);
    assert_eq!(record["status"], "failed");
    assert_eq!(record["leak_check"], "ok");
    assert_eq!(result.status.code(), Some(1));
}

// ---------------------------------------------------------------------------
// The committed inventory and harness startup.
// ---------------------------------------------------------------------------

#[test]
fn python_entry_point_resolves_the_committed_suite_from_any_working_directory() {
    for working_directory in [root(), scratch("cwd")] {
        let output = scratch("plan-output").join("plan.json");
        let result = Command::new("python3")
            .arg(root().join(SUITE).join("run.py"))
            .arg("--dry-run")
            .arg("--output")
            .arg(&output)
            .current_dir(&working_directory)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "run.py failed from {}: {}",
            working_directory.display(),
            String::from_utf8_lossy(&result.stderr)
        );
        let document = document(&output);
        assert_eq!(document["schema"], "benchmark.cross_language.plan.v1");
        let pairs = document["pairs"].as_array().unwrap();
        assert!(
            pairs.len() >= 9,
            "the committed adapter roster names at least 9 languages: {}",
            pairs.len()
        );
        let implemented: Vec<&str> = pairs
            .iter()
            .filter(|row| row["implemented"] == true)
            .map(|row| row["language"].as_str().unwrap())
            .collect();
        for language in ["semaprax", "rust", "typescript"] {
            assert!(
                implemented.contains(&language),
                "{language} must be a wired adapter: {implemented:?}"
            );
        }
        for row in pairs {
            if row["implemented"] == true {
                assert_eq!(row["exists"], true, "{row} names a missing task directory");
            }
        }
    }
}

#[test]
fn a_missing_task_inventory_fails_concisely() {
    let elsewhere = scratch("no-inventory");
    std::fs::copy(root().join(SUITE).join("run.py"), elsewhere.join("run.py")).unwrap();
    let result = Command::new("python3")
        .arg(elsewhere.join("run.py"))
        .arg("--dry-run")
        .arg("--output")
        .arg(elsewhere.join("plan.json"))
        .current_dir(root())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("task inventory not found"), "{stderr}");
    assert!(!stderr.contains("Traceback"), "{stderr}");
}

#[test]
fn every_committed_task_language_directory_exists_and_hidden_adds_something() {
    let inventory: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join(SUITE).join("tasks.json")).unwrap(),
    )
    .unwrap();
    let tasks = inventory["tasks"].as_array().unwrap();
    assert!(!tasks.is_empty());
    for task in tasks {
        let languages = task["languages"].as_object().unwrap();
        assert!(!languages.is_empty(), "{} declares no language", task["id"]);
        for (language, paths) in languages {
            let public = root().join(paths["public"].as_str().unwrap());
            let hidden = root().join(paths["hidden"].as_str().unwrap());
            assert!(
                public.is_dir(),
                "{}::{language} names a missing public dir",
                task["id"]
            );
            assert!(
                hidden.is_dir(),
                "{}::{language} names a missing hidden dir",
                task["id"]
            );

            let public_files = collect_relative_files(&public);
            let hidden_files = collect_relative_files(&hidden);
            assert!(
                !hidden_files.is_empty(),
                "{}::{language} hidden dir is empty",
                task["id"]
            );
            // The overlay must change something: either it adds a relative
            // path the public tree lacks, or it replaces a shared path with
            // different bytes (this task's overlay style: the same source
            // file, extended with hidden vectors). An overlay that is
            // byte-identical to the public tree everywhere tests nothing the
            // public tests do not already cover.
            let hidden_only = hidden_files.iter().any(|f| !public_files.contains(f));
            let differs_somewhere = hidden_files.iter().any(|relative| {
                public_files.contains(relative)
                    && std::fs::read(hidden.join(relative)).unwrap()
                        != std::fs::read(public.join(relative)).unwrap()
            });
            assert!(
                hidden_only || differs_somewhere,
                "{}::{language}'s hidden overlay is byte-identical to its public tree \
                 wherever they share a path, and adds no new path; it cannot be testing \
                 anything the public tests do not already cover",
                task["id"]
            );
        }
    }
}

fn collect_relative_files(directory: &Path) -> std::collections::BTreeSet<String> {
    let mut files = std::collections::BTreeSet::new();
    fn walk(base: &Path, current: &Path, files: &mut std::collections::BTreeSet<String>) {
        for entry in std::fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    walk(directory, directory, &mut files);
    files
}

#[test]
fn declared_adapters_pin_exact_tool_invocations_with_no_mutable_selectors() {
    // Issue #211 rules out "using mutable `latest` dependencies or model
    // aliases" as a matter of scope, not merely of taste. This pins that as
    // a regression guard over the committed adapter roster.
    let source = std::fs::read_to_string(root().join(SUITE).join("adapters.json")).unwrap();
    for forbidden in ["latest", "@main", "@master", "HEAD"] {
        assert!(
            !source.contains(forbidden),
            "adapters.json must not reference a mutable selector `{forbidden}`"
        );
    }
    let document: Value = serde_json::from_str(&source).unwrap();
    for adapter in document["adapters"].as_array().unwrap() {
        if adapter["implemented"] == true {
            assert!(
                adapter.get("version_command").is_some(),
                "{}: an implemented adapter must declare a version probe",
                adapter["id"]
            );
        } else {
            assert!(
                adapter.get("blocked_reason").is_some(),
                "{}: an unimplemented adapter must state why",
                adapter["id"]
            );
        }
    }
}

#[test]
fn the_result_schema_carries_no_timing_field() {
    // Pinned at the schema level so a future edit cannot silently reintroduce
    // a wall-clock number on this contended host. See
    // `benchmarks/cross-language-v1/docs/METHODOLOGY.md`'s "No timing"
    // section for why this is a deliberate absence, not a gap.
    let directory = scratch("no-timing");
    let task_dir = directory.join("task");
    write_mock_language(
        &task_dir,
        &MockLanguage {
            id: "mockok",
            exit_code: 0,
            hidden_exit_code: None,
        },
    );
    let adapters = mock_adapters_document(&["mockok"]);
    let tasks = mock_tasks_document("mock-task", &["mockok"]);
    let adapters_path = directory.join("adapters.json");
    let tasks_path = directory.join("tasks.json");
    write_json(&adapters_path, &adapters);
    write_json(&tasks_path, &tasks);

    let output = directory.join("result.json");
    let result = runner()
        .arg("--root")
        .arg(&directory)
        .arg("--tasks")
        .arg(&tasks_path)
        .arg("--adapters")
        .arg(&adapters_path)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert!(result.status.success());

    let raw = std::fs::read_to_string(&output).unwrap();
    for forbidden in ["wall_ms", "duration_ms", "elapsed_ms", "\"timing_ms\""] {
        assert!(
            !raw.contains(forbidden),
            "result document must carry no {forbidden} field: {raw}"
        );
    }
    let document = document(&output);
    assert_eq!(document["timing"]["collected"], false);
    assert!(document["timing"]["reason"]
        .as_str()
        .unwrap()
        .contains("contention"));
}
