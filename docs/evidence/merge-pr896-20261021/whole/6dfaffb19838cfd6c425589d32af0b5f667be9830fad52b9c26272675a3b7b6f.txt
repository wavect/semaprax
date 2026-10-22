//! Fixed immutable proof metadata; never reads live Rust inputs at runtime.
//! Exact absence skips unused data only; the all-file owner checks availability.
use sha2::{Digest, Sha256};
#[derive(Clone, Copy)]
pub(super) enum Fixture {
    M00,
    M01,
    M02,
    M03,
    M04,
    M05,
    M06,
    M07,
    M08,
    M09,
    M10,
    M11,
    M12,
    M13,
    M14,
    M15,
    M16,
    M17,
    M18,
    M19,
    M20,
    M21,
    M22,
    M23,
    M24,
    M25,
    M26,
    M27,
    M28,
    M29,
    M30,
    M31,
    M32,
    M33,
    M34,
    M35,
}
pub(super) struct Spec {
    pub(super) path: &'static str,
    pub(super) length: usize,
    pub(super) sha256: [u8; 32],
}
impl Fixture {
    pub(super) fn spec(self) -> Spec {
        match self {
            Self::M00 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/next-bitmap-singleton-main2600-20261016/current-to-4fba.json"), length: 157100, sha256: [121, 213, 16, 149, 158, 174, 20, 178, 9, 45, 54, 149, 117, 25, 18, 196, 94, 186, 182, 86, 65, 192, 181, 20, 125, 242, 27, 89, 136, 250, 198, 111] },
            Self::M01 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/next-main-beb0-847-joined-20261016/current-to-8bb.json"), length: 38850, sha256: [228, 165, 46, 174, 79, 72, 177, 229, 89, 78, 73, 197, 46, 44, 212, 212, 29, 211, 164, 103, 106, 15, 165, 14, 128, 245, 213, 250, 55, 7, 60, 106] },
            Self::M02 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/private-175-actual-fixes-20261015/current-to-82fea.json"), length: 62014, sha256: [77, 204, 155, 219, 67, 215, 38, 89, 114, 156, 37, 170, 156, 133, 140, 250, 251, 172, 85, 239, 85, 212, 90, 236, 202, 56, 235, 74, 24, 115, 118, 189] },
            Self::M03 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/boxed-state-local-owner-849-20261016/current-to-35c.json"), length: 2533, sha256: [86, 188, 194, 50, 88, 249, 198, 248, 182, 214, 195, 141, 151, 131, 151, 182, 78, 19, 194, 135, 34, 172, 249, 184, 120, 221, 81, 77, 114, 32, 138, 63] },
            Self::M04 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/boxed-state-receiving-849-20261016/current-to-main019d.json"), length: 70247, sha256: [40, 96, 61, 105, 218, 178, 248, 29, 232, 55, 220, 90, 165, 247, 82, 142, 66, 101, 220, 5, 50, 188, 32, 58, 134, 158, 52, 246, 198, 190, 8, 232] },
            Self::M05 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/next-unit-branch-receiving-joined-20261016/final-to-7c.json"), length: 61775, sha256: [242, 61, 92, 16, 49, 222, 132, 148, 92, 121, 205, 184, 119, 157, 186, 153, 50, 39, 196, 82, 172, 211, 155, 106, 41, 13, 252, 161, 22, 209, 132, 108] },
            Self::M06 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/preflight-binding-successor/current-to-fac.json"), length: 17919, sha256: [143, 207, 117, 204, 112, 107, 94, 100, 237, 206, 222, 27, 8, 64, 45, 203, 173, 33, 42, 158, 139, 90, 177, 52, 54, 90, 95, 10, 187, 253, 183, 19] },
            Self::M07 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/current-to-2d4b.json"), length: 134751, sha256: [26, 114, 149, 65, 65, 9, 161, 105, 245, 97, 119, 4, 72, 102, 141, 64, 27, 223, 127, 57, 254, 120, 208, 116, 37, 83, 88, 244, 60, 37, 176, 169] },
            Self::M08 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/compile-fixture-successor-5aac-20261016/current-to-5aac.json"), length: 21844, sha256: [62, 120, 21, 189, 198, 48, 63, 199, 141, 137, 176, 64, 251, 74, 130, 164, 244, 218, 197, 111, 62, 248, 215, 192, 178, 9, 171, 179, 141, 52, 74, 77] },
            Self::M09 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main531-independent-groups-20261016/current-to-2d4b.json"), length: 42339, sha256: [217, 244, 169, 62, 234, 127, 96, 253, 0, 42, 40, 94, 120, 128, 218, 212, 43, 159, 193, 208, 173, 60, 154, 223, 24, 192, 159, 57, 11, 199, 85, 188] },
            Self::M10 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main531-independent-groups-20261016/runtime-main-root.json"), length: 1610, sha256: [94, 9, 241, 101, 112, 207, 100, 243, 213, 22, 187, 32, 131, 72, 192, 151, 117, 79, 147, 40, 219, 14, 242, 64, 230, 227, 231, 70, 249, 254, 65, 19] },
            Self::M11 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main531-original-tests-child-20261016/current-to-7e9f.json"), length: 15245, sha256: [164, 36, 84, 252, 194, 178, 85, 176, 18, 61, 99, 188, 42, 238, 243, 134, 246, 190, 66, 226, 247, 0, 34, 0, 134, 160, 89, 173, 121, 113, 163, 129] },
            Self::M12 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/b3a-compile-fixture-task722-join-20261016/current-to-b3a.json"), length: 38491, sha256: [78, 8, 237, 206, 76, 42, 34, 204, 122, 158, 123, 41, 129, 47, 123, 2, 44, 172, 230, 169, 232, 121, 12, 0, 253, 146, 89, 217, 116, 86, 38, 108] },
            Self::M13 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/post-main0ab1-singleton-join-20261016/current-to-3177.json"), length: 32995, sha256: [136, 102, 216, 195, 99, 117, 229, 162, 92, 6, 121, 126, 239, 137, 203, 98, 243, 179, 189, 203, 239, 48, 95, 193, 165, 187, 2, 151, 39, 14, 120, 20] },
            Self::M14 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main696-funded-receiving-joined-20261016/current-to-ff7e.json"), length: 55120, sha256: [18, 190, 221, 34, 204, 104, 20, 100, 138, 162, 94, 249, 3, 119, 98, 115, 242, 170, 100, 100, 33, 85, 252, 97, 88, 250, 102, 81, 18, 37, 16, 1] },
            Self::M15 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main696-source-reconciliation-20261016/current-to-main696.json"), length: 86527, sha256: [55, 91, 220, 75, 73, 86, 227, 106, 204, 74, 189, 200, 123, 28, 91, 37, 63, 129, 178, 193, 188, 2, 98, 224, 180, 207, 181, 154, 11, 67, 158, 178] },
            Self::M16 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main696-source-reconciliation-20261016/current-to-364d.json"), length: 50009, sha256: [235, 205, 222, 33, 169, 13, 80, 227, 175, 180, 176, 183, 120, 212, 219, 63, 24, 202, 149, 148, 31, 128, 171, 57, 113, 4, 23, 34, 251, 160, 14, 109] },
            Self::M17 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/next-unit-branch-receiving-joined-20261016/final-to-bac.json"), length: 97630, sha256: [145, 59, 38, 62, 128, 201, 134, 181, 17, 212, 241, 137, 222, 175, 182, 227, 156, 248, 68, 101, 204, 247, 49, 79, 227, 10, 220, 232, 235, 241, 229, 224] },
            Self::M18 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/actual50-fixture-and-facade-corrections-20261016/current-to-183.json"), length: 11661, sha256: [226, 98, 54, 232, 93, 191, 14, 94, 67, 96, 58, 127, 28, 158, 56, 39, 35, 174, 230, 121, 41, 188, 172, 84, 148, 107, 135, 239, 42, 150, 130, 206] },
            Self::M19 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/task722-held-fitting759-join-20261016/current-to-0e7b.json"), length: 37874, sha256: [83, 3, 88, 214, 53, 101, 35, 183, 75, 123, 230, 131, 119, 44, 143, 175, 90, 27, 35, 89, 252, 41, 123, 242, 175, 16, 36, 119, 232, 33, 1, 33] },
            Self::M20 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/actual47f-pin-fixture-successor-20261016/current-to-47f.json"), length: 48010, sha256: [107, 28, 108, 236, 172, 136, 16, 32, 6, 155, 212, 69, 177, 8, 91, 110, 207, 78, 135, 178, 245, 77, 124, 220, 223, 248, 151, 121, 217, 20, 22, 5] },
            Self::M21 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main8fb-source-reconciliation-20261016/current-to-main8fb.json"), length: 453, sha256: [185, 126, 178, 124, 167, 22, 237, 227, 231, 244, 24, 67, 224, 109, 33, 116, 228, 87, 24, 115, 186, 127, 38, 69, 238, 208, 17, 211, 62, 226, 200, 217] },
            Self::M22 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future-main-output-851-joined-20261016/current-to-bcd.json"), length: 152825, sha256: [79, 21, 210, 229, 115, 65, 251, 30, 251, 86, 154, 13, 251, 225, 91, 19, 65, 251, 109, 186, 36, 253, 97, 51, 45, 34, 108, 138, 31, 253, 21, 220] },
            Self::M23 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/future-main-output-851-joined-20261016/current-to-ff5.json"), length: 190037, sha256: [145, 180, 34, 223, 21, 34, 95, 85, 131, 230, 161, 160, 224, 200, 222, 196, 58, 121, 39, 75, 21, 172, 251, 69, 196, 248, 2, 139, 224, 247, 38, 42] },
            Self::M24 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/post21-alias-observer-source-join-20261016/current-to-36f.json"), length: 14038, sha256: [109, 209, 241, 127, 149, 62, 0, 149, 166, 15, 168, 119, 149, 121, 52, 245, 0, 241, 80, 247, 76, 107, 6, 129, 1, 44, 13, 1, 208, 123, 234, 127] },
            Self::M25 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main83-source-join-20261015/current-to-4f5b.json"), length: 22353, sha256: [230, 197, 142, 9, 97, 92, 150, 91, 198, 153, 96, 141, 130, 6, 27, 95, 195, 208, 250, 106, 90, 90, 207, 165, 190, 222, 15, 242, 125, 156, 122, 185] },
            Self::M26 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/private-175-actual-fixes-20261015/current-to-c159.json"), length: 183580, sha256: [10, 150, 156, 98, 28, 101, 52, 206, 102, 169, 39, 131, 230, 119, 36, 45, 187, 102, 204, 18, 107, 132, 123, 155, 112, 217, 75, 78, 190, 32, 159, 6] },
            Self::M27 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordered-amortized-root-domain-20261015/current-to-35cc.json"), length: 2809, sha256: [148, 139, 138, 182, 130, 112, 115, 25, 201, 51, 12, 23, 202, 184, 220, 86, 233, 180, 108, 93, 158, 217, 19, 223, 149, 2, 254, 42, 211, 206, 56, 211] },
            Self::M28 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/storage-oracle-compiler-fix-join-20261015/current-to-d719.json"), length: 21891, sha256: [166, 70, 90, 103, 156, 159, 153, 62, 78, 20, 249, 156, 194, 152, 110, 209, 149, 66, 11, 64, 166, 170, 238, 16, 27, 228, 166, 151, 90, 31, 23, 59] },
            Self::M29 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main-dd0-compiled-custody-join-20261016/compiled-before-to-main.json"), length: 1872, sha256: [0, 134, 144, 15, 188, 5, 160, 123, 219, 44, 88, 146, 47, 85, 47, 160, 187, 66, 38, 26, 132, 172, 50, 218, 134, 110, 5, 39, 239, 158, 190, 245] },
            Self::M30 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main-b80-complete-source-join-20261016/current-to-main.json"), length: 701524, sha256: [43, 246, 142, 2, 4, 40, 86, 239, 164, 60, 168, 149, 167, 0, 127, 252, 53, 18, 155, 79, 76, 143, 41, 227, 225, 8, 99, 170, 101, 97, 202, 14] },
            Self::M31 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main-b80-complete-source-join-20261016/current-to-df2.json"), length: 61962, sha256: [64, 18, 6, 56, 54, 248, 158, 176, 154, 35, 249, 123, 112, 56, 116, 189, 127, 152, 36, 104, 224, 69, 174, 136, 235, 221, 2, 174, 247, 177, 127, 144] },
            Self::M32 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/reference-coverage-paid-receiving-781-next-20261016/current-drivers-to-9a3.json"), length: 1337, sha256: [93, 68, 39, 117, 0, 104, 187, 220, 152, 6, 36, 203, 114, 250, 140, 161, 152, 140, 2, 248, 233, 209, 67, 184, 158, 27, 58, 146, 22, 31, 104, 49] },
            Self::M33 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/session-reference-instance-joined-20261016/current-to-17b.json"), length: 78812, sha256: [185, 10, 125, 27, 132, 15, 147, 205, 92, 34, 205, 160, 215, 45, 126, 188, 98, 239, 29, 245, 44, 127, 187, 185, 186, 235, 255, 0, 74, 14, 248, 169] },
            Self::M34 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/session-reference-instance-joined-20261016/current-to-reference.json"), length: 42357, sha256: [209, 228, 150, 152, 223, 44, 75, 116, 88, 116, 242, 243, 76, 252, 114, 24, 251, 194, 100, 20, 7, 123, 84, 143, 243, 140, 94, 123, 48, 232, 251, 41] },
            Self::M35 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main-dd0-compiled-custody-join-20261016/current-to-83ca.json"), length: 52015, sha256: [76, 180, 14, 111, 10, 110, 34, 189, 179, 116, 60, 193, 58, 148, 133, 103, 111, 182, 111, 18, 221, 140, 235, 159, 253, 3, 111, 215, 222, 15, 95, 44] },
        }
    }
    fn keys(self) -> &'static [&'static str] {
        match self {
            Self::M00 => &[".github/workflows/ci.yml", "CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "docs/QUALITY-GATES.md", "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs", "scripts/ci-msrv.py", "src/cleanup_plan/renewal_profile/ordinary/consuming/tests.rs", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/factored/canonical_shared.rs", "src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs", "src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias/tests.rs", "src/cleanup_plan/replay/factored/canonical_shared/held_auth_tests.rs", "src/cleanup_plan/replay/factored/canonical_shared/ordered_weights.rs", "src/cleanup_plan/replay/path_join.rs", "src/cleanup_plan/replay/path_join/boxed_states.rs", "src/source_reflows_merge.rs", "src/workspace_graph/source_schema/static_text/primitive/helpers/tests.rs", "tests/hir_module_boundaries.rs", "tests/project_product_acceptance_ci_contract.rs", "tests/wasm/scalar_browser_ci_contract.rs", "tests/wasm/scalar_browser_ci_contract/historical_builder.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M01 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "src/cleanup_plan/replay/factored/canonical_shared.rs", "src/source_reflows_merge.rs", "src/workspace_graph.rs", "src/workspace_graph/constructor_failure/typed.rs", "src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs", "src/workspace_graph/function_alias/before_core.rs", "src/workspace_graph/function_alias/before_core/tests.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M02 => &["crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M03 => &["src/cleanup_plan/replay/path_join/boxed_states.rs", "src/source_reflows_merge.rs"],
            Self::M04 => &["crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "src/bounded_output/classifier_string.rs", "src/cleanup_plan/replay/path_join.rs", "src/cleanup_plan/replay/path_join/boxed_states.rs", "src/cleanup_plan/replay/path_join/receiver_tests.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M05 => &[".github/workflows/ci.yml", "CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "docs/evidence/darwin-native-link-reference-v1/README.md", "scripts/ci-darwin-native-reference.py", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone.rs", "src/cleanup_plan/replay/branch_clone/tests.rs", "src/cleanup_plan/replay/path_join.rs", "src/cleanup_plan/replay/path_join/singleton_tests.rs", "src/source_reflows_merge.rs", "src/workspace_graph/function_alias/before_core/tests.rs", "tests/hir_module_boundaries.rs", "tests/project_product_acceptance_ci_contract.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M06 => &["src/cleanup_plan/replay/factored/canonical_shared.rs", "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs", "src/source_reflows_merge.rs"],
            Self::M07 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "src/cleanup_plan/replay/factored/canonical_shared.rs", "src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs", "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M08 => &["crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone/before.rs", "src/cleanup_plan/replay/factored/canonical_shared/before_empty.rs", "src/cleanup_plan/replay/factored/canonical_shared/before_empty/memo/retained.rs", "src/cleanup_plan/replay/factored/canonical_shared/before_empty/ordered_weights.rs", "src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/tests.rs", "src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight/tests.rs", "src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs", "src/source_reflows_merge.rs", "src/workspace_graph/identity_resolution.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M09 => &[".github/workflows/ci.yml", "CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "docs/evidence/darwin-native-link-reference-v1/README.md", "scripts/ci-darwin-native-reference.py", "src/cleanup_plan/replay/path_join.rs", "src/cleanup_plan/replay/path_join/singleton_tests.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/project_product_acceptance_ci_contract.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M10 => &["main531::path_join"],
            Self::M11 => &["crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "src/cleanup_plan/replay/path_join/main_single_groups/mod.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M12 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "crates/semaprax-toolchain/tests/cli_help_surface_v1.rs", "docs/ARCHITECTURE.md", "docs/SUMMARY.md", "src/cleanup_plan/replay/path_array/before.rs", "src/cleanup_plan/replay/path_array/funded_tests.rs", "src/cli/help/authoring.rs", "src/source_reflows_merge.rs", "tests/cli_help_surface_v1.rs", "tests/hir_module_boundaries.rs", "tests/project/standard_library.rs", "tests/project/standard_library/catalog.rs", "tests/support/production_source_inventory.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M13 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/PUBLIC-CXX-OWNED-DATA-PACKAGE-V1.md", "src/cleanup_plan/replay/path_join.rs", "src/project/cxx_owned_data.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/project/command_argument_borrow.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M14 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/factored/canonical_shared.rs", "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs", "src/cleanup_plan/replay/path_array.rs", "src/cleanup_plan/replay/path_array/before.rs", "src/cleanup_plan/replay/path_array/tests.rs", "src/source_reflows_merge.rs", "src/workspace_graph/function_alias/before_core/tests.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M15 => &["src/cleanup_plan/replay.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M16 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/SUMMARY.md", "docs/evidence/boxed-path-states-v1/README.md", "docs/evidence/branch-clone-capacity-v1/README.md", "docs/evidence/browser-graph-pins-17104e20c/README.md", "docs/evidence/browser-graph-pins-2600e49eb/README.md", "docs/evidence/browser-graph-pins-268cb0523/README.md", "docs/evidence/browser-graph-pins-841efe3e2/README.md", "docs/evidence/darwin-native-link-reference-v1/README.md", "docs/evidence/expression-fact-arena-cursor-v1/README.md", "docs/evidence/format-hygiene-scratch-v1/README.md", "docs/evidence/optional-helper-source-work-v1/README.md", "docs/evidence/pattern-hygiene-scratch-v1/README.md", "docs/evidence/record-presence-phase-v1/README.md", "docs/evidence/row-loan-source-refusal-v1/README.md", "docs/evidence/single-state-groups-v1/README.md", "docs/evidence/storage-oracle-refusal-v1/README.md", "docs/evidence/structural-call-cursor-v1/README.md", "docs/evidence/supplemental-cursor-growth-v1/README.md", "docs/evidence/unit-weight-closure-v1/README.md", "src/cleanup_plan/replay.rs", "src/source_reflows.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M17 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone.rs", "src/cleanup_plan/replay/factored/canonical_shared.rs", "src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs", "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M18 => &["src/cleanup_plan/replay/branch_clone/tests.rs", "src/source_reflows_merge.rs", "src/workspace_graph/function_alias/before_core/tests.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs"],
            Self::M19 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "docs/LEXICAL-STRUCTURED-EXITS-V1.md", "docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md", "src/cleanup_plan/replay/factored/strategy.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M20 => &["crates/semaprax-toolchain/tests/cli_help_surface_v1.rs", "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs", "src/cleanup_plan/replay/path_array/funded_tests.rs", "src/cleanup_plan/replay/path_array/tests.rs", "src/source_reflows_merge.rs", "src/workspace_graph/function_alias/before_core/tests.rs", "src/workspace_graph/type_proof_tests.rs", "tests/cli_help_surface_v1.rs", "tests/hir_module_boundaries.rs", "tests/wasm/scalar_browser_ci_contract.rs", "tests/wasm/scalar_browser_ci_contract/browser_custody.rs", "tests/wasm/scalar_browser_ci_contract/historical_builder.rs", "tests/wasm/scalar_browser_ci_contract/historical_main2600.rs", "tests/workspace/semantic_graph.rs"],
            Self::M21 => &["src/source_verify/declaration.rs"],
            Self::M22 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "docs/evidence/receiver-source-custody-v1/README.md", "docs/evidence/receiver-source-custody-v1/inverse.json", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone/inline_tests.rs", "src/cleanup_plan/replay/path_join.rs", "src/source_reflows/semantic_receivers.rs", "src/source_reflows_merge.rs", "src/source_verify/declaration.rs", "src/source_verify/hints.rs", "tests/hir_module_boundaries.rs", "tests/quickstart_v1/fused_source.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M23 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "crates/semaprax-toolchain/tests/cli_help_surface_v1.rs", "docs/ARCHITECTURE.md", "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone/inline_tests.rs", "src/cleanup_plan/replay/path_array/funded_tests.rs", "src/cleanup_plan/replay/path_array/tests.rs", "src/cleanup_plan/replay/path_join.rs", "src/source_reflows_merge.rs", "src/source_verify/hints.rs", "src/workspace_graph/function_alias/before_core/tests.rs", "src/workspace_graph/type_proof_tests.rs", "tests/cli_help_surface_v1.rs", "tests/hir_module_boundaries.rs", "tests/wasm/scalar_browser_ci_contract.rs", "tests/wasm/scalar_browser_ci_contract/browser_custody.rs", "tests/wasm/scalar_browser_ci_contract/historical_builder.rs", "tests/wasm/scalar_browser_ci_contract/historical_main2600.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M24 => &["CHANGELOG.md", "docs/ARCHITECTURE.md", "src/cleanup_plan/replay/path_array/funded_tests.rs", "src/source_reflows_merge.rs", "src/workspace_graph.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs"],
            Self::M25 => &["CHANGELOG.md", "docs/evidence/canonical-preparation-stage-829/retry-precharge-fixture/HANDOFF.md", "docs/evidence/canonical-preparation-stage-829/retry-precharge-fixture/exact-inverse.json", "src/source_reflows_merge.rs", "src/workspace_graph/tests/builder_limits.rs", "src/workspace_graph/tests/record_traversal.rs", "tests/project_candidate/string_builtin_calls.rs"],
            Self::M26 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/AGENT-SKILL-BUNDLE-V1.json", "docs/ARCHITECTURE.md", "examples/shiftsim-typed-record-successor/src/model.spx", "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs", "src/bounded_output/classifier_string.rs", "src/bounded_output/stage_debits.rs", "src/bounded_output/stage_debits/tests.rs", "src/cleanup_plan/renewal_profile/ordinary/consuming.rs", "src/cleanup_plan/renewal_profile/ordinary/consuming/tests.rs", "src/cleanup_plan/renewal_profile/ordinary/mod.rs", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/factored/canonical_shared/inline_weighted/source.rs", "src/cleanup_plan/replay/factored/canonical_shared/receiving_layer_tests.rs", "src/cleanup_plan/replay/factored/compact_nodes.rs", "src/cleanup_plan/replay/factored/compact_nodes/tests.rs", "src/cleanup_plan/replay/path_join.rs", "src/cleanup_plan/replay/path_join/tests.rs", "src/loop_calls/byte_writer/module_index/ordinary.rs", "src/source_reflows_merge.rs", "src/source_reflows_merge/runtime_json.rs", "src/source_reflows_merge/runtime_json/tests.rs", "src/workspace_graph/function_alias/before_core/tests.rs", "src/workspace_graph/source_schema/static_text/primitive/helpers/tests.rs", "src/workspace_graph/type_proof_tests.rs", "tests/hir_module_boundaries.rs", "tests/owned_data/owned_leaf_vec/copy_field_update.rs", "tests/project/command_argument_borrow.rs", "tests/support/record_copy_field_update.rs", "tests/wasm/scalar_browser_ci_contract.rs", "tests/wasm/scalar_browser_ci_contract/historical_builder.rs", "tests/workspace/semantic_change/reporting.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs", "tests/workspace/semantic_structural_change/reporting.rs"],
            Self::M27 => &["src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/amortized_source.rs", "src/source_reflows_merge.rs"],
            Self::M28 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "src/cleanup_plan/replay/factored/continuation_receiver.rs", "src/cleanup_plan/replay/factored/continuation_receiver/tests.rs", "src/cleanup_plan/replay/factored/oracle_hook.rs", "src/source_reflows_merge.rs", "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs", "src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs", "src/workspace_graph/tests/builder_limits.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M29 => &["src/cleanup_plan/replay/compact_stage/before.rs", "src/cleanup_plan/replay/compact_stage/lexical_before.rs"],
            Self::M30 => &["src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone/tests.rs", "src/cleanup_plan/replay/factored.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M31 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "docs/SUMMARY.md", "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs", "src/cleanup_plan/replay.rs", "src/cleanup_plan/replay/branch_clone/before.rs", "src/cleanup_plan/replay/branch_clone/before_inline.rs", "src/cleanup_plan/replay/branch_clone/inline_tests.rs", "src/cleanup_plan/replay/factored.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs", "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs", "src/cleanup_plan/replay/factored/compact_nodes/before/mod.rs", "src/cleanup_plan/replay/factored/compact_nodes/tests.rs", "src/cleanup_plan/replay/factored/oracle_order.rs", "src/cleanup_plan/replay/factored/oracle_order/tests.rs", "src/cleanup_plan/replay/final_output.rs", "src/cleanup_plan/replay/final_output/before.rs", "src/cleanup_plan/replay/final_output/tests.rs", "src/cleanup_plan/replay/path_array/main696.rs", "src/cleanup_plan/replay/structural_maps.rs", "src/cleanup_plan/replay/structural_maps/tests.rs", "src/source_reflows.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/wasm/scalar_browser_ci_contract.rs", "tests/wasm/scalar_browser_ci_contract/browser_custody.rs", "tests/wasm/scalar_browser_ci_contract/historical_builder.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M32 => &["src/cleanup_plan/replay/lexical_replay.rs", "src/cleanup_plan/replay/scalar_replay.rs"],
            Self::M33 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "docs/DECLARATION-IDENTITY-BACKING-V1.md", "docs/SELECTED-REPLAY-RECOVERING-CONSTRUCTOR-V1.md", "src/bounded_output.rs", "src/cleanup_plan/renewal_profile/child_refusal.rs", "src/cleanup_plan/replay/branch_clone.rs", "src/cleanup_plan/replay/compact_stage.rs", "src/cleanup_plan/replay/lexical_replay.rs", "src/cleanup_plan/replay/scalar_replay.rs", "src/cleanup_plan/replay/selected_context/child.rs", "src/cleanup_plan/replay/selected_entry.rs", "src/cleanup_plan/replay/selected_entry/tests.rs", "src/cleanup_plan/replay/structural_maps.rs", "src/cleanup_plan/replay/structural_maps/tests.rs", "src/hir/ids.rs", "src/hir/nodes.rs", "src/hir/validation.rs", "src/hir/validation/generic_template.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M34 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "docs/ARCHITECTURE.md", "docs/DECLARATION-IDENTITY-BACKING-V1.md", "src/bounded_output.rs", "src/cleanup_plan/replay/branch_clone.rs", "src/cleanup_plan/replay/compact_stage.rs", "src/hir/ids.rs", "src/hir/nodes.rs", "src/hir/validation.rs", "src/hir/validation/generic_template.rs", "src/source_reflows_merge.rs", "tests/hir_module_boundaries.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
            Self::M35 => &["CHANGELOG.md", "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs", "src/cleanup_plan/replay/compact_stage/source_custody.rs", "src/source_reflows_merge.rs", "src/workspace_graph/type_proof_tests.rs", "tests/hir_module_boundaries.rs", "tests/native/string_settlement/bulk_utf8.rs", "tests/native/string_settlement/codec_allocation.rs", "tests/native/string_settlement/codec_allocation/string_vectors.rs", "tests/native_string_settlement_v1/contents.rs", "tests/owned_data/bulk_utf8.rs", "tests/owned_data/owned_nested_outcome.rs", "tests/owned_data/projected_string_views.rs", "tests/root_unsafe_quarantine.rs", "tests/wasm/scalar_browser_ci_contract.rs", "tests/wasm/scalar_browser_ci_contract/browser_custody.rs", "tests/wasm/scalar_browser_ci_contract/historical_builder.rs", "tests/workspace/semantic_graph.rs", "tests/workspace/semantic_graph/source_inventory.rs"],
        }
    }
}
pub(super) fn project(fixture: Fixture, source: &str, current: &str) -> String {
    if fixture.keys().binary_search(&source).is_err() {
        return current.to_owned();
    }
    let payload = super::runtime_json::metadata(fixture);
    super::project(&payload, source, current)
}
pub(super) fn normalize(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge/runtime_json.rs" {
        return current.to_owned();
    }
    let actual: [u8; 32] = Sha256::digest(current.as_bytes()).into();
    if actual == COMMON_ARM {
        return current.to_owned();
    }
    assert_eq!(actual, EMBEDDED_ARM, "exact current metadata transport arm");
    assert_eq!(
        current
            .matches("runtime_json/transport_embedded.rs")
            .count(),
        1
    );
    assert!(!current.contains("runtime_json/transport_runtime.rs"));
    let common = current.replacen(
        "runtime_json/transport_embedded.rs",
        "runtime_json/transport_runtime.rs",
        1,
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(common.as_bytes())),
        COMMON_ARM
    );
    common
}
const COMMON_ARM: [u8; 32] = [
    109, 211, 107, 83, 170, 230, 226, 101, 109, 178, 228, 50, 131, 133, 161, 21, 121, 32, 160, 196,
    159, 80, 166, 33, 102, 249, 135, 167, 106, 251, 153, 156,
];
const EMBEDDED_ARM: [u8; 32] = [
    120, 222, 34, 55, 73, 237, 115, 95, 188, 147, 57, 98, 248, 110, 74, 163, 164, 130, 214, 78,
    186, 212, 35, 182, 105, 178, 32, 120, 197, 85, 121, 123,
];
pub(super) fn previous(source: &str, current: &str) -> String {
    let common = normalize(source, current);
    let current = common.as_str();
    super::project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/fixed-metadata-runtime-848-next-20261016/current-to-e204.json"
        )),
        source,
        current,
    )
}
#[test]
fn fixed_metadata_bytes_and_exact_source_membership_remain_complete() {
    let fixtures = [
        Fixture::M00,
        Fixture::M01,
        Fixture::M02,
        Fixture::M03,
        Fixture::M04,
        Fixture::M05,
        Fixture::M06,
        Fixture::M07,
        Fixture::M08,
        Fixture::M09,
        Fixture::M10,
        Fixture::M11,
        Fixture::M12,
        Fixture::M13,
        Fixture::M14,
        Fixture::M15,
        Fixture::M16,
        Fixture::M17,
        Fixture::M18,
        Fixture::M19,
        Fixture::M20,
        Fixture::M21,
        Fixture::M22,
        Fixture::M23,
        Fixture::M24,
        Fixture::M25,
        Fixture::M26,
        Fixture::M27,
        Fixture::M28,
        Fixture::M29,
        Fixture::M30,
        Fixture::M31,
        Fixture::M32,
        Fixture::M33,
        Fixture::M34,
        Fixture::M35,
    ];
    assert_eq!(fixtures.len(), 36);
    for fixture in fixtures {
        let spec = fixture.spec();
        let payload = super::runtime_json::metadata(fixture);
        assert_eq!(payload.len(), spec.length);
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(payload.as_bytes())),
            spec.sha256
        );
        let rows: serde_json::Value = serde_json::from_str(&payload).unwrap();
        let mut keys = rows
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|row| row["source"].as_str())
            .collect::<Vec<_>>();
        keys.sort(); // Retain duplicates: the unchanged projector refuses them.
        assert_eq!(keys, fixture.keys());
        assert_eq!(project(fixture, "\0", "unchanged"), "unchanged");
    }
    // Final fixed tables use the same authenticated reader; no payload embed.
    for fixture in super::final_custody::ALL {
        let spec = fixture.spec();
        let payload = super::runtime_json::final_custody(*fixture);
        assert_eq!(payload.len(), spec.length);
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(payload.as_bytes())),
            spec.sha256
        );
        let rows: serde_json::Value = serde_json::from_str(&payload).unwrap();
        let mut keys = rows
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|row| row["source"].as_str())
            .collect::<Vec<_>>();
        keys.sort();
        assert_eq!(keys, fixture.keys());
    }
}
