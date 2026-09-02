//! Property-Test Generation widening evidence: the full Copy-scalar surface
//! (`i64`, `i32`, `u8`, `char`, `f32`, `f64`, `bool`), canonical widened
//! renderings, while/assignment body admission, and the still-closed shapes.
//!
//! Read-only like the v1 tranche: no symbolic execution, no shrinking, no
//! test running, no target execution.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::properties::{self, PropertyTestOptions};
use serde_json::Value;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_source(name: &str, source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-property-widen-v1-{}-{}-{name}.spx",
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

/// Builds a one-parameter probe whose first surviving candidate lands on
/// lattice entry `filters.len()`: every earlier lattice value is excluded by
/// one `requires` filter, and the always-false ensures clause then records a
/// counterexample carrying that candidate's canonical rendering.
fn lattice_probe_source(result_type: &str, filters: &[&str]) -> String {
    let clauses = filters
        .iter()
        .map(|filter| format!("    requires value != {filter}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"
module test.widen;

@id("t.probe")
fn probe(value: {result_type}) -> {result_type}
{clauses}
    ensures result != result
{{ value }}

@id("app.main")
fn main() -> i64 {{ 0 }}
"#
    )
}

/// Pins the whole boundary lattice of one widened type by walking its
/// counterexamples one case at a time.
fn assert_lattice(type_text: &str, source_filters: &[&str], rendered: &[&str]) {
    // The final lattice entry needs no filter: it is the first surviving
    // candidate once every earlier entry is excluded.
    assert_eq!(
        rendered.len(),
        source_filters.len() + 1,
        "lattice alignment"
    );
    for pinned in 0..=source_filters.len() {
        let source = lattice_probe_source(type_text, &source_filters[..pinned]);
        let path = write_source("lattice", &source);
        let options = PropertyTestOptions::new(pinned + 1, 8, 64 * 1024, 7).unwrap();
        let report = generate_value(&path, &options);
        let probe = function_entry(&report, "probe");
        assert_eq!(probe["outcome"], "analyzed", "{type_text} case {pinned}");
        assert_eq!(
            probe["cases_attempted"].as_u64().unwrap(),
            pinned as u64 + 1,
            "{type_text} case {pinned}"
        );
        assert_eq!(
            probe["filtered_cases"].as_u64().unwrap(),
            pinned as u64,
            "{type_text} case {pinned}"
        );
        let counterexample = &probe["counterexample"];
        assert_eq!(counterexample["index"], 0, "{type_text} case {pinned}");
        let arguments = counterexample["arguments"].as_array().unwrap();
        assert_eq!(arguments.len(), 1);
        assert_eq!(
            arguments[0]["value"].as_str().unwrap(),
            rendered[pinned],
            "{type_text} lattice entry {pinned} must render canonically"
        );
    }
}

#[test]
fn i64_lattice_pins_canonical_decimal_renderings() {
    // Source filters avoid bare extreme literals: `-i64::MAX - 1` is the only
    // way to spell `i64::MIN` in source because its positive magnitude alone
    // is outside the i64 literal range.
    assert_lattice(
        "i64",
        &[
            "0",
            "1",
            "-1",
            "2",
            "-2",
            "3",
            "-3",
            "-9223372036854775807 - 1",
            "9223372036854775807",
            "-9223372036854775807",
        ],
        &[
            "0",
            "1",
            "-1",
            "2",
            "-2",
            "3",
            "-3",
            "-9223372036854775808",
            "9223372036854775807",
            "-9223372036854775807",
            "9223372036854775806",
        ],
    );
}

#[test]
fn i32_lattice_pins_suffixed_renderings() {
    assert_lattice(
        "i32",
        &[
            "0i32",
            "1i32",
            "-1i32",
            "2i32",
            "-2i32",
            "3i32",
            "-3i32",
            "-2147483647i32 - 1i32",
            "2147483647i32",
            "-2147483647i32",
        ],
        &[
            "0i32",
            "1i32",
            "-1i32",
            "2i32",
            "-2i32",
            "3i32",
            "-3i32",
            "-2147483648i32",
            "2147483647i32",
            "-2147483647i32",
            "2147483646i32",
        ],
    );
}

#[test]
fn u8_lattice_pins_suffixed_renderings() {
    assert_lattice(
        "u8",
        &["0u8", "1u8", "2u8", "3u8", "255u8", "254u8"],
        &["0u8", "1u8", "2u8", "3u8", "255u8", "254u8", "253u8"],
    );
}

#[test]
fn char_lattice_pins_canonical_escape_renderings() {
    assert_lattice(
        "char",
        &[
            "'a'", "'A'", "'0'", "' '", "'\\n'", "'\\t'", "'\\r'", "'\\0'", "'\\\\'", "'\\''",
        ],
        &[
            "'a'",
            "'A'",
            "'0'",
            "' '",
            "'\\n'",
            "'\\t'",
            "'\\r'",
            "'\\0'",
            "'\\\\'",
            "'\\''",
            "'\\u{10ffff}'",
        ],
    );
}

#[test]
fn f32_lattice_pins_finite_boundary_renderings() {
    // `f32` contexts need explicit suffixes: bare float literals are f64.
    assert_lattice(
        "f32",
        &[
            "0.0f32",
            "1.0f32",
            "-1.0f32",
            "2.0f32",
            "-2.0f32",
            "3.0f32",
            "-3.0f32",
            "-3.4028235e38f32",
            "3.4028235e38f32",
            "0.5f32",
        ],
        &[
            "00000000", "3f800000", "bf800000", "40000000", "c0000000", "40400000", "c0400000",
            "ff7fffff", "7f7fffff", "3f000000", "bf000000",
        ],
    );
}

#[test]
fn f64_lattice_pins_finite_boundary_renderings() {
    assert_lattice(
        "f64",
        &[
            "0.0",
            "1.0",
            "-1.0",
            "2.0",
            "-2.0",
            "3.0",
            "-3.0",
            "-1.7976931348623157e308",
            "1.7976931348623157e308",
            "0.5",
        ],
        &[
            "0000000000000000",
            "3ff0000000000000",
            "bff0000000000000",
            "4000000000000000",
            "c000000000000000",
            "4008000000000000",
            "c008000000000000",
            "ffefffffffffffff",
            "7fefffffffffffff",
            "3fe0000000000000",
            "bfe0000000000000",
        ],
    );
}

#[test]
fn bool_lattice_pins_true_before_false() {
    assert_lattice("bool", &["true"], &["true", "false"]);
}

const MIXED_SOURCE: &str = r#"
module test.widen;

@id("t.mixed")
fn mixed(a: i64, b: i32, c: u8, d: char, e: f32, f: f64, g: bool) -> bool
    requires a >= -3
    requires b != 0i32 || g
{
    c <= 200u8 && d != '\n' && e >= -100.0f32 && f <= 100.0
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

#[test]
fn mixed_signature_generation_is_byte_deterministic_and_typed() {
    let path = write_source("mixed", MIXED_SOURCE);
    let options = PropertyTestOptions::default();
    let first = properties::generate(&path, &options).unwrap();
    let second = properties::generate(&path, &options).unwrap();
    assert_eq!(first, second, "two runs must be byte-identical");

    let report: Value = serde_json::from_str(&first).unwrap();
    let mixed = function_entry(&report, "mixed");
    assert_eq!(mixed["outcome"], "analyzed");
    assert_eq!(
        mixed["signature"],
        serde_json::json!({
            "params": [
                {"name": "a", "type": "i64"},
                {"name": "b", "type": "i32"},
                {"name": "c", "type": "u8"},
                {"name": "d", "type": "char"},
                {"name": "e", "type": "f32"},
                {"name": "f", "type": "f64"},
                {"name": "g", "type": "bool"},
            ],
            "result": "bool",
        })
    );
    assert_eq!(mixed["counterexample"], Value::Null);
    assert!(mixed["discharged_cases"].as_u64().unwrap() > 0);
}

#[test]
fn requires_filter_on_widened_type_reports_exact_counterexample() {
    let source = r#"
module test.widen;

@id("t.clamp8")
fn clamp8(value: u8) -> u8
    requires value >= 200u8
    ensures result <= 200u8
{ if value > 200u8 { 200u8 } else { value } }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("filtered-u8", source);
    let report = generate_value(&path, &PropertyTestOptions::default());
    let clamp8 = function_entry(&report, "clamp8");
    // Lattice cases 0..=3 (`0u8`..`3u8`) are filtered by the requires clause
    // before any body evaluation; later candidates keep running because no
    // ensures violation ever fires on this correct implementation.
    assert!(clamp8["filtered_cases"].as_u64().unwrap() >= 4);
    assert!(clamp8["discharged_cases"].as_u64().unwrap() > 0);
    assert_eq!(clamp8["counterexample"], Value::Null);

    // A wrong implementation must produce a counterexample whose widened
    // argument renders canonically with the u8 suffix.
    let wrong = r#"
module test.widen;

@id("t.wrong")
fn wrong(value: u8) -> u8
    requires value >= 200u8
    ensures result == value
{ 0u8 }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let wrong_path = write_source("wrong-u8", wrong);
    let wrong_report = generate_value(&wrong_path, &PropertyTestOptions::default());
    let wrong_entry = function_entry(&wrong_report, "wrong");
    let counterexample = &wrong_entry["counterexample"];
    assert_eq!(counterexample["index"], 0);
    assert_eq!(
        counterexample["arguments"][0]["value"].as_str().unwrap(),
        "255u8",
        "the first surviving lattice candidate is the counterexample"
    );
    assert_eq!(counterexample["result"], "0u8");
}

#[test]
fn while_loops_and_mut_locals_are_admitted_and_bounded() {
    let terminating = r#"
module test.widen;

@id("t.count")
fn count(limit: i64) -> i64
    requires limit >= 0
    requires limit <= 100
    ensures result == limit * 2
{
    let mut total = 0;
    let mut index = 0;
    while index < limit {
        total = total + 2;
        index = index + 1;
        index <= limit
    }
    total
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("count", terminating);
    let report = generate_value(&path, &PropertyTestOptions::default());
    let count = function_entry(&report, "count");
    assert_eq!(count["outcome"], "analyzed");
    assert!(count["discharged_cases"].as_u64().unwrap() > 0);
    assert_eq!(count["counterexample"], Value::Null);
    assert_eq!(count["runtime_failures"], 0);

    // A non-terminating loop fails closed through the shared step budget:
    // the function produces no entry and the run reports step_budget.
    let spinning = r#"
module test.widen;

@id("t.spin")
fn spin(value: i64) -> i64 {
    let mut index = 0;
    while index < value {
        index = index + 1;
        index < value
    }
    index
}

@id("t.after")
fn after(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let spin_path = write_source("spin", spinning);
    let spin_report_text = properties::generate(
        &spin_path,
        &PropertyTestOptions::new(16, 16, 64 * 1024, 7).unwrap(),
    )
    .unwrap();
    let spin_report: Value = serde_json::from_str(&spin_report_text).unwrap();
    assert_eq!(spin_report["truncation"]["truncated"], true);
    assert!(
        spin_report["truncation"]["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason == "step_budget"),
        "step-budget exhaustion must be reported as truncation"
    );
    let names: Vec<&str> = spin_report["functions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert!(
        names.is_empty(),
        "the exhausted probe stops the run before any entry is emitted"
    );
    assert_eq!(spin_report["summary"]["functions_total"], 3);
    assert_eq!(spin_report["truncation"]["omitted_functions"], 3);
}

#[test]
fn runtime_reasons_cover_widened_integer_widths() {
    let source = r#"
module test.widen;

@id("t.divide32")
fn divide32(left: i32, right: i32) -> i32 { left / right }

@id("t.negate32")
fn negate32(value: i32) -> i32 { -value }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("runtime-widened", source);
    let report = generate_value(&path, &PropertyTestOptions::default());
    let divide32 = function_entry(&report, "divide32");
    assert!(divide32["runtime_failures"].as_u64().unwrap() > 0);
    assert!(divide32["runtime_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason == "division_by_zero"));
    let negate32 = function_entry(&report, "negate32");
    assert!(negate32["runtime_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason == "negation_overflow"));
}

#[test]
fn still_closed_shapes_defer_with_the_legacy_vocabulary() {
    let source = r#"
module test.widen;

permit { clock.read }

record Pair {
    left: i64,
    right: bool,
}

variant Choice {
    None,
    Number { value: i64, },
}

@id("t.stringy")
fn stringy(value: string) -> string { value }

@id("t.generic")
fn generic<T>(value: T) -> T { value }

@id("t.effectful")
fn effectful(value: i64) -> i64
    uses { clock.read }
{ value + 1 }

@id("t.constructs")
fn constructs(value: i64) -> i64 {
    let pair = Pair { left: value, right: true };
    pair.left
}

@id("t.branches")
fn branches(value: i64, flag: bool) -> i64 {
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
    let path = write_source("closed-shapes", source);
    let report = generate_value(&path, &PropertyTestOptions::default());
    let expected: &[(&str, &str)] = &[
        ("stringy", "unsupported_parameter_type"),
        ("generic", "generic_function"),
        ("effectful", "declared_effects"),
        ("constructs", "record_construction"),
        ("branches", "variant_construction"),
    ];
    for (name, reason) in expected {
        let entry = function_entry(&report, name);
        assert_eq!(entry["outcome"], "deferred", "{name}");
        assert_eq!(entry["reason"], *reason, "{name}");
    }
}

#[test]
fn reports_bind_exact_source_bytes_and_fail_closed() {
    let source = r#"
module test.widen;

@id("t.identity")
fn identity(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("digest", source);
    let report_text = properties::generate(&path, &PropertyTestOptions::default()).unwrap();

    use sha2::{Digest as _, Sha256};
    let source_bytes = std::fs::read(&path).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.property-tests.source.v1\0");
    hasher.update((source_bytes.len() as u64).to_le_bytes());
    hasher.update(&source_bytes);
    let expected_digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );

    let report: Value = serde_json::from_str(&report_text).unwrap();
    assert_eq!(report["source"]["sha256"], expected_digest.as_str());
    assert_eq!(report["schema"], "semaprax.property-tests.v1");

    // Tampered bytes bind to a different digest; nothing is cached or stale.
    let mut tampered = source_bytes.clone();
    tampered.truncate(tampered.len() - 1);
    let tampered_path = write_source("tampered", &String::from_utf8(tampered).unwrap());
    let tampered_report: Value = serde_json::from_str(
        &properties::generate(&tampered_path, &PropertyTestOptions::default()).unwrap(),
    )
    .unwrap();
    assert_ne!(
        tampered_report["source"]["sha256"],
        expected_digest.as_str(),
        "tampered sources must bind their own digest"
    );

    // Verification failures fail closed with error diagnostics.
    let invalid = r#"
module test.widen;

@id("t.bad")
fn bad(value: i64) -> i64
    ensures result == missing
{ value }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let invalid_path = write_source("invalid", invalid);
    let outcome = properties::generate(&invalid_path, &PropertyTestOptions::default());
    let errors = outcome.expect_err("verification errors must fail closed");
    assert!(errors.iter().any(|item| item.severity.is_error()));
    assert_eq!(
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(["properties", invalid_path.to_str().unwrap()])
            .output()
            .unwrap()
            .status
            .code(),
        Some(1),
        "the CLI surfaces verification failures with exit code 1"
    );
}

#[test]
fn cli_exit_codes_stay_closed_for_usage_errors() {
    let cli = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(arguments)
            .output()
            .unwrap()
    };
    let missing_argument = cli(&["properties"]);
    assert_eq!(missing_argument.status.code(), Some(2));
    let unknown_option = cli(&["properties", "examples/meaning.spx", "--shrink", "1"]);
    assert_eq!(unknown_option.status.code(), Some(2));
    let out_of_bounds = cli(&["properties", "examples/meaning.spx", "--max-cases", "0"]);
    assert_eq!(out_of_bounds.status.code(), Some(2));

    let good = write_source("cli-good", MIXED_SOURCE);
    let success = cli(&["properties", good.to_str().unwrap()]);
    assert_eq!(
        success.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    let stdout: Value = serde_json::from_str(&String::from_utf8(success.stdout).unwrap())
        .expect("successful CLI runs print exactly one JSON report");
    assert_eq!(stdout["schema"], "semaprax.property-tests.v1");
}
