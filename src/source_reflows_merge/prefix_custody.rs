//! Final fixed test-only custody. No arbitrary path or compiler authority.
use sha2::{Digest, Sha256};
#[derive(Clone, Copy)]
pub(super) enum Fixture {
    SealedB5,
    MainF1,
}

pub(super) struct Spec {
    pub(super) path: &'static str,
    pub(super) length: usize,
    pub(super) sha256: [u8; 32],
}
impl Fixture {
    pub(super) fn spec(self) -> Spec {
        match self {
Self::SealedB5 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main-f1-prefix-lexical-subsumption-20261016/current-to-sealedb5.json"), length: 51913, sha256: [232, 175, 124, 110, 120, 26, 139, 202, 151, 161, 238, 218, 11, 78, 99, 107, 184, 227, 213, 154, 24, 126, 95, 39, 27, 53, 121, 132, 7, 150, 239, 246] },
Self::MainF1 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main-f1-prefix-lexical-subsumption-20261016/current-to-main-f1.json"), length: 166453, sha256: [34, 16, 209, 210, 37, 81, 72, 121, 172, 90, 53, 176, 126, 120, 186, 163, 11, 160, 107, 207, 29, 44, 53, 179, 25, 23, 195, 64, 4, 60, 133, 138] },
}
    }
    pub(super) fn keys(self) -> &'static [&'static str] {
        match self {
Self::SealedB5 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "src/cleanup_plan/replay.rs", "src/source_reflows.rs", "src/source_reflows_merge.rs", "src/source_reflows_merge/runtime_json.rs", "src/source_reflows_merge/runtime_json/tests.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
Self::MainF1 => &["src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone/tests.rs", "src/cleanup_plan/replay/factored.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs", "src/cleanup_plan/replay/factored/oracle_hook.rs", "src/hir.rs", "src/hir/expr_nodes.rs", "src/hir/validation.rs", "src/source_verify/declaration.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
}
    }
}
const COMMON: [u8; 32] = [
    213, 120, 7, 214, 148, 218, 74, 175, 232, 245, 68, 52, 193, 26, 47, 116, 97, 125, 38, 110, 37,
    17, 0, 49, 167, 104, 147, 122, 147, 212, 219, 199,
];
const EMBEDDED: [u8; 32] = [
    101, 222, 108, 93, 219, 42, 145, 6, 9, 133, 249, 172, 21, 235, 76, 253, 70, 38, 134, 129, 88,
    223, 36, 8, 104, 99, 190, 14, 246, 203, 23, 57,
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
        &super::runtime_json::prefix_custody(fixture),
        source,
        current,
    )
}
