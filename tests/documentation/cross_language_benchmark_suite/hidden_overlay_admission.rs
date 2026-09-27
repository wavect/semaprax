//! Runtime admission regressions for missing, empty, and ineffective overlays.
//! The fixture uses the harness's deterministic Python mock adapter and does
//! not require a benchmark language toolchain at runtime. Compiling this Rust
//! test harness still requires Cargo and rustc.

use super::{
    document, mock_adapters_document, mock_tasks_document, result_for, runner, scratch, write_json,
};
use serde_json::Value;

fn run_case(
    name: &str,
    hidden_setup: impl FnOnce(&std::path::Path, &std::path::Path),
) -> (std::process::Output, Value) {
    let directory = scratch(name);
    let task_dir = directory.join("task");
    hidden_setup(&task_dir.join("public/mock"), &task_dir.join("hidden/mock"));
    let tasks = directory.join("tasks.json");
    let adapters = directory.join("adapters.json");
    write_json(&tasks, &mock_tasks_document("overlay-task", &["mock"]));
    write_json(&adapters, &mock_adapters_document(&["mock"]));
    let output = directory.join("result.json");
    let result = runner()
        .arg("--root")
        .arg(&directory)
        .arg("--tasks")
        .arg(&tasks)
        .arg("--adapters")
        .arg(&adapters)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    (result, document(&output))
}

fn write_public(public: &std::path::Path) {
    std::fs::create_dir_all(public).unwrap();
    std::fs::write(public.join("prog.py"), "import sys\nsys.exit(0)\n").unwrap();
}

#[test]
fn missing_hidden_overlay_fails_before_a_mock_run() {
    let (result, document) = run_case("hidden-missing", |public, _hidden| write_public(public));
    assert_eq!(result.status.code(), Some(1));
    let row = result_for(&document, "overlay-task::mock");
    assert_eq!(row["status"], "failed");
    assert!(row["reason"]
        .as_str()
        .unwrap()
        .starts_with("missing hidden directory:"));
}

#[test]
fn empty_and_noop_hidden_overlays_fail_closed() {
    let (result, document) = run_case("hidden-empty", |public, hidden| {
        write_public(public);
        std::fs::create_dir_all(hidden).unwrap();
    });
    assert_eq!(result.status.code(), Some(1));
    assert!(result_for(&document, "overlay-task::mock")["reason"]
        .as_str()
        .unwrap()
        .starts_with("empty hidden overlay:"));

    let (result, document) = run_case("hidden-noop", |public, hidden| {
        write_public(public);
        std::fs::create_dir_all(hidden).unwrap();
        std::fs::copy(public.join("prog.py"), hidden.join("prog.py")).unwrap();
    });
    assert_eq!(result.status.code(), Some(1));
    assert!(result_for(&document, "overlay-task::mock")["reason"]
        .as_str()
        .unwrap()
        .starts_with("hidden overlay adds no changes:"));
}

#[test]
fn changed_same_path_overlay_still_runs_both_phases() {
    let (result, document) = run_case("hidden-changed", |public, hidden| {
        write_public(public);
        std::fs::create_dir_all(hidden).unwrap();
        std::fs::write(
            hidden.join("prog.py"),
            "# changed\nimport sys\nsys.exit(0)\n",
        )
        .unwrap();
    });
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let row = result_for(&document, "overlay-task::mock");
    assert_eq!(row["status"], "ok");
    assert_eq!(row["public"]["passed"], true);
    assert_eq!(row["hidden"]["passed"], true);
}
