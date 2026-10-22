//! Exact whole original caller and new source closure remain bound.
#[test]
fn whole_old_lexical_caller_and_all_new_inventory_sources_remain_bound() {
    let rows: serde_json::Value =
        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(
            crate::source_reflows_merge::ReceiverFixture::LexicalOperands,
        ))
        .unwrap();
    let sources = [
        (
            "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
            include_str!(
                "../../../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving.rs",
            include_str!("../../../../../src/cleanup_plan/replay/expression_facts/receiving.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/directory.rs",
            include_str!(
                "../../../../../src/cleanup_plan/replay/expression_facts/receiving/directory.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/return_operands.rs",
            include_str!(
                "../../../../../src/cleanup_plan/replay/expression_facts/receiving/return_operands.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/return_operands/exits.rs",
            include_str!(
                "../../../../../src/cleanup_plan/replay/expression_facts/receiving/return_operands/exits.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/return_operands/tests.rs",
            include_str!(
                "../../../../../src/cleanup_plan/replay/expression_facts/receiving/return_operands/tests.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/lexical_replay.rs",
            include_str!("../../../../../src/cleanup_plan/replay/lexical_replay.rs"),
        ),
        (
            "src/cleanup_plan/replay/lexical_replay/before_operands.rs",
            include_str!(
                "../../../../../src/cleanup_plan/replay/lexical_replay/before_operands.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/lexical_replay/operand_tests.rs",
            include_str!("../../../../../src/cleanup_plan/replay/lexical_replay/operand_tests.rs"),
        ),
        (
            "src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs",
            include_str!(
                "../../../../../src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs"
            ),
        ),
        (
            "src/cleanup_plan/replay/lexical_replay/ordinary.rs",
            include_str!("../../../../../src/cleanup_plan/replay/lexical_replay/ordinary.rs"),
        ),
        (
            "src/cleanup_plan/replay/live_oracle/source_custody/b.rs",
            include_str!("../../../../../src/cleanup_plan/replay/live_oracle/source_custody/b.rs"),
        ),
        (
            "tests/hir_module_boundaries.rs",
            include_str!("../../../../../tests/hir_module_boundaries.rs"),
        ),
        (
            "tests/workspace/semantic_graph/source_inventory.rs",
            include_str!("../../../../../tests/workspace/semantic_graph/source_inventory.rs"),
        ),
    ];
    assert_eq!(sources.len(), rows.as_array().unwrap().len());
    for (path, source) in sources {
        let projected = crate::source_reflows_merge::consumer_input("lexical832", path, source);
        let source = projected.as_str();
        let row = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == path)
            .unwrap();
        let mut current = source.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(current.matches(new).count(), 1, "{path}");
            current = current.replacen(new, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(
            current,
            row["original_bytes"].as_str().unwrap(),
            "whole original {path}"
        );
    }
    assert_eq!(
        include_str!("../before_operands.rs"),
        include_str!(
            "../../../../../docs/evidence/ordinary-lexical-return-operands-832/original/src__cleanup_plan__replay__lexical_replay.rs.before.txt"
        )
    );
}
