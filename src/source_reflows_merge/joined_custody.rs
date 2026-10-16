//! Final fixed test-only custody. No arbitrary path or compiler authority.
use sha2::{Digest, Sha256};
#[derive(Clone, Copy)]
pub(super) enum Fixture {
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
            Self::MainF1 => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/f1-pieces-compile-joined-20261016/current-to-mainf1.json"
                ),
                length: 237392,
                sha256: [
                    145, 147, 200, 25, 237, 158, 187, 135, 93, 7, 69, 200, 70, 77, 13, 178, 1, 160,
                    156, 158, 66, 212, 160, 204, 171, 219, 150, 40, 193, 20, 147, 159,
                ],
            },
        }
    }
    pub(super) fn keys(self) -> &'static [&'static str] {
        match self {
Self::MainF1 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "src/cleanup_plan/replay/flat_records/paid_leaves.rs", "src/cleanup_plan/replay/lexical_boundary/paid.rs", "src/cleanup_plan/replay/structural_maps/paid.rs", "src/source_reflows_merge.rs", "src/source_reflows_merge/runtime_json.rs", "src/source_reflows_merge/runtime_json/tests.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
}
    }
}
const COMMON: [u8; 32] = [
    156, 178, 157, 246, 173, 64, 131, 169, 118, 72, 181, 104, 127, 131, 216, 90, 44, 158, 46, 130,
    96, 119, 56, 27, 125, 164, 67, 61, 240, 52, 18, 214,
];
const EMBEDDED: [u8; 32] = [
    216, 204, 17, 9, 210, 41, 192, 80, 41, 84, 96, 121, 154, 160, 139, 175, 57, 215, 178, 177, 53,
    73, 97, 122, 153, 193, 91, 18, 144, 88, 167, 40,
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
pub(super) fn input(source: &str, current: &str) -> String {
    let normalized = normalize(source, current);
    project(Fixture::MainF1, source, &normalized)
}
pub(super) fn project(fixture: Fixture, source: &str, current: &str) -> String {
    if fixture.keys().binary_search(&source).is_err() {
        return current.to_owned();
    }
    super::project(
        &super::runtime_json::joined_custody(fixture),
        source,
        current,
    )
}
