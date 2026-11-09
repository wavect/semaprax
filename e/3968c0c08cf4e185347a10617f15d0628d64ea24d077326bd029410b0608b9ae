#[test]
fn complete_original_and_all_source_audit_receivers_are_byte_exact() {
    use sha2::{Digest, Sha256};
    let packet: serde_json::Value =
        serde_json::from_str(super::metadata(super::Fixture::Lifecycle)).unwrap();
    let inputs=[
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/function_scope/lifecycle/completed.rs")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
    ];
    let originals = [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/lifecycle-inline-active-path/completed.rs.before.txt"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/lifecycle-inline-active-path/hir_module_boundaries.rs.before.txt"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/lifecycle-inline-active-path/interop_source_inventory.rs.before.txt"
        )),
    ];
    let rows = packet.as_array().unwrap();
    assert_eq!(rows.len(), inputs.len());
    for ((row, current), original) in rows.iter().zip(inputs).zip(originals) {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            row["original_sha256"].as_str().unwrap()
        );
        let mut restored = current.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, original);
    }
    assert_eq!(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/hir/validation/function_scope/lifecycle/completed/before_inline.rs"
        ))
        .to_owned()
            + "\n#[cfg(test)]\nmod tests;\n",
        originals[0]
    );
    let mut legacy = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/hir/validation/function_scope/lifecycle.rs"
    ))
    .to_owned();
    legacy = crate::source_reflows::legacy_ci_expected_input(
        "src/hir/validation/function_scope/lifecycle.rs",
        "4359a3991e559f31cbb1ad650886ed3433270cbf17ce535169b3ff04fe64027a",
        &legacy,
    )
    .into_owned();
    for (current, original) in [
        (
            "#[cfg(test)]\npub(super) fn extend_funded(",
            "pub(super) fn extend_funded(",
        ),
        ("#[cfg(test)]\nfn collect<'a>(", "fn collect<'a>("),
    ] {
        assert_eq!(legacy.matches(current).count(), 1);
        legacy = legacy.replacen(current, original, 1);
    }

    let binding: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/lifecycle-inline-active-path/source-binding.json"
    )))
    .unwrap();
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(legacy.as_bytes()))
        ),
        binding["original_lifecycle_sha256"].as_str().unwrap()
    );
    // Actual compile-time authority inventories include every new child exactly once.
    for input in &inputs[1..] {
        for name in [
            "completed/inline_path.rs",
            "completed/inline_path/tests.rs",
            "completed/before_inline.rs",
        ] {
            assert_eq!(input.matches(name).count(), 1);
        }
    }
}
