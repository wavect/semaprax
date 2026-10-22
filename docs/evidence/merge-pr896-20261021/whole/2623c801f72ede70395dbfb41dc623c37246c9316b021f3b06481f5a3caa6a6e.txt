//! Sealed Main043 custody descriptors; no compiler authority.
#[derive(Clone, Copy)]
pub(crate) enum Fixture {
    ExitPrefix,
    ExitDirectory,
    LegacyNominal,
    Lifecycle,
    Prefix,
    Receiving,
    Edge,
    Selected,
    Status,
    Formatting,
    Receivers,
    LiveAudits,
    HistoricalCiExtras,
}
pub(super) struct Spec {
    pub(super) path: &'static str,
    pub(super) length: usize,
    pub(super) sha256: [u8; 32],
}
impl Fixture {
    pub(super) fn spec(self) -> Spec {
        match self {
Self::ExitPrefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-0.json"), length: 2301258, sha256: [202, 80, 102, 101, 47, 255, 34, 247, 216, 156, 64, 86, 192, 184, 195, 112, 195, 237, 142, 222, 199, 134, 43, 111, 178, 216, 106, 69, 198, 156, 149, 7] },
Self::ExitDirectory => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-1.json"), length: 3982867, sha256: [40, 143, 213, 10, 113, 59, 133, 227, 199, 108, 158, 137, 22, 142, 57, 47, 128, 167, 122, 93, 50, 190, 191, 183, 137, 125, 63, 245, 150, 202, 234, 125] },
Self::LegacyNominal => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-2.json"), length: 1741493, sha256: [166, 227, 189, 204, 192, 173, 7, 95, 214, 217, 87, 146, 7, 111, 178, 125, 173, 124, 55, 75, 104, 51, 123, 206, 85, 10, 43, 31, 114, 161, 80, 25] },
Self::Lifecycle => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-3.json"), length: 1505412, sha256: [209, 138, 200, 100, 101, 172, 189, 157, 220, 99, 133, 100, 187, 240, 23, 241, 185, 54, 83, 54, 61, 109, 30, 215, 11, 243, 103, 124, 52, 9, 202, 214] },
Self::Prefix => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-4.json"), length: 1390621, sha256: [16, 117, 83, 136, 216, 121, 29, 92, 13, 140, 112, 159, 218, 160, 198, 12, 151, 21, 109, 14, 122, 51, 117, 240, 163, 238, 159, 164, 159, 34, 224, 98] },
Self::Receiving => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-5.json"), length: 1802004, sha256: [30, 144, 137, 104, 206, 118, 37, 79, 28, 78, 51, 67, 78, 228, 228, 30, 181, 10, 28, 90, 97, 201, 117, 157, 157, 111, 211, 78, 63, 37, 99, 37] },
Self::Edge => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-6.json"), length: 2405930, sha256: [148, 14, 194, 47, 72, 235, 20, 27, 155, 250, 120, 51, 138, 11, 130, 219, 87, 57, 164, 73, 99, 244, 162, 147, 174, 51, 184, 170, 180, 186, 83, 25] },
Self::Selected => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-7.json"), length: 3000973, sha256: [129, 119, 109, 62, 233, 72, 132, 65, 176, 91, 230, 98, 201, 19, 166, 127, 80, 173, 121, 191, 199, 145, 216, 14, 52, 190, 84, 66, 225, 192, 130, 214] },
Self::Status => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/live/table-8.json"), length: 2405985, sha256: [221, 24, 227, 107, 143, 75, 166, 108, 133, 26, 173, 226, 205, 158, 77, 149, 203, 15, 6, 103, 177, 11, 198, 199, 114, 49, 247, 78, 46, 213, 128, 151] },
Self::Formatting => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Formatting.json"), length: 455600, sha256: [138, 233, 25, 13, 178, 42, 46, 229, 139, 88, 89, 106, 68, 199, 39, 220, 180, 25, 251, 224, 231, 180, 233, 234, 104, 189, 65, 20, 123, 13, 36, 72] },
Self::Receivers => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/Receivers.json"), length: 91294, sha256: [117, 72, 249, 231, 212, 41, 231, 239, 143, 178, 92, 66, 45, 155, 38, 20, 208, 136, 104, 26, 99, 66, 163, 225, 25, 87, 73, 56, 29, 75, 222, 18] },
Self::LiveAudits => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/incoming/LiveAudits.json"), length: 24038, sha256: [178, 67, 123, 65, 69, 209, 43, 201, 254, 32, 251, 29, 191, 44, 136, 140, 26, 104, 50, 242, 230, 153, 165, 0, 100, 1, 116, 121, 177, 18, 78, 74] },
Self::HistoricalCiExtras => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main043-independent-custody-20261016/historical-ci-extra-inputs.json"), length: 266989, sha256: [158, 141, 221, 103, 92, 215, 252, 174, 70, 32, 208, 98, 59, 57, 161, 222, 210, 251, 115, 144, 248, 247, 170, 202, 28, 41, 238, 78, 57, 7, 234, 100] },
}
    }
}
pub(super) fn input_spec() -> Spec {
    Spec {
        path: concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main043-independent-custody-20261016/current-to-main043-helper14.json"
        ),
        length: 2155584,
        sha256: [
            206, 123, 172, 108, 88, 166, 151, 165, 7, 209, 180, 180, 5, 93, 117, 147, 201, 8, 79,
            111, 86, 26, 253, 177, 94, 106, 62, 78, 4, 219, 67, 175,
        ],
    }
}
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
    if KEYS.binary_search(&source).is_err() {
        return current.to_owned();
    }
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
    assert!(
        KEYS.binary_search(&source).is_ok(),
        "unregistered historical CI comparison"
    );
    super::project(
        &super::runtime_json::main043_metadata(Fixture::HistoricalCiExtras),
        source,
        current,
    )
}
