use super::nested_owned::{
    graph_schema_includes_loans, graph_schema_includes_modern_composite_facts,
    graph_schema_includes_projected_provenance, reject_nested_native_flags,
};

#[test]
fn nested_cleanup_versions_are_closed_and_legacy_selection_is_unchanged() {
    assert!(reject_nested_native_flags(false, false).is_ok());
    assert!(reject_nested_native_flags(false, true).is_ok());
    assert!(reject_nested_native_flags(true, false).is_ok());
    assert!(reject_nested_native_flags(true, true).is_err());

    for schema in ["semaprax.graph.v26", "semaprax.graph.v27"] {
        assert!(graph_schema_includes_modern_composite_facts(schema));
    }
    assert!(!graph_schema_includes_loans("semaprax.graph.v26"));
    assert!(graph_schema_includes_loans("semaprax.graph.v27"));
    assert!(!graph_schema_includes_projected_provenance(
        "semaprax.graph.v26"
    ));
    assert!(graph_schema_includes_projected_provenance(
        "semaprax.graph.v27"
    ));
    for schema in [
        "semaprax.graph.v25",
        "semaprax.graph.v28",
        "semaprax.graph.v27 ",
    ] {
        assert!(!graph_schema_includes_modern_composite_facts(schema));
        assert!(!graph_schema_includes_loans(schema));
        assert!(!graph_schema_includes_projected_provenance(schema));
    }
}
