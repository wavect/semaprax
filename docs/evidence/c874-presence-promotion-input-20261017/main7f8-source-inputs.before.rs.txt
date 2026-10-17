//! Closed whole-source inputs for incoming historical owners, outside probes.
//! Actual behavioral and compiled-receiver checks keep their raw current reads.
use sha2::{Digest, Sha256};
#[path = "../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
const PATH: &str = "docs/evidence/main7f8-source-fronts-20261017/inputs.json";
const LENGTH: usize = 1357563;
const SHA256: &str = "a2ca1d35b681f1a9d8f1e53c7b8b01fffbb8a33126b278d0342ee5aedf40dc2c";
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
fn records() -> serde_json::Value {
    // Same one-regular-handle checked reader and hostile controls as Main7f8.
    let text = reader::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(PATH),
        LENGTH,
        SHA256,
        LENGTH,
    )
    .expect("exact authenticated incoming owner input table");
    let records: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(records.as_array().unwrap().len(), 15);
    records
}
fn project(rows: &serde_json::Value, domain: &str, path: &str, current: &str) -> String {
    let rows = rows.as_array().unwrap();
    assert_eq!(
        rows.iter()
            .filter(|r| r["domain"] == domain && r["path"] == path)
            .count(),
        1,
        "closed incoming owner/source domain"
    );
    let row = rows
        .iter()
        .find(|r| r["domain"] == domain && r["path"] == path)
        .unwrap();
    assert_eq!(
        digest(current),
        row["current_sha256"],
        "complete actual incoming-owner input"
    );
    assert_eq!(current, row["current"].as_str().unwrap());
    let incoming = row["incoming"].as_str().unwrap();
    assert_eq!(digest(incoming), row["incoming_sha256"]);
    incoming.to_owned()
}
pub(crate) fn source(domain: &str, path: &str, current: &str) -> String {
    // A later combined batch must bind its new complete raw input to THIS
    // immutable checkpoint before entering this owner; never repin these hashes.
    let outer = super::final_ready_inputs::source_owner(domain, path, current);
    project(&records(), domain, path, &outer)
}
pub(crate) fn controls() {
    super::final_ready_inputs::controls();
    reader::controls();
    let records = records();
    for row in records.as_array().unwrap() {
        let domain = row["domain"].as_str().unwrap();
        let path = row["path"].as_str().unwrap();
        let current = row["current"].as_str().unwrap();
        assert_eq!(
            project(&records, domain, path, current),
            row["incoming"].as_str().unwrap()
        );
        for wrong in [
            String::new(),
            format!("{current} "),
            format!("{current}{current}"),
        ] {
            assert!(std::panic::catch_unwind(|| project(&records, domain, path, &wrong)).is_err());
        }
        assert!(std::panic::catch_unwind(|| project(&records, "foreign", path, current)).is_err());
        assert!(
            std::panic::catch_unwind(|| project(&records, domain, "foreign", current)).is_err()
        );
    }
}
