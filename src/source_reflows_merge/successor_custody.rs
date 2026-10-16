//! Final fixed test-only custody. No arbitrary path or compiler authority.
use sha2::{Digest, Sha256};
#[derive(Clone, Copy)]
pub(super) enum Fixture {
    Sealed724,
    MainAfba,
}

pub(super) struct Spec {
    pub(super) path: &'static str,
    pub(super) length: usize,
    pub(super) sha256: [u8; 32],
}
impl Fixture {
    pub(super) fn spec(self) -> Spec {
        match self {
            Self::Sealed724 => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/afba-presence-successor-20261016/current-to-sealed724.json"
                ),
                length: 73519,
                sha256: [
                    166, 121, 44, 137, 14, 18, 56, 161, 104, 136, 13, 249, 66, 3, 251, 130, 76,
                    166, 3, 209, 157, 67, 47, 96, 78, 210, 13, 62, 98, 249, 1, 203,
                ],
            },
            Self::MainAfba => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/afba-presence-successor-20261016/current-to-main-afba.json"
                ),
                length: 163627,
                sha256: [
                    49, 176, 64, 220, 120, 198, 131, 88, 91, 73, 138, 177, 109, 16, 193, 147, 155,
                    32, 237, 124, 58, 42, 116, 194, 156, 67, 148, 230, 82, 129, 7, 96,
                ],
            },
        }
    }
    pub(super) fn keys(self) -> &'static [&'static str] {
        match self {
Self::Sealed724 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/disposal.rs", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/NESTED-OWNED-BYTE-RECORDS-V1.md", "docs/ORDINARY-RENEWAL-DIRECTORY-V1.md", "src/cleanup_plan/renewal_profile/ordinary/mod.rs", "src/cleanup_plan/renewal_profile/ordinary/record_presence.rs", "src/cleanup_plan/renewal_profile/ordinary/record_presence/tests.rs", "src/hir/expr_nodes.rs", "src/hir/validation.rs", "src/loop_calls/byte_writer/module_index/ordinary.rs", "src/source_reflows.rs", "src/source_reflows_merge.rs", "src/source_reflows_merge/runtime_json.rs", "src/source_reflows_merge/runtime_json/tests.rs", "tests/hir_module_boundaries.rs", "tests/owned_data/byte_record_algebra.rs", "tests/owned_data/nested_owned_record_frontend_hir.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
Self::MainAfba => &["src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone/tests.rs", "src/cleanup_plan/replay/factored.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs", "src/cleanup_plan/replay/factored/oracle_hook.rs", "src/hir.rs", "src/hir/expr_nodes.rs", "src/hir/validation.rs", "src/source_verify/declaration.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
}
    }
}
const COMMON: [u8; 32] = [
    224, 32, 152, 159, 129, 201, 80, 34, 96, 202, 175, 135, 136, 140, 232, 108, 157, 21, 27, 232,
    9, 108, 210, 147, 15, 150, 249, 79, 83, 89, 152, 65,
];
const EMBEDDED: [u8; 32] = [
    97, 110, 92, 175, 129, 74, 192, 4, 47, 105, 200, 67, 215, 223, 192, 180, 210, 103, 6, 40, 98,
    84, 103, 210, 21, 220, 183, 190, 57, 30, 215, 127,
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
        &super::runtime_json::successor_custody(fixture),
        source,
        current,
    )
}
