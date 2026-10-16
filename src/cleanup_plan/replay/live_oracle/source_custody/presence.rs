//! Exact named delta for the independently qualified 512-depth presence scan.
//! Historical receivers keep every original byte after this complete-input
//! authentication; the default-stack and forged-bottom gates exercise current.
use sha2::{Digest, Sha256};
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
pub(super) fn restore(path: &str, current: &mut String) {
    *current = crate::source_reflows_merge::main_eb2_inputs::presence(path, current);
    let records: serde_json::Value = serde_json::from_str(include_str!("presence.json")).unwrap();
    let record = records
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == path)
        .unwrap();
    assert_eq!(digest(current), record["after_sha256"]);
    for edit in record["inverse"].as_array().unwrap() {
        super::undo(
            current,
            edit["after"].as_str().unwrap(),
            edit["before"].as_str().unwrap(),
        );
    }
    assert_eq!(digest(current), record["before_sha256"]);
}
