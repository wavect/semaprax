//! Sealed Main043 custody descriptors; no compiler authority.
#[derive(Clone, Copy)]
pub(crate) enum Fixture { ExitPrefix, ExitDirectory, LegacyNominal, Lifecycle, Prefix, Receiving, Edge, Selected, Status, Formatting, Receivers, LiveAudits, HistoricalCiExtras }
pub(super) struct Spec { pub(super) path: &'static str, pub(super) length: usize, pub(super) sha256: [u8;32] }
impl Fixture { pub(super) fn spec(self) -> Spec { match self {
Self::ExitPrefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-0.json"), length: 2286309, sha256: [148, 230, 41, 95, 106, 36, 222, 73, 144, 31, 89, 8, 165, 95, 63, 153, 157, 73, 236, 243, 156, 231, 18, 31, 154, 205, 75, 37, 84, 127, 232, 65] },
Self::ExitDirectory => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-1.json"), length: 3935349, sha256: [199, 239, 133, 61, 215, 209, 168, 80, 241, 163, 134, 38, 83, 8, 121, 22, 72, 76, 162, 50, 29, 68, 6, 241, 113, 191, 240, 247, 169, 23, 63, 102] },
Self::LegacyNominal => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-2.json"), length: 1725989, sha256: [55, 70, 167, 194, 196, 52, 202, 23, 79, 93, 69, 33, 2, 41, 94, 29, 126, 82, 164, 22, 136, 99, 93, 79, 242, 195, 166, 218, 136, 227, 193, 59] },
Self::Lifecycle => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-3.json"), length: 1495854, sha256: [202, 194, 111, 226, 182, 152, 75, 118, 188, 94, 174, 86, 205, 109, 41, 188, 217, 125, 102, 116, 17, 52, 29, 190, 21, 211, 175, 79, 21, 147, 41, 178] },
Self::Prefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-4.json"), length: 1358095, sha256: [21, 233, 222, 108, 69, 239, 143, 6, 126, 60, 71, 1, 31, 220, 91, 105, 194, 24, 96, 209, 64, 162, 98, 29, 192, 86, 194, 85, 41, 61, 158, 98] },
Self::Receiving => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-5.json"), length: 1750888, sha256: [91, 138, 122, 148, 205, 84, 118, 110, 98, 161, 65, 124, 57, 61, 110, 137, 112, 75, 179, 143, 172, 169, 175, 225, 155, 35, 115, 72, 170, 173, 188, 31] },
Self::Edge => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-6.json"), length: 2390981, sha256: [81, 48, 152, 237, 219, 15, 145, 175, 24, 6, 40, 143, 171, 87, 193, 173, 218, 48, 238, 247, 25, 250, 60, 53, 133, 115, 152, 254, 97, 174, 232, 181] },
Self::Selected => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-7.json"), length: 2986898, sha256: [70, 229, 93, 173, 176, 23, 29, 69, 81, 170, 240, 162, 42, 77, 40, 28, 76, 9, 254, 44, 6, 17, 201, 209, 133, 107, 206, 171, 81, 121, 66, 2] },
Self::Status => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-8.json"), length: 2391036, sha256: [20, 225, 78, 171, 25, 114, 94, 136, 161, 79, 80, 120, 247, 43, 139, 239, 186, 11, 226, 1, 34, 67, 235, 67, 182, 236, 229, 82, 202, 195, 207, 174] },
Self::Formatting => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Formatting.json"), length: 455600, sha256: [138, 233, 25, 13, 178, 42, 46, 229, 139, 88, 89, 106, 68, 199, 39, 220, 180, 25, 251, 224, 231, 180, 233, 234, 104, 189, 65, 20, 123, 13, 36, 72] },
Self::Receivers => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Receivers.json"), length: 91294, sha256: [117, 72, 249, 231, 212, 41, 231, 239, 143, 178, 92, 66, 45, 155, 38, 20, 208, 136, 104, 26, 99, 66, 163, 225, 25, 87, 73, 56, 29, 75, 222, 18] },
Self::LiveAudits => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/LiveAudits.json"), length: 24038, sha256: [178, 67, 123, 65, 69, 209, 43, 201, 254, 32, 251, 29, 191, 44, 136, 140, 26, 104, 50, 242, 230, 153, 165, 0, 100, 1, 116, 121, 177, 18, 78, 74] },
Self::HistoricalCiExtras => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/historical-ci-extra-inputs.json"), length: 266989, sha256: [158, 141, 221, 103, 92, 215, 252, 174, 70, 32, 208, 98, 59, 57, 161, 222, 210, 251, 115, 144, 248, 247, 170, 202, 28, 41, 238, 78, 57, 7, 234, 100] },
}}}
pub(super) fn input_spec() -> Spec { Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/current-to-main043-helper14.json"), length: 2122399, sha256: [111, 20, 175, 160, 132, 13, 23, 121, 213, 32, 218, 4, 86, 194, 9, 46, 243, 185, 24, 58, 173, 233, 97, 181, 48, 59, 204, 56, 65, 208, 194, 152] } }
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
