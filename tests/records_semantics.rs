use std::path::Path;

use semaprax::{codegen, hir, parse, verify, wasm};

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
fn moving_a_resource_field_leaves_an_available_sibling_usable() {
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

    assert!(diagnostic_codes(source).is_empty());
}

#[test]
fn same_field_parent_and_conditional_field_reuse_are_rejected() {
    let source = r#"
module test.record_partial_moves;
@id("buffer.type")
resource Buffer;
@id("envelope.type")
record Envelope {
    @id("envelope.payload") payload: Buffer,
    @id("envelope.code") code: i64,
}
@id("buffer.consume")
fn consume(value: own Buffer) -> i64 { 1 }
@id("envelope.borrow")
fn borrow_envelope(value: borrow Envelope) -> i64 { value.code }
@id("envelope.same")
fn same(value: own Envelope) -> i64 {
    consume(value.payload) + consume(value.payload)
}
@id("envelope.parent")
fn parent(value: own Envelope) -> i64 {
    consume(value.payload) + borrow_envelope(value)
}
@id("envelope.conditional")
fn conditional(flag: bool, value: own Envelope) -> i64 {
    (if flag { consume(value.payload) } else { 0 }) + consume(value.payload)
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

    let codes = diagnostic_codes(source);
    assert_eq!(codes.iter().filter(|code| **code == "SPX-O109").count(), 2);
    assert_eq!(codes.iter().filter(|code| **code == "SPX-O110").count(), 1);
    assert!(!codes.contains(&"SPX-O101"));
    assert!(!codes.contains(&"SPX-O107"));
}

#[test]
fn different_branch_field_moves_definitely_invalidate_only_the_parent() {
    let source = r#"
module test.record_split_moves;
@id("buffer.type")
resource Buffer;
@id("pair.type")
record Pair {
    @id("pair.left") left: Buffer,
    @id("pair.right") right: Buffer,
}
@id("buffer.consume")
fn consume(value: own Buffer) -> i64 { 1 }
@id("pair.inspect")
fn inspect(value: borrow Pair) -> i64 { 1 }
@id("pair.parent")
fn parent(flag: bool, value: own Pair) -> i64 {
    let moved = if flag { consume(value.left) } else { consume(value.right) };
    moved + inspect(value)
}
@id("pair.left_after")
fn left_after(flag: bool, value: own Pair) -> i64 {
    let moved = if flag { consume(value.left) } else { consume(value.right) };
    moved + consume(value.left)
}
@id("pair.right_after")
fn right_after(flag: bool, value: own Pair) -> i64 {
    let moved = if flag { consume(value.left) } else { consume(value.right) };
    moved + consume(value.right)
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

    let codes = diagnostic_codes(source);
    assert_eq!(codes.iter().filter(|code| **code == "SPX-O109").count(), 1);
    assert_eq!(codes.iter().filter(|code| **code == "SPX-O110").count(), 2);
}

#[test]
fn lazy_field_moves_join_without_poisoning_sibling_fields() {
    let source = r#"
module test.record_lazy_move;
@id("buffer.type")
resource Buffer;
@id("envelope.type")
record Envelope {
    @id("envelope.payload") payload: Buffer,
    @id("envelope.ok") ok: bool,
}
@id("buffer.consume")
fn consume(value: own Buffer) -> bool { true }
@id("envelope.good")
fn good(flag: bool, value: own Envelope) -> bool {
    let moved = flag && consume(value.payload);
    moved || value.ok
}
@id("envelope.bad")
fn bad(flag: bool, value: own Envelope) -> bool {
    (flag && consume(value.payload)) || consume(value.payload)
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

    assert_eq!(
        diagnostic_codes(source)
            .iter()
            .filter(|code| **code == "SPX-O110")
            .count(),
        1
    );
}

#[test]
fn borrowed_and_shared_record_fields_cannot_cross_owned_boundaries() {
    let source = r#"
module test.record_borrowed_field_move;
@id("buffer.type")
resource Buffer;
@id("envelope.type")
record Envelope { @id("envelope.payload") payload: Buffer, }
@id("buffer.consume")
fn consume(value: own Buffer) -> i64 { 1 }
@id("envelope.borrowed")
fn borrowed(value: borrow Envelope) -> i64 { consume(value.payload) }
@id("envelope.shared")
fn shared(value: shared Envelope) -> i64 { consume(value.payload) }
@id("app.main")
fn main() -> i64 { 0 }
"#;

    assert_eq!(
        diagnostic_codes(source)
            .iter()
            .filter(|code| **code == "SPX-O108")
            .count(),
        2
    );
}

#[test]
fn borrowed_resource_record_construction_fails_before_hir_resolution() {
    let source = r#"
module test.record_borrowed_constructor;
@id("handle.type")
resource Handle;
@id("envelope.type")
record Envelope { @id("envelope.handle") handle: Handle, }
@id("envelope.wrap")
fn wrap(handle: borrow Handle) -> Envelope { Envelope { handle } }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("borrowed-constructor.spx")).unwrap();
    let verifier = verify::verify(&program);
    let analysis = hir::analyze(&program);

    assert_eq!(verifier.len(), 1);
    assert_eq!(verifier[0].code, "SPX-O108");
    assert_eq!(
        verifier.iter().map(|item| item.json()).collect::<Vec<_>>(),
        analysis
            .diagnostics
            .iter()
            .map(|item| item.json())
            .collect::<Vec<_>>()
    );
    assert!(analysis.resolved.is_none());
}

#[test]
fn executable_backends_fail_closed_until_record_cleanup_and_layout_land() {
    let source = r#"
module test.record_backend_gate;
@id("platform.handle")
resource Handle;
@id("geometry.point")
record Point { @id("geometry.point.x") x: i64, }
@id("app.main")
fn main() -> i64 { Point { x: 42 }.x }
"#;
    let program = parse(source, Path::new("record-backend-gate.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(codegen::emit_c(&program).unwrap_err().code, "SPX-B103");
    assert_eq!(wasm::emit_module(&program).unwrap_err().code, "SPX-W110");

    let resolved = hir::resolve(&program).unwrap();
    assert_eq!(codegen::emit_hir_c(&resolved).unwrap_err().code, "SPX-B103");
    assert_eq!(
        wasm::emit_resolved_module(&resolved).unwrap_err().code,
        "SPX-W110"
    );
}
