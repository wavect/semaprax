#[test]
fn complete_original_arena_and_every_added_source_join_have_exact_full_inverses() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value =
        serde_json::from_str(&crate::source_reflows_merge::main8a_metadata(
            crate::source_reflows_merge::Main8aFixture::PrefixCertified3,
        ))
        .unwrap();
    let sources = [
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prefix-certified-expression-lookup/arena.rs.before.txt")), "94e056acbccb322afa35d42ba81af4672725b036bb4cf730c9495a8bbbb44d17"),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prefix-certified-expression-lookup/hir_module_boundaries.rs.before.txt")), "57db1e409226dc2ddf5c9519071ad69822e646d6f71d11cfd16e88d51ad8c4fc"),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prefix-certified-expression-lookup/source_custody_b.rs.before.txt")), "ead6bb549f1bd85d904eecbf182f14103639489320ec8ad09f0d29232f2fe90b"),
    ];
    assert_eq!(sources.len(), records.as_array().unwrap().len());
    for ((current, original, fixed), record) in sources.into_iter().zip(records.as_array().unwrap())
    {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            fixed
        );
        assert_eq!(record["original_sha256"].as_str().unwrap(), fixed);
        let before_885 = super::super::retained::tests::before_885_source(
            record["source"].as_str().unwrap(),
            current,
        );
        // Keep the complete original 885 proof. The current-source entry
        // separately acquires the exact current compiled source bytes.
        drop(before_885);
        let current_main =
            crate::source_reflows_merge::main8a_input(record["source"].as_str().unwrap(), current);
        let mut restored = current_main;
        for edit in record["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, original);
    }
}
