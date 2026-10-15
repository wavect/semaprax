//! Authenticate the complete current source through the paid-insertion checkpoint.
pub(super) fn checkpoint(path: &str, source: &str) -> String {
    let patch = match path {
        "../../canonical_shared.rs" => include_str!(
            "../../../../../../docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared.rs.patch.txt"
        ),
        "../../../../../../tests/hir_module_boundaries.rs" => include_str!(
            "../../../../../../docs/evidence/weighted-storage-838/amortized/tests__hir_module_boundaries.rs.patch.txt"
        ),
        "../../../../../../tests/workspace/semantic_graph/source_inventory.rs" => include_str!(
            "../../../../../../docs/evidence/weighted-storage-838/amortized/tests__workspace__semantic_graph__source_inventory.rs.patch.txt"
        ),
        _ => return source.to_owned(),
    };
    super::source::apply(source, patch, true)
}

#[test]
fn ordered_amortized_checkpoint_full_sources_and_inverses_are_exact() {
    macro_rules! bind {
        ($current:literal, $old:literal, $patch:literal) => {{
            let current = include_str!($current);
            let old = include_str!($old);
            let patch = include_str!($patch);
            assert_eq!(super::source::apply(old, patch, false), current);
            assert_eq!(super::source::apply(current, patch, true), old);
        }};
    }
    bind!(
        "../ordered_weights.rs",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared__ordered_weights.rs.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared__ordered_weights.rs.patch.txt"
    );
    bind!(
        "tests.rs",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared__ordered_weights__tests.rs.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared__ordered_weights__tests.rs.patch.txt"
    );
    bind!(
        "../../canonical_shared.rs",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared.rs.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared.rs.patch.txt"
    );
    bind!(
        "source.rs",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared__ordered_weights__source.rs.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared__ordered_weights__source.rs.patch.txt"
    );
    bind!(
        "../../../../../../tests/hir_module_boundaries.rs",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/tests__hir_module_boundaries.rs.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/tests__hir_module_boundaries.rs.patch.txt"
    );
    bind!(
        "../../../../../../tests/workspace/semantic_graph/source_inventory.rs",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/tests__workspace__semantic_graph__source_inventory.rs.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/tests__workspace__semantic_graph__source_inventory.rs.patch.txt"
    );
    bind!(
        "../../../../../../docs/evidence/weighted-storage-838/README.md",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/docs__evidence__weighted-storage-838__README.md.before.txt",
        "../../../../../../docs/evidence/weighted-storage-838/amortized/docs__evidence__weighted-storage-838__README.md.patch.txt"
    );
}
