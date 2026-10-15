//! Complete closed current source and independently retained originals.
#[test]
fn finite_child_cut_sources_and_complete_old_receivers_stay_exact() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../docs/evidence/selected-child-refusal-781/exact-source-inverses.json"
    ))
    .unwrap();
    assert_eq!(
        include_str!("../../paid_before_children.rs").replacen(
            "fn new_paid_before_children(",
            "fn new_paid(",
            1
        ),
        include_str!(
            "../../../../../../docs/evidence/selected-child-refusal-781/original/src__cleanup_plan__replay__selected_context__paid.rs.before.txt"
        )
    );
    let files = [
        (
            "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
            include_str!(
                "../../../../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
            ),
        ),
        (
            "src/cleanup_plan/renewal_profile.rs",
            include_str!("../../../../../../src/cleanup_plan/renewal_profile.rs"),
        ),
        (
            "src/cleanup_plan/renewal_profile/child_refusal.rs",
            include_str!("../../../../../../src/cleanup_plan/renewal_profile/child_refusal.rs"),
        ),
        (
            "src/cleanup_plan/renewal_profile/selected.rs",
            include_str!("../../../../../../src/cleanup_plan/renewal_profile/selected.rs"),
        ),
        (
            "src/cleanup_plan/renewal_profile/selected_inline.rs",
            include_str!("../../../../../../src/cleanup_plan/renewal_profile/selected_inline.rs"),
        ),
        (
            "src/cleanup_plan/renewal_profile/selected_inline/append.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/renewal_profile/selected_inline/append.rs"
            ),
        ),
        (
            "src/cleanup_plan/renewal_profile/selected_inline/cells.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/renewal_profile/selected_inline/cells.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/live_oracle/source_custody/b.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/live_oracle/source_custody/b.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/selected_context.rs",
            include_str!("../../../../../../src/cleanup_plan/replay/selected_context.rs"),
        ),
        (
            "src/cleanup_plan/replay/selected_context/child.rs",
            include_str!("../../../../../../src/cleanup_plan/replay/selected_context/child.rs"),
        ),
        (
            "src/cleanup_plan/replay/selected_context/child/tests.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/selected_context/child/tests.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/selected_context/child/tests/source.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/selected_context/child/tests/source.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/selected_context/paid.rs",
            include_str!("../../../../../../src/cleanup_plan/replay/selected_context/paid.rs"),
        ),
        (
            "src/cleanup_plan/replay/selected_context/paid_before_children.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/selected_context/paid_before_children.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/semantic_refusal.rs",
            include_str!("../../../../../../src/cleanup_plan/replay/semantic_refusal.rs"),
        ),
        (
            "src/hir/lexical_exit.rs",
            include_str!("../../../../../../src/hir/lexical_exit.rs"),
        ),
        (
            "src/hir/lexical_exit/selected_child.rs",
            include_str!("../../../../../../src/hir/lexical_exit/selected_child.rs"),
        ),
        (
            "src/loop_calls/byte_writer.rs",
            include_str!("../../../../../../src/loop_calls/byte_writer.rs"),
        ),
        (
            "src/loop_calls/byte_writer/resolved.rs",
            include_str!("../../../../../../src/loop_calls/byte_writer/resolved.rs"),
        ),
        (
            "src/loop_calls/byte_writer/resolved/selected_inline.rs",
            include_str!(
                "../../../../../../src/loop_calls/byte_writer/resolved/selected_inline.rs"
            ),
        ),
        (
            "tests/hir_module_boundaries.rs",
            include_str!("../../../../../../tests/hir_module_boundaries.rs"),
        ),
        (
            "tests/workspace/semantic_graph/source_inventory.rs",
            include_str!("../../../../../../tests/workspace/semantic_graph/source_inventory.rs"),
        ),
    ];
    assert_eq!(rows.as_array().unwrap().len(), files.len());
    for (path, current) in files {
        let matching = rows
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["source"] == path)
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1, "{path}");
        let row = matching[0];
        let mut value = current.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(value.matches(new).count(), 1, "{path}");
            value = value.replacen(new, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(
            value,
            row["original_bytes"].as_str().unwrap(),
            "complete original {path}"
        );
    }
}
