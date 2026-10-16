//! Sealed Main043 custody descriptors; no compiler authority.
#[derive(Clone, Copy)]
pub(crate) enum Fixture { ExitPrefix, ExitDirectory, LegacyNominal, Lifecycle, Prefix, Receiving, Edge, Selected, Status, Formatting, Receivers, LiveAudits, HistoricalCiExtras }
pub(super) struct Spec { pub(super) path: &'static str, pub(super) length: usize, pub(super) sha256: [u8;32] }
impl Fixture { pub(super) fn spec(self) -> Spec { match self {
Self::ExitPrefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-0.json"), length: 2275834, sha256: [9, 225, 53, 199, 209, 30, 68, 21, 59, 150, 105, 220, 253, 229, 62, 93, 79, 26, 109, 233, 1, 12, 28, 87, 155, 224, 200, 44, 128, 66, 204, 236] },
Self::ExitDirectory => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-1.json"), length: 3924698, sha256: [232, 74, 125, 191, 75, 106, 245, 227, 99, 167, 8, 221, 148, 195, 54, 165, 1, 61, 239, 145, 219, 41, 1, 107, 248, 84, 86, 157, 150, 206, 235, 204] },
Self::LegacyNominal => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-2.json"), length: 1614608, sha256: [242, 200, 68, 6, 115, 125, 173, 104, 136, 171, 36, 127, 253, 88, 231, 8, 129, 237, 232, 25, 28, 220, 67, 69, 98, 167, 232, 245, 190, 208, 107, 91] },
Self::Lifecycle => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-3.json"), length: 1485494, sha256: [249, 57, 236, 66, 105, 213, 91, 218, 155, 133, 244, 112, 237, 88, 192, 154, 155, 127, 180, 94, 131, 115, 135, 104, 3, 148, 40, 103, 44, 81, 47, 154] },
Self::Prefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-4.json"), length: 1353344, sha256: [22, 0, 224, 195, 23, 92, 167, 196, 149, 232, 230, 252, 199, 125, 79, 26, 150, 37, 92, 148, 183, 48, 131, 126, 57, 152, 104, 217, 214, 25, 255, 40] },
Self::Receiving => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-5.json"), length: 1740413, sha256: [151, 47, 160, 169, 43, 41, 163, 189, 179, 136, 185, 71, 184, 218, 43, 237, 217, 20, 120, 247, 46, 32, 110, 23, 24, 75, 101, 8, 238, 0, 222, 101] },
Self::Edge => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-6.json"), length: 2380506, sha256: [40, 119, 5, 194, 158, 195, 158, 189, 201, 142, 195, 5, 110, 125, 97, 121, 180, 176, 233, 22, 217, 159, 163, 76, 53, 255, 125, 201, 157, 78, 73, 111] },
Self::Selected => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-7.json"), length: 2970724, sha256: [107, 45, 229, 125, 182, 197, 230, 200, 51, 169, 248, 22, 246, 22, 109, 146, 226, 242, 40, 66, 229, 84, 136, 130, 88, 85, 138, 47, 28, 234, 103, 94] },
Self::Status => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-8.json"), length: 2380561, sha256: [248, 115, 97, 44, 214, 125, 97, 77, 253, 154, 21, 38, 61, 214, 238, 46, 98, 64, 10, 52, 15, 15, 114, 204, 76, 39, 173, 158, 62, 50, 136, 96] },
Self::Formatting => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Formatting.json"), length: 455600, sha256: [138, 233, 25, 13, 178, 42, 46, 229, 139, 88, 89, 106, 68, 199, 39, 220, 180, 25, 251, 224, 231, 180, 233, 234, 104, 189, 65, 20, 123, 13, 36, 72] },
Self::Receivers => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Receivers.json"), length: 91294, sha256: [117, 72, 249, 231, 212, 41, 231, 239, 143, 178, 92, 66, 45, 155, 38, 20, 208, 136, 104, 26, 99, 66, 163, 225, 25, 87, 73, 56, 29, 75, 222, 18] },
Self::LiveAudits => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/LiveAudits.json"), length: 24038, sha256: [178, 67, 123, 65, 69, 209, 43, 201, 254, 32, 251, 29, 191, 44, 136, 140, 26, 104, 50, 242, 230, 153, 165, 0, 100, 1, 116, 121, 177, 18, 78, 74] },
Self::HistoricalCiExtras => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/historical-ci-extra-inputs.json"), length: 266989, sha256: [158, 141, 221, 103, 92, 215, 252, 174, 70, 32, 208, 98, 59, 57, 161, 222, 210, 251, 115, 144, 248, 247, 170, 202, 28, 41, 238, 78, 57, 7, 234, 100] },
}}}
pub(super) fn input_spec() -> Spec { Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/current-to-main043-helper14.json"), length: 2084214, sha256: [228, 138, 170, 36, 178, 222, 216, 85, 158, 30, 81, 54, 195, 44, 68, 172, 158, 162, 13, 84, 57, 54, 165, 65, 158, 122, 240, 22, 98, 214, 64, 162] } }
pub(super) fn input(source: &str, current: &str) -> String {
const KEYS: &[&str] = &[
"src/cleanup_plan/replay.rs",
"src/cleanup_plan/replay/branch_clone/tests.rs",
"src/cleanup_plan/replay/factored.rs",
"src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs",
"src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs",
"src/cleanup_plan/replay/factored/oracle_hook.rs",
"src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs",
"src/hir.rs",
"src/hir/expr_nodes.rs",
"src/hir/nominal_payload/legacy/source_tests.rs",
"src/hir/validation.rs",
"src/source_verify/declaration.rs",
"tests/hir_module_boundaries.rs",
"tests/workspace/semantic_graph/source_inventory.rs",
];
if KEYS.binary_search(&source).is_err() { return current.to_owned(); }
super::project(&super::runtime_json::main043_input(), source, current)
}
pub(super) fn historical_ci_extra(source: &str, current: &str) -> String {
const KEYS: &[&str] = &[
"src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs",
"src/cleanup_plan/replay/expression_facts/receiving/retained/edge_tests.rs",
"src/cleanup_plan/replay/expression_facts/receiving/retained/exit_before.rs",
"src/cleanup_plan/replay/expression_facts/receiving/retained/exit_tests.rs",
"src/cleanup_plan/replay/expression_facts/receiving/retained/status_before.rs",
];
assert!(KEYS.binary_search(&source).is_ok(), "unregistered historical CI comparison");
super::project(&super::runtime_json::main043_metadata(Fixture::HistoricalCiExtras), source, current)
}
