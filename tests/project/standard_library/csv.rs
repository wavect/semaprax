//! Focused interpreter regressions for CSV logical-record framing and decoded
//! field copies. Package conformance supplies the native and Core Wasm lanes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const CURSORS: &str = include_str!("../../../std/io/src/io.spx");
const CSV: &str = include_str!("../../../std/data-csv/src/csv.spx");

fn source(main: &str) -> String {
    let cursors = CURSORS.replacen("module std.io;", "module app;", 1);
    let csv = CSV
        .lines()
        .filter(|line| !line.starts_with("module ") && !line.starts_with("use "))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{cursors}\n{csv}\n{main}\n")
}

fn interpretation(main: &str) -> interpreter::Interpretation {
    let program = parse(&source(main), "std-csv-reader.spx").expect("CSV fixture parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "CSV fixture source diagnostics: {diagnostics:?}"
    );
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "std-csv-reader.spx").expect("canonical CSV reparses");
    hir::resolve(&reparsed).expect("checked CSV fixture resolves");
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path: PathBuf = std::env::temp_dir().join(format!(
        "semaprax-std-csv-reader-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, canonical).unwrap();
    let result = interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
        .expect("CSV interpreter entry is admitted");
    interpreter::verify_envelope(&result.envelope).expect("interpreter envelope is canonical");
    std::fs::remove_file(path).unwrap();
    result
}

fn returns_zero(main: &str) {
    let result = interpretation(main);
    assert!(result.returned, "expected return: {}", result.envelope);
    let document: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    assert_eq!(document["payload"]["outcome"]["value"].as_str(), Some("0"));
}

fn fails_contract(main: &str) {
    let result = interpretation(main);
    assert!(!result.returned, "invalid CSV operation returned");
    let document: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    assert_eq!(document["payload"]["outcome"]["kind"], "failed");
    assert_eq!(
        document["payload"]["outcome"]["status"]["class"],
        "contract"
    );
}

#[test]
fn csv_record_decoding_executes_on_all_three_backends() {
    super::run_examples_and_conformance(
        super::packages()
            .into_iter()
            .filter(|p| p.module == "std.data.csv")
            .collect(),
    );
}

#[test]
fn multiline_records_and_decoded_quotes_preserve_exact_bytes() {
    returns_zero(
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8, 44u8, 34u8, 115u8, 97u8, 121u8, 32u8, 34u8, 34u8, 104u8, 105u8, 34u8, 34u8, 13u8, 10u8, 110u8, 101u8, 120u8, 116u8, 34u8, 44u8, 13u8, 10u8, 122u8];
    let view = array_as_slice(source);
    let field = csv_record_field_next(view, 0usize, 0usize);
    let written = csv_record_field_into(view, 0usize, field, Writer { data: bytes_set(bytes_zeroed(15usize), 14usize, 88u8), position: 0usize });
    let cursor = writer_position(written);
    let bytes = writer_finish(written);
    let bytes_view = bytes_as_slice(bytes);
    let quote = match byte_get(bytes_view, 4usize) { Option::Some { value } => value, Option::None {} => 0u8, };
    let cr = match byte_get(bytes_view, 8usize) { Option::Some { value } => value, Option::None {} => 0u8, };
    let lf = match byte_get(bytes_view, 9usize) { Option::Some { value } => value, Option::None {} => 0u8, };
    let suffix = match byte_get(bytes_view, 14usize) { Option::Some { value } => value, Option::None {} => 0u8, };
    let expected = [115u8, 97u8, 121u8, 32u8, 34u8, 104u8, 105u8, 34u8, 13u8, 10u8, 110u8, 101u8, 120u8, 116u8];
    let mut index = 0usize;
    let mut exact = cursor == 14usize;
    while exact && index < 14usize {
        let actual = match byte_get(bytes_view, index) { Option::Some { value } => value, Option::None {} => 0u8, };
        let wanted = match byte_get(array_as_slice(expected), index) { Option::Some { value } => value, Option::None {} => 1u8, };
        exact = actual == wanted;
        index = index + 1usize;
        exact && index < 14usize
    }
    let framed = csv_record_end(view, 0usize) == 21usize && csv_record_next(view, 0usize) == 23usize && csv_record_end(view, 23usize) == 24usize;
    if framed && exact && quote == 34u8 && cr == 13u8 && lf == 10u8 && suffix == 88u8 { 0 } else { 1 }
}
"#,
    );
}

#[test]
fn retained_multiline_oracle_decodes_every_row_and_trailing_empty_field() {
    let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/project/standard_library/fixtures");
    let input = std::fs::read(fixture_root.join("csv-multiline.csv")).unwrap();
    let expected: Vec<Vec<String>> = serde_json::from_slice(
        &std::fs::read(fixture_root.join("csv-multiline.expected.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(expected[0], ["name", "note", "tail"]);
    assert_eq!(expected[1][1], "line one\r\nline two");
    assert_eq!(expected[2], ["beta", "say \"hi\"", ""]);
    let array = |bytes: &[u8]| {
        if bytes.is_empty() {
            "[0u8; 0]".to_owned()
        } else {
            format!(
                "[{}]",
                bytes
                    .iter()
                    .map(|byte| format!("{byte}u8"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    };
    let mut body = format!(
        r#"
@id("app.equal")
fn equal(left: borrow Slice<u8>, right: borrow Slice<u8>) -> bool
{{
    let mut index = 0usize;
    let mut same = byte_len(left) == byte_len(right);
    while same && index < byte_len(left) {{
        let a = match byte_get(left, index) {{ Option::Some {{ value }} => value, Option::None {{}} => 0u8, }};
        let b = match byte_get(right, index) {{ Option::Some {{ value }} => value, Option::None {{}} => 1u8, }};
        same = a == b;
        index = index + 1usize;
        same && index < byte_len(left)
    }}
    same
}}
@id("app.main")
fn main() -> i64
{{
    let input = {};
    let view = array_as_slice(input);
    let mut record = 0usize;
    let mut field = 0usize;
    let mut exact = true;
"#,
        array(&input)
    );
    for (row_index, row) in expected.iter().enumerate() {
        body.push_str(
            "    exact = exact && csv_record_available(view, record);\n    field = record;\n",
        );
        for (field_index, value) in row.iter().enumerate() {
            let label = format!("{row_index}_{field_index}");
            let bytes = value.as_bytes();
            body.push_str(&format!(
                "    let expected_{label} = {};\n    let written_{label} = csv_record_field_into(view, record, field, Writer {{ data: bytes_zeroed({}usize), position: 0usize }});\n    let cursor_{label} = writer_position(written_{label});\n    let decoded_{label} = writer_finish(written_{label});\n    exact = exact && csv_record_field_present(view, record, field) && cursor_{label} == {}usize && equal(bytes_as_slice(decoded_{label}), array_as_slice(expected_{label}));\n",
                array(bytes),
                bytes.len(),
                bytes.len()
            ));
            let has_next = field_index + 1 < row.len();
            body.push_str(&format!(
                "    exact = exact && csv_record_field_has_next(view, record, field) == {has_next};\n"
            ));
            if has_next {
                body.push_str("    field = csv_record_field_next(view, record, field);\n");
            }
        }
        body.push_str("    record = csv_record_next(view, record);\n");
    }
    body.push_str(
        "    exact = exact && record == byte_len(view) && !csv_record_available(view, record);\n    if exact { 0 } else { 1 }\n}\n",
    );
    returns_zero(&body);
}

#[test]
fn decoded_copy_rejects_short_capacity_malformed_input_and_bad_offsets() {
    for main in [
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8, 98u8, 99u8];
    let written = csv_record_field_into(array_as_slice(source), 0usize, 1usize, writer_from_bytes(bytes_zeroed(2usize)));
    if writer_position(written) == 2usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [34u8, 97u8, 34u8, 34u8, 98u8, 34u8];
    let written = csv_record_field_into(array_as_slice(source), 0usize, 0usize, Writer { data: bytes_zeroed(3usize), position: 1usize });
    if writer_position(written) == 4usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [34u8, 97u8];
    let written = csv_record_field_into(array_as_slice(source), 0usize, 0usize, writer_from_bytes(bytes_zeroed(2usize)));
    if writer_position(written) == 1usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let source = [97u8];
    if csv_record_end(array_as_slice(source), 2usize) == 1usize { 0 } else { 1 }
}
"#,
    ] {
        fails_contract(main);
    }
}
