//! Independent complete incoming Main7f8 provider proof; ours remains unchanged.
pub(super) fn complete_incoming_receiver() {
    use sha2::{Digest, Sha256};
    let manifest_text = crate::workspace_graph::identity_resolution::main7f8_provider_manifest();
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text).unwrap();
    let rows = manifest["whole_sources"].as_array().unwrap();
    assert_eq!(rows.len(), 17);
    let provider_inventory_raw =
        include_str!("../../../../../tests/workspace/semantic_graph/source_inventory.rs");
    let provider_inventory_input =
        crate::workspace_graph::identity_resolution::main7f8_provider_inventory_input(
            "tests/workspace/semantic_graph/source_inventory.rs",
            provider_inventory_raw,
        );
    let provider_inventory =
        crate::workspace_graph::identity_resolution::historical_provider_inventory(
            &provider_inventory_input,
        );
    assert_eq!(
        provider_inventory,
        crate::workspace_graph::identity_resolution::main7f8_provider_checkpoint(
            "tests/workspace/semantic_graph/source_inventory.rs",
            provider_inventory_raw
        )
    );
    let provider_native_inventory_raw = include_str!("../../../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs");
    let provider_native_inventory_input = crate::workspace_graph::identity_resolution::main7f8_provider_inventory_input("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", provider_native_inventory_raw);
    let provider_native_inventory =
        crate::workspace_graph::identity_resolution::historical_provider_inventory(
            &provider_native_inventory_input,
        );
    assert_eq!(provider_native_inventory, crate::workspace_graph::identity_resolution::main7f8_provider_checkpoint("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", provider_native_inventory_raw));
    for row in rows {
        let (current, original) = match row["file"].as_str().unwrap() {
            "src/loop_calls/byte_writer/source_index.rs" => (include_str!("../../../../../src/loop_calls/byte_writer/source_index.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/0.before.txt")),
            "src/loop_calls/byte_writer.rs" => (include_str!("../../../../../src/loop_calls/byte_writer.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/1.before.txt")),
            "src/workspace_graph/expected_projection/import_stub/byte_helper.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/import_stub/byte_helper.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/2.before.txt")),
            "src/workspace_graph/expected_projection/import_stub.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/import_stub.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/3.before.txt")),
            "src/workspace_graph/expected_projection/held_projection.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/held_projection.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/4.before.txt")),
            "src/workspace_graph/expected_projection/borrowed_imports.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/borrowed_imports.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/5.before.txt")),
            "src/workspace_graph/expected_projection/borrowed_leaf.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/borrowed_leaf.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/6.before.txt")),
            "src/workspace_graph/expected_projection/uncached_peak.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/uncached_peak.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/7.before.txt")),
            "src/workspace_graph/expected_projection.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/8.before.txt")),
            "src/workspace_graph.rs" => (include_str!("../../../../../src/workspace_graph.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/9.before.txt")),
            "tests/workspace/semantic_graph/source_inventory.rs" => (include_str!("../../../../../tests/workspace/semantic_graph/source_inventory.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/10.before.txt")),
            "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => (include_str!("../../../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/11.before.txt")),
            "src/loop_calls/byte_writer/source_index_original.rs" => (include_str!("../../../../../src/loop_calls/byte_writer/source_index_original.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/0.before.txt")),
            "src/workspace_graph/expected_projection/import_stub/byte_helper_original.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/import_stub/byte_helper_original.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/2.before.txt")),
            "src/workspace_graph/expected_projection/import_stub_original.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/import_stub_original.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/3.before.txt")),
            "src/workspace_graph/expected_projection/held_projection_original.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/held_projection_original.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/4.before.txt")),
            "src/workspace_graph/expected_projection/borrowed_imports_original.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/borrowed_imports_original.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/5.before.txt")),
            other => panic!("unregistered complete source {other}"),
        };
        let current = crate::workspace_graph::identity_resolution::main7f8_provider_checkpoint(
            row["file"].as_str().unwrap(),
            current,
        );
        if row["file"] == "src/workspace_graph.rs" {
            assert_eq!(
                current,
                crate::workspace_graph::identity_resolution::historical_root()
            );
        }
        if row["file"] == "tests/workspace/semantic_graph/source_inventory.rs" {
            assert_eq!(current, provider_inventory);
        }
        if row["file"] == "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" {
            assert_eq!(current, provider_native_inventory);
        }
        let current = current.as_str();
        assert_eq!(
            original.len(),
            row["original_bytes"].as_u64().unwrap() as usize
        );
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            row["original_sha256"].as_str().unwrap()
        );
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            row["current_sha256"].as_str().unwrap()
        );
        let mut restored = current.to_owned();
        for hook in row["inverse"].as_array().unwrap() {
            let present = hook["current"].as_str().unwrap();
            assert_eq!(
                restored.matches(present).count(),
                1,
                "{} {}",
                row["file"],
                hook["hook"]
            );
            restored = restored.replacen(present, hook["before"].as_str().unwrap(), 1);
        }
        assert_eq!(
            restored.as_bytes(),
            original.as_bytes(),
            "complete original {}",
            row["file"]
        );
    }
    let parent = ci_str!("../../import_stub.rs");
    assert!(
        parent.contains("#[path = \"import_stub/core_proofs.rs\"]\npub(super) mod core_proofs;")
    );
    let source = ci_str!("../../../../loop_calls/byte_writer.rs");
    assert!(source.contains("#[cfg(test)]\npub(crate) mod source_index_original;"));
}
