#![allow(
    clippy::duplicate_mod,
    reason = "The complete fixture intentionally compiles independently against current and predecessor owner types."
)]
//! Authenticate the complete current source through the paid-insertion checkpoint.
pub(super) fn checkpoint(path: &str, source: &str) -> String {
    let projected = current40f(path, source);
    let source = projected.as_str();
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

// Complete current input is authenticated before either historical patch hop.
fn current40f(path: &str, current: &str) -> String {
    let observed = if path == "tests.rs" {
        crate::source_reflows_merge::consumer_child(
            "ordered838",
            "src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/tests.rs",
            current,
        )
    } else {
        current.to_owned()
    };
    let current = observed.as_str();
    let held = if path == "../ordered_weights.rs" {
        crate::cleanup_plan::replay::physical_budget::authentication::previous(
            "src/cleanup_plan/replay/factored/canonical_shared/ordered_weights.rs",
            current,
        )
    } else {
        crate::cleanup_plan::replay::physical_budget::authentication::previous_include(
            path, current,
        )
    };
    let current = held.as_str();
    let merged = crate::source_reflows_merge::weighted(path, current);
    let current = merged.as_str();
    use sha2::{Digest, Sha256};
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/weighted-storage-838-main2b5-adapter-20261015/runtime-projections.json"
    )))
    .expect("fixed protected ordered source views");
    let matching: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["include_path"] == path)
        .collect();
    assert!(matching.len() <= 1, "unique ordered current source view");
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["current_sha256"]
    );
    let mut result = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert_eq!(
            result.matches(new).count(),
            1,
            "whole unique ordered source inverse"
        );
        result = result.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(result.as_bytes()))
        ),
        row["original_sha256"]
    );
    result
}

#[test]
fn ordered_amortized_checkpoint_full_sources_and_inverses_are_exact() {
    macro_rules! bind {
        ($current:literal, $old:literal, $patch:literal) => {{
            let projected = current40f($current, include_str!($current));
            let current = projected.as_str();
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
