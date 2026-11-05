//! Whole-input source custody; no omitted literal, helper or assertion bodies.
use sha2::{Digest, Sha256};
const DATA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/held-closure-receiver-843/source-inverses.json"
));
fn hash(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
#[test]
fn held_closure_every_source_and_full_immediate_inverse_are_exact() {
    let rows: serde_json::Value = serde_json::from_str(DATA).unwrap();
    let sources = SOURCES;
    assert_eq!(sources.len(), rows.as_array().unwrap().len());
    for &(path, current) in sources {
        let projected = crate::source_reflows_merge::assembly_view("843", path, current);
        let current = projected.as_str();
        let rows: Vec<_> = rows
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["path"] == path)
            .collect();
        assert_eq!(rows.len(), 1);
        let row = rows[0];
        let acquired = crate::source_reflows_merge::main_e177_final_inputs::value(
            path,
            row["after_sha256"].as_str().unwrap(),
            current,
        );
        let current = acquired.as_str();
        assert_eq!(hash(current), row["after_sha256"]);
        assert_eq!(current, row["after"].as_str().unwrap());
        if let Some(before) = row["before"].as_str() {
            assert_eq!(hash(before), row["before_sha256"]);
            assert_eq!(
                crate::source_reflows_merge::closure_previous(path, current),
                before
            );
            assert_eq!(current.replacen(current, before, 1), before);
            assert_eq!(before.replacen(before, current, 1), current);
        } else {
            assert!(row["before_sha256"].is_null());
        }
    }
}
#[test]
fn held_closure_complete_old_canonical_and_caller_are_independent_byte_exact_bodies() {
    let old = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/held-closure-receiver-843/canonical_shared.rs.before.txt"
    ));
    let body = old
        .split_once("type SharedWeighted")
        .unwrap()
        .1
        .split_once("\n#[cfg(test)]\nmod tests;")
        .unwrap()
        .0;
    let old_body = format!("type SharedWeighted{body}")
        .replace("    fn closure(", "    pub(super) fn closure(")
        .replace("    fn successors(", "    pub(super) fn successors(")
        .replace(
            "pub(super) struct Canonicalizer",
            "pub(in crate::cleanup_plan::replay::factored) struct Canonicalizer",
        )
        .replace(
            "    pub(super) fn new(",
            "    pub(in crate::cleanup_plan::replay::factored) fn new(",
        )
        .replace(
            "    pub(super) fn id(",
            "    pub(in crate::cleanup_plan::replay::factored) fn id(",
        );
    let mut old_body = old_body;
    // Exact compact-node API wiring; every original algorithm byte remains compared.
    for (before, after, count) in [
        (
            "for target in &state.epsilon {",
            "for target in state.epsilon() {",
            2,
        ),
        ("state.moves.is_empty()", "state.moves().is_empty()", 2),
        ("state.epsilon.len()", "state.epsilon().len()", 1),
        ("state.epsilon[0]", "state.epsilon()[0]", 1),
        (
            "let moves = &self.automaton.nodes[*node].moves;",
            "let moves = self.automaton.nodes[*node].moves();",
            1,
        ),
    ] {
        assert_eq!(old_body.matches(before).count(), count);
        old_body = old_body.replace(before, after);
    }
    let full = crate::source_reflows::original_fixture_source(
        "src/cleanup_plan/replay/factored/canonical_shared/before_closure_receiver.rs",
        "4ff99e3d3876f3fd2db66621b6efed4bda76e80542e37be27283ef6e0dd3b7e9",
        include_str!("../before_closure_receiver.rs"),
    );
    let actual = full
        .split_once("type SharedWeighted")
        .unwrap()
        .1
        .split_once("\nimpl Canonicalizer<'_> {")
        .unwrap()
        .0;
    assert_eq!(format!("type SharedWeighted{actual}"), old_body);
    assert!(full.contains("ordered_weights, successor_targets, tests};"));
    assert!(full.contains("closures: memo::RetainedMemo<Rc<ordered_weights::OrderedWeights>>"));
    let old_caller = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/held-closure-receiver-843/factored.rs.before.txt"
    ));
    let body = old_caller
        .split_once("fn compare_with_index_storage<")
        .unwrap()
        .1
        .split_once("\nstruct BuiltComparison")
        .unwrap()
        .0;
    let expected = format!("pub(super) fn compare_with_index_storage<{body}\n");
    let caller = crate::source_reflows::original_fixture_source(
        "src/cleanup_plan/replay/factored/before_closure_invocation.rs",
        "a7ae1113a5ef03e8aedb9e842c6ae2ae09d7e0667190cb6ace342326b55558ce",
        include_str!("../../before_closure_invocation.rs"),
    );
    assert_eq!(
        caller
            .split_once("pub(super) fn compare_with_index_storage<")
            .map(|(_, tail)| format!("pub(super) fn compare_with_index_storage<{tail}"))
            .unwrap(),
        expected
    );
    assert!(caller
        .contains("use super::canonical_shared::before_closure_receiver as canonical_shared;"));
    let current = include_str!("../../canonical_shared.rs");
    let producer = current
        .split_once("    fn closure_inner_into(")
        .unwrap()
        .1
        .split_once("    fn successors_with_receiving(")
        .unwrap()
        .0;
    assert!(
        !producer.contains("-> Result")
            && !producer.contains("return Err(")
            && !producer.contains("return Ok(")
    );
    assert!(
        producer.contains("receiving.finish_overflow()")
            && producer.contains("receiving.refuse(error)")
    );
    let destination = include_str!("../closure_receiving.rs");
    assert!(destination.contains("pub(super) fn finish_overflow(&mut self) {\n        self.overflow = true;\n        self.ready = true;\n    }"));
    assert!(current.contains("if receiving.overflow {\n            return Ok(None);\n        }"));
    let boundary = current
        .split_once("    fn closure_with_receiving(")
        .unwrap()
        .1
        .split_once("    fn closure_inner_into(")
        .unwrap()
        .0;
    assert!(boundary.contains("if receiving.refusal.is_some()"));
    assert!(boundary.contains("self.closure_inner_into(function, root, budget, receiving);"));
    assert!(
        !boundary.contains("Result<Option<Rc<ordered_weights::OrderedWeights>>, Diagnostic>>()")
    );
}
// Literal inventory generated once from the complete candidate, not globbed.
const SOURCES: &[(&str, &str)] = &[
    (
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
        )),
    ),
    (
        "src/cleanup_plan/replay/factored.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/factored.rs"
        )),
    ),
    (
        "src/cleanup_plan/replay/factored/before_closure_invocation.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/factored/before_closure_invocation.rs"
        )),
    ),
    (
        "src/cleanup_plan/replay/factored/canonical_shared.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/factored/canonical_shared.rs"
        )),
    ),
    (
        "src/cleanup_plan/replay/factored/canonical_shared/before_closure_receiver.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/factored/canonical_shared/before_closure_receiver.rs"
        )),
    ),
    (
        "src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs"
        )),
    ),
    (
        "src/cleanup_plan/replay/factored/canonical_shared/closure_receiving/source.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/factored/canonical_shared/closure_receiving/source.rs"
        )),
    ),
    (
        "src/cleanup_plan/replay/factored/canonical_shared/closure_receiving/tests.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/factored/canonical_shared/closure_receiving/tests.rs"
        )),
    ),
    (
        "src/source_reflows_merge.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/source_reflows_merge.rs"
        )),
    ),
    (
        "tests/hir_module_boundaries.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/hir_module_boundaries.rs"
        )),
    ),
    (
        "tests/workspace/semantic_graph/source_inventory.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/workspace/semantic_graph/source_inventory.rs"
        )),
    ),
];
