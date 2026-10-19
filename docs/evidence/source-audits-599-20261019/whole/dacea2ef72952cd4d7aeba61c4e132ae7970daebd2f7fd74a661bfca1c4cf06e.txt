//! Finite complete-input views for independently retained incoming source owners.
use sha2::{Digest, Sha256};
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
const DATA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/main-eb2-source-composition-20261016/inputs.json"
));
const DATA_LENGTH: usize = 17256;
const DATA_SHA256: &str = "805be510a86e46dfb8cd58b63eca7b89e347053674e3357e8cb3c48924ce4955";
pub(crate) fn input(path: &str, current: &str) -> String {
    let current = super::main7f8_source_inputs::later_452_input("eb2-incoming", path, current);
    project("incoming", path, &current)
}
pub(crate) fn presence(path: &str, current: &str) -> String {
    project("presence", path, current)
}
fn project(domain: &str, path: &str, current: &str) -> String {
    assert_eq!(DATA.len(), DATA_LENGTH);
    assert_eq!(digest(DATA), DATA_SHA256);
    let rows: serde_json::Value = serde_json::from_str(DATA).unwrap();
    let current_sha = digest(current);
    let matching: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            row["domain"] == domain && row["path"] == path && row["current_sha256"] == current_sha
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "exact finite complete source/domain choice"
    );
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
            let source = complete_incoming_input(domain, path, &source);
            let incoming = project(domain, path, &source);
            assert_eq!(digest(&incoming), row["incoming_sha256"].as_str().unwrap());
            for wrong in [
                String::new(),
                format!("{source} "),
                format!("{source}{source}"),
            ] {
                assert!(std::panic::catch_unwind(|| project(domain, path, &wrong)).is_err());
            }
            assert!(std::panic::catch_unwind(|| project("foreign", path, &source)).is_err());
            if domain == "incoming" && source != incoming {
                assert!(std::panic::catch_unwind(|| project(domain, path, &incoming)).is_err());
            }
        }
    }
}

// Later source-only/presence repairs precede the immutable incoming domains.
// Admit only these exact whole afterimages, then run the original table and
// all original rejection checks unchanged; archived inputs remain unchanged.
fn complete_incoming_input(domain: &str, path: &str, current: &str) -> String {
    if domain != "incoming" {
        return current.to_owned();
    }
    let closure92 =
        super::main7f8_source_inputs::closure92_walk_input("eb2-incoming-live", path, current);
    let acquired = super::main7f8_source_inputs::later_452_input("eb2-incoming", path, &closure92);
    let current = acquired.as_str();
    let presence =
        super::main7f8_source_inputs::presence_before_later_inventory(domain, path, current);
    let current = presence.as_str();
    let data = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/fade-custody-promotions-20261017/eb2-incoming-bridge.json"
    ));
    assert_eq!(
        digest(data),
        "549664ca658d0661599ace0e9dcd5b19c6ad34711dda25ab38b9a42856ae3acf"
    );
    let rows: serde_json::Value = serde_json::from_str(data).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 3);
    let hash = digest(current);
    let matching: Vec<_> = rows
        .iter()
        .filter(|row| row["path"] == path && row["current_sha256"] == hash)
        .collect();
    assert!(matching.len() <= 1, "unique finite incoming bridge");
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    let mut restored = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert_eq!(
            restored.matches(new).count(),
            1,
            "whole incoming bridge inverse"
        );
        restored = restored.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_eq!(digest(&restored), row["input_sha256"]);
    restored
}

#[test]
fn later_incoming_bridges_preserve_complete_original_domains_and_refusals() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/fade-custody-promotions-20261017/eb2-incoming-bridge.json"
    )))
    .unwrap();
    let original: serde_json::Value = serde_json::from_str(DATA).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let physical = std::fs::read_to_string(root.join(path)).unwrap();
        let raw = super::main7f8_source_inputs::presence_before_later_inventory(
            "incoming", path, &physical,
        );
        assert_eq!(digest(&raw), row["current_sha256"]);
        let input = complete_incoming_input("incoming", path, &physical);
        if raw != physical {
            assert!(
                std::panic::catch_unwind(|| complete_incoming_input("incoming", path, &raw))
                    .is_err(),
                "checkpoint must not be reacquired as raw"
            );
        }
        assert_eq!(digest(&input), row["input_sha256"]);
        let proof = original
            .as_array()
            .unwrap()
            .iter()
            .find(|proof| proof["domain"] == "incoming" && proof["path"] == path)
            .unwrap();
        assert_eq!(digest(&input), proof["current_sha256"]);
        assert_eq!(
            digest(&project("incoming", path, &input)),
            proof["incoming_sha256"]
        );
        assert!(std::panic::catch_unwind(|| project("incoming", path, &raw)).is_err());
        for wrong in [String::new(), format!("{raw} "), format!("{raw}{raw}")] {
            assert!(std::panic::catch_unwind(|| project("incoming", path, &wrong)).is_err());
        }
        assert!(std::panic::catch_unwind(|| project("foreign", path, &raw)).is_err());
        assert!(std::panic::catch_unwind(|| project("incoming", "foreign", &raw)).is_err());
    }
}
