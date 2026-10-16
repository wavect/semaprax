//! Final fixed test-only custody. No arbitrary path or compiler authority.
use sha2::{Digest, Sha256};
#[derive(Clone, Copy)]
pub(super) enum Fixture {
    Previous2c,
    Main24,
    Receivers24,
    Compact24,
}

pub(super) struct Spec {
    pub(super) path: &'static str,
    pub(super) length: usize,
    pub(super) sha256: [u8; 32],
}
impl Fixture {
    pub(super) fn spec(self) -> Spec {
        match self {
Self::Previous2c => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/release-755-main24-arena-joined-20261016/current-to-previous2c.json"), length: 111551, sha256: [79, 154, 223, 51, 207, 32, 204, 75, 17, 216, 203, 18, 198, 30, 38, 24, 201, 67, 191, 164, 36, 132, 114, 22, 173, 149, 231, 92, 127, 147, 115, 206] },
Self::Main24 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/release-755-main24-arena-joined-20261016/current-to-main24.json"), length: 203022, sha256: [239, 14, 154, 73, 101, 42, 200, 80, 10, 141, 64, 31, 202, 58, 153, 184, 6, 191, 63, 90, 66, 195, 64, 23, 242, 88, 66, 180, 110, 96, 137, 44] },
Self::Receivers24 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main24-prefix-independent-next-20261016/receiver-inverse12.json"), length: 53838, sha256: [65, 255, 50, 253, 86, 33, 14, 193, 62, 96, 129, 220, 75, 216, 30, 165, 105, 74, 89, 196, 36, 94, 205, 254, 18, 165, 227, 208, 72, 65, 183, 167] },
Self::Compact24 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main24-prefix-independent-next-20261016/compact-inverse7.json"), length: 27233, sha256: [100, 4, 179, 229, 215, 216, 245, 52, 31, 228, 12, 23, 255, 9, 44, 28, 36, 218, 162, 250, 195, 125, 255, 77, 163, 19, 246, 54, 180, 178, 13, 18] },
}
    }
    pub(super) fn keys(self) -> &'static [&'static str] {
        match self {
Self::Previous2c => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "docs/SELECTED-REPLAY-RECOVERING-CONSTRUCTOR-V1.md", "src/cleanup_plan/renewal_profile.rs", "src/cleanup_plan/renewal_profile/child_refusal.rs", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/factored/canonical_shared.rs", "src/cleanup_plan/replay/factored/canonical_shared/arena_source.rs", "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs", "src/cleanup_plan/replay/factored/canonical_shared/before_arena.rs", "src/cleanup_plan/replay/flat_records/tests.rs", "src/cleanup_plan/replay/lexical_boundary.rs", "src/cleanup_plan/replay/lexical_boundary/paid.rs", "src/cleanup_plan/replay/lexical_cert/paid.rs", "src/cleanup_plan/replay/lexical_cert/paid/storage.rs", "src/cleanup_plan/replay/path_join.rs", "src/cleanup_plan/replay/semantic_refusal.rs", "src/source_reflows/semantic_receivers_main_f1.rs", "src/source_reflows_merge.rs", "src/source_reflows_merge/runtime_json.rs", "src/source_reflows_merge/runtime_json/tests.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
Self::Main24 => &["src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone/tests.rs", "src/cleanup_plan/replay/factored.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs", "src/cleanup_plan/replay/factored/oracle_hook.rs", "src/hir.rs", "src/hir/expr_nodes.rs", "src/hir/validation.rs", "src/source_verify/declaration.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
Self::Receivers24 => &[],
Self::Compact24 => &[],
}
    }
}
const COMMON: [u8; 32] = [
    4, 133, 132, 163, 135, 225, 156, 228, 77, 70, 66, 128, 114, 219, 66, 254, 221, 74, 79, 148,
    246, 127, 63, 172, 6, 34, 162, 206, 63, 192, 150, 31,
];
const EMBEDDED: [u8; 32] = [
    224, 136, 184, 61, 10, 66, 166, 81, 193, 37, 230, 36, 179, 140, 220, 22, 5, 77, 42, 189, 166,
    14, 68, 224, 186, 141, 141, 150, 39, 252, 81, 122,
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
        &super::runtime_json::release_custody(fixture),
        source,
        current,
    )
}
