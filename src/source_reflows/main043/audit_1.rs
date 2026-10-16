fn complete_source_archives() {
    use sha2::{Digest, Sha256};
    let rows: serde_json::Value = serde_json::from_str(super::metadata(super::Fixture::ExitDirectory))
    .unwrap();
    let sources = [
        ("src/cleanup_plan/replay.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay.rs.before.txt")), "2a4db37f10435075809a43d112073b686710d55f4f5f43e0f553686a63504796"),
        ("src/cleanup_plan/replay/expression_facts/receiving.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__expression_facts__receiving.rs.before.txt")), "2e8e01a0d509c00cc39953fa8358d7533c862ea1367c9eba6fe7fcb8c0179762"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__expression_facts__receiving__retained.rs.before.txt")), "5460420073056042a13515edc037e2c6737da39286c91105b3feae4f942aeab2"),
        ("src/cleanup_plan/replay/expression_facts/receiving/remaining.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/remaining.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__expression_facts__receiving__remaining.rs.before.txt")), "ba562e69301e6283bff9bec4517444ccea0fc2094f56959214cbbf78e3915b1b"),
        ("src/cleanup_plan/replay/expression_facts/receiving/remaining/selected.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/remaining/selected.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__expression_facts__receiving__remaining__selected.rs.before.txt")), "446459444ef20047d571618c4c85c4cb1d8151f52bb1b587221c795bff0b52b9"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/a.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/a.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__live_oracle__source_custody__a.rs.before.txt")), "e539844c397f769ad20cdf13324de0dfd598ed053e88ad57bdb18ee766c7a951"),
        ("src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__expression_facts__receiving__remaining__tests.rs.before.txt")), "ac2685e11c2cd09f9caee373b6b5744b5544b5680032cc07fbead0650ceef474"),
        ("tests/hir_module_boundaries.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/tests__hir_module_boundaries.rs.before.txt")), "ef30bafb7e74ece87a8e4ccc2a056906a5775e3689139e538c74db8d8c4d4b0d"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt")), "4cc35f98927317e03348e7cb2508fec59d99b697def2b35f620fcac0060735a6"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt")), "a349da31a1efabca38017cca4b1b7305a694ff7bd5f923e8eddad84d9e82053d"),
    ];
    assert_eq!(rows.as_array().unwrap().len(), sources.len());
    for (row, (path, current, old, fixed)) in rows.as_array().unwrap().iter().zip(sources) {
        assert_eq!(row["source"].as_str().unwrap(), path);
        assert_eq!(row["sha256"].as_str().unwrap(), fixed);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))
            ),
            fixed
        );
        let mut restored = current.to_owned();
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, old);
    }
}

#[test]
fn complete_original_owner_is_executed() { complete_source_archives(); }
