//! Focused source, graph, and cross-backend regressions for the bundled
//! `std.encoding.base64` package: padded encoding is a pull-based digit
//! accessor over a borrowed view, with no buffer and no allocation.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, graph, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const ENCODING: &str = include_str!("../../../std/encoding/src/encoding.spx");
const BASE64: &str = include_str!("../../../std/encoding-base64/src/base64.spx");

/// One checked module holds both libraries: the digit table the package
/// imports across its dependency, and the padded encoder itself.
fn source(main: &str) -> String {
    let table = ENCODING.replacen("module std.encoding;", "module app;", 1);
    let encoder: String = BASE64
        .lines()
        .filter(|line| !line.starts_with("module ") && !line.starts_with("use "))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{table}\n{encoder}\n{main}\n")
}

fn canonical_checked(main: &str) -> String {
    let program = parse(&source(main), "std-base64.spx").expect("base64 fixture parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "base64 fixture source diagnostics: {diagnostics:?}"
    );
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "std-base64.spx").expect("canonical fixture reparses");
    assert_eq!(format::canonical(&reparsed), canonical);
    hir::resolve(&reparsed).expect("checked base64 fixture resolves");
    canonical
}

fn interpretation(main: &str) -> interpreter::Interpretation {
    let canonical = canonical_checked(main);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path: PathBuf = std::env::temp_dir().join(format!(
        "semaprax-std-base64-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, &canonical).expect("writes temporary checked source");
    let result = interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
        .expect("base64 interpreter entry is admitted");
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
    assert!(!result.returned, "out-of-range digit request returned");
    let document: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    let outcome = &document["payload"]["outcome"];
    assert_eq!(outcome["kind"], "failed", "{document}");
    assert_eq!(outcome["status"]["class"], "contract", "{document}");
}

#[test]
fn base64_encoding_executes_on_all_three_backends() {
    super::run_examples_and_conformance(
        super::packages()
            .into_iter()
            .filter(|p| p.module == "std.encoding.base64")
            .collect(),
    );
}

#[test]
fn base64_digits_are_positional_and_padding_is_exact() {
    // "Man" is the canonical three-byte group, "M" the one-byte residue, and
    // digits may be pulled out of order because nothing is buffered.
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let three = [77u8, 97u8, 110u8];
    let full = array_as_slice(three);
    let exact = base64_byte(full, 3usize) == 117 && base64_byte(full, 0usize) == 84 && base64_byte(full, 1usize) == 87 && base64_byte(full, 2usize) == 70;
    let one = [77u8];
    let short = array_as_slice(one);
    let padded = base64_byte(short, 0usize) == 84 && base64_byte(short, 1usize) == 81 && base64_byte(short, 2usize) == 61 && base64_byte(short, 3usize) == 61;
    let sized = base64_len(0usize) == 0usize && base64_len(1usize) == 4usize && base64_len(3usize) == 4usize && base64_len(4usize) == 8usize;
    let sourced = byte_at_or_zero(full, 2usize) == 110 && byte_at_or_zero(full, 7usize) == 0;
    if exact && padded && sized && sourced { 0 } else { 1 }
}
"#,
        "0",
    );
}

#[test]
fn base64_rejects_a_digit_beyond_the_padded_length() {
    for main in [
        // One byte encodes to exactly four digits; the fifth is out of range.
        r#"
@id("app.main")
fn main() -> i64
{
    let one = [77u8];
    let view = array_as_slice(one);
    if base64_byte(view, 4usize) == 61 { 0 } else { 1 }
}
"#,
        // An empty input has no digits at all.
        r#"
@id("app.main")
fn main() -> i64
{
    let empty = bytes_zeroed(0usize);
    let view = bytes_as_slice(empty);
    if base64_byte(view, 0usize) == 61 { 0 } else { 1 }
}
"#,
    ] {
        fails(main);
    }
}

#[test]
fn base64_graph_keeps_the_borrowed_view_out_of_every_owned_inventory() {
    let canonical = canonical_checked(
        r#"
@id("app.main")
fn main() -> i64
{
    let three = [77u8, 97u8, 110u8];
    let view = array_as_slice(three);
    if base64_len(byte_len(view)) == 4usize && base64_byte(view, 0usize) == 84 { 0 } else { 1 }
}
"#,
    );
    let program = parse(&canonical, "std-base64-graph.spx").unwrap();
    let json = graph::to_json(&program).expect("base64 graph serializes");
    graph::verify_json(&program, &json).expect("derived base64 graph independently replays");
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();
    let node = |id: &str| {
        document["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing node {id}"))
            .clone()
    };
    for id in [
        "std.encoding.base64.byte",
        "std.encoding.base64.byte_at_or_zero",
    ] {
        let accessor = node(id);
        assert_eq!(accessor["params"][0]["ownership_mode"], "borrow", "{id}");
        assert_eq!(accessor["result"]["ownership_mode"], "value", "{id}");
        assert!(
            accessor["cleanup"]["entry_state"]["live_owned_parameters"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{id} put a borrowed view in the owned inventory"
        );
    }
    let length = node("std.encoding.base64.len");
    assert_eq!(length["params"][0]["ownership_mode"], "value");
    assert_eq!(length["result"]["ownership_mode"], "value");
    // A one-byte change to the padding policy must fail replay.
    let drifted = parse(&canonical.replace("{ 61 }", "{ 62 }"), "drifted-base64.spx").unwrap();
    assert_ne!(format::canonical(&drifted), canonical);
    assert!(graph::verify_json(&drifted, &json).is_err());
}
