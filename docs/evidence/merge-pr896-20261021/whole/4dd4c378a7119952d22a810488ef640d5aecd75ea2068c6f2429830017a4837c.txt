#[test]
fn complete_before_status_fact_receiver_and_full_source_remain_exact() {
    use sha2::{Digest, Sha256};
    let old = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/status-fact-borrow-reuse/retained.rs.before.txt"
    ));
    let rows: serde_json::Value =
        serde_json::from_str(super::metadata(super::Fixture::Status)).unwrap();
    let fixed = [
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs", "c6069082ff7abb15cff8c832bc0a6086f8ef5c57103c450448ffc0494aa5eeee"),
        ("tests/hir_module_boundaries.rs", "83dc4425c79ca5720aa4d98d61561bdae8690701a0d7fb8daa0fbdfd0076f259"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "485f5b066ebe1e2edb3a42c9bcdfeb6d6f310d2691736eaf63777a02ac803676"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "44edd8afa0f32ce7d479b54e369ed7ae1220eb0bfdd057edb915808090b29c18"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "d1c619e451e181d4f1a9832f87f3055e4231f64b79099fcb4f15c4dbef592d5e"),
    ];
    let sources = [
        ("tests/hir_module_boundaries.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/status-fact-borrow-reuse/tests__hir_module_boundaries.rs.before.txt"))),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/status-fact-borrow-reuse/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt"))),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/status-fact-borrow-reuse/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt"))),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/status-fact-borrow-reuse/retained.rs.before.txt"))),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/status-fact-borrow-reuse/retained_tests.rs.before.txt"))),
    ];
    for row in rows.as_array().unwrap() {
        let fixed_hash = fixed
            .iter()
            .find(|(path, _)| *path == row["source"].as_str().unwrap())
            .unwrap()
            .1;
        assert_eq!(row["sha256"].as_str().unwrap(), fixed_hash);
        let (_, current, archive) = sources
            .iter()
            .find(|(path, _, _)| *path == row["source"].as_str().unwrap())
            .unwrap();
        let mut source = (*current).to_owned();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))
            ),
            fixed_hash
        );
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(source.matches(after).count(), 1);
            source = source.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(source, *archive);
    }
    let start = old
        .find("pub(in crate::cleanup_plan::replay) fn validate_status_sources_retained")
        .unwrap();
    let end = old.find("\n// The retained key proof").unwrap();
    assert_eq!(super::historical_ci_extra("src/cleanup_plan/replay/expression_facts/receiving/retained/status_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/status_before.rs"))), format!(
        "//! Complete pre-reuse status receiver, independently compiled.\nuse super::*;\n\n{}\n", &old[start..end]));
}
