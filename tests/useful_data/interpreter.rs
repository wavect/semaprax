use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.useful_data_interpreter;

@id("bytes.forward")
fn forward(value: own Bytes) -> Bytes {
    value
}

@id("array.length")
fn array_length(value: [u8; 4]) -> usize {
    let view = array_as_slice(value);
    byte_len(view)
}

@id("bytes.from-array")
fn from_array() -> i64 {
    let data = [0u8, 255u8, 7u8, 0u8];
    let view = array_as_slice(data);
    let copied = bytes_copy(view);
    let forwarded = forward(copied);
    let copied_view = bytes_as_slice(forwarded);
    match byte_get(copied_view, 1usize) {
        Option::Some { value: byte } => if byte == 255u8 { 41 } else { 2 },
        Option::None {} => 3,
    }
}

@id("bytes.empty")
fn empty_copies_are_values() -> i64 {
    let first_array = [0u8; 0];
    let first_view = array_as_slice(first_array);
    let first = bytes_copy(first_view);
    let second_array = [];
    let second_view = array_as_slice(second_array);
    let second = bytes_copy(second_view);
    let first_bytes = bytes_as_slice(first);
    let second_bytes = bytes_as_slice(second);
    if byte_len(first_bytes) == 0usize && byte_len(second_bytes) == 0usize {
        1
    } else {
        0
    }
}

@id("bytes.text")
fn text_byte(value: borrow str) -> u8 {
    let view = str_as_bytes(value);
    match byte_get(view, 2usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    }
}

@id("bytes.mixed-roots")
fn mixed_roots(text: borrow str, bytes: borrow Slice<u8>) -> usize {
    let text_view = str_as_bytes(text);
    byte_len(text_view) + byte_len(bytes)
}

@id("app.main")
fn main() -> i64 {
    if array_length([9u8; 4]) == 4usize {
        from_array() + empty_copies_are_values()
    } else {
        0
    }
}
"#;

fn source_file() -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-useful-data-interpreter-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    path
}

fn returned_value(envelope: &str) -> String {
    let document: serde_json::Value = serde_json::from_str(envelope).unwrap();
    document["payload"]["outcome"]["value"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn arrays_owned_bytes_and_views_execute_through_the_verified_hir() {
    let program = parse(SOURCE, "useful-data-interpreter.spx").unwrap();
    assert!(verify::verify(&program).is_empty());
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();

    let path = source_file();
    let result =
        interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
    assert!(result.returned);
    assert_eq!(returned_value(&result.envelope), "42");
    interpreter::verify_envelope(&result.envelope).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn str_as_bytes_preserves_utf8_bytes_including_embedded_nul() {
    let path = source_file();
    let result = interpreter::interpret(
        &path,
        "bytes.text",
        &["\"A\\u0000Z\"".to_owned()],
        &InterpreterOptions::default(),
    )
    .unwrap();
    assert!(result.returned);
    assert_eq!(returned_value(&result.envelope), "90u8");
    interpreter::verify_envelope(&result.envelope).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn text_and_slice_roots_share_one_exact_invocation_budget() {
    let path = source_file();
    // The canonical envelope intentionally echoes admitted arguments, so this
    // boundary test raises only the output carrier budget; it does not widen
    // the language's shared 65,536-byte external-root limit.
    let options = InterpreterOptions::new(512 * 1024, interpreter::DEFAULT_MAX_STEPS).unwrap();
    let text = serde_json::to_string(&"a".repeat(32_768)).unwrap();
    let exact_bytes = serde_json::to_string(&vec![7u8; 32_768]).unwrap();
    let exact = interpreter::interpret(
        &path,
        "bytes.mixed-roots",
        &[text.clone(), exact_bytes],
        &options,
    )
    .unwrap();
    assert_eq!(returned_value(&exact.envelope), "65536usize");

    let overflow_bytes = serde_json::to_string(&vec![7u8; 32_769]).unwrap();
    let overflow = interpreter::interpret(
        &path,
        "bytes.mixed-roots",
        &[text, overflow_bytes],
        &options,
    )
    .unwrap_err();
    assert_eq!(overflow.first().map(|item| item.code), Some("SPX-F105"));
    std::fs::remove_file(path).unwrap();
}
