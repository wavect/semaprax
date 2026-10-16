//! Sealed Main043 custody descriptors; no compiler authority.
#[derive(Clone, Copy)]
pub(crate) enum Fixture { ExitPrefix, ExitDirectory, LegacyNominal, Lifecycle, Prefix, Receiving, Edge, Selected, Status, Formatting, Receivers, LiveAudits, HistoricalCiExtras }
pub(super) struct Spec { pub(super) path: &'static str, pub(super) length: usize, pub(super) sha256: [u8;32] }
impl Fixture { pub(super) fn spec(self) -> Spec { match self {
Self::ExitPrefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-0.json"), length: 2286839, sha256: [160, 199, 23, 68, 42, 38, 46, 51, 188, 212, 197, 96, 176, 198, 120, 190, 253, 248, 11, 119, 6, 86, 105, 76, 67, 125, 10, 229, 66, 187, 88, 255] },
Self::ExitDirectory => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-1.json"), length: 3935879, sha256: [113, 97, 219, 211, 217, 182, 7, 7, 73, 94, 188, 51, 142, 163, 69, 174, 126, 90, 244, 242, 186, 65, 122, 213, 31, 178, 159, 117, 118, 252, 125, 203] },
Self::LegacyNominal => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-2.json"), length: 1726775, sha256: [113, 88, 104, 200, 48, 180, 59, 61, 96, 121, 219, 142, 22, 214, 180, 214, 153, 225, 10, 243, 150, 108, 162, 3, 150, 49, 7, 32, 166, 45, 159, 159] },
Self::Lifecycle => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-3.json"), length: 1496384, sha256: [108, 12, 29, 212, 205, 12, 165, 204, 253, 48, 2, 68, 210, 184, 1, 108, 18, 201, 58, 15, 124, 224, 63, 209, 196, 131, 123, 69, 216, 187, 63, 157] },
Self::Prefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-4.json"), length: 1358342, sha256: [169, 66, 124, 122, 92, 49, 151, 202, 190, 226, 71, 81, 150, 217, 0, 131, 72, 217, 225, 127, 152, 198, 156, 77, 57, 19, 196, 114, 72, 195, 165, 96] },
Self::Receiving => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-5.json"), length: 1751418, sha256: [167, 219, 131, 228, 80, 47, 143, 169, 168, 15, 133, 69, 79, 202, 164, 207, 132, 115, 189, 149, 83, 6, 155, 9, 162, 250, 66, 5, 198, 12, 208, 73] },
Self::Edge => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-6.json"), length: 2391511, sha256: [142, 105, 166, 142, 41, 45, 228, 190, 191, 235, 69, 109, 217, 211, 217, 10, 93, 14, 31, 76, 15, 244, 160, 149, 188, 198, 63, 179, 86, 82, 171, 184] },
Self::Selected => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-7.json"), length: 2987684, sha256: [51, 252, 129, 11, 34, 144, 36, 225, 97, 150, 195, 237, 168, 12, 118, 127, 148, 84, 110, 86, 251, 15, 55, 226, 33, 86, 146, 240, 145, 112, 143, 170] },
Self::Status => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-8.json"), length: 2391566, sha256: [96, 143, 214, 26, 195, 227, 1, 92, 97, 17, 110, 58, 9, 97, 206, 24, 17, 239, 183, 102, 98, 71, 251, 154, 37, 191, 180, 146, 234, 90, 158, 186] },
Self::Formatting => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Formatting.json"), length: 455600, sha256: [138, 233, 25, 13, 178, 42, 46, 229, 139, 88, 89, 106, 68, 199, 39, 220, 180, 25, 251, 224, 231, 180, 233, 234, 104, 189, 65, 20, 123, 13, 36, 72] },
Self::Receivers => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Receivers.json"), length: 91294, sha256: [117, 72, 249, 231, 212, 41, 231, 239, 143, 178, 92, 66, 45, 155, 38, 20, 208, 136, 104, 26, 99, 66, 163, 225, 25, 87, 73, 56, 29, 75, 222, 18] },
Self::LiveAudits => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/LiveAudits.json"), length: 24038, sha256: [178, 67, 123, 65, 69, 209, 43, 201, 254, 32, 251, 29, 191, 44, 136, 140, 26, 104, 50, 242, 230, 153, 165, 0, 100, 1, 116, 121, 177, 18, 78, 74] },
Self::HistoricalCiExtras => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/historical-ci-extra-inputs.json"), length: 266989, sha256: [158, 141, 221, 103, 92, 215, 252, 174, 70, 32, 208, 98, 59, 57, 161, 222, 210, 251, 115, 144, 248, 247, 170, 202, 28, 41, 238, 78, 57, 7, 234, 100] },
}}}
pub(super) fn input_spec() -> Spec { Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/current-to-main043-helper14.json"), length: 2124610, sha256: [207, 179, 170, 110, 62, 53, 89, 139, 77, 133, 20, 114, 201, 39, 208, 236, 140, 158, 251, 158, 41, 188, 64, 244, 76, 16, 17, 116, 52, 41, 9, 138] } }
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
