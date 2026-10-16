//! Sealed Main043 custody descriptors; no compiler authority.
#[derive(Clone, Copy)]
pub(crate) enum Fixture { ExitPrefix, ExitDirectory, LegacyNominal, Lifecycle, Prefix, Receiving, Edge, Selected, Status, Formatting, Receivers, LiveAudits, HistoricalCiExtras }
pub(super) struct Spec { pub(super) path: &'static str, pub(super) length: usize, pub(super) sha256: [u8;32] }
impl Fixture { pub(super) fn spec(self) -> Spec { match self {
Self::ExitPrefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-0.json"), length: 2279030, sha256: [103, 46, 172, 113, 166, 4, 202, 131, 5, 202, 190, 52, 142, 61, 69, 103, 152, 250, 133, 107, 215, 227, 2, 137, 59, 106, 157, 203, 161, 227, 194, 221] },
Self::ExitDirectory => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-1.json"), length: 3927894, sha256: [253, 107, 242, 145, 196, 250, 27, 40, 34, 29, 243, 127, 138, 6, 3, 240, 8, 159, 151, 77, 134, 55, 71, 72, 237, 189, 34, 74, 41, 229, 24, 251] },
Self::LegacyNominal => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-2.json"), length: 1620044, sha256: [42, 166, 181, 182, 53, 74, 240, 2, 13, 113, 85, 187, 241, 99, 179, 22, 147, 156, 109, 173, 230, 167, 143, 107, 71, 198, 209, 105, 120, 8, 95, 189] },
Self::Lifecycle => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-3.json"), length: 1488690, sha256: [177, 134, 134, 239, 88, 93, 156, 3, 32, 213, 187, 240, 85, 6, 165, 230, 109, 131, 117, 74, 160, 50, 180, 254, 131, 138, 78, 170, 64, 141, 56, 123] },
Self::Prefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-4.json"), length: 1354644, sha256: [173, 150, 94, 135, 97, 206, 22, 183, 23, 109, 235, 64, 10, 90, 90, 234, 85, 253, 77, 151, 195, 58, 170, 198, 144, 235, 54, 214, 145, 204, 18, 222] },
Self::Receiving => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-5.json"), length: 1743609, sha256: [105, 231, 89, 106, 56, 90, 33, 100, 99, 158, 252, 214, 109, 41, 143, 128, 75, 20, 137, 130, 132, 49, 225, 204, 109, 60, 249, 180, 21, 241, 194, 92] },
Self::Edge => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-6.json"), length: 2383702, sha256: [70, 83, 143, 140, 12, 164, 59, 117, 89, 233, 89, 142, 5, 50, 38, 109, 219, 58, 57, 191, 138, 63, 119, 32, 192, 241, 205, 42, 145, 190, 173, 94] },
Self::Selected => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-7.json"), length: 2976160, sha256: [30, 211, 72, 150, 143, 128, 164, 156, 196, 114, 174, 99, 82, 210, 225, 123, 64, 103, 148, 83, 217, 18, 27, 5, 120, 123, 1, 141, 73, 137, 65, 21] },
Self::Status => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-8.json"), length: 2383757, sha256: [58, 189, 152, 46, 81, 183, 69, 170, 3, 34, 189, 43, 94, 249, 35, 103, 233, 139, 254, 84, 19, 70, 89, 237, 235, 60, 98, 13, 72, 62, 111, 95] },
Self::Formatting => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Formatting.json"), length: 455600, sha256: [138, 233, 25, 13, 178, 42, 46, 229, 139, 88, 89, 106, 68, 199, 39, 220, 180, 25, 251, 224, 231, 180, 233, 234, 104, 189, 65, 20, 123, 13, 36, 72] },
Self::Receivers => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Receivers.json"), length: 91294, sha256: [117, 72, 249, 231, 212, 41, 231, 239, 143, 178, 92, 66, 45, 155, 38, 20, 208, 136, 104, 26, 99, 66, 163, 225, 25, 87, 73, 56, 29, 75, 222, 18] },
Self::LiveAudits => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/LiveAudits.json"), length: 24038, sha256: [178, 67, 123, 65, 69, 209, 43, 201, 254, 32, 251, 29, 191, 44, 136, 140, 26, 104, 50, 242, 230, 153, 165, 0, 100, 1, 116, 121, 177, 18, 78, 74] },
Self::HistoricalCiExtras => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/historical-ci-extra-inputs.json"), length: 266989, sha256: [158, 141, 221, 103, 92, 215, 252, 174, 70, 32, 208, 98, 59, 57, 161, 222, 210, 251, 115, 144, 248, 247, 170, 202, 28, 41, 238, 78, 57, 7, 234, 100] },
}}}
pub(super) fn input_spec() -> Spec { Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/current-to-main043-helper14.json"), length: 2111692, sha256: [250, 137, 116, 242, 137, 113, 28, 75, 43, 53, 183, 198, 10, 145, 193, 232, 55, 89, 90, 48, 114, 127, 29, 102, 230, 122, 226, 205, 62, 237, 131, 219] } }
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
