//! Complete preceding receiver and full source closure, without clipped interiors.
#[test]
fn complete_old_receiver_source_and_all_new_joined_sources_stay_exact() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/evidence/private-layout-entry-capacity-834/exact-source-inverses.json"
    ))
    .unwrap();
    let files = [
        (
            "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
            include_str!(
                "../../../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/live_oracle/source_custody/b.rs",
            include_str!("../../../../../src/cleanup_plan/replay/live_oracle/source_custody/b.rs"),
        ),
        (
            "src/workspace_graph/expected_projection/private_layout.rs",
            include_str!(
                "../../../../../src/workspace_graph/expected_projection/private_layout.rs"
            ),
        ),
        (
            "src/workspace_graph/expected_projection/private_layout/capacity_before.rs",
            include_str!(
                "../../../../../src/workspace_graph/expected_projection/private_layout/capacity_before.rs"
            ),
        ),
        (
            "src/workspace_graph/expected_projection/private_layout/capacity_tests.rs",
            include_str!(
                "../../../../../src/workspace_graph/expected_projection/private_layout/capacity_tests.rs"
            ),
        ),
        (
            "src/workspace_graph/expected_projection/private_layout/capacity_tests/owner_controls.rs",
            include_str!(
                "../../../../../src/workspace_graph/expected_projection/private_layout/capacity_tests/owner_controls.rs"
            ),
        ),
        (
            "src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs",
            include_str!(
                "../../../../../src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs"
            ),
        ),
        (
            "tests/hir_module_boundaries.rs",
            include_str!("../../../../../tests/hir_module_boundaries.rs"),
        ),
        (
            "tests/workspace/semantic_graph/source_inventory.rs",
            include_str!("../../../../../tests/workspace/semantic_graph/source_inventory.rs"),
        ),
    ];
    assert_eq!(files.len(), rows.as_array().unwrap().len());
    for (path, source) in files {
        let row = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == path)
            .unwrap();
        let mut current = source.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(current.matches(new).count(), 1, "{path}");
            current = current.replacen(new, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(
            current,
            row["original_bytes"].as_str().unwrap(),
            "whole original {path}"
        );
    }
    let full = include_str!(
        "../../../../../docs/evidence/private-layout-entry-capacity-834/original/src__workspace_graph__expected_projection__private_layout.rs.before.txt"
    );
    let before = include_str!("../capacity_before.rs");
    let before=before.strip_prefix("//! Complete preceding cold receiver; only method visibility/name differ.\nuse super::{Absence, AuthoredDeclaration, BTreeMap, DEPTH, Diagnostic, Entry, Frame, LayoutClosure, Program, debit, explicit_only, ledger, names, owner_index, private_name, refusal, reserve_builder_structure, sorting, visit};\nimpl<'a> LayoutClosure<'a> {\n").unwrap().strip_suffix("}\n").unwrap()
        .replacen("    pub(super) fn derive_with_work_before_capacity(","    fn derive_with_work(",1);
    let start = full.find("    fn derive_with_work(").unwrap();
    let end = full.find("    pub(super) fn add_costs(").unwrap();
    assert_eq!(before, &full[start..end]);
}
