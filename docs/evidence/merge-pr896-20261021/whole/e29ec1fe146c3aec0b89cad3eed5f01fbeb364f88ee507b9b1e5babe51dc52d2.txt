#[test]
fn selected_session_complete_source_inverses_and_old_layout_definitions_stay_bound() {
    selected_session_layout_mirror_is_derived_from_complete_historical_definitions();
    let rows: serde_json::Value =
        serde_json::from_str(super::metadata(super::Fixture::Selected)).unwrap();
    let files = [
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt"))),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt"))),
        ("src/cleanup_plan/replay/selected_context.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__selected_context.rs.before.txt"))),
        ("src/cleanup_plan/replay/selected_entry.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__selected_entry.rs.before.txt"))),
        ("src/cleanup_plan/replay/selected_entry/paid.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/paid.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__selected_entry__paid.rs.before.txt"))),
        ("src/cleanup_plan/replay/selected_entry/tests.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__selected_entry__tests.rs.before.txt"))),
        ("src/cleanup_plan/replay/selected_entry/tests/structural_errors.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests/structural_errors.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__selected_entry__tests__structural_errors.rs.before.txt"))),
        ("src/cleanup_plan/replay/selected_owner.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_owner.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__selected_owner.rs.before.txt"))),
        ("src/cleanup_plan/replay/selected_profile.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__selected_profile.rs.before.txt"))),
        ("src/cleanup_plan/replay/semantic_refusal.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/semantic_refusal.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__semantic_refusal.rs.before.txt"))),
        ("src/cleanup_plan/replay/semantic_refusal/tests.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/semantic_refusal/tests.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__semantic_refusal__tests.rs.before.txt"))),
        ("tests/hir_module_boundaries.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/tests__hir_module_boundaries.rs.before.txt"))),
        ("tests/workspace/semantic_graph/source_inventory.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/tests__workspace__semantic_graph__source_inventory.rs.before.txt"))),
        ("src/cleanup_plan/replay/selected_context/paid.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_context/paid.rs")),""),
        ("src/cleanup_plan/replay/selected_owner/paid.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_owner/paid.rs")),""),
        ("src/cleanup_plan/replay/selected_profile/paid.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_profile/paid.rs")),""),
        ("src/cleanup_plan/replay/selected_entry/tests/session_errors.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests/session_errors.rs")),""),
        ("src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs")),""),
        ("src/cleanup_plan/replay/expression_facts/errors.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/errors.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__expression_facts__errors.rs.before.txt"))),
    ];
    assert_eq!(rows.as_array().unwrap().len(), files.len());
    for ((path, current, old), row) in files.into_iter().zip(rows.as_array().unwrap()) {
        assert_eq!(row["source"].as_str().unwrap(), path);
        let mut restored = current.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(restored.matches(new).count(), 1, "{path}");
            restored = restored.replacen(new, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, old, "{path}");
    }
}

fn selected_session_layout_mirror_is_derived_from_complete_historical_definitions() {
    fn body<'a>(source: &'a str, marker: &str) -> &'a str {
        let rest = &source[source.find(marker).unwrap() + marker.len()..];
        &rest[..rest.find('}').unwrap()]
    }
    fn compact(source: &str) -> String {
        source.chars().filter(|c| !c.is_whitespace()).collect()
    }
    let atom=include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-session-static-refusal-781/original/src__cleanup_plan__replay__semantic_refusal.rs.before.txt"));
    let error = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/selected-session-static-refusal-781/original/errors.rs.before.txt"
    ));
    let mirror = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cleanup_plan/replay/selected_entry/tests/session_errors.rs"
    ));
    assert_eq!(
        compact(body(atom, "enum Atom {")),
        compact(body(mirror, "enum Atom {"))
    );
    assert_eq!(
        compact(body(error, "enum EngineError {")),
        compact(body(mirror, "enum EngineError {")).replace("expression_facts::errors::", "")
    );
    assert_eq!(
        compact(body(atom, "source: ExpressionId,")),
        compact(body(mirror, "source: ExpressionId,"))
    );
}
