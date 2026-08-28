use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = include_str!("owned_byte_variant_v1_fixture.spx");

fn source_file(source: &str) -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-owned-variant-interpreter-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn interpret(source: &str, function: &str) -> interpreter::Interpretation {
    let path = source_file(source);
    let result =
        interpreter::interpret(&path, function, &[], &InterpreterOptions::default()).unwrap();
    std::fs::remove_file(path).unwrap();
    result
}

fn outcome(document: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(document).unwrap()["payload"]["outcome"].clone()
}

#[test]
fn authored_and_prelude_owned_byte_variants_execute_with_borrow_then_own() {
    let parsed = parse(SOURCE, "owned-byte-variant-interpreter-v1.spx").unwrap();
    assert!(verify::verify(&parsed).is_empty());
    hir::validate(&hir::resolve(&parsed).unwrap()).unwrap();

    for _ in 0..4 {
        let result = interpret(SOURCE, "app.main");
        assert!(result.returned);
        assert_eq!(outcome(&result.envelope)["value"], "132");
        interpreter::verify_envelope(&result.envelope).unwrap();
    }
}

#[test]
fn active_bytes_payload_settles_when_the_selected_arm_fails() {
    let source = SOURCE.replace(
        "@id(\"app.main\")\nfn main() -> i64 {",
        r#"@id("sum.fail-after-own")
fn fail_after_own() -> i64 {
    let source = [7u8, 9u8];
    let value = make(array_as_slice(source));
    match own value {
        Choice::None {} => 0,
        Choice::Data { payload, marker } =>
            if byte_len(bytes_as_slice(payload)) == 2usize {
                9223372036854775807 + 1
            } else {
                marker
            },
        Choice::Error { code } => code,
    }
}

@id("app.main")
fn main() -> i64 {"#,
    );
    for _ in 0..3 {
        let result = interpret(&source, "sum.fail-after-own");
        assert!(!result.returned);
        assert_eq!(outcome(&result.envelope)["kind"], "failed");
        interpreter::verify_envelope(&result.envelope).unwrap();
    }
}

#[test]
fn inactive_bytes_cases_execute_without_materializing_an_owned_token() {
    let source = SOURCE.replace(
        "@id(\"app.main\")\nfn main() -> i64 {",
        r#"@id("sum.inactive-cases")
fn inactive_cases() -> i64 {
    consume(Choice::None {}) + consume(Choice::Error { code: 42 })
}

@id("app.main")
fn main() -> i64 {"#,
    );
    let result = interpret(&source, "sum.inactive-cases");
    assert!(result.returned);
    assert_eq!(outcome(&result.envelope)["value"], "42");
    interpreter::verify_envelope(&result.envelope).unwrap();
}

#[test]
fn aggregate_interpreter_cli_selection_remains_closed() {
    let path = source_file(SOURCE);
    let diagnostic =
        interpreter::interpret(&path, "sum.make", &[], &InterpreterOptions::default()).unwrap_err();
    std::fs::remove_file(path).unwrap();
    assert_eq!(diagnostic[0].code, "SPX-F102");
}
