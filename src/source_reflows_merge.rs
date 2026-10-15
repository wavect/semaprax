//! Complete physical-input successors for the immutable CI and Source36 views.
//! This test-only projection owns no compiler authority and grants no admission.
use sha2::{Digest, Sha256};
fn project(rows: &str, source: &str, current: &str) -> String {
    let rows: serde_json::Value = serde_json::from_str(rows).expect("fixed merge projections");
    let matching: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["source"] == source)
        .collect();
    assert!(matching.len() <= 1, "unique merge source view");
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
            "complete unique merge inverse"
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
pub(crate) fn ours(source: &str, current: &str) -> String {
    let staged = project(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-composition-f813-held-20261015/pre-ours.json")), source, current);
    let current = staged.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source36-main88884-merge-20261015/ours-projections.json"
        )),
        source,
        current,
    )
}
pub(crate) fn main(source: &str, current: &str) -> String {
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-composition-f813-held-20261015/main-projections.json"
        )),
        source,
        current,
    )
}

pub(crate) fn held(source: &str, current: &str) -> String {
    project(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-composition-f813-held-20261015/held-projections.json")), source, current)
}

pub(crate) fn weighted(path: &str, current: &str) -> String {
    if path != "../epsilon_alias/tests.rs" { return current.to_owned(); }
    project(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-composition-f813-held-20261015/weighted-projections.json")), "src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias/tests.rs", current)
}
