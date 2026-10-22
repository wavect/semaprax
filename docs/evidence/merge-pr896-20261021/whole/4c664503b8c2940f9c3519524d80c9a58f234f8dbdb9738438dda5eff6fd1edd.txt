//! Complete preceding receiver and full source closure, without clipped interiors.
#[test]
fn complete_old_receiver_source_and_all_new_joined_sources_stay_exact() {
    let rows: serde_json::Value =
        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(
            crate::source_reflows_merge::ReceiverFixture::PrivateLayoutCapacity,
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
        let entry = capacity_receiver_entry(path, source);
        let projected = crate::source_reflows_merge::consumer_input("capacity834", path, &entry);
        let source = projected.as_str();
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
    let before = capacity_receiver_entry(
        "src/workspace_graph/expected_projection/private_layout/capacity_before.rs",
        include_str!("../capacity_before.rs"),
    );
    let before=before.strip_prefix("//! Complete preceding cold receiver; only method visibility/name differ.\nuse super::{Absence, AuthoredDeclaration, BTreeMap, DEPTH, Diagnostic, Entry, Frame, LayoutClosure, Program, debit, explicit_only, ledger, names, owner_index, private_name, refusal, reserve_builder_structure, sorting, visit};\nimpl<'a> LayoutClosure<'a> {\n").unwrap().strip_suffix("}\n").unwrap()
        .replacen("    pub(super) fn derive_with_work_before_capacity(","    fn derive_with_work(",1);
    let start = full.find("    fn derive_with_work(").unwrap();
    let end = full.find("    pub(super) fn add_costs(").unwrap();
    assert_eq!(before, &full[start..end]);
}

// Closed whole-input acquisition precedes every unchanged capacity inverse.
fn capacity_receiver_entry(source: &str, compiled: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = |s: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes()))
        )
    };
    match source {
        "src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs" => {
            let prior = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
                source,
                "592ff99b199000a529e68f39e91e986fc133871718965889231f0139ff6a575e",
                compiled,
            );
            assert_eq!(prior.len(), 4326);
            assert_eq!(
                digest(&prior),
                "592ff99b199000a529e68f39e91e986fc133871718965889231f0139ff6a575e"
            );
            prior
        }
        "src/workspace_graph/expected_projection/private_layout/capacity_before.rs" => {
            let current = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
                source,
                "e0dfdd0ec25d52c0855f8c53027e1c4b23fbe8f7d24b045d5f0d34f1ee91ccff",
                compiled,
            );
            assert_eq!(current.len(), 3983);
            assert_eq!(
                digest(&current),
                "e0dfdd0ec25d52c0855f8c53027e1c4b23fbe8f7d24b045d5f0d34f1ee91ccff"
            );
            let formatted = "use super::{\n    debit, explicit_only, ledger, names, owner_index, private_name, refusal,\n    reserve_builder_structure, sorting, visit, Absence, AuthoredDeclaration, BTreeMap, Diagnostic,\n    Entry, Frame, LayoutClosure, Program, DEPTH,\n};\n";
            let original = "use super::{Absence, AuthoredDeclaration, BTreeMap, DEPTH, Diagnostic, Entry, Frame, LayoutClosure, Program, debit, explicit_only, ledger, names, owner_index, private_name, refusal, reserve_builder_structure, sorting, visit};\n";
            assert_eq!(current.matches(formatted).count(), 1);
            let prior = current.replacen(formatted, original, 1);
            assert_eq!(prior.len(), 3968);
            assert_eq!(
                digest(&prior),
                "8f3117e619ea89862c658ef2104c50022c05a0876298737f8446e0168e32e75d"
            );
            prior
        }
        _ => compiled.to_owned(),
    }
}
