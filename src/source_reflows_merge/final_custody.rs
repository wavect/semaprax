//! Final fixed test-only custody. No arbitrary path or compiler authority.
use sha2::{Digest, Sha256};
#[derive(Clone, Copy)]
pub(super) enum Fixture {
    Common,
    Arena,
    Main7c6,
    ArenaReceiving,
    ArenaJoined,
    ArenaPrevious,
}
pub(super) const ALL: &[Fixture] = &[
    Fixture::Common,
    Fixture::Arena,
    Fixture::Main7c6,
    Fixture::ArenaReceiving,
    Fixture::ArenaJoined,
    Fixture::ArenaPrevious,
];
pub(super) struct Spec {
    pub(super) path: &'static str,
    pub(super) length: usize,
    pub(super) sha256: [u8; 32],
}
impl Fixture {
    pub(super) fn spec(self) -> Spec {
        match self {
Self::Common => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/owned-session-arena-bulk-joined-20261016/current-to-common.json"), length: 198673, sha256: [114, 50, 186, 195, 208, 185, 226, 56, 177, 183, 107, 183, 117, 7, 57, 202, 166, 172, 211, 97, 161, 107, 206, 188, 174, 70, 54, 72, 191, 33, 23, 110] },
Self::Arena => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/owned-session-arena-bulk-joined-20261016/current-to-arena426.json"), length: 221939, sha256: [220, 222, 160, 101, 170, 206, 159, 175, 179, 126, 109, 192, 21, 215, 220, 211, 61, 229, 229, 7, 174, 217, 15, 34, 153, 190, 201, 245, 1, 128, 67, 10] },
Self::Main7c6 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/owned-session-arena-bulk-joined-20261016/current-to-main7c6.json"), length: 161484, sha256: [137, 21, 185, 35, 231, 69, 85, 98, 197, 27, 47, 76, 222, 39, 134, 226, 250, 135, 243, 99, 203, 115, 34, 111, 197, 130, 6, 241, 59, 17, 182, 252] },
Self::ArenaReceiving => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/canonical-intern-arena-854-receiving-20261016/current-to-9db.json"), length: 11008, sha256: [133, 126, 166, 41, 235, 145, 192, 14, 146, 135, 106, 213, 116, 33, 238, 15, 111, 135, 103, 215, 231, 146, 246, 176, 218, 135, 15, 184, 59, 242, 199, 140] },
Self::ArenaJoined => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/canonical-intern-arena-854-joined-20261016/current-to-ab13.json"), length: 49169, sha256: [30, 82, 226, 146, 89, 121, 36, 111, 147, 163, 140, 254, 207, 169, 94, 17, 123, 25, 183, 97, 34, 53, 82, 138, 98, 214, 74, 15, 29, 108, 97, 9] },
Self::ArenaPrevious => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/canonical-intern-arena-854-next-20261016/current-to-3c388.json"), length: 13897, sha256: [242, 136, 210, 129, 78, 251, 57, 217, 75, 251, 54, 79, 2, 218, 19, 67, 244, 54, 207, 255, 194, 100, 133, 236, 71, 75, 17, 129, 176, 98, 238, 107] },
}
    }
    pub(super) fn keys(self) -> &'static [&'static str] {
        match self {
Self::Common => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/capacity/hir_owned.rs", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/hir_traversal.rs", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "docs/SELECTED-REPLAY-RECOVERING-CONSTRUCTOR-V1.md", "src/cleanup_plan/renewal_profile.rs", "src/cleanup_plan/renewal_profile/child_refusal.rs", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/compact_stage.rs", "src/cleanup_plan/replay/factored.rs", "src/cleanup_plan/replay/factored/canonical_shared.rs", "src/cleanup_plan/replay/factored/canonical_shared/inline_weighted.rs", "src/cleanup_plan/replay/flat_records.rs", "src/cleanup_plan/replay/flat_records/tests.rs", "src/cleanup_plan/replay/leaf_index.rs", "src/cleanup_plan/replay/lexical_boundary.rs", "src/cleanup_plan/replay/lexical_cert.rs", "src/cleanup_plan/replay/lexical_replay.rs", "src/cleanup_plan/replay/resolved_call.rs", "src/cleanup_plan/replay/scalar_replay.rs", "src/cleanup_plan/replay/scalar_replay/records.rs", "src/cleanup_plan/replay/selected_context/child.rs", "src/cleanup_plan/replay/selected_profile/paid_children.rs", "src/cleanup_plan/replay/semantic_refusal.rs", "src/cleanup_plan/replay/status_construction.rs", "src/cleanup_plan/replay/structural_maps.rs", "src/codegen/native_emit/expression/owned_try.rs", "src/hir.rs", "src/hir/cache_codec.rs", "src/hir/capacity_probe.rs", "src/hir/expr_nodes.rs", "src/hir/monomorphize.rs", "src/hir/owned_result_try.rs", "src/hir/resolve_program.rs", "src/hir/validation.rs", "src/interpreter/owned_try.rs", "src/loan_plan/children_tests.rs", "src/loan_plan/tests.rs", "src/source_reflows.rs", "src/source_reflows_merge.rs", "src/source_reflows_merge/metadata_json.rs", "src/source_reflows_merge/runtime_json.rs", "src/source_reflows_merge/runtime_json/tests.rs", "src/wasm/aggregate.rs", "src/workspace_graph/expected_projection/declaration_cost.rs", "tests/hir_module_boundaries.rs", "tests/language/owned_result_variants.rs", "tests/language/result_try_semantics.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
Self::Arena => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/capacity/hir_owned.rs", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/hir_traversal.rs", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "docs/SELECTED-REPLAY-RECOVERING-CONSTRUCTOR-V1.md", "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs", "src/cleanup_plan/renewal_profile.rs", "src/cleanup_plan/renewal_profile/child_refusal.rs", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/compact_stage.rs", "src/cleanup_plan/replay/compact_stage/source_custody.rs", "src/cleanup_plan/replay/flat_records.rs", "src/cleanup_plan/replay/flat_records/tests.rs", "src/cleanup_plan/replay/leaf_index.rs", "src/cleanup_plan/replay/lexical_boundary.rs", "src/cleanup_plan/replay/lexical_cert.rs", "src/cleanup_plan/replay/lexical_replay.rs", "src/cleanup_plan/replay/resolved_call.rs", "src/cleanup_plan/replay/scalar_replay.rs", "src/cleanup_plan/replay/scalar_replay/records.rs", "src/cleanup_plan/replay/selected_context/child.rs", "src/cleanup_plan/replay/selected_entry/tests/driver_local_receivers.rs", "src/cleanup_plan/replay/selected_profile/paid_children.rs", "src/cleanup_plan/replay/semantic_refusal.rs", "src/cleanup_plan/replay/status_construction.rs", "src/cleanup_plan/replay/structural_maps.rs", "src/cleanup_plan/replay/structural_maps/tests.rs", "src/codegen/native_emit/expression/owned_try.rs", "src/hir.rs", "src/hir/cache_codec.rs", "src/hir/capacity_probe.rs", "src/hir/expr_nodes.rs", "src/hir/monomorphize.rs", "src/hir/owned_result_try.rs", "src/hir/resolve_program.rs", "src/hir/validation.rs", "src/interpreter/owned_try.rs", "src/loan_plan/children_tests.rs", "src/loan_plan/tests.rs", "src/source_reflows.rs", "src/source_reflows_merge.rs", "src/source_reflows_merge/runtime_json.rs", "src/source_reflows_merge/runtime_json/tests.rs", "src/wasm/aggregate.rs", "src/workspace_graph/expected_projection/declaration_cost.rs", "src/workspace_graph/type_proof_tests.rs", "tests/hir_module_boundaries.rs", "tests/language/owned_result_variants.rs", "tests/language/result_try_semantics.rs", "tests/native/string_settlement/bulk_utf8.rs", "tests/native/string_settlement/codec_allocation.rs", "tests/native/string_settlement/codec_allocation/string_vectors.rs", "tests/native_string_settlement_v1/contents.rs", "tests/owned_data/bulk_utf8.rs", "tests/owned_data/owned_nested_outcome.rs", "tests/owned_data/projected_string_views.rs", "tests/root_unsafe_quarantine.rs", "tests/wasm/scalar_browser_ci_contract.rs", "tests/wasm/scalar_browser_ci_contract/browser_custody.rs", "tests/wasm/scalar_browser_ci_contract/historical_builder.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
Self::Main7c6 => &["src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone/tests.rs", "src/cleanup_plan/replay/factored.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs", "src/cleanup_plan/replay/factored/oracle_hook.rs", "src/hir.rs", "src/hir/expr_nodes.rs", "src/hir/validation.rs", "src/source_verify/declaration.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
Self::ArenaReceiving => &["CHANGELOG.md", "src/cleanup_plan/replay/factored/canonical_shared.rs", "src/cleanup_plan/replay/factored/canonical_shared/arena_source.rs", "src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs"],
Self::ArenaJoined => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "src/cleanup_plan/replay/factored/canonical_shared.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
Self::ArenaPrevious => &["src/cleanup_plan/replay/factored.rs", "src/cleanup_plan/replay/factored/canonical_shared.rs", "src/cleanup_plan/replay/factored/canonical_shared/inline_weighted.rs"],
}
    }
}
const COMMON: [u8; 32] = [
    41, 40, 101, 158, 217, 171, 243, 250, 240, 237, 253, 161, 198, 42, 165, 177, 143, 227, 67, 77,
    0, 134, 28, 104, 74, 11, 253, 55, 4, 97, 129, 96,
];
const EMBEDDED: [u8; 32] = [
    239, 44, 155, 51, 19, 147, 94, 2, 167, 72, 25, 140, 242, 29, 31, 245, 50, 214, 83, 23, 13, 243,
    163, 17, 28, 197, 73, 133, 69, 100, 147, 8,
];
pub(super) fn normalize(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge/runtime_json.rs" {
        return current.to_owned();
    }
    let actual: [u8; 32] = Sha256::digest(current.as_bytes()).into();
    if actual == COMMON {
        return current.to_owned();
    }
    assert_eq!(actual, EMBEDDED, "complete final transport-arm source");
    assert_eq!(
        current
            .matches("runtime_json/transport_embedded.rs")
            .count(),
        1
    );
    let normalized = current.replacen(
        "runtime_json/transport_embedded.rs",
        "runtime_json/transport_runtime.rs",
        1,
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(normalized.as_bytes())),
        COMMON
    );
    normalized
}
// Use only at an actual raw entry. Historical inner stages call project directly.
pub(super) fn input(fixture: Fixture, source: &str, current: &str) -> String {
    let normalized = normalize(source, current);
    project(fixture, source, &normalized)
}
pub(super) fn project(fixture: Fixture, source: &str, current: &str) -> String {
    if fixture.keys().binary_search(&source).is_err() {
        return current.to_owned();
    }
    super::project(
        &super::runtime_json::final_custody(fixture),
        source,
        current,
    )
}
