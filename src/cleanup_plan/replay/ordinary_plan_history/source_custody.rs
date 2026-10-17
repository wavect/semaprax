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

const PACKET_LENGTH: usize = 2230;
const PACKET_SHA256: &str = "1b03e7b910f14010da7dadb4afd0321761b274b83363bd5fd1580d55159628f6";
fn records() -> serde_json::Value {
    assert!(INVERSE.len() <= 2 * 1024 * 1024);
    assert_eq!(INVERSE.len(), PACKET_LENGTH);
    assert_eq!(digest(INVERSE), PACKET_SHA256);
    serde_json::from_str(INVERSE).unwrap()
}
pub(crate) fn prior_root(current: &str) -> String {
    let packet = records();
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
    let packet = records();
    assert_eq!(digest(BEFORE), packet["before_sha256"]);
    let current = include_str!("../../replay.rs");
    let physical_root = current;
    let live_before = include_str!("before.rs");
    let acquired = crate::source_reflows_merge::ordinary_plan_history_root_checkpoint(current, live_before);
    let current = acquired.as_str();
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
    // Independent live compiled old owner: the historical root alone is insufficient.
    assert_eq!(live_before.len(), 27915);
    assert_eq!(digest(live_before), "9e07f08634f9a79e9d38955da089790acf951521362e46ba23b073e67e9ba3a5");
    assert!(live_before.contains(original));
    assert!(std::panic::catch_unwind(|| crate::source_reflows_merge::ordinary_plan_history_root_checkpoint(
        &(physical_root.to_owned() + "\n"), live_before)).is_err());
    assert!(std::panic::catch_unwind(|| crate::source_reflows_merge::ordinary_plan_history_root_checkpoint(
        physical_root, &(live_before.to_owned() + "\n"))).is_err());
}
