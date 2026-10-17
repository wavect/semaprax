//! Exact new-source acquisition before every unchanged historical raw guard.
use sha2::{Digest, Sha256};
const PACKET: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/ordinary-plan-history-c8740045-v1/inputs.json"
));
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
const PACKET_LENGTH: usize = 784669; // sealed by the source binding recipe
const PACKET_SHA256: &str = "a5d995cad4d5ecff6ea81eb310580a9a2caaf63150718d4992726cb99b877278";
fn records() -> serde_json::Value {
    assert!(
        PACKET.len() <= 2 * 1024 * 1024,
        "fixed custody packet exceeds cap"
    );
    assert_eq!(PACKET.len(), PACKET_LENGTH, "whole custody packet length");
    assert_eq!(digest(PACKET), PACKET_SHA256, "whole custody packet digest");
    serde_json::from_str(PACKET).unwrap()
}
const CURRENT: [(&str, &str); 6] = [
    (
        "src/cleanup_plan/replay.rs",
        include_str!("../cleanup_plan/replay.rs"),
    ),
    (
        "src/source_reflows_merge.rs",
        include_str!("../source_reflows_merge.rs"),
    ),
    (
        "src/source_reflows_merge/main67107_inputs.rs",
        include_str!("main67107_inputs.rs"),
    ),
    (
        "tests/hir_module_boundaries.rs",
        include_str!("../../tests/hir_module_boundaries.rs"),
    ),
    (
        "tests/workspace/semantic_graph/source_inventory.rs",
        include_str!("../../tests/workspace/semantic_graph/source_inventory.rs"),
    ),
    ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
        include_str!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
];
fn restore(row: &serde_json::Value, current: &str) -> String {
    assert_eq!(
        digest(current),
        row["after_sha256"],
        "whole ordinary plan source input"
    );
    let mut result = current.to_owned();
    for edit in row["inverse"].as_array().unwrap() {
        let new = edit["after"].as_str().unwrap();
        assert_eq!(
            result.matches(new).count(),
            1,
            "unique complete source inverse"
        );
        result = result.replacen(new, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(digest(&result), row["before_sha256"]);
    assert_eq!(result, row["before"].as_str().unwrap());
    result
}
pub(super) fn acquire(source: &str, current: &str) -> String {
    if !matches!(
        source,
        "src/cleanup_plan/replay.rs"
            | "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
            | "src/source_reflows_merge.rs"
            | "src/source_reflows_merge/main67107_inputs.rs"
            | "tests/hir_module_boundaries.rs"
            | "tests/workspace/semantic_graph/source_inventory.rs"
    ) {
        return current.to_owned();
    }
    let rows = records();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 6);
    let matching: Vec<_> = rows.iter().filter(|row| row["path"] == source).collect();
    assert!(matching.len() <= 1);
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    // Only this exact successor is acquired here. Other historical inputs keep
    // their unchanged expected-input owner and strict downstream byte guard.
    if digest(current) != row["after_sha256"] {
        return current.to_owned();
    }
    restore(row, current)
}
#[test]
fn complete_originals_inverse_and_foreign_inputs_remain_independent() {
    let rows = records();
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let current = CURRENT.iter().find(|(name, _)| *name == path).unwrap().1;
        assert!(current.len() <= 2 * 1024 * 1024);
        let before = restore(row, &current);
        assert_eq!(acquire(path, &current), before);
        assert_eq!(acquire(path, &before), before);
        let bad = format!("{current} ");
        assert!(std::panic::catch_unwind(|| restore(row, &bad)).is_err());
        let mut forged = row.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| restore(&forged, &current)).is_err());
        assert_eq!(acquire("foreign", current), current);
    }
}
