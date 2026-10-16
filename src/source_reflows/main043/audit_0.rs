#[test]
fn complete_before_exit_receiver_and_full_source_inverses_remain_exact() {
    use sha2::{Digest, Sha256};
    let old = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/retained.rs.before.txt"));
    let rows: serde_json::Value = serde_json::from_str(super::metadata(super::Fixture::ExitPrefix))
    .unwrap();
    let fixed = [
        ("tests/hir_module_boundaries.rs", "03fde58554846398c379bbaa6f60cd1368d2573ff9fc715d340074f4c6ea56dc"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "bb72229bff1d33b7e6f03887a4022003465f077710cd643d75be9afcf4577961"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "7e0364a194df13c8fc374e4dd1ae2464609a8040ea2087429d06843a00f417c7"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "222853f211323dde7b91b7e5ba0dfebf0aeee26a4383174075718c662386bbe7"),
    ];
    let sources = [
        ("tests/hir_module_boundaries.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/tests__hir_module_boundaries.rs.before.txt"))),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt"))),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt"))),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/retained.rs.before.txt"))),
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
    let current_tests = super::historical_ci_extra("src/cleanup_plan/replay/expression_facts/receiving/retained/exit_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/exit_tests.rs")));
    let copied = current_tests
        .split_once("\n// BEGIN COMPLETE PAID EXIT COMPARISON BRIDGE\n")
        .unwrap()
        .1
        .split_once("// END COMPLETE PAID EXIT COMPARISON BRIDGE\n")
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
    let full_before = super::historical_ci_extra("src/cleanup_plan/replay/expression_facts/receiving/retained/exit_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/exit_before.rs")));
    let full_before = &full_before[full_before.find("// The retained key proof").unwrap()..];
    assert_eq!(restored, full_before);
    let start = old.find("// The retained key proof").unwrap();
    let end = old.find("\n#[cfg(test)]\nmod tests;").unwrap();
    assert_eq!(super::historical_ci_extra("src/cleanup_plan/replay/expression_facts/receiving/retained/exit_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/exit_before.rs"))), format!(
        "//! Complete pre-exit-elimination receiving body, independently compiled.\nuse super::*;\n\n{}\n", &old[start..end]));
}
