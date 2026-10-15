//! Full immediate-source and old-receiver custody; no production authority.
use sha2::{Digest, Sha256};
fn hex(source: &str) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes())))
}
#[test]
fn inline_weighted_complete_source_inverses_and_immediate_receiver_are_exact() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/inline-weighted-successor-842/source-inverses.json"))).unwrap();
    let sources = [
        ("src/cleanup_plan/replay/factored/canonical_shared.rs", include_str!("../../canonical_shared.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/tests.rs", include_str!("../tests.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/successor_targets.rs", include_str!("../successor_targets.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs", include_str!("../scratch_tests.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/receiving_layer_tests.rs", include_str!("../receiving_layer_tests.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/singleton_target_tests.rs", include_str!("../singleton_target_tests.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/held_auth_tests.rs", include_str!("../held_auth_tests.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/receiver_tests.rs", include_str!("../ordered_weights/receiver_tests.rs")),
        ("src/source_reflows_merge.rs", include_str!("../../../../../source_reflows_merge.rs")),
        ("docs/ARCHITECTURE.md", include_str!("../../../../../../docs/ARCHITECTURE.md")),
        ("tests/hir_module_boundaries.rs", include_str!("../../../../../../tests/hir_module_boundaries.rs")),
        ("tests/workspace/semantic_graph/source_inventory.rs", include_str!("../../../../../../tests/workspace/semantic_graph/source_inventory.rs")),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", include_str!("../../../../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
    ];
    assert_eq!(sources.len(), rows.as_array().unwrap().len());
    for (path, current) in sources {
        let matching: Vec<_> = rows.as_array().unwrap().iter().filter(|row| row["source"] == path).collect();
        assert_eq!(matching.len(), 1);
        let row = matching[0];
        assert_eq!(hex(current), row["current_sha256"]);
        let edits = row["edits"].as_array().unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(current, edits[0]["new"].as_str().unwrap());
        assert_eq!(hex(edits[0]["old"].as_str().unwrap()), row["original_sha256"]);
        assert_eq!(current.replacen(current, edits[0]["old"].as_str().unwrap(), 1), edits[0]["old"].as_str().unwrap());
    }
    let old = include_str!("../../../../../../docs/evidence/inline-weighted-successor-842/canonical_shared.rs.before.txt");
    let start = old.find("type SharedWeighted").unwrap();
    let end = old[start..].find("\n#[cfg(test)]\nmod tests;").unwrap() + start;
    let mut expected = "//! Complete independent 7800 predecessor; original weighted backing and charges.\nuse super::super::{Automaton, BTreeMap, Diagnostic, Label, Rc, ReplayBudget, ResolvedFunction, Weighted, physical_clone, replay_error};\nuse super::{memo, ordered_alias, ordered_weights, successor_targets, app_census, tests};\n\n".to_owned();
    let mut body = old[start..end].to_owned();
    for name in ["own_weighted", "weighted_successor", "share_weighted"] {
        body = body.replacen(&format!("fn {name}("), &format!("pub(super) fn {name}("), 1);
    }
    body = body.replacen("    fn successors(", "    pub(super) fn successors(", 1);
    expected.push_str(&body);
    expected.push_str("\nimpl Canonicalizer<'_> {\n    pub(super) fn signatures(&self) -> &BTreeMap<(u64, Vec<(Label, usize)>), usize> { &self.ids }\n    pub(super) fn states(&self) -> &BTreeMap<SharedWeighted, usize> { &self.states }\n}\n");
    assert_eq!(expected, include_str!("../before_inline_weighted.rs"));
}
#[test]
fn inline_weighted_every_new_owner_remains_in_all_three_full_source_audits() {
    let owners = ["before_inline_weighted.rs", "inline_weighted.rs", "inline_weighted/tests.rs", "inline_weighted/source.rs"];
    let audits = [
        (include_str!("../../../../../../tests/hir_module_boundaries.rs"), "../"),
        (include_str!("../../../../../../tests/workspace/semantic_graph/source_inventory.rs"), "../../"),
        (include_str!("../../../../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"), "../../../../../"),
    ];
    for owner in owners {
        for (audit, prefix) in audits {
            assert_eq!(audit.matches(&format!("include_str!(\"{prefix}src/cleanup_plan/replay/factored/canonical_shared/{owner}\")")).count(), 1);
        }
    }
}
