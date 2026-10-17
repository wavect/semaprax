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
            | "src/source_reflows_merge.rs"
            | "src/source_reflows_merge/main67107_inputs.rs"
            | "tests/hir_module_boundaries.rs"
            | "tests/workspace/semantic_graph/source_inventory.rs"
    ) {
        return current.to_owned();
    }
    let rows: serde_json::Value = serde_json::from_str(PACKET).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 5);
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
    let rows: serde_json::Value = serde_json::from_str(PACKET).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = restore(row, &current);
        assert_eq!(acquire(path, &current), before);
        assert_eq!(acquire(path, &before), before);
        let bad = format!("{current} ");
        assert!(std::panic::catch_unwind(|| restore(row, &bad)).is_err());
        let mut forged = row.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| restore(&forged, &current)).is_err());
        assert_eq!(acquire("foreign", &current), current);
    }
}
