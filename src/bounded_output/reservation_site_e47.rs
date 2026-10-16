//! Complete independent Main e47 controls and unchanged older source packets.
use sha2::{Digest, Sha256};
fn hash(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
#[test]
fn complete_e47_stage_guards_keep_originals_and_both_incoming_inverse_packets() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (source, original, incoming) in [
        (
            "src/bounded_output/canonical_stage_tests.rs",
            include_str!("../../docs/evidence/maine47-independent-custody-20261016/canonical_stage_tests.rs.16c.txt"),
            include_str!("../../docs/evidence/maine47-independent-custody-20261016/canonical_stage_tests.rs.e47.txt"),
        ),
        (
            "src/bounded_output/loan_inner_stage_tests.rs",
            include_str!("../../docs/evidence/maine47-independent-custody-20261016/loan_inner_stage_tests.rs.16c.txt"),
            include_str!("../../docs/evidence/maine47-independent-custody-20261016/loan_inner_stage_tests.rs.e47.txt"),
        ),
    ] {
        let raw = std::fs::read_to_string(root.join(source)).unwrap();
        let current = crate::source_reflows_merge::main_e47_input(source, &raw);
        assert_eq!(current, incoming, "every corrected Main guard byte");
        let record: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/evidence/maine47-independent-custody-20261016/guard-source-correspondence.json"
        )).unwrap();
        let row = record["guards"].as_array().unwrap().iter()
            .find(|row| row["source"] == source).unwrap();
        assert_eq!(hash(original), row["original16c_sha256"]);
        assert_eq!(hash(incoming), row["main_e47_sha256"]);
    }
    use crate::source_reflows_merge::{maina3_metadata, Maina3Fixture};
    for (fixture, expected) in [
        (Maina3Fixture::LoanEndpoint827, vec![
            "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
            "src/bounded_output.rs", "src/bounded_output/loan_inner_stage_tests.rs",
            "src/loan_plan/cfg_directory.rs", "src/loan_plan/paid_materialize.rs", "src/loan_plan/storage.rs",
        ]),
        (Maina3Fixture::LoanInner829, vec![
            "src/bounded_output.rs", "src/bounded_output/loan_inner_stage_tests.rs",
            "src/hir/canonical_loan_storage/caller.rs", "src/hir/canonical_loan_storage/caller/stage_tests.rs",
            "src/loan_plan/funded_flat.rs", "src/loan_plan/paid_cfg.rs", "tests/hir_module_boundaries.rs",
            "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
            "src/cleanup_plan/replay/live_oracle/source_custody/b.rs",
        ]),
    ] {
        let text = maina3_metadata(fixture);
        let rows: serde_json::Value = serde_json::from_str(&text).unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), expected.len(), "complete incoming packet");
        assert_eq!(rows.iter().map(|row| row["source"].as_str().unwrap()).collect::<Vec<_>>(), expected);
        // Other entries retain their independent existing source-view owners.
        // Exercise the changed guard against both complete incoming packets.
        let source = "src/bounded_output/loan_inner_stage_tests.rs";
        let row = rows.iter().find(|row| row["source"] == source).unwrap();
        let raw = std::fs::read_to_string(root.join(source)).unwrap();
        let mut current = crate::source_reflows_merge::main_e47_input(source, &raw);
        assert_eq!(hash(&current), row["current_sha256"]);
        for edit in row["edits"].as_array().unwrap() {
            let after = edit["new"].as_str().unwrap();
            assert_eq!(current.matches(after).count(), 1);
            current = current.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(hash(&current), row["original_sha256"]);
        match row["archive"].as_str() {
            Some(path) => assert_eq!(current, std::fs::read_to_string(root.join(path)).unwrap()),
            None => assert!(current.is_empty()),
        }
    }
}
