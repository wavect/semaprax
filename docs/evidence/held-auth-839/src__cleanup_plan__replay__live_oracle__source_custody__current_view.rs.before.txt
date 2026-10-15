//! Authenticated whole-source views for the unchanged original inverse guards.
use sha2::{Digest, Sha256};

pub(super) fn project(source: &str, current: &str) -> String {
    let merged = crate::source_reflows_merge::ours(source, current);
    let current = merged.as_str();
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/source36-after-next-838-839-20261015/source-projections.json"
    )))
    .expect("fixed current source projections");
    let rows = rows.as_array().expect("source projection array");
    let matching: Vec<_> = rows.iter().filter(|row| row["source"] == source).collect();
    if matching.is_empty() {
        return current.to_owned();
    }
    assert_eq!(matching.len(), 1, "unique authenticated source view");
    let row = matching[0];
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["current_sha256"]
    );
    let mut projected = current.to_owned();
    for edit in row["edits"].as_array().expect("complete source inverse") {
        let new = edit["new"].as_str().expect("current source bytes");
        let old = edit["old"].as_str().expect("checkpoint source bytes");
        assert_eq!(projected.matches(new).count(), 1, "unique complete inverse");
        projected = projected.replacen(new, old, 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(projected.as_bytes()))
        ),
        row["original_sha256"]
    );
    projected
}
