use super::*;
const SOURCE: &str = r#"module scoped.graph;
@id("row") record Row {@id("row.title") title:string,@id("row.marker") marker:i64,}
@id("inspect") fn inspect(values:borrow Vec<Row>,index:usize)->i64 {
 let view=str_as_bytes(vec_field<Row>(values,index,"title"));
 let marker=vec_field<Row>(values,index,"marker");
 marker+i64_from_usize(byte_len(view))
}
@id("app.main") fn main()->i64 {0}
"#;

#[test]
fn graph_binds_stable_field_and_dynamic_vector_view_without_selector_expression() {
    let parsed = crate::check(SOURCE, "scoped-graph.spx").unwrap();
    let graph = crate::graph::to_json(&parsed).unwrap();
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(document["schema"], SCHEMA);
    assert_eq!(document["scoped_vec_field_reads"]["authority"], false);
    assert!(graph.contains("\"kind\":\"vec_field_read\""));
    assert!(graph.contains("\"field\":\"row.title\",\"bytes\":true"));
    assert!(graph.contains("\"root_kind\":\"owned_vector_field\""));
    assert!(graph.contains("\"vector_field\":{\"element_type\":"));
    assert!(!graph.contains("\"kind\":\"string\",\"value\":\"title\""));
    let resolved = hir::resolve(&parsed).unwrap();
    assert_eq!(graph_schema(&resolved).unwrap(), SCHEMA);
    assert!(legacy_graph_schema(&resolved).is_err());
    assert!(crate::graph::reject_evidence_schema(SCHEMA).is_err());
    assert_eq!(crate::graph::to_json(&parsed).unwrap(), graph);
}

#[test]
fn feature_free_program_keeps_its_existing_graph_schema_and_wire() {
    let parsed =
        crate::check("module old;@id(\"app.main\") fn main()->i64 {0}", "old.spx").unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    assert!(!requires(&resolved));
    assert_eq!(
        graph_schema(&resolved).unwrap(),
        super::super::owned_nested_outcome::graph_schema(&resolved).unwrap()
    );
    let selected_functions = resolved.functions.iter().map(|f| f.id.clone()).collect();
    let selected_types = resolved.types.iter().map(|t| t.id.clone()).collect();
    let revision = crate::graph::revision(&parsed);
    let before = super::super::owned_nested_outcome::graph_json(
        &resolved,
        &revision,
        &selected_functions,
        &selected_types,
        &GraphView::Module,
    )
    .unwrap();
    assert_eq!(
        graph_json(
            &resolved,
            &revision,
            &selected_functions,
            &selected_types,
            &GraphView::Module
        )
        .unwrap(),
        before
    );
}

#[test]
fn feature_discovery_includes_deferred_closure_bodies() {
    let parsed = crate::check(
        r#"module scoped.deferred;
@id("row") record Row {@id("row.title") title:string,@id("row.marker") marker:i64,}
@id("app.main") fn main()->i64 {
 let reader=fn(offset:i64)->i64 {
  let empty=vec_with_capacity<Row>(1usize);
  let rows=vec_push<Row>(empty,Row{title:"deferred",marker:7});
  vec_field<Row>(rows,0usize,"marker")+offset
 };
 reader(0)
}
"#,
        "scoped-deferred.spx",
    )
    .unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    assert!(requires(&resolved));
    assert!(crate::codegen::native_vec::owned_leaf::program_uses_field_reads(&resolved));
    assert_eq!(graph_schema(&resolved).unwrap(), SCHEMA);
    let graph = crate::graph::to_json(&parsed).unwrap();
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(document["schema"], SCHEMA);
    assert!(graph.contains("\"kind\":\"vec_field_read\""));
}
