//! Exact full-source custody for frozen Main44 source predicates.
const INPUTS: &[(&str, &str, &str)] = &[
    ("src/cleanup_plan/replay.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay.rs.txt"))),
    ("src/cleanup_plan/replay/branch_clone/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/branch_clone/tests.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/branch_clone/tests.rs.txt"))),
    ("src/cleanup_plan/replay/compact_stage/before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/compact_stage/before.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/compact_stage/before.rs.txt"))),
    ("src/cleanup_plan/replay/compact_stage/lexical_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/compact_stage/lexical_before.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/compact_stage/lexical_before.rs.txt"))),
    ("src/cleanup_plan/replay/factored.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/factored.rs.txt"))),
    ("src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs.txt"))),
    ("src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs.txt"))),
    ("src/cleanup_plan/replay/factored/oracle_hook.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/oracle_hook.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/factored/oracle_hook.rs.txt"))),
    ("src/cleanup_plan/replay/path_array.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/path_array.rs.txt"))),
    ("src/cleanup_plan/replay/path_array/before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array/before.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/path_array/before.rs.txt"))),
    ("src/cleanup_plan/replay/path_array/final_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array/final_before.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/path_array/final_before.rs.txt"))),
    ("src/cleanup_plan/replay/path_array/sequence.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array/sequence.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/path_array/sequence.rs.txt"))),
    ("src/cleanup_plan/replay/path_array/sequence/before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array/sequence/before.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/path_array/sequence/before.rs.txt"))),
    ("src/cleanup_plan/replay/path_array/sequence/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array/sequence/tests.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/path_array/sequence/tests.rs.txt"))),
    ("src/cleanup_plan/replay/path_array/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array/tests.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/path_array/tests.rs.txt"))),
    ("src/cleanup_plan/replay/sequence_prefix/before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/sequence_prefix/before.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/sequence_prefix/before.rs.txt"))),
    ("src/cleanup_plan/replay/sequence_vector.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/sequence_vector.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/sequence_vector.rs.txt"))),
    ("src/cleanup_plan/replay/sequence_vector/before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array/main44/vector_before.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/sequence_vector/before.rs.txt"))),
    ("src/cleanup_plan/replay/sequence_vector/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_array/main44/vector_tests.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/sequence_vector/tests.rs.txt"))),
    ("src/cleanup_plan/replay/skeleton_work.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/cleanup_plan/replay/skeleton_work.rs.txt"))),
    ("src/hir.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/hir.rs.txt"))),
    ("src/hir/expr_nodes.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/expr_nodes.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/hir/expr_nodes.rs.txt"))),
    ("src/hir/validation.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/hir/validation.rs.txt"))),
    ("src/source_verify/declaration.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/declaration.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/src/source_verify/declaration.rs.txt"))),
    ("tests/hir_module_boundaries.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/tests/hir_module_boundaries.rs.txt"))),
    ("tests/workspace/semantic_graph/source_inventory.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/raw/tests/workspace/semantic_graph/source_inventory.rs.txt"))),

];
#[test]
fn every_current_source_has_the_exact_complete_incoming_main44_preimage() {
    for (path, current, original) in INPUTS {
        let restored = crate::source_reflows_merge::main44_input(path, current);
        assert_eq!(
            restored.as_str(),
            *original,
            "complete Main44 source domain: {path}"
        );
        let changed = format!("{current} ");
        assert!(
            std::panic::catch_unwind(|| crate::source_reflows_merge::main44_input(path, &changed))
                .is_err(),
            "physical source drift: {path}"
        );
    }
}

pub(super) fn current(source: &str) -> Option<&'static str> {
    INPUTS
        .iter()
        .find_map(|(path, current, _)| (*path == source).then_some(*current))
}
