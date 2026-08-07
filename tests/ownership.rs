use std::path::Path;

use semaprax::{graph, parse, verify};

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("ownership.spx")).unwrap();
    verify::verify(&program)
}

const HEADER: &str = r#"
module test.ownership;
@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}
@id("buffer.inspect")
fn inspect(buffer: borrow Buffer) -> i64 { 1 }
@id("buffer.consume")
fn consume(buffer: own Buffer) -> i64 { inspect(buffer) }
"#;

const MAIN: &str = r#"
@id("app.main")
fn main() -> i64 { 42 }
"#;

#[test]
fn borrow_then_move_is_valid() {
    let source = format!(
        "{HEADER}\n@id(\"buffer.pipeline\")\nfn pipeline(buffer: own Buffer) -> i64 {{ inspect(buffer) + consume(buffer) }}\n{MAIN}"
    );
    assert!(diagnostics(&source).is_empty());
}

#[test]
fn use_after_move_is_rejected() {
    let source = format!(
        "{HEADER}\n@id(\"buffer.bad\")\nfn bad(buffer: own Buffer) -> i64 {{ consume(buffer) + inspect(buffer) }}\n{MAIN}"
    );
    let found = diagnostics(&source);
    assert!(found.iter().any(|item| item.code == "SPX-O101"));
}

#[test]
fn borrowed_resource_cannot_be_transferred() {
    let source = format!(
        "{HEADER}\n@id(\"buffer.bad\")\nfn bad(buffer: borrow Buffer) -> i64 {{ consume(buffer) }}\n{MAIN}"
    );
    let found = diagnostics(&source);
    assert!(found.iter().any(|item| item.code == "SPX-O102"));
}

#[test]
fn resource_boundary_requires_an_ownership_mode() {
    let source =
        format!("{HEADER}\n@id(\"buffer.bad\")\nfn bad(buffer: Buffer) -> i64 {{ 1 }}\n{MAIN}");
    let found = diagnostics(&source);
    assert!(found.iter().any(|item| item.code == "SPX-O001"));
}

#[test]
fn borrowed_resource_cannot_escape_as_owned() {
    let source = format!(
        "{HEADER}\n@id(\"buffer.leak\")\nfn leak(buffer: borrow Buffer) -> Buffer {{ buffer }}\n{MAIN}"
    );
    let found = diagnostics(&source);
    assert!(found.iter().any(|item| item.code == "SPX-O104"));
}

#[test]
fn contracts_cannot_consume_resources() {
    let source = format!(
        "{HEADER}\n@id(\"buffer.contract\")\nfn guarded(buffer: own Buffer) -> i64 ensures consume(buffer) == 1 {{ 1 }}\n{MAIN}"
    );
    let found = diagnostics(&source);
    assert!(found.iter().any(|item| item.code == "SPX-O105"));
}

#[test]
fn graph_exposes_resource_and_parameter_ownership() {
    let source = format!("{HEADER}\n{MAIN}");
    let program = parse(&source, Path::new("ownership.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let json = graph::to_json(&program).unwrap();
    assert!(json.contains("\"kind\":\"resource\""));
    assert!(json.contains("\"id\":\"buffer.type\""));
    assert!(json.contains("\"drop\":\"buffer.type.drop\""));
    assert!(json.contains("\"id\":\"buffer.type.drop\",\"kind\":\"resource_drop\""));
    assert!(json.contains("\"strategy\":\"trivial\""));
    assert!(json.contains("\"ownership_mode\":\"borrow\""));
    assert!(json.contains("\"ownership_mode\":\"own\""));
}
