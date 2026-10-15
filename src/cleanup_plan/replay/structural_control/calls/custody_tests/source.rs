//! Closed whole-source originals and complete old receiving-layout definitions.
#[test]
fn every_call_receiver_source_and_complete_before_body_remains_bound() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../docs/evidence/selected-call-refusal-781/exact-source-inverses.json"
    ))
    .unwrap();
    let files = [
        (
            "src/cleanup_plan/replay/selected_entry/tests/session_errors.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/selected_entry/tests/session_errors.rs"
            ),
        ),
        (
            "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
            include_str!(
                "../../../../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/lexical_replay.rs",
            include_str!("../../../../../../src/cleanup_plan/replay/lexical_replay.rs"),
        ),
        (
            "src/cleanup_plan/replay/live_oracle/source_custody/b.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/live_oracle/source_custody/b.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/scalar_replay.rs",
            include_str!("../../../../../../src/cleanup_plan/replay/scalar_replay.rs"),
        ),
        (
            "src/cleanup_plan/replay/selected_profile/paid_children/collections.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/selected_profile/paid_children/collections.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/semantic_refusal.rs",
            include_str!("../../../../../../src/cleanup_plan/replay/semantic_refusal.rs"),
        ),
        (
            "src/cleanup_plan/replay/semantic_refusal/tests.rs",
            include_str!("../../../../../../src/cleanup_plan/replay/semantic_refusal/tests.rs"),
        ),
        (
            "src/cleanup_plan/replay/structural_control.rs",
            include_str!("../../../../../../src/cleanup_plan/replay/structural_control.rs"),
        ),
        (
            "src/cleanup_plan/replay/structural_control/blocks.rs",
            include_str!("../../../../../../src/cleanup_plan/replay/structural_control/blocks.rs"),
        ),
        (
            "src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/structural_control/blocks_before_receipt.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/structural_control/call_receiving.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/structural_control/call_receiving.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/structural_control/calls.rs",
            include_str!("../../../../../../src/cleanup_plan/replay/structural_control/calls.rs"),
        ),
        (
            "src/cleanup_plan/replay/structural_control/calls/custody_tests.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/structural_control/calls/custody_tests.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/structural_control/calls/custody_tests/source.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/structural_control/calls_before_receipt.rs",
            include_str!(
                "../../../../../../src/cleanup_plan/replay/structural_control/calls_before_receipt.rs"
            ),
        ),
        (
            "tests/hir_module_boundaries.rs",
            include_str!("../../../../../../tests/hir_module_boundaries.rs"),
        ),
        (
            "tests/workspace/semantic_graph/source_inventory.rs",
            include_str!("../../../../../../tests/workspace/semantic_graph/source_inventory.rs"),
        ),
    ];
    assert_eq!(files.len(), rows.as_array().unwrap().len());
    for (name, source) in files {
        let found = rows
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["source"] == name)
            .collect::<Vec<_>>();
        assert_eq!(found.len(), 1, "{name}");
        let row = found[0];
        let mut current = source.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(current.matches(new).count(), 1, "{name}");
            current = current.replacen(new, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(
            current,
            row["original_bytes"].as_str().unwrap(),
            "full old {name}"
        );
    }
    let before = include_str!("../../calls_before_receipt.rs")
        .replacen("pub(super) enum Signature<", "enum Signature<", 1)
        .replacen("pub(super) fn signature<", "fn signature<", 1)
        .replacen("    pub(super) fn parameter(", "    fn parameter(", 1)
        .replacen("    pub(super) fn accepts_type(", "    fn accepts_type(", 1)
        .replacen("    pub(super) fn len(", "    fn len(", 1);
    assert_eq!(
        before,
        include_str!(
            "../../../../../../docs/evidence/selected-call-refusal-781/original/src__cleanup_plan__replay__structural_control__calls.rs.before.txt"
        )
    );
    assert_eq!(
        include_str!("../../blocks_before_receipt.rs").replacen(
            "use super::*;\nuse super::calls_before_receipt as calls;",
            "use super::*;",
            1
        ),
        include_str!(
            "../../../../../../docs/evidence/selected-call-refusal-781/original/src__cleanup_plan__replay__structural_control__blocks.rs.before.txt"
        )
    );
    fn definition<'a>(text: &'a str, marker: &str) -> &'a str {
        let start = text.find(marker).unwrap();
        let rest = &text[start..];
        let end = rest.find('}').unwrap() + 1;
        &rest[..end]
    }
    fn compact(text: &str) -> String {
        text.chars().filter(|c| !c.is_whitespace()).collect()
    }
    let old = include_str!(
        "../../../../../../docs/evidence/selected-call-refusal-781/original/src__cleanup_plan__replay__semantic_refusal.rs.before.txt"
    );
    let olderror = include_str!(
        "../../../../../../docs/evidence/selected-call-refusal-781/original/errors.rs.before.txt"
    );
    let mirror = include_str!("../custody_tests.rs");
    assert_eq!(
        compact(definition(old, "pub(super) enum Atom {")),
        compact(definition(mirror, "pub(super) enum Atom {"))
    );
    let oldreceipt =
        definition(old, "pub(crate) struct Receipt {").replacen("pub(crate)", "pub(super)", 1);
    assert_eq!(
        compact(&oldreceipt),
        compact(definition(mirror, "pub(super) struct Receipt {"))
    );
    let olderror = definition(olderror, "pub(crate) enum EngineError {")
        .replacen("pub(crate)", "pub(super)", 1)
        .replacen(
            "Fact(FactReceipt)",
            "Fact(expression_facts::errors::FactReceipt)",
            1,
        )
        .replacen(
            "Structural(semantic_refusal::Receipt)",
            "Structural(Receipt)",
            1,
        );
    assert_eq!(
        compact(&olderror),
        compact(definition(mirror, "pub(super) enum EngineError {"))
    );
}
