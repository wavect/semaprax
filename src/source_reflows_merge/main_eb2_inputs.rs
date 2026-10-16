//! Finite complete-input views for independently retained incoming source owners.
use sha2::{Digest, Sha256};
fn digest(text: &str) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes())))
}
const DATA: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main-eb2-source-composition-20261016/inputs.json"));
const DATA_LENGTH: usize = 17256;
const DATA_SHA256: &str = "805be510a86e46dfb8cd58b63eca7b89e347053674e3357e8cb3c48924ce4955";
pub(crate) fn input(path: &str, current: &str) -> String {
    project("incoming", path, current)
}
pub(crate) fn presence(path: &str, current: &str) -> String {
    project("presence", path, current)
}
fn project(domain: &str, path: &str, current: &str) -> String {
    assert_eq!(DATA.len(), DATA_LENGTH);
    assert_eq!(digest(DATA), DATA_SHA256);
    let rows: serde_json::Value = serde_json::from_str(DATA).unwrap();
    let current_sha = digest(current);
    let matching: Vec<_> = rows.as_array().unwrap().iter().filter(|row| {
        row["domain"] == domain && row["path"] == path && row["current_sha256"] == current_sha
    }).collect();
    assert_eq!(matching.len(), 1, "exact finite complete source/domain choice");
    let row = matching[0];
    let mut restored = String::new();
    let mut cursor = 0;
    for edit in row["edits"].as_array().unwrap() {
        let start = edit["start"].as_u64().unwrap() as usize;
        let end = edit["end"].as_u64().unwrap() as usize;
        assert!(cursor <= start && start <= end && end <= current.len());
        assert_eq!(&current[start..end], edit["after"].as_str().unwrap());
        restored.push_str(&current[cursor..start]);
        restored.push_str(edit["before"].as_str().unwrap());
        cursor = end;
    }
    restored.push_str(&current[cursor..]);
    assert_eq!(digest(&restored), row["incoming_sha256"].as_str().unwrap());
    restored
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_incoming_and_presence_views_refuse_foreign_truncated_and_duplicate_inputs() {
        let rows: serde_json::Value = serde_json::from_str(DATA).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for row in rows.as_array().unwrap() {
            let domain = row["domain"].as_str().unwrap();
            let path = row["path"].as_str().unwrap();
            let source = if let Some(archive) = row["input_archive"].as_str() {
                std::fs::read_to_string(root.join(archive)).unwrap()
            } else {
                std::fs::read_to_string(root.join(path)).unwrap()
            };
            let incoming = project(domain, path, &source);
            assert_eq!(digest(&incoming), row["incoming_sha256"].as_str().unwrap());
            for wrong in [String::new(), format!("{source} "), format!("{source}{source}")] {
                assert!(std::panic::catch_unwind(|| project(domain, path, &wrong)).is_err());
            }
            assert!(std::panic::catch_unwind(|| project("foreign", path, &source)).is_err());
            if domain == "incoming" && source != incoming {
                assert!(std::panic::catch_unwind(|| project(domain, path, &incoming)).is_err());
            }
        }
    }
}
