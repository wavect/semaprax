//! Fixed complete incoming Main8a metadata. No arbitrary path or compiler authority.
#[derive(Clone, Copy)]
pub(crate) enum Fixture {
    Compact7,
    Receivers12,
    LegacyNominal18,
    PrefixCertified3,
    SelectedSession19,
}
pub(super) struct Spec {
    pub(super) path: &'static str,
    pub(super) length: usize,
    pub(super) sha256: [u8; 32],
}
impl Fixture {
    pub(super) fn spec(self) -> Spec {
        match self {
            Self::Compact7 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main8a-independent-custody-20261016/compact-inverse7.json"), length: 52575, sha256: [216, 188, 115, 221, 226, 74, 75, 44, 195, 76, 189, 126, 72, 54, 247, 52, 225, 233, 231, 35, 125, 117, 99, 44, 81, 168, 126, 10, 206, 255, 121, 10] },
            Self::Receivers12 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main8a-independent-custody-20261016/receiver-inverse12.json"), length: 79180, sha256: [60, 83, 218, 107, 27, 36, 23, 101, 137, 223, 140, 169, 245, 112, 24, 35, 67, 28, 68, 226, 37, 90, 246, 31, 170, 196, 209, 127, 176, 122, 141, 204] },
            Self::LegacyNominal18 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main8a-independent-custody-20261016/legacy-nominal-inverse18.json"), length: 894317, sha256: [250, 117, 99, 118, 148, 37, 14, 203, 253, 197, 98, 10, 187, 50, 5, 125, 177, 71, 204, 22, 165, 134, 81, 209, 174, 191, 157, 149, 200, 15, 227, 102] },
            Self::PrefixCertified3 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main8a-independent-custody-20261016/prefix-certified-inverse3.json"), length: 1055184, sha256: [72, 253, 65, 133, 229, 140, 37, 48, 167, 12, 125, 244, 110, 42, 138, 197, 110, 196, 230, 249, 80, 122, 10, 28, 204, 233, 195, 94, 231, 215, 221, 27] },
            Self::SelectedSession19 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main8a-independent-custody-20261016/selected-session-inverse19.json"), length: 2037683, sha256: [203, 231, 208, 198, 62, 201, 57, 3, 121, 224, 169, 103, 202, 119, 123, 238, 243, 8, 255, 78, 71, 82, 159, 177, 120, 32, 79, 50, 239, 249, 199, 140] },
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Input {
    Previous55e,
    Main8a,
}
impl Input {
    pub(super) fn keys(self) -> &'static [&'static str] {
        match self {
Self::Previous55e => &[".github/workflows/ci.yml", "CHANGELOG.md", "Cargo.toml", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/scope_projection.rs", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "crates/semaprax-toolchain/tests/cli_help_surface_v1.rs", "docs/ARCHITECTURE.md", "docs/FINITE-OWNED-RECORD-DISCARD-LOOPS-V1.md", "docs/SUMMARY.md", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/compact_stage.rs", "src/cleanup_plan/replay/compact_stage/tests.rs", "src/cleanup_plan/replay/expression_facts/arena/prefix.rs", "src/cleanup_plan/replay/factored/oracle_order/tests.rs", "src/cleanup_plan/replay/final_output.rs", "src/cleanup_plan/replay/path_array.rs", "src/cleanup_plan/replay/selected_entry/tests/session_errors.rs", "src/graph/copy_discard.rs", "src/graph/owned_collection_records.rs", "src/graph/owned_nested_outcome.rs", "src/hir/items.rs", "src/hir/nominal_payload/legacy.rs", "src/hir/resolver_scope/source_audit.rs", "src/source_reflows.rs", "src/source_reflows_merge.rs", "src/source_reflows_merge/runtime_json.rs", "src/workspace_graph/source_schema.rs", "tests/doctor_production_provisioner_v1.rs", "tests/hir_module_boundaries.rs", "tests/hir_module_boundaries/oracle_source_inventory.rs", "tests/quality_routing.rs", "tests/wasm/scalar_browser_ci_contract/browser_custody.rs", "tests/wasm/scalar_browser_ci_contract/historical_builder.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
Self::Main8a => &["crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone/tests.rs", "src/cleanup_plan/replay/compact_stage/before.rs", "src/cleanup_plan/replay/compact_stage/lexical_before.rs", "src/cleanup_plan/replay/factored.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs", "src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "src/cleanup_plan/replay/selected_context.rs", "src/cleanup_plan/replay/selected_context/paid.rs", "src/cleanup_plan/replay/selected_entry.rs", "src/cleanup_plan/replay/selected_entry/tests.rs", "src/cleanup_plan/replay/selected_entry/tests/session_errors.rs", "src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs", "src/cleanup_plan/replay/selected_owner.rs", "src/cleanup_plan/replay/selected_owner/paid.rs", "src/cleanup_plan/replay/selected_profile.rs", "src/cleanup_plan/replay/selected_profile/paid.rs", "src/cleanup_plan/replay/semantic_refusal.rs", "src/cleanup_plan/replay/semantic_refusal/tests.rs", "src/hir.rs", "src/hir/expr_nodes.rs", "src/hir/identity_resolution/nominal_tests/legacy.rs", "src/hir/nominal_payload/legacy.rs", "src/hir/nominal_payload/legacy/source_tests.rs", "src/hir/validation.rs", "src/source_reflows.rs", "tests/hir_module_boundaries.rs", "tests/wasm/scalar_browser_ci_contract.rs", "tests/wasm/scalar_browser_ci_contract/historical_builder.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
}
    }
    pub(super) fn spec(self) -> Spec {
        match self {
            Self::Previous55e => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/main8a-independent-custody-20261016/current-to55e.json"
                ),
                length: 182613,
                sha256: [40, 102, 75, 127, 244, 98, 164, 238, 101, 133, 189, 187, 69, 96, 148, 218, 93, 60, 102, 181, 167, 99, 176, 60, 176, 116, 23, 64, 251, 46, 223, 36],
            },
            Self::Main8a => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/main8a-independent-custody-20261016/current-toMain8a.json"
                ),
                length: 1559304,
                sha256: [198, 120, 43, 76, 218, 183, 80, 222, 238, 37, 213, 234, 114, 170, 119, 90, 213, 166, 71, 100, 214, 163, 222, 43, 75, 19, 205, 149, 118, 246, 20, 170],
            },
        }
    }
}
pub(super) fn input(which: Input, source: &str, current: &str) -> String {
    if which.keys().binary_search(&source).is_err() {
        return current.to_owned();
    }
    super::project(&super::runtime_json::main8a_input(which), source, current)
}
pub(super) fn normalize(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge/runtime_json.rs" {
        return current.to_owned();
    }
    use sha2::{Digest, Sha256};
    let digest: [u8; 32] = Sha256::digest(current.as_bytes()).into();
    if digest
        == [
            248, 6, 220, 255, 145, 5, 39, 80, 67, 29, 236, 103, 79, 227, 2, 136, 232, 218, 240, 65,
            121, 208, 38, 44, 87, 180, 128, 55, 115, 53, 102, 210,
        ]
    {
        return current.to_owned();
    }
    assert_eq!(
        digest,
        [
            82, 239, 12, 76, 71, 65, 26, 117, 2, 202, 27, 59, 179, 194, 173, 180, 247, 19, 178, 2,
            137, 201, 205, 176, 133, 88, 67, 42, 31, 169, 118, 199
        ],
        "complete Main8a transport arm"
    );
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
        [
            248, 6, 220, 255, 145, 5, 39, 80, 67, 29, 236, 103, 79, 227, 2, 136, 232, 218, 240, 65,
            121, 208, 38, 44, 87, 180, 128, 55, 115, 53, 102, 210
        ]
    );
    normalized
}
