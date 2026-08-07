use std::path::Path;

use semaprax::ast::{ExprKind, Statement, Type, TypeDeclarationKind};
use semaprax::{format, parse};

const RECORDS: &str = r#"
module examples.records;
permit { storage.read, network.send }

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
    let x = 40;
    let point = Point { y: 2, x };
    let line = Line { start: point, end: Point { x: 0, y: 0 } };
    line.start.x
}
"#;

const CANONICAL: &str = r#"module examples.records;

permit { storage.read, network.send }

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
fn main() -> i64
{
    let x = 40;
    let point = Point { y: 2, x: x };
    let line = Line { start: point, end: Point { x: 0, y: 0 } };
    line.start.x
}
"#;

#[test]
fn records_round_trip_with_stable_member_ids_and_exact_canonical_source() {
    let program = parse(RECORDS, Path::new("records.spx")).unwrap();
    assert_eq!(program.module, "examples.records");
    assert_eq!(program.permits, ["storage.read", "network.send"]);
    assert_eq!(program.types.len(), 2);

    let point = &program.types[0];
    assert_eq!(point.stable_id, "geometry.point");
    let TypeDeclarationKind::Record { fields } = &point.kind else {
        panic!("Point should parse as a record")
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].stable_id, "geometry.point.x");
    assert_eq!(fields[0].ty, Type::I64);
    assert_eq!(fields[1].stable_id, "geometry.point.y");

    let canonical = format::canonical(&program);
    assert_eq!(canonical, CANONICAL);
    let reparsed = parse(&canonical, Path::new("records-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn construction_preserves_initializer_order_expands_shorthand_and_chains_projection() {
    let program = parse(RECORDS, Path::new("records.spx")).unwrap();
    let ExprKind::Block { statements, tail } = &program.functions[0].body.kind else {
        panic!("function body should be a block")
    };

    let Statement::Let { value, .. } = &statements[1];
    let ExprKind::ConstructRecord { fields, .. } = &value.kind else {
        panic!("local should contain a record constructor")
    };
    assert_eq!(fields[0].name, "y");
    assert_eq!(fields[1].name, "x");
    assert!(matches!(&fields[1].value.kind, ExprKind::Var(name) if name == "x"));

    let ExprKind::Project { base, field, .. } = &tail.kind else {
        panic!("tail should be a projection")
    };
    assert_eq!(field, "x");
    assert!(matches!(
        &base.kind,
        ExprKind::Project { field, .. } if field == "start"
    ));
}

#[test]
fn dotted_names_remain_qualified_while_expression_dots_are_projections() {
    let source = r#"
module ecosystem.example;
permit { filesystem.read }
record Box { value: ecosystem.Value, }
fn main() -> i64 { if true { 1 } else { Box { value: item }.value } }
"#;
    let program = parse(source, Path::new("qualified.spx")).unwrap();
    assert_eq!(program.module, "ecosystem.example");
    assert_eq!(program.permits, ["filesystem.read"]);
    let TypeDeclarationKind::Record { fields } = &program.types[0].kind else {
        panic!("Box should parse as a record")
    };
    assert_eq!(fields[0].ty, Type::Named("ecosystem.Value".to_owned()));
}

#[test]
fn malformed_record_syntax_has_stable_parser_diagnostics() {
    let missing_field_comma = "module bad; record Point { x: i64 } fn main() -> i64 { 0 }";
    assert_eq!(
        parse(missing_field_comma, Path::new("missing-field-comma.spx"))
            .unwrap_err()
            .code,
        "SPX-P106"
    );

    let missing_initializer_close =
        "module bad; record Point { x: i64, } fn main() -> i64 { Point { x: 1 }";
    assert_eq!(
        parse(
            missing_initializer_close,
            Path::new("missing-initializer-close.spx")
        )
        .unwrap_err()
        .code,
        "SPX-P106"
    );

    let missing_projection_field =
        "module bad; record Point { x: i64, } fn main() -> i64 { Point { x: 1 }. }";
    assert_eq!(
        parse(
            missing_projection_field,
            Path::new("missing-projection-field.spx")
        )
        .unwrap_err()
        .code,
        "SPX-P105"
    );
}
