//! The performance macrobenchmark suite must start from any working directory.
//!
//! `benchmarks/performance-v1/` owns `run.py`, `run.sh`, `scenarios.json` and
//! `results/`; scenario `path` entries are repository-relative. The runner once
//! resolved its inventory as `<repository>/benchmarks/benchmark/scenarios.json`
//! and the wrapper built a nonexistent script path, so both documented entry
//! points aborted before a single scenario ran. These cases pin the resolution
//! rule, the wrapper's argument quoting, and the startup failures, against the
//! committed suite layout rather than a synthetic one.
//!
//! `python3` is assumed present, exactly as `tests/ci_msrv_sharding_contract.rs`
//! assumes it for `scripts/ci-msrv.py`.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const SUITE: &str = "benchmarks/performance-v1";
static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A fresh owned directory outside the repository, so a runner invocation with
/// an unrelated working directory is actually unrelated.
fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "{}{}-{}-{}",
        "spx-performance-benchmark-suite-",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::SeqCst),
        name
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn plan(output: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(output).unwrap()).unwrap()
}

fn assert_committed_plan(document: &Value) {
    assert_eq!(
        document["root"].as_str().unwrap(),
        root().canonicalize().unwrap().to_str().unwrap(),
        "the plan must resolve the repository root"
    );
    assert_eq!(
        document["suite"].as_str().unwrap(),
        root().join(SUITE).canonicalize().unwrap().to_str().unwrap(),
        "the plan must resolve the suite directory"
    );
    let scenarios = document["scenarios"].as_array().unwrap();
    assert!(
        scenarios.len() >= 20,
        "the committed inventory has {} scenarios",
        scenarios.len()
    );
    for scenario in scenarios {
        assert!(
            scenario["exists"].as_bool().unwrap(),
            "{} resolves to a missing path {}",
            scenario["id"].as_str().unwrap(),
            scenario["resolved_path"].as_str().unwrap()
        );
        assert!(Path::new(scenario["resolved_path"].as_str().unwrap()).exists());
    }
}

#[test]
fn python_entry_point_resolves_the_committed_suite_from_any_working_directory() {
    for working_directory in [root(), scratch("cwd")] {
        let output_directory = scratch("python-output");
        let output = output_directory.join("plan.json");
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
        assert_committed_plan(&plan(&output));
    }
}

#[test]
fn shell_wrapper_resolves_the_committed_suite_from_any_working_directory() {
    for working_directory in [root(), scratch("wrapper-cwd")] {
        // A caller-selected output path containing spaces must survive the
        // wrapper as one argument.
        let output_directory = scratch("wrapper output");
        let output = output_directory.join("bench plan.json");
        let result = Command::new(root().join(SUITE).join("run.sh"))
            .arg("--dry-run")
            .arg("--output")
            .arg(&output)
            .current_dir(&working_directory)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "run.sh failed from {}: {}",
            working_directory.display(),
            String::from_utf8_lossy(&result.stderr)
        );
        assert_committed_plan(&plan(&output));
    }
}

#[test]
fn wrapper_forwards_a_compare_path_containing_spaces_as_one_argument() {
    let baseline_directory = scratch("baseline directory");
    let baseline = baseline_directory.join("prior baseline.json");
    // The plan records the caller-selected paths verbatim. A wrapper that split
    // either argument on whitespace fails the runner's own argument parsing or
    // records a truncated path here.
    let output = scratch("compare output").join("plan.json");
    let result = Command::new(root().join(SUITE).join("run.sh"))
        .arg("--dry-run")
        .arg("--compare")
        .arg(&baseline)
        .arg("--output")
        .arg(&output)
        .current_dir(root())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let document = plan(&output);
    assert_eq!(
        document["compare"].as_str().unwrap(),
        baseline.to_str().unwrap()
    );
    assert_eq!(
        document["output"].as_str().unwrap(),
        output.to_str().unwrap()
    );
}

#[test]
fn a_missing_scenario_inventory_fails_concisely() {
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
    assert!(
        stderr.contains("scenario inventory not found"),
        "expected an actionable failure, got: {stderr}"
    );
    assert!(
        !stderr.contains("Traceback"),
        "startup failures must not print a traceback: {stderr}"
    );
}

#[test]
fn a_missing_output_argument_fails_concisely() {
    let result = Command::new("python3")
        .arg(root().join(SUITE).join("run.py"))
        .arg("--dry-run")
        .current_dir(root())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success());
    assert!(
        stderr.contains("--output"),
        "expected the missing argument to be named: {stderr}"
    );
    assert!(!stderr.contains("Traceback"), "{stderr}");
}

#[test]
fn suite_documentation_and_scripts_name_the_committed_layout() {
    let suite = root().join(SUITE);
    for relative in ["README.md", "docs/METHODOLOGY.md", "run.py", "run.sh"] {
        let path = suite.join(relative);
        let source = std::fs::read_to_string(&path).unwrap();
        for stale in [
            "benchmark/run.py",
            "benchmark/run.sh",
            "benchmark/scenarios",
        ] {
            assert!(
                !source.contains(stale),
                "{} still names the removed `{stale}` path",
                path.display()
            );
        }
    }
}

#[test]
fn every_committed_scenario_path_exists() {
    let inventory: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join(SUITE).join("scenarios.json")).unwrap(),
    )
    .unwrap();
    let scenarios = inventory["scenarios"].as_array().unwrap();
    assert!(!scenarios.is_empty());
    for scenario in scenarios {
        let id = scenario["id"].as_str().unwrap();
        let path = root().join(scenario["path"].as_str().unwrap());
        assert!(
            path.exists(),
            "{id} names a missing path {}",
            path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Result accounting: what was measured, on what host, with which inputs, and
// what happened. A fast failure must never be scoreable as an improvement.
// ---------------------------------------------------------------------------

const GOOD_SOURCE: &str =
    "module bench.smoke;\n\n@id(\"bench.smoke.main\")\nfn main() -> i64\n{\n    42\n}\n";
const BROKEN_SOURCE: &str = "module bench.broken;\n\nfn (\n";

fn runner() -> Command {
    let mut command = Command::new("python3");
    command.arg(root().join(SUITE).join("run.py"));
    command
}

/// Run the runner over a caller-owned inventory and return its exit code with
/// the parsed result document.
fn measure(directory: &Path, inventory: &Value, extra: &[&str]) -> (Option<i32>, Value, String) {
    let inventory_path = directory.join("scenarios.json");
    std::fs::write(&inventory_path, serde_json::to_string(inventory).unwrap()).unwrap();
    let output = directory.join("result.json");
    let result = runner()
        .arg("--root")
        .arg(directory)
        .arg("--scenarios")
        .arg(&inventory_path)
        .arg("--semaprax")
        .arg(env!("CARGO_BIN_EXE_semaprax"))
        .arg("--quick")
        .arg("--output")
        .arg(&output)
        .args(extra)
        .current_dir(root())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&result.stdout).into_owned();
    let document = std::fs::read_to_string(&output)
        .map(|text| serde_json::from_str(&text).unwrap())
        .unwrap_or(Value::Null);
    (result.status.code(), document, stdout)
}

fn record<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["id"] == id)
        .unwrap_or_else(|| panic!("no record for {id}"))
}

#[test]
fn the_recorded_host_and_subject_come_from_the_run_that_happened() {
    let directory = scratch("host");
    std::fs::write(directory.join("good.spx"), GOOD_SOURCE).unwrap();
    let inventory = serde_json::json!({
        "schema": "benchmark.scenarios.v1",
        "scenarios": [
            {"id": "check-good", "kind": "single", "path": "good.spx", "command": "check", "repetitions": 2}
        ]
    });
    let (code, document, _) = measure(&directory, &inventory, &[]);
    assert_eq!(code, Some(0));

    // The platform tag is observed, never a literal. A Linux run cannot report
    // macOS, because this is the same observation the runner makes.
    let observed = Command::new("python3")
        .args([
            "-c",
            "import platform;print(platform.system().lower()+'-'+platform.machine().lower())",
        ])
        .output()
        .unwrap();
    let expected = String::from_utf8_lossy(&observed.stdout).trim().to_owned();
    assert_eq!(document["host"]["platform"].as_str().unwrap(), expected);
    assert!(document["host"]["cpu_count"].as_u64().unwrap() >= 1);
    assert!(document["host"]["rustc"]
        .as_str()
        .unwrap()
        .contains("rustc"));

    // The measured binary is identified, and it is the one that was selected.
    let subject = &document["subject"];
    assert_eq!(
        subject["binary"].as_str().unwrap(),
        std::fs::canonicalize(env!("CARGO_BIN_EXE_semaprax"))
            .unwrap()
            .to_str()
            .unwrap()
    );
    assert_eq!(subject["profile"].as_str().unwrap(), "provided");
    assert!(subject["digest"].as_str().unwrap().starts_with("sha256:"));
    assert!(subject["version"].as_str().unwrap().contains("semaprax"));
    assert!(subject["commit"].as_str().is_some());
    assert_eq!(
        document["schema"].as_str().unwrap(),
        "benchmark.performance.v2"
    );

    let checked = record(&document, "check-good");
    assert_eq!(checked["status"].as_str().unwrap(), "ok");
    assert_eq!(checked["completed_samples"].as_u64().unwrap(), 2);
    assert_eq!(
        checked["wall_ms"]["samples"].as_array().unwrap().len(),
        2,
        "a successful record publishes exactly its completed samples"
    );
    assert_eq!(checked["verification"]["status"].as_str().unwrap(), "ok");
}

#[test]
fn a_failing_scenario_is_a_failure_and_can_never_be_scored_as_an_improvement() {
    let directory = scratch("failure");
    std::fs::write(directory.join("good.spx"), GOOD_SOURCE).unwrap();
    std::fs::write(directory.join("broken.spx"), BROKEN_SOURCE).unwrap();
    let inventory = serde_json::json!({
        "schema": "benchmark.scenarios.v1",
        "scenarios": [
            {"id": "check-good", "kind": "single", "path": "good.spx", "command": "check", "repetitions": 2},
            {"id": "check-broken", "kind": "single", "path": "broken.spx", "command": "check", "repetitions": 2}
        ]
    });
    let (code, document, _) = measure(&directory, &inventory, &[]);
    assert_eq!(code, Some(1), "a failing scenario must fail the run");
    assert_eq!(document["summary"]["failed"].as_u64().unwrap(), 1);
    assert_eq!(document["summary"]["ok"].as_u64().unwrap(), 1);

    let broken = record(&document, "check-broken");
    assert_eq!(broken["status"].as_str().unwrap(), "failed");
    assert!(
        broken.get("wall_ms").is_none(),
        "a failure must publish no comparable timing: {broken}"
    );

    // Scored against a baseline where the same scenario succeeded, the failure
    // is incomparable rather than a large improvement.
    let baseline = directory.join("baseline.json");
    let mut succeeded = document.clone();
    let scenarios = succeeded["scenarios"].as_array_mut().unwrap();
    for row in scenarios.iter_mut() {
        if row["id"] == "check-broken" {
            row["status"] = Value::from("ok");
            row["wall_ms"] = serde_json::json!({"p50": 900.0, "p95": 950.0, "samples": [900.0]});
        }
    }
    std::fs::write(&baseline, serde_json::to_string(&succeeded).unwrap()).unwrap();
    let (_, _, stdout) = measure(
        &directory,
        &inventory,
        &["--compare", baseline.to_str().unwrap()],
    );
    let line = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("check-broken:"))
        .unwrap_or_else(|| panic!("no comparison line for check-broken:\n{stdout}"));
    assert!(
        line.contains("incomparable"),
        "a failure must not be scored: {line}"
    );
    assert!(
        !line.contains("improvement") && !line.contains("regression"),
        "a failure carries no verdict: {line}"
    );
}

#[test]
fn changing_an_included_project_source_changes_the_recorded_subject_identity() {
    let directory = scratch("identity");
    let project = directory.join("examples/calculator-project");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let shipped = root().join("examples/calculator-project");
    std::fs::copy(shipped.join("semaprax.toml"), project.join("semaprax.toml")).unwrap();
    for source in ["app.spx", "core.spx", "tests.spx"] {
        std::fs::copy(
            shipped.join("src").join(source),
            project.join("src").join(source),
        )
        .unwrap();
    }
    let inventory = serde_json::json!({
        "schema": "benchmark.scenarios.v1",
        "scenarios": [
            {"id": "check-project", "kind": "project",
             "path": "examples/calculator-project/semaprax.toml",
             "command": "check", "repetitions": 2}
        ]
    });
    let inventory_path = directory.join("scenarios.json");
    std::fs::write(&inventory_path, serde_json::to_string(&inventory).unwrap()).unwrap();

    let identify = |output: &Path| -> Value {
        let result = runner()
            .arg("--dry-run")
            .arg("--root")
            .arg(&directory)
            .arg("--scenarios")
            .arg(&inventory_path)
            .arg("--output")
            .arg(output)
            .current_dir(root())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        serde_json::from_str(&std::fs::read_to_string(output).unwrap()).unwrap()
    };

    let before = identify(&directory.join("before.json"));
    let subject = &record(&before, "check-project")["subject"];
    assert_eq!(
        subject["inputs"].as_array().unwrap().len(),
        4,
        "a project's identity binds its manifest and every declared source"
    );
    let manifest_digest = subject["inputs"][0]["digest"].clone();
    let identity = subject["digest"].as_str().unwrap().to_owned();

    // The manifest is untouched; one included source changes.
    let core = project.join("src/core.spx");
    let mut edited = std::fs::read_to_string(&core).unwrap();
    edited.push_str("\n// edited\n");
    std::fs::write(&core, edited).unwrap();

    let after = identify(&directory.join("after.json"));
    let subject = &record(&after, "check-project")["subject"];
    assert_eq!(
        subject["inputs"][0]["digest"], manifest_digest,
        "the manifest bytes did not change"
    );
    assert_ne!(
        subject["digest"].as_str().unwrap(),
        identity,
        "an included source change must change the subject identity"
    );
}

#[test]
fn a_mismatched_expected_digest_fails_closed_without_timing() {
    let directory = scratch("drift");
    std::fs::write(directory.join("good.spx"), GOOD_SOURCE).unwrap();
    let inventory = serde_json::json!({
        "schema": "benchmark.scenarios.v1",
        "scenarios": [
            {"id": "check-good", "kind": "single", "path": "good.spx", "command": "check",
             "repetitions": 2, "expected_digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000"}
        ]
    });
    let (code, document, _) = measure(&directory, &inventory, &[]);
    assert_eq!(code, Some(1), "drift must fail the run");
    let checked = record(&document, "check-good");
    assert_eq!(checked["status"].as_str().unwrap(), "drifted");
    assert!(checked.get("wall_ms").is_none());
    assert_eq!(document["summary"]["drifted"].as_u64().unwrap(), 1);
}

#[test]
fn build_repetitions_receive_fresh_owned_destinations() {
    // The runner once wrote every build repetition to a deterministic
    // `/tmp/semaprax-bench-<id>`, which conflicts with fresh-output publication
    // and risks deleting a path the runner does not own.
    let script = format!(
        r#"
import pathlib, runpy, shutil, tempfile
runner = runpy.run_path({:?})
first = runner['build_destination']('scenario')
second = runner['build_destination']('scenario')
assert first != second, (first, second)
for destination in (first, second):
    assert not destination.exists(), destination
    assert destination.parent.is_dir(), destination.parent
    assert destination.parent != pathlib.Path(tempfile.gettempdir()), destination
    assert str(destination.parent).startswith(tempfile.gettempdir()), destination
    shutil.rmtree(destination.parent)
print('ok')
"#,
        root().join(SUITE).join("run.py").to_str().unwrap()
    );
    let result = Command::new("python3")
        .args(["-c", &script])
        .current_dir(root())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn the_committed_baseline_records_no_measurement_and_cannot_be_compared_against() {
    let baseline = root().join(SUITE).join("results/baseline.json");
    let document: Value =
        serde_json::from_str(&std::fs::read_to_string(&baseline).unwrap()).unwrap();
    assert_eq!(document["recorded"], Value::Bool(false));
    assert!(document["scenarios"].as_array().unwrap().is_empty());
    for claimed in ["host", "subject", "rustc", "commit", "timestamp"] {
        assert!(
            document.get(claimed).is_none(),
            "an unrecorded baseline must claim no {claimed} provenance"
        );
    }
    assert!(
        root().join(SUITE).join("results/baseline.md").exists(),
        "the rendering the README links must exist"
    );

    let directory = scratch("empty-baseline");
    std::fs::write(directory.join("good.spx"), GOOD_SOURCE).unwrap();
    let inventory = serde_json::json!({
        "schema": "benchmark.scenarios.v1",
        "scenarios": [
            {"id": "check-good", "kind": "single", "path": "good.spx", "command": "check", "repetitions": 2}
        ]
    });
    let inventory_path = directory.join("scenarios.json");
    std::fs::write(&inventory_path, serde_json::to_string(&inventory).unwrap()).unwrap();
    let result = runner()
        .arg("--root")
        .arg(&directory)
        .arg("--scenarios")
        .arg(&inventory_path)
        .arg("--semaprax")
        .arg(env!("CARGO_BIN_EXE_semaprax"))
        .arg("--quick")
        .arg("--output")
        .arg(directory.join("result.json"))
        .arg("--compare")
        .arg(&baseline)
        .current_dir(root())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("baseline holds no recorded measurement"),
        "{stderr}"
    );
}

#[test]
fn one_committed_scenario_is_measurable_from_an_unrelated_working_directory() {
    let directory = scratch("committed-scenario");
    let output = directory.join("one.json");
    let result = runner()
        .arg("--only")
        .arg("check-meaning")
        .arg("--semaprax")
        .arg(env!("CARGO_BIN_EXE_semaprax"))
        .arg("--quick")
        .arg("--output")
        .arg(&output)
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let document: Value = serde_json::from_str(&std::fs::read_to_string(&output).unwrap()).unwrap();
    let measured = record(&document, "check-meaning");
    assert_eq!(measured["status"].as_str().unwrap(), "ok");
    assert_eq!(measured["completed_samples"].as_u64().unwrap(), 2);
    assert_eq!(
        measured["subject"]["inputs"][0]["path"].as_str().unwrap(),
        "examples/meaning.spx"
    );
}
