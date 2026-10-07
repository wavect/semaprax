//! Focused source, graph, and cross-backend regressions for the additive
//! `std.bytes` span cursors: ASCII trimming and delimiter-separated fields.
//! Every operation is a borrowed-view offset computation with no allocation.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, graph, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const LIBRARY: &str = include_str!("../../../std/bytes/src/bytes.spx");

fn source(main: &str) -> String {
    format!(
        "{}\n{main}\n",
        LIBRARY.replacen("module std.bytes;", "module app;", 1)
    )
}

fn canonical_checked(main: &str) -> String {
    let program = parse(&source(main), "std-bytes-spans.spx").expect("std.bytes fixture parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "std.bytes fixture source diagnostics: {diagnostics:?}"
    );
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "std-bytes-spans.spx").expect("canonical fixture reparses");
    assert_eq!(format::canonical(&reparsed), canonical);
    hir::resolve(&reparsed).expect("checked std.bytes fixture resolves");
    canonical
}

fn interpretation(main: &str) -> interpreter::Interpretation {
    let canonical = canonical_checked(main);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path: PathBuf = std::env::temp_dir().join(format!(
        "semaprax-std-bytes-spans-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, &canonical).expect("writes temporary checked source");
    let result = interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
        .expect("std.bytes interpreter entry is admitted");
    interpreter::verify_envelope(&result.envelope).expect("interpreter envelope is canonical");
    std::fs::remove_file(path).expect("removes temporary checked source");
    result
}

fn returns(main: &str, expected: &str) {
    let result = interpretation(main);
    assert!(
        result.returned,
        "expected returned {expected}: {:?}",
        result.envelope
    );
    let document: serde_json::Value =
        serde_json::from_str(&result.envelope).expect("envelope JSON");
    assert_eq!(
        document["payload"]["outcome"]["value"].as_str(),
        Some(expected)
    );
}

fn fails(main: &str) {
    let result = interpretation(main);
    assert!(!result.returned, "invalid offset request returned");
    let document: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    let outcome = &document["payload"]["outcome"];
    assert_eq!(outcome["kind"], "failed", "{document}");
    assert_eq!(outcome["status"]["class"], "contract", "{document}");
}

#[test]
fn byte_spans_execute_on_all_three_backends() {
    super::run_examples_and_conformance(
        super::packages()
            .into_iter()
            .filter(|p| p.module == "std.bytes")
            .collect(),
    );
}

#[test]
fn byte_span_walk_preserves_empty_fields_and_trims_only_ascii_space() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let line = [32u8, 97u8, 44u8, 44u8, 98u8, 9u8];
    let view = array_as_slice(line);
    let bounds = trim_start(view) == 1usize && trim_end(view) == 5usize;
    let first = field_end(view, 0usize, 44u8) == 2usize && field_start(view, 0usize, 44u8) == 3usize;
    let empty = field_end(view, 3usize, 44u8) == 3usize && field_start(view, 3usize, 44u8) == 4usize;
    let last = field_end(view, 4usize, 44u8) == 6usize && field_start(view, 4usize, 44u8) == 6usize;
    let counted = field_count(view, 44u8) == 3usize;
    let blank = [32u8, 9u8];
    let empty_view = byte_range(view, 6usize, 6usize);
    let blankness = is_blank(array_as_slice(blank)) && is_blank(empty_view) && !is_blank(view);
    if bounds && first && empty && last && counted && blankness { 0 } else { 1 }
}
"#,
        "0",
    );
}

#[test]
fn byte_span_cursors_reject_a_start_beyond_the_view() {
    for main in [
        r#"
@id("app.main")
fn main() -> i64
{
    let line = [97u8, 44u8];
    let view = array_as_slice(line);
    if field_end(view, 3usize, 44u8) == 2usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let line = [97u8, 44u8];
    let view = array_as_slice(line);
    if field_start(view, 3usize, 44u8) == 2usize { 0 } else { 1 }
}
"#,
    ] {
        fails(main);
    }
}

#[test]
fn byte_span_graph_keeps_borrowed_views_out_of_every_owned_inventory() {
    let canonical = canonical_checked(
        r#"
@id("app.main")
fn main() -> i64
{
    let line = [97u8, 44u8, 98u8];
    let view = array_as_slice(line);
    if field_count(view, 44u8) == 2usize && trim_start(view) == 0usize { 0 } else { 1 }
}
"#,
    );
    let program = parse(&canonical, "std-bytes-spans-graph.spx").unwrap();
    let json = graph::to_json(&program).expect("span graph serializes");
    graph::verify_json(&program, &json).expect("derived span graph independently replays");
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();
    for id in [
        "std.bytes.trim_start",
        "std.bytes.trim_end",
        "std.bytes.is_blank",
        "std.bytes.field_end",
        "std.bytes.field_start",
        "std.bytes.field_count",
    ] {
        let node = document["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing node {id}"));
        assert_eq!(node["params"][0]["ownership_mode"], "borrow", "{id}");
        assert_eq!(node["result"]["ownership_mode"], "value", "{id}");
        assert_eq!(
            node["cleanup"]["schema"], "semaprax.cleanup-plan.v2",
            "{id}"
        );
        assert!(
            node["cleanup"]["entry_state"]["live_owned_parameters"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{id} put a borrowed view in the owned inventory"
        );
    }
    let drifted = parse(
        &canonical.replace("byte == 32u8", "byte == 33u8"),
        "drifted.spx",
    )
    .unwrap();
    assert_ne!(format::canonical(&drifted), canonical);
    assert!(graph::verify_json(&drifted, &json).is_err());
}

#[test]
fn byte_and_position_conversion_work_is_constant_and_preserves_results() {
    let steps = |library: &str, expression: &str| {
        let source = format!(
            "{}\n@id(\"app.main\") fn main() -> i64 {{ {expression} }}\n",
            library.replacen("module std.bytes;", "module app;", 1)
        );
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-std-byte-conversion-{}-{id}.spx",
            std::process::id()
        ));
        std::fs::write(&path, &source).unwrap();
        let result =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        std::fs::remove_file(path).unwrap();
        let document: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        assert!(result.returned, "{}", result.envelope);
        (
            document["payload"]["outcome"]["value"]
                .as_str()
                .unwrap()
                .to_owned(),
            document["payload"]["fuel"]["steps_used"].as_u64().unwrap(),
        )
    };
    let (zero, zero_steps) = steps(LIBRARY, "byte_to_i64(0u8)");
    let (max, max_steps) = steps(LIBRARY, "byte_to_i64(255u8)");
    assert_eq!(zero, "0");
    assert_eq!(max, "255");
    assert_eq!(zero_steps, max_steps);
    let (first, first_steps) = steps(LIBRARY, "position_of(1usize)");
    let (large, large_steps) = steps(LIBRARY, "position_of(65537usize)");
    assert_eq!(first, "0");
    assert_eq!(large, "65536");
    assert_eq!(first_steps, large_steps);
    // Independent pre-fix semantics: the old conversion counted each byte unit.
    let legacy = LIBRARY.replace("    i64_from_u8(byte)", "    let mut value = 0;\n    let mut probe = 0u8;\n    while probe != byte { value = value + 1; probe = probe + 1u8; probe != byte }\n    value");
    let (old_result, old_steps) = steps(&legacy, "byte_to_i64(255u8)");
    assert_eq!(old_result, max);
    assert!(
        old_steps > max_steps * 100,
        "old={old_steps}, new={max_steps}"
    );
    // Compare an unchanged representative buffer on the same default budget.
    let comparison = "let data = [255u8; 64]; equals(array_as_slice(data), array_as_slice(data))";
    let expression = format!("if {{ {comparison} }} {{ 1 }} else {{ 0 }}");
    let (old_value, old_compare_steps) = steps(&legacy, &expression);
    let (new_value, new_compare_steps) = steps(LIBRARY, &expression);
    assert_eq!(old_value, new_value);
    assert!(
        old_compare_steps > new_compare_steps * 10,
        "old={old_compare_steps}, new={new_compare_steps}"
    );
    println!("byte 255: {old_steps} -> {max_steps} steps; position 65536: {large_steps} steps; 64-byte comparison: {old_compare_steps} -> {new_compare_steps}");
}
