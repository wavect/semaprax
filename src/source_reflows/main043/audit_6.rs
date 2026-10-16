#[test]
fn complete_before_edge_receiver_and_current_full_source_remain_exact() {
    use sha2::{Digest, Sha256};
    let old = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/referenced-edge-ownership-proof/retained.rs.before.txt"));
    let rows: serde_json::Value = serde_json::from_str(super::metadata(super::Fixture::Edge))
    .unwrap();
    let fixed = [
        ("tests/hir_module_boundaries.rs", "5be1edee738ee12b426939fbfbc09635229d4c7a3da4f45d3c1074254e0eb0e2"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "844628e0b1a3f3dafd563a7a9ef3396f513e664dc63e7d70237912736d6558ea"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "52fd9fb9d542ec3916b7427610885d934f7a9e0e9987e515feea226a0bbabfdb"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "02901e6e9fb8922ee4be72dab3899cbafaba25b942a9f62daf0979ea1fb8556d"),
    ];
    let complete_sources = [
        ("tests/hir_module_boundaries.rs", "tests__hir_module_boundaries.rs.before.txt", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/referenced-edge-ownership-proof/tests__hir_module_boundaries.rs.before.txt"))),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/referenced-edge-ownership-proof/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt"))),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/referenced-edge-ownership-proof/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt"))),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "retained.rs.before.txt", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/referenced-edge-ownership-proof/retained.rs.before.txt"))),
    ];
    assert_eq!(rows.as_array().unwrap().len(), complete_sources.len());
    for row in rows.as_array().unwrap() {
        let fixed_hash = fixed
            .iter()
            .find(|(path, _)| *path == row["source"].as_str().unwrap())
            .unwrap()
            .1;
        assert_eq!(row["sha256"].as_str().unwrap(), fixed_hash);
        let (_, archive_name, current, archive) = complete_sources
            .iter()
            .find(|(source, _, _, _)| *source == row["source"].as_str().unwrap())
            .unwrap();
        assert_eq!(row["archive"].as_str().unwrap(), *archive_name);
        let mut source = (*current).to_owned();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))
            ),
            row["sha256"].as_str().unwrap()
        );
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(source.matches(after).count(), 1);
            source = source.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(source, *archive);
    }
    let current_tests = super::historical_ci_extra("src/cleanup_plan/replay/expression_facts/receiving/retained/edge_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/edge_tests.rs")));
    let copied = current_tests
        .split_once("\n// BEGIN COMPLETE PAID EDGE COMPARISON BRIDGE\n")
        .unwrap()
        .1
        .split_once("// END COMPLETE PAID EDGE COMPARISON BRIDGE\n")
        .unwrap()
        .0;
    let copied = copied.replace(
        "fn paid_before_blocks_and_edges_retained(",
        "fn validate_blocks_and_edges_retained(",
    );
    let mut restored = copied;
    let mut substitutions = 0;
    for (before, after) in [
        (
            "validate_place(function, destination, storage, leaves)",
            "helpers::validate_place(function, destination, storage, leaves, budget)",
        ),
        (
            "validate_place(function, binding, storage, leaves)",
            "helpers::validate_place(function, binding, storage, leaves, budget)",
        ),
        (
            "validate_place(function, source, storage, leaves)",
            "helpers::validate_place(function, source, storage, leaves, budget)",
        ),
        (
            "validate_place(function, &argument.source, storage, leaves)",
            "helpers::validate_place(function, &argument.source, storage, leaves, budget)",
        ),
    ] {
        substitutions += restored.matches(after).count();
        restored = restored.replace(after, before);
    }
    assert_eq!(substitutions, 11);
    let full_before = super::historical_ci_extra("src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs")));
    let full_before = &full_before[full_before.find("// The retained key proof").unwrap()..];
    assert_eq!(restored, full_before);
    let start = old
        .find("// The retained key proof is the sole additional borrowed argument")
        .unwrap();
    let end = old.find("\n#[cfg(test)]\nmod tests;").unwrap();
    assert_eq!(super::historical_ci_extra("src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs"))), format!(
        "//! Complete pre-elimination receiving body, independently compiled.\nuse super::*;\n\n{}\n", &old[start..end]));
}
