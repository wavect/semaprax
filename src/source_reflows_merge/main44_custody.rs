//! Fixed complete incoming Main44 metadata and exact source views; test authority only.
#[derive(Clone, Copy)]
pub(crate) enum Fixture { Compact7, Receivers12 }
pub(super) struct Spec { pub(super) path: &'static str, pub(super) length: usize, pub(super) sha256: [u8;32] }
impl Fixture { pub(super) fn spec(self) -> Spec { match self {
Self::Compact7 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/compact-inverse7.json"), length: 64689, sha256: [153, 168, 166, 192, 186, 118, 148, 200, 38, 143, 84, 161, 14, 105, 173, 157, 112, 255, 173, 144, 219, 53, 74, 198, 11, 128, 210, 106, 69, 174, 111, 128] },
Self::Receivers12 => Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/receiver-inverse12.json"), length: 91294, sha256: [117, 72, 249, 231, 212, 41, 231, 239, 143, 178, 92, 66, 45, 155, 38, 20, 208, 136, 104, 26, 99, 66, 163, 225, 25, 87, 73, 56, 29, 75, 222, 18] },
}}}
pub(super) fn input_spec() -> Spec {
    // MAIN44_INPUT_SPEC
    Spec { path: concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main44-independent-custody-20261016/current-to-main44.json"), length: 2475709, sha256: [81, 53, 15, 31, 30, 208, 162, 167, 143, 150, 107, 171, 3, 203, 226, 4, 250, 94, 20, 30, 158, 211, 247, 84, 156, 100, 55, 135, 36, 248, 245, 50] }
}
pub(super) fn input(source: &str, current: &str) -> String {
    const KEYS: &[&str] = &[
        "src/cleanup_plan/replay.rs",
        "src/cleanup_plan/replay/branch_clone/tests.rs",
        "src/cleanup_plan/replay/compact_stage/before.rs",
        "src/cleanup_plan/replay/compact_stage/lexical_before.rs",
        "src/cleanup_plan/replay/factored.rs",
        "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs",
        "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs",
        "src/cleanup_plan/replay/factored/oracle_hook.rs",
        "src/cleanup_plan/replay/path_array.rs",
        "src/cleanup_plan/replay/path_array/before.rs",
        "src/cleanup_plan/replay/path_array/final_before.rs",
        "src/cleanup_plan/replay/path_array/sequence.rs",
        "src/cleanup_plan/replay/path_array/sequence/before.rs",
        "src/cleanup_plan/replay/path_array/sequence/tests.rs",
        "src/cleanup_plan/replay/path_array/tests.rs",
        "src/cleanup_plan/replay/sequence_prefix/before.rs",
        "src/cleanup_plan/replay/sequence_vector.rs",
        "src/cleanup_plan/replay/sequence_vector/before.rs",
        "src/cleanup_plan/replay/sequence_vector/tests.rs",
        "src/cleanup_plan/replay/skeleton_work.rs",
        "src/hir.rs",
        "src/hir/expr_nodes.rs",
        "src/hir/validation.rs",
        "src/source_verify/declaration.rs",
        "tests/hir_module_boundaries.rs",
        "tests/workspace/semantic_graph/source_inventory.rs",

    ];
    if KEYS.binary_search(&source).is_err() { return current.to_owned(); }
    super::project(&super::runtime_json::main44_input(), source, current)
}
