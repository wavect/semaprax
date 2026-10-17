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
        // Acquire the complete integrated physical owner before exercising the
        // unchanged complete legacy assertions below on its exact old stage.
        let current = legacy_checkpoint(path, row["before_sha256"].as_str().unwrap(), current);
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

// Distinct expected namespace for the finite integrated six-source successor.
// The original packet, restore and acquire above remain independent controls.
const INTEGRATED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/ordinary-plan-integrated-e24-v1/stages.json"
));
const INTEGRATED_LENGTH: usize = 1539271;
const INTEGRATED_SHA256: &str = "da99e3b9599f64e879ba43646aef4b69f927f566b25007c96cd6d0c932a815bd";
fn integrated_records() -> serde_json::Value {
    assert!(INTEGRATED.len() <= 2 * 1024 * 1024);
    assert_eq!(INTEGRATED.len(), INTEGRATED_LENGTH);
    assert_eq!(digest(INTEGRATED), INTEGRATED_SHA256);
    let rows: serde_json::Value = serde_json::from_str(INTEGRATED).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 6);
    rows
}
fn edge(row: &serde_json::Value, text: &str, name: &str) -> String {
    let edge = &row[name];
    assert_eq!(
        digest(text),
        edge["after_sha256"],
        "whole integrated source edge"
    );
    let mut restored = text.to_owned();
    let mut last = restored.len();
    for edit in edge["inverse"].as_array().unwrap() {
        let start = edit["start_byte"].as_u64().unwrap() as usize;
        let after = edit["after"].as_str().unwrap();
        let end = start
            .checked_add(after.len())
            .expect("bounded inverse endpoint");
        assert!(end <= last);
        assert_eq!(restored.get(start..end), Some(after));
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
        last = start;
    }
    assert_eq!(digest(&restored), edge["before_sha256"]);
    restored
}
fn legacy_checkpoint(source: &str, expected: &str, current: &str) -> String {
    let rows = integrated_records();
    let matching = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["path"] == source)
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1, "finite integrated source owner");
    let row = matching[0];
    assert_eq!(
        row["expected_sha256"], expected,
        "explicit original expected namespace"
    );
    let physical = CURRENT.iter().find(|(path, _)| *path == source).unwrap().1;
    assert!(physical.len() <= 2 * 1024 * 1024);
    assert_eq!(
        physical.len() as u64,
        row["current_length"].as_u64().unwrap()
    );
    assert_eq!(
        digest(physical),
        row["current_sha256"],
        "whole actual integrated source"
    );
    let checkpoint = edge(row, physical, "current_to_e24");
    assert_eq!(checkpoint, row["e24"].as_str().unwrap());
    let legacy = edge(row, &checkpoint, "e24_to_legacy");
    assert_eq!(legacy, row["legacy"].as_str().unwrap());
    let old = records();
    let old = old
        .as_array()
        .unwrap()
        .iter()
        .find(|old| old["path"] == source)
        .unwrap();
    assert_eq!(digest(&legacy), old["after_sha256"]);
    assert_eq!(row["expected_sha256"], old["before_sha256"]);
    assert!(
        current == physical
            || current == checkpoint
            || current == legacy
            || current == old["before"].as_str().unwrap(),
        "foreign integrated source stage"
    );
    legacy
}
pub(super) fn acquire_expected(source: &str, expected: &str, current: &str) -> String {
    if !matches!(source,
        "src/cleanup_plan/replay.rs" | "src/source_reflows_merge.rs"
        | "src/source_reflows_merge/main67107_inputs.rs" | "tests/hir_module_boundaries.rs"
        | "tests/workspace/semantic_graph/source_inventory.rs"
        | "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs") {
        return current.to_owned();
    }
    let old = records();
    let matching = old
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["path"] == source)
        .collect::<Vec<_>>();
    assert!(matching.len() <= 1);
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    // A current/raw or older expected namespace is never normalized here.
    if row["before_sha256"] != expected {
        return current.to_owned();
    }
    let legacy = legacy_checkpoint(source, expected, current);
    restore(row, &legacy)
}
#[test]
fn finite_integrated_expected_namespace_authenticates_every_complete_stage() {
    let rows = integrated_records();
    let old = records();
    for row in rows.as_array().unwrap() {
        let source = row["path"].as_str().unwrap();
        let expected = row["expected_sha256"].as_str().unwrap();
        let physical = CURRENT.iter().find(|(path, _)| *path == source).unwrap().1;
        let original = old
            .as_array()
            .unwrap()
            .iter()
            .find(|old| old["path"] == source)
            .unwrap()["before"]
            .as_str()
            .unwrap();
        for stage in [
            physical,
            row["e24"].as_str().unwrap(),
            row["legacy"].as_str().unwrap(),
            original,
        ] {
            assert_eq!(acquire_expected(source, expected, stage), original);
        }
        assert_eq!(
            acquire_expected(source, &digest(physical), physical),
            physical
        );
        assert_eq!(
            acquire_expected(source, "older selected namespace", "older source stage"),
            "older source stage"
        );
        for bad in [
            String::new(),
            format!("{physical} "),
            format!("{physical}{physical}"),
            "foreign".to_owned(),
        ] {
            assert!(std::panic::catch_unwind(|| acquire_expected(source, expected, &bad)).is_err());
        }
    }
    assert_eq!(
        acquire_expected("foreign", "foreign", "unchanged"),
        "unchanged"
    );
}
