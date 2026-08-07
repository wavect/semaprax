use std::path::Path;

use semaprax::{codegen, parse, verify, wasm};

fn diagnostic_codes(source: &str) -> Vec<&'static str> {
    let program = parse(source, Path::new("records-semantics.spx")).unwrap();
    verify::verify(&program)
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn nested_scalar_records_construct_and_project_by_declared_type() {
    let source = r#"
module test.records;
@id("geometry.point")
record Point {
    @id("geometry.point.x") x: i64,
    @id("geometry.point.y") y: i64,
}
@id("geometry.line")
record Line {
    @id("geometry.line.start") start: Point,
    @id("geometry.line.end") end: Point,
}
@id("app.main")
fn main() -> i64 {
    let point = Point { y: 22, x: 20 };
    let line = Line { end: Point { x: 0, y: 0 }, start: point };
    line.start.x + line.start.y
}
"#;

    assert!(diagnostic_codes(source).is_empty());
}

#[test]
fn constructor_diagnostics_preserve_source_then_declaration_order() {
    let source = r#"
module test.record_errors;
@id("geometry.point")
record Point {
    @id("geometry.point.x") x: i64,
    @id("geometry.point.flag") flag: bool,
}
@id("app.main")
fn main() -> i64 {
    let point = Point { missing: 1, x: true, x: 2 };
    point.x
}
"#;

    assert_eq!(
        diagnostic_codes(source),
        ["SPX-T212", "SPX-T215", "SPX-T212", "SPX-T213"]
    );
}

#[test]
fn direct_and_indirect_by_value_record_recursion_are_rejected() {
    let direct = r#"
module test.direct_record_cycle;
@id("cycle.node")
record Node { @id("cycle.node.next") next: Node, }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert!(diagnostic_codes(direct).contains(&"SPX-T217"));

    let indirect = r#"
module test.indirect_record_cycle;
@id("cycle.left")
record Left { @id("cycle.left.right") right: Right, }
@id("cycle.right")
record Right { @id("cycle.right.left") left: Left, }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert!(diagnostic_codes(indirect).contains(&"SPX-T217"));
}

#[test]
fn resource_fields_use_a_conservative_whole_record_move_until_partial_places_land() {
    let source = r#"
module test.record_resource_move;
@id("buffer.type")
resource Buffer;
@id("envelope.type")
record Envelope {
    @id("envelope.payload") payload: Buffer,
    @id("envelope.code") code: i64,
}
@id("buffer.consume")
fn consume(value: own Buffer) -> i64 { 1 }
@id("envelope.inspect")
fn inspect(value: own Envelope) -> i64 { consume(value.payload) + value.code }
@id("app.main")
fn main() -> i64 { 0 }
"#;

    assert!(diagnostic_codes(source).contains(&"SPX-O101"));
}

#[test]
fn executable_backends_fail_closed_until_record_cleanup_and_layout_land() {
    let source = r#"
module test.record_backend_gate;
@id("geometry.point")
record Point { @id("geometry.point.x") x: i64, }
@id("app.main")
fn main() -> i64 { Point { x: 42 }.x }
"#;
    let program = parse(source, Path::new("record-backend-gate.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(codegen::emit_c(&program).unwrap_err().code, "SPX-B103");
    assert_eq!(wasm::emit_module(&program).unwrap_err().code, "SPX-W110");
}
