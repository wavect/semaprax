use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::properties::{self, PropertyTestOptions};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_source(name: &str, source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-property-tests-v1-{}-{}-{name}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn generate_value(source_path: &Path, options: &PropertyTestOptions) -> Value {
    let report = properties::generate(source_path, options).unwrap();
    serde_json::from_str(&report).unwrap()
}

fn function_entry<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == name)
        .unwrap_or_else(|| panic!("missing function entry {name}"))
}

#[test]
fn meaning_example_is_deterministic_and_discharges() {
    let source_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/meaning.spx");
    let options = PropertyTestOptions::default();
    let first = properties::generate(&source_path, &options).unwrap();
    let second = properties::generate(&source_path, &options).unwrap();
    assert_eq!(first, second, "repeated generation must be byte-identical");

    let report: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(report["schema"], "semaprax.property-tests.v1");
    assert_eq!(
        report["seed"],
        properties::DEFAULT_SEED.to_string(),
        "the default seed must be serialized as a decimal string"
    );

    let source_bytes = std::fs::read(&source_path).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.property-tests.source.v1\0");
    hasher.update((source_bytes.len() as u64).to_le_bytes());
    hasher.update(&source_bytes);
    let expected_digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    assert_eq!(report["source"]["sha256"], expected_digest.as_str());
    assert!(
        report["source"]["revision"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"),
        "the bound graph revision must be a digest string"
    );
    assert_eq!(report["limits"]["max_cases"], 64);
    assert_eq!(report["limits"]["max_functions"], 64);
    assert_eq!(report["limits"]["max_bytes"], 65536);
    assert_eq!(report["truncation"]["truncated"], false);
    assert_eq!(report["truncation"]["reasons"], serde_json::json!([]));
    assert_eq!(
        report["nonclaims"],
        serde_json::json!([
            "no_symbolic_execution_or_smt",
            "no_static_contract_discharge",
            "no_counterexample_minimization",
            "no_statistical_coverage_guarantee",
            "not_a_test_runner",
            "no_target_execution",
        ])
    );

    let summary = &report["summary"];
    let analyzed = summary["functions_analyzed"].as_u64().unwrap();
    let deferred = summary["functions_deferred"].as_u64().unwrap();
    assert_eq!(summary["functions_total"], analyzed + deferred);
    assert_eq!(
        report["budget"]["used_functions"],
        report["functions"].as_array().unwrap().len()
    );
    assert!(analyzed >= 2, "add and main must both be analyzed");

    let add = function_entry(&report, "add");
    assert_eq!(add["outcome"], "analyzed");
    assert_eq!(
        add["signature"],
        serde_json::json!({
            "params": [
                {"name": "left", "type": "i64"},
                {"name": "right", "type": "i64"},
            ],
            "result": "i64",
        })
    );
    assert_eq!(
        add["requires"],
        serde_json::json!([
            {"index": 0, "text": "left >= 0"},
            {"index": 1, "text": "right >= 0"},
        ])
    );
    assert_eq!(
        add["ensures"],
        serde_json::json!([
            {"index": 0, "text": "result == left + right"},
        ])
    );
    assert!(add["filtered_cases"].as_u64().unwrap() > 0);
    assert!(add["discharged_cases"].as_u64().unwrap() > 0);
    assert_eq!(add["counterexample"], Value::Null);

    let main = function_entry(&report, "main");
    assert_eq!(main["outcome"], "analyzed");
    assert_eq!(
        main["discharged_cases"], main["cases_attempted"],
        "main must discharge every generated case"
    );
}

#[test]
fn ensures_violation_reports_exact_counterexample_and_stops() {
    let source = r#"
module test.probe;

@id("probe.bad")
fn bad(x: i64) -> i64
    ensures result == x + 1
{ x }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("counterexample.spx", source);
    let report = generate_value(&path, &PropertyTestOptions::default());
    assert_eq!(report["summary"]["functions_with_counterexamples"], 1);
    let bad = function_entry(&report, "bad");
    assert_eq!(bad["cases_attempted"], 1);
    assert_eq!(
        bad["counterexample"],
        serde_json::json!({
            "index": 0,
            "text": "result == x + 1",
            "arguments": [{"name": "x", "value": "0"}],
            "result": "0",
        })
    );
}

#[test]
fn runtime_failures_report_closed_reasons() {
    let source = r#"
module test.probe;

@id("probe.divide")
fn divide(left: i64, right: i64) -> i64 { left / right }

@id("probe.remainder")
fn remainder(left: i64, right: i64) -> i64 { left % right }

@id("probe.negate")
fn negate(value: i64) -> i64 { -value }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("runtime.spx", source);
    let report = generate_value(&path, &PropertyTestOptions::default());
    for name in ["divide", "remainder", "negate"] {
        let entry = function_entry(&report, name);
        assert!(
            entry["runtime_failures"].as_u64().unwrap() > 0,
            "{name} must observe at least one runtime failure"
        );
        assert_eq!(entry["counterexample"], Value::Null);
    }
    assert!(function_entry(&report, "divide")["runtime_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason == "division_by_zero"));
    assert!(function_entry(&report, "remainder")["runtime_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason == "remainder_by_zero"));
    assert!(function_entry(&report, "negate")["runtime_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason == "negation_overflow"));
}

#[test]
fn unsupported_shapes_defer_with_closed_reasons() {
    let source = r#"
module test.probe;

permit { clock.read }

record Pair {
    left: i64,
    right: bool,
}

variant Choice {
    None,
    Number { value: i64, },
}

@id("probe.generic")
fn generic<T>(value: T) -> T { value }

@id("probe.effectful")
fn effectful(value: i64) -> i64
    uses { clock.read }
{ value + 1 }

@id("probe.floaty")
fn floaty(value: f32) -> f32 { value }

@id("probe.constructs")
fn constructs(value: i64) -> i64 {
    let pair = Pair { left: value, right: true };
    pair.left
}

@id("probe.maybe")
fn maybe(flag: bool) -> Result<i64, bool> {
    if flag {
        Result<i64, bool>::Ok { value: 7 }
    } else {
        Result<i64, bool>::Err { error: false }
    }
}

@id("probe.branching")
fn branching(value: i64, flag: bool) -> i64 {
    let choice = if flag {
        Choice::None {}
    } else {
        Choice::Number { value: value }
    };
    match choice {
        Choice::None {} => 0,
        Choice::Number { value: number } => number,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("deferred.spx", source);
    let report = generate_value(&path, &PropertyTestOptions::default());
    let expected: &[(&str, &str)] = &[
        ("generic", "generic_function"),
        ("effectful", "declared_effects"),
        ("floaty", "unsupported_parameter_type"),
        ("constructs", "record_construction"),
        ("maybe", "unsupported_result_type"),
        ("branching", "variant_construction"),
    ];
    for (name, reason) in expected {
        let entry = function_entry(&report, name);
        assert_eq!(entry["outcome"], "deferred", "{name}");
        assert_eq!(entry["reason"], *reason, "{name}");
    }
    assert_eq!(
        report["summary"]["functions_deferred"],
        expected.len(),
        "every unsupported shape must defer"
    );
}

#[test]
fn function_budget_truncates_with_stable_order() {
    let source = r#"
module test.probe;

@id("probe.first")
fn first(value: i64) -> i64 { value }

@id("probe.second")
fn second(value: i64) -> i64 { value }

@id("probe.third")
fn third(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("caps.spx", source);
    let options = PropertyTestOptions::new(4, 2, 64 * 1024, 7).unwrap();
    let report = generate_value(&path, &options);
    assert_eq!(options.max_cases, 4);
    assert_eq!(report["budget"]["used_functions"], 2);
    assert_eq!(report["truncation"]["omitted_functions"], 2);
    assert_eq!(report["truncation"]["truncated"], true);
    assert_eq!(
        report["truncation"]["reasons"],
        serde_json::json!(["function_budget"])
    );
    let names: Vec<&str> = report["functions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["first", "second"]);

    let limited_cases = generate_value(
        &path,
        &PropertyTestOptions::new(1, 64, 64 * 1024, 7).unwrap(),
    );
    for entry in limited_cases["functions"].as_array().unwrap() {
        if entry["outcome"] == "analyzed" {
            assert_eq!(entry["cases_attempted"], 1);
        }
    }
}

#[test]
fn byte_budget_truncates_prefix_without_invalid_json() {
    let source = r#"
module test.probe;

@id("probe.first")
fn first(value: i64) -> i64
    requires value >= 0
{ value }

@id("probe.second")
fn second(value: i64) -> i64
    requires value >= 0
{ value }

@id("probe.third")
fn third(value: i64) -> i64
    requires value >= 0
{ value }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("bytes.spx", source);
    let options = PropertyTestOptions::new(8, 16, 1024, 11).unwrap();
    let report_text = properties::generate(&path, &options).unwrap();
    let report: Value = serde_json::from_str(&report_text)
        .unwrap_or_else(|error| panic!("truncated output must stay valid JSON: {error}"));
    assert_eq!(report["truncation"]["truncated"], true);
    let reasons = report["truncation"]["reasons"].as_array().unwrap();
    assert!(
        reasons.iter().any(|reason| reason == "byte_budget"),
        "byte budget exhaustion must be reported"
    );
    assert_eq!(
        report["truncation"]["omitted_functions"],
        4 - report["functions"].as_array().unwrap().len()
    );
}

#[test]
fn seeds_change_sampled_cases_deterministically() {
    let source = r#"
module test.probe;

@id("probe.positive")
fn positive(value: i64) -> i64
    requires value > 0
{ value }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("seeds.spx", source);
    let baseline = properties::generate(
        &path,
        &PropertyTestOptions::new(64, 8, 64 * 1024, 1000).unwrap(),
    )
    .unwrap();
    let repeat = properties::generate(
        &path,
        &PropertyTestOptions::new(64, 8, 64 * 1024, 1000).unwrap(),
    )
    .unwrap();
    assert_eq!(baseline, repeat, "same seed must reproduce exact bytes");
    let other = properties::generate(
        &path,
        &PropertyTestOptions::new(64, 8, 64 * 1024, 1001).unwrap(),
    )
    .unwrap();
    assert_ne!(baseline, other, "different seeds must sample differently");
}

#[test]
fn cli_rejects_unknown_options_and_missing_paths() {
    let cli = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(arguments)
            .output()
            .unwrap()
    };
    let missing = cli(&["properties"]);
    assert_eq!(missing.status.code(), Some(2));
    let unknown = cli(&["properties", "examples/meaning.spx", "--wat", "1"]);
    assert_eq!(unknown.status.code(), Some(2));
    let duplicate = cli(&[
        "properties",
        "examples/meaning.spx",
        "--max-cases",
        "2",
        "--max-cases",
        "3",
    ]);
    assert_eq!(duplicate.status.code(), Some(2));
    let noncanonical = cli(&["properties", "examples/meaning.spx", "--max-cases", "02"]);
    assert_eq!(noncanonical.status.code(), Some(2));
}
