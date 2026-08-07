use std::path::Path;

use semaprax::hir::{
    self, DeclarationId, DeclarationKind, OwnershipMode, PlaceProjection, ResolvedExprKind,
    ResolvedType, ResolvedTypeDeclarationKind,
};
use semaprax::{codegen, parse, wasm};

const RECORDS: &str = r#"
module test.hir_records;

@id("geometry.point")
record Point {
    @id("geometry.point.x")
    x: i64,
    @id("geometry.point.y")
    y: i64,
}

@id("geometry.line")
record Line {
    @id("geometry.line.start")
    start: Point,
    @id("geometry.line.end")
    end: Point,
}

@id("app.main")
fn main() -> i64 {
    let point = Point { y: 2, x: 40 };
    let line = Line { end: Point { x: 0, y: 0 }, start: point };
    line.start.x
}
"#;

fn resolved(source: &str) -> hir::ResolvedProgram {
    let ast = parse(source, Path::new("hir-records.spx")).unwrap();
    hir::resolve(&ast).unwrap()
}

#[test]
fn records_resolve_to_stable_type_field_and_place_identities() {
    let first = resolved(RECORDS);
    let second = resolved(RECORDS);
    assert_eq!(first, second);

    let point = first
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == "geometry.point")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &point.kind else {
        panic!("Point must resolve as a record");
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].id.as_str(), "geometry.point.x");
    assert_eq!(fields[0].index, 0);
    assert_eq!(fields[1].id.as_str(), "geometry.point.y");
    assert_eq!(fields[1].index, 1);
    assert_eq!(fields[0].ty, ResolvedType::I64);

    let x = first
        .declarations
        .declaration(&fields[0].id)
        .expect("field declaration must be indexed");
    assert_eq!(x.kind, DeclarationKind::Field);
    assert_eq!(x.owner.as_ref().unwrap().as_str(), "geometry.point");
    assert_eq!(
        first
            .declarations
            .field_id(&DeclarationId::new("geometry.point"), "x")
            .unwrap(),
        &fields[0].id
    );

    let main = first
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, tail } = &main.body.kind else {
        panic!("main must resolve to a block");
    };
    let hir::ResolvedStatement::Let { value: point, .. } = &statements[0];
    let ResolvedExprKind::ConstructRecord { record, fields } = &point.kind else {
        panic!("point initializer must resolve as record construction");
    };
    assert_eq!(record.as_str(), "geometry.point");
    assert_eq!(fields[0].field.as_str(), "geometry.point.y");
    assert_eq!(fields[1].field.as_str(), "geometry.point.x");
    assert!(fields[0]
        .value
        .id
        .as_str()
        .ends_with("body.s0.value.field.0.value"));
    assert!(fields[1]
        .value
        .id
        .as_str()
        .ends_with("body.s0.value.field.1.value"));

    let ResolvedExprKind::Place(place) = &tail.kind else {
        panic!("projection chain rooted at a local must resolve as a place");
    };
    assert_eq!(place.projections.len(), 2);
    assert_eq!(
        place.projections,
        [
            PlaceProjection::Field(DeclarationId::new("geometry.line.start")),
            PlaceProjection::Field(DeclarationId::new("geometry.point.x")),
        ]
    );
    assert_eq!(tail.ty, ResolvedType::I64);
    assert_eq!(tail.ownership, OwnershipMode::Value);
}

#[test]
fn rvalue_projection_remains_explicit_and_uses_a_stable_field_id() {
    let source = r#"
module test.hir_rvalue_projection;
@id("geometry.point")
record Point {
    @id("geometry.point.x")
    x: i64,
}
@id("geometry.make")
fn make() -> Point { Point { x: 42 } }
@id("app.main")
fn main() -> i64 { make().x }
"#;
    let program = resolved(source);
    let main = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &main.body.kind else {
        panic!("main must resolve to a block");
    };
    let ResolvedExprKind::Project { base, field } = &tail.kind else {
        panic!("projection from a call result must remain explicit");
    };
    assert_eq!(field.as_str(), "geometry.point.x");
    assert!(matches!(
        &base.kind,
        ResolvedExprKind::Call { callee, .. } if callee.as_str() == "geometry.make"
    ));
}

#[test]
fn recursive_record_facts_propagate_resources_and_stable_layouts() {
    let source = r#"
module test.hir_record_facts;
@id("handle.type")
resource Handle;
@id("packet.inner")
record Inner {
    @id("packet.inner.handle")
    handle: Handle,
}
@id("packet.outer")
record Outer {
    @id("packet.outer.code")
    code: i64,
    @id("packet.outer.inner")
    inner: Inner,
}
@id("packet.wrap")
fn wrap(handle: own Handle) -> Outer {
    Outer { code: 7, inner: Inner { handle } }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = resolved(source);
    let outer = ResolvedType::Nominal {
        declaration: DeclarationId::new("packet.outer"),
        arguments: Vec::new(),
    };
    let facts = program.declarations.type_facts(&outer).unwrap();
    assert!(!facts.copy);
    assert!(facts.contains_resource);
    assert!(facts.sized);
    assert!(facts.needs_drop);
    assert!(facts.layout_key.starts_with("record:12:packet.outer:2:"));
    assert!(facts.layout_key.contains("packet.outer.code"));
    assert!(facts.layout_key.contains("packet.outer.inner"));
    assert!(facts.layout_key.contains("handle.type"));

    let renamed = source
        .replace("record Outer", "record Envelope")
        .replace("-> Outer", "-> Envelope")
        .replace("Outer { code", "Envelope { code")
        .replace("code: i64", "status: i64")
        .replace("code: 7", "status: 7");
    let renamed = resolved(&renamed);
    let renamed_facts = renamed.declarations.type_facts(&outer).unwrap();
    assert_eq!(facts, renamed_facts);
}

#[test]
fn by_value_record_recursion_is_rejected_before_hir_is_exposed() {
    let source = r#"
module test.hir_recursive_record;
@id("recursive.node")
record Node {
    @id("recursive.node.next")
    next: Node,
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let ast = parse(source, Path::new("recursive-record.spx")).unwrap();
    let diagnostics = hir::resolve(&ast).unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T217"));
}

#[test]
fn hostile_record_hir_fields_constructors_and_projections_are_rejected() {
    let program = resolved(RECORDS);

    let mut forged_index = program.clone();
    let point = forged_index
        .types
        .iter_mut()
        .find(|declaration| declaration.id.as_str() == "geometry.point")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut point.kind else {
        panic!("Point must be a record");
    };
    fields[0].index = 9;
    assert_eq!(hir::validate(&forged_index).unwrap_err().code, "SPX-H006");

    let mut reordered = program.clone();
    let point = reordered
        .types
        .iter_mut()
        .find(|declaration| declaration.id.as_str() == "geometry.point")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut point.kind else {
        panic!("Point must be a record");
    };
    fields.swap(0, 1);
    assert_eq!(hir::validate(&reordered).unwrap_err().code, "SPX-H006");

    let mut foreign_constructor_field = program.clone();
    let main = foreign_constructor_field
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!("main must be a block");
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0];
    let ResolvedExprKind::ConstructRecord { fields, .. } = &mut value.kind else {
        panic!("local must be a constructor");
    };
    fields[0].field = DeclarationId::new("geometry.line.start");
    assert_eq!(
        hir::validate(&foreign_constructor_field).unwrap_err().code,
        "SPX-H006"
    );

    let mut duplicate_constructor_field = program.clone();
    let main = duplicate_constructor_field
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!("main must be a block");
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0];
    let ResolvedExprKind::ConstructRecord { fields, .. } = &mut value.kind else {
        panic!("local must be a constructor");
    };
    fields[1].field = fields[0].field.clone();
    assert_eq!(
        hir::validate(&duplicate_constructor_field)
            .unwrap_err()
            .code,
        "SPX-H006"
    );

    let mut foreign_projection = program;
    let main = foreign_projection
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut main.body.kind else {
        panic!("main must be a block");
    };
    let ResolvedExprKind::Place(place) = &mut tail.kind else {
        panic!("tail must be a place");
    };
    place.projections[1] = PlaceProjection::Field(DeclarationId::new("geometry.line.end"));
    assert_eq!(
        hir::validate(&foreign_projection).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn hostile_hir_cannot_reuse_a_field_after_forging_an_owned_call() {
    let source = r#"
module test.hir_partial_move_replay;
@id("buffer.type")
resource Buffer;
@id("envelope.type")
record Envelope { @id("envelope.payload") payload: Buffer, }
@id("buffer.inspect")
fn inspect(value: borrow Buffer) -> i64 { 1 }
@id("buffer.consume")
fn consume(value: own Buffer) -> i64 { 1 }
@id("envelope.replay")
fn replay(value: own Envelope) -> i64 {
    inspect(value.payload) + inspect(value.payload)
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let mut program = resolved(source);
    let replay = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "envelope.replay")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut replay.body.kind else {
        panic!("replay must resolve to a block");
    };
    let ResolvedExprKind::Binary { left, .. } = &mut tail.kind else {
        panic!("replay tail must be binary");
    };
    let ResolvedExprKind::Call { callee, .. } = &mut left.kind else {
        panic!("left operand must be a call");
    };
    *callee = DeclarationId::new("buffer.consume");

    assert_eq!(hir::validate(&program).unwrap_err().code, "SPX-H006");
    assert_eq!(codegen::emit_hir_c(&program).unwrap_err().code, "SPX-H006");
    assert_eq!(
        wasm::emit_resolved_module(&program).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn hostile_hir_tracks_a_definite_parent_move_across_different_branch_fields() {
    let source = r#"
module test.hir_split_move_replay;
@id("buffer.type")
resource Buffer;
@id("pair.type")
record Pair {
    @id("pair.left") left: Buffer,
    @id("pair.right") right: Buffer,
}
@id("buffer.inspect")
fn inspect_buffer(value: borrow Buffer) -> i64 { 1 }
@id("buffer.consume")
fn consume(value: own Buffer) -> i64 { 1 }
@id("pair.inspect")
fn inspect_pair(value: borrow Pair) -> i64 { 1 }
@id("pair.replay")
fn replay(flag: bool, value: own Pair) -> i64 {
    let selected = if flag { inspect_buffer(value.left) } else { inspect_buffer(value.right) };
    selected + inspect_pair(value)
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let mut program = resolved(source);
    let replay = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "pair.replay")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut replay.body.kind else {
        panic!("replay must resolve to a block");
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0];
    let ResolvedExprKind::If {
        then_branch,
        else_branch,
        ..
    } = &mut value.kind
    else {
        panic!("selected must resolve to an if expression");
    };
    for branch in [then_branch, else_branch] {
        let ResolvedExprKind::Block { tail, .. } = &mut branch.kind else {
            panic!("branch must resolve to a block");
        };
        let ResolvedExprKind::Call { callee, .. } = &mut tail.kind else {
            panic!("branch tail must be a call");
        };
        *callee = DeclarationId::new("buffer.consume");
    }

    assert_eq!(hir::validate(&program).unwrap_err().code, "SPX-H006");
    assert_eq!(codegen::emit_hir_c(&program).unwrap_err().code, "SPX-H006");
    assert_eq!(
        wasm::emit_resolved_module(&program).unwrap_err().code,
        "SPX-H006"
    );
}
