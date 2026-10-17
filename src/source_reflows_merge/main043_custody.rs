//! Sealed Main043 custody descriptors; no compiler authority.
#[derive(Clone, Copy)]
pub(crate) enum Fixture { ExitPrefix, ExitDirectory, LegacyNominal, Lifecycle, Prefix, Receiving, Edge, Selected, Status, Formatting, Receivers, LiveAudits, HistoricalCiExtras }
pub(super) struct Spec { pub(super) path: &'static str, pub(super) length: usize, pub(super) sha256: [u8;32] }
impl Fixture { pub(super) fn spec(self) -> Spec { match self {
Self::ExitPrefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-0.json"), length: 2298655, sha256: [28, 110, 166, 239, 18, 1, 42, 101, 144, 158, 61, 222, 163, 62, 242, 19, 174, 145, 34, 133, 60, 65, 244, 177, 43, 89, 82, 35, 174, 89, 149, 198] },
Self::ExitDirectory => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-1.json"), length: 3977860, sha256: [225, 185, 185, 17, 33, 37, 37, 58, 146, 242, 253, 4, 2, 94, 139, 142, 167, 65, 22, 211, 52, 85, 250, 126, 72, 194, 240, 39, 169, 118, 116, 213] },
Self::LegacyNominal => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-2.json"), length: 1737703, sha256: [69, 214, 58, 121, 17, 53, 45, 142, 24, 31, 145, 178, 59, 214, 184, 61, 4, 21, 24, 146, 61, 13, 234, 160, 157, 133, 11, 89, 92, 255, 208, 230] },
Self::Lifecycle => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-3.json"), length: 1502954, sha256: [102, 61, 212, 248, 178, 74, 245, 174, 212, 228, 18, 149, 62, 47, 34, 14, 223, 116, 117, 142, 163, 78, 63, 184, 132, 189, 119, 182, 244, 50, 34, 135] },
Self::Prefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-4.json"), length: 1389331, sha256: [79, 29, 154, 178, 108, 184, 148, 234, 237, 129, 116, 149, 215, 196, 62, 93, 109, 210, 231, 50, 146, 176, 91, 170, 154, 78, 221, 121, 29, 232, 21, 228] },
Self::Receiving => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-5.json"), length: 1799401, sha256: [142, 152, 207, 6, 213, 117, 195, 103, 141, 164, 36, 65, 60, 58, 205, 138, 92, 118, 141, 130, 85, 240, 148, 225, 121, 212, 238, 188, 50, 108, 2, 27] },
Self::Edge => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-6.json"), length: 2403327, sha256: [253, 77, 158, 184, 191, 145, 85, 42, 11, 124, 149, 62, 174, 97, 174, 215, 238, 188, 60, 2, 189, 118, 18, 226, 193, 63, 97, 105, 246, 247, 173, 129] },
Self::Selected => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-7.json"), length: 2997183, sha256: [80, 40, 153, 151, 251, 193, 175, 131, 208, 61, 71, 186, 185, 210, 77, 226, 174, 66, 183, 47, 190, 83, 182, 109, 164, 75, 150, 174, 150, 0, 55, 56] },
Self::Status => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-8.json"), length: 2403382, sha256: [8, 101, 57, 135, 187, 49, 199, 140, 127, 2, 240, 115, 190, 95, 8, 112, 23, 233, 173, 34, 208, 38, 186, 57, 218, 198, 217, 183, 181, 154, 67, 67] },
Self::Formatting => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Formatting.json"), length: 455600, sha256: [138, 233, 25, 13, 178, 42, 46, 229, 139, 88, 89, 106, 68, 199, 39, 220, 180, 25, 251, 224, 231, 180, 233, 234, 104, 189, 65, 20, 123, 13, 36, 72] },
Self::Receivers => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Receivers.json"), length: 91294, sha256: [117, 72, 249, 231, 212, 41, 231, 239, 143, 178, 92, 66, 45, 155, 38, 20, 208, 136, 104, 26, 99, 66, 163, 225, 25, 87, 73, 56, 29, 75, 222, 18] },
Self::LiveAudits => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/LiveAudits.json"), length: 24038, sha256: [178, 67, 123, 65, 69, 209, 43, 201, 254, 32, 251, 29, 191, 44, 136, 140, 26, 104, 50, 242, 230, 153, 165, 0, 100, 1, 116, 121, 177, 18, 78, 74] },
Self::HistoricalCiExtras => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/historical-ci-extra-inputs.json"), length: 266989, sha256: [158, 141, 221, 103, 92, 215, 252, 174, 70, 32, 208, 98, 59, 57, 161, 222, 210, 251, 115, 144, 248, 247, 170, 202, 28, 41, 238, 78, 57, 7, 234, 100] },
}}}
pub(super) fn input_spec() -> Spec { Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/current-to-main043-helper14.json"), length: 2150672, sha256: [221, 33, 101, 13, 101, 16, 26, 50, 19, 96, 86, 88, 106, 75, 163, 41, 38, 249, 225, 43, 180, 151, 222, 111, 29, 80, 96, 132, 232, 239, 77, 53] } }
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
