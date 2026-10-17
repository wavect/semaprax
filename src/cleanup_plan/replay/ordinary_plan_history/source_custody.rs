//! Exact old source and additive root-route inverse; no runtime authority.
use sha2::{Digest, Sha256};
const INVERSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/ordinary-plan-history-c8740045-v1/root-inverse.json"
));
const BEFORE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/ordinary-plan-history-c8740045-v1/replay.rs.before.txt"
));
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}

pub(crate) fn prior_root(current: &str) -> String {
    let packet: serde_json::Value = serde_json::from_str(INVERSE).unwrap();
    let mut restored = current.to_owned();
    for edit in packet["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(
            restored.matches(after).count(),
            1,
            "ordinary plan history inverse must be unique"
        );
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    restored
}
#[test]
fn complete_predecessor_and_inverse_are_bound_to_exact_root_bytes() {
    let packet: serde_json::Value = serde_json::from_str(INVERSE).unwrap();
    assert_eq!(digest(BEFORE), packet["before_sha256"]);
    let current = include_str!("../../replay.rs");
    assert_eq!(digest(current), packet["after_sha256"]);
    assert_eq!(prior_root(current), BEFORE);
    let original = BEFORE
        .split("fn plan_skeleton_paths(")
        .nth(1)
        .unwrap()
        .split("\nfn storage_regions(")
        .next()
        .unwrap();
    assert!(current.contains(original));
}
