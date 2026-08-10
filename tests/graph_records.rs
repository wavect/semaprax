use std::path::Path;

use semaprax::{graph, parse};

const RECORD_GRAPH: &str = r#"
module examples.record_graph;

@id("geometry.point")
record Point {
    @id("geometry.point.x")
    x: i64,
    @id("geometry.point.y")
    y: i64,
}

@id("app.main")
fn main() -> i64 { Point { x: 20, y: 22 }.x }
"#;

const UPDATE_RECORD_GRAPH: &str = r#"
module examples.record_update_graph;

@id("geometry.point")
record Point {
    @id("geometry.point.x")
    x: i64,
    @id("geometry.point.y")
    y: i64,
}

@id("app.main")
fn main() -> i64 {
    let point = Point { x: 20, y: 0 };
    let updated = point with { y: 22 };
    updated.y
}
"#;

#[test]
fn record_graph_matches_exact_v10_snapshot() {
    let program = parse(RECORD_GRAPH, Path::new("record-graph.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert_eq!(
        format!("{json}\n"),
        include_str!("snapshots/records.graph.json")
    );
}

#[test]
fn record_graph_uses_persistent_field_ids_for_construction_update_and_projection() {
    let program = parse(UPDATE_RECORD_GRAPH, Path::new("record-update-graph.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(json.contains("\"schema\":\"semaprax.graph.v10\""));
    assert!(!json.contains("semaprax.graph.v6"));
    assert!(json.contains("\"id\":\"geometry.point\",\"kind\":\"record\",\"name\":\"Point\""));
    assert!(json.contains(
        "\"id\":\"geometry.point.x\",\"kind\":\"field\",\"name\":\"x\",\"identity_origin\":\"explicit\",\"persistent\":true,\"owner\":\"geometry.point\",\"index\":0,\"type_id\":\"i64\""
    ));
    assert!(json.contains(
        "\"kind\":\"construct_record\",\"record\":\"geometry.point\",\"fields\":[{\"field\":\"geometry.point.x\""
    ));
    assert!(json.contains("\"kind\":\"update_record\""));
    assert!(
        json.contains("\"record\":\"geometry.point\",\"fields\":[{\"field\":\"geometry.point.y\"")
    );
    assert!(json.contains("\"field\":\"geometry.point.y\""));
}

#[test]
fn record_context_closes_over_nested_field_types_but_excludes_unrelated_records() {
    let source = r#"
module test.record_context;
@id("geometry.point")
record Point { @id("geometry.point.x") x: i64, }
@id("geometry.line")
record Line { @id("geometry.line.start") start: Point, }
@id("unrelated.type")
record Unrelated { @id("unrelated.value") value: bool, }
@id("geometry.inspect")
fn inspect(line: Line, point: Point) -> i64 {
    let updated = point with { x: line.start.x };
    updated.x
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("record-context.spx")).unwrap();
    let context = graph::context_json(&program, "geometry.inspect", 0)
        .unwrap()
        .unwrap();

    for id in [
        "geometry.line",
        "geometry.line.start",
        "geometry.point",
        "geometry.point.x",
    ] {
        assert!(
            context.contains(&format!("\"id\":\"{id}\"")),
            "missing {id}"
        );
    }
    assert!(context.contains("\"type_id\":\"i64\""));
    assert!(context.contains("\"kind\":\"update_record\""));
    assert!(context.contains("\"record\":\"geometry.point\""));
    assert!(context.contains("\"field\":\"geometry.point.x\""));
    assert!(!context.contains("unrelated.type"));
    assert!(!context.contains("unrelated.value"));
}

#[test]
fn automatic_record_and_field_identities_are_marked_unstable() {
    let source = r#"
module test.automatic_record_graph;
record Point { x: i64, }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("automatic-record-graph.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(json.contains(
        "\"kind\":\"record\",\"name\":\"Point\",\"identity_origin\":\"automatic\",\"persistent\":false"
    ));
    assert!(json.contains(
        "\"kind\":\"field\",\"name\":\"x\",\"identity_origin\":\"automatic\",\"persistent\":false"
    ));
}
