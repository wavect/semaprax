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
    let acquired = later_452_input(domain, path, current);
    let current = acquired.as_str();
    let presence = presence_before_later_inventory(domain, path, current);
    let outer = super::final_ready_inputs::source_owner(domain, path, &presence);
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

// Only the two existing presence consumers acquire this compiled raw source.
// The old Main7f8 checkpoint and its downstream EB2 inverse remain immutable.
pub(crate) fn presence_before_later_inventory(domain: &str, path: &str, current: &str) -> String {
    if path != "src/cleanup_plan/replay/live_oracle/source_custody/presence.json"
        || !matches!(domain, "loan-stack" | "eb2-compact" | "incoming") {
        return current.to_owned();
    }
    let raw = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/c874-presence-promotion-input-20261017/presence.c874-43ce-current.json.txt"));
    assert_eq!(raw.len(), 48_138, "fixed c874 whole presence length");
    assert_eq!(digest(raw), "43ce0a20e3e727960c010da44561b8ec6c0e8353ed48d5067662c955be0ed7cc", "fixed c874 whole presence SHA");
    let compiled = include_str!("../cleanup_plan/replay/live_oracle/source_custody/presence.json");
    assert_eq!(compiled, raw, "actual compiled presence equals independent fixed c874 endpoint");
    assert_eq!(current.len(), raw.len(), "whole compiled presence input length");
    assert_eq!(digest(current), digest(raw), "whole compiled presence input");
    assert_eq!(current, raw, "complete compiled presence input");
    let checkpoint = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/c874-presence-promotion-input-20261017/presence.73f72-checkpoint.json.txt"));
    assert_eq!(checkpoint.len(), 20_477);
    assert_eq!(digest(checkpoint), "73f72ac12231befb30913d153afa06c36a2f1af4f27336fba3fdf2c80a84bfba");
    checkpoint.to_owned()
}

#[test]
fn complete_presence_inventory_acquisition_preserves_both_domains_and_refuses_drift() {
    let path = "src/cleanup_plan/replay/live_oracle/source_custody/presence.json";
    let raw = include_str!("../cleanup_plan/replay/live_oracle/source_custody/presence.json");
    let rows = records();
    for domain in ["loan-stack", "eb2-compact"] {
        let before = presence_before_later_inventory(domain, path, raw);
        assert_ne!(raw, before);
        let row = rows.as_array().unwrap().iter().find(|row| row["domain"] == domain && row["path"] == path).unwrap();
        assert_eq!(before, row["current"].as_str().unwrap());
        assert_eq!(project(&rows, domain, path, &before), row["incoming"].as_str().unwrap());
        for bad in [String::new(), raw[..raw.len()-1].to_owned(), format!("{raw} "), format!("{raw}{raw}"), before.clone()] {
            assert!(std::panic::catch_unwind(|| presence_before_later_inventory(domain, path, &bad)).is_err());
        }
    }
    assert_eq!(presence_before_later_inventory("foreign", path, "foreign"), "foreign");
    assert_eq!(presence_before_later_inventory("incoming", "foreign", "foreign"), "foreign");
}

// Exact later input generation before immutable EB2/cache-fit/compact domains.
// Physical source and supplied stage are separate authenticated channels.
pub(crate) fn later_452_input(domain: &str, path: &str, current: &str) -> String {
    if !matches!((domain, path),
        ("eb2-incoming", "src/cleanup_plan/replay/physical_clone.rs")
        | ("cache-fit", "src/workspace_graph.rs")
        | ("compact-original", "src/cleanup_plan/replay/live_oracle/source_custody/presence.json")) {
        return current.to_owned();
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = root.join("docs/evidence/source-custody-452-inputs-20261017");
    let table = reader::read(&directory.join("inputs.json"), 1751,
        "aea83e0739377303ed5043183b222f5925b4f1facceef2e16e27ec1b16b17ac3", 65536)
        .expect("fixed complete452 input table");
    let rows: serde_json::Value = serde_json::from_str(&table).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 3);
    let matching: Vec<_> = rows.iter().filter(|r| r["domain"] == domain && r["path"] == path).collect();
    assert_eq!(matching.len(), 1, "closed452 whole input domain");
    let row = matching[0];
    let read = |field: &str, actual: bool| {
        let length = usize::try_from(row[format!("{field}_length")].as_u64().unwrap()).unwrap();
        let sha = row[format!("{field}_sha256")].as_str().unwrap();
        let file = if actual { root.join(path) } else { directory.join(row[format!("{field}_archive")].as_str().unwrap()) };
        reader::read(&file, length, sha, 2_097_152).expect("whole452 bounded regular input")
    };
    let physical = read("physical", true);
    assert_eq!(physical, read("physical", false), "actual physical source archive");
    let input = read("current", false);
    assert_eq!(current, input, "whole supplied452 source stage");
    read("previous", false)
}
#[test]
fn later_452_source_inputs_preserve_complete_frozen_domains_and_refuse_wrong_stages() {
    for (domain, path, current) in [
        ("eb2-incoming", "src/cleanup_plan/replay/physical_clone.rs", include_str!("../cleanup_plan/replay/physical_clone.rs")),
        ("cache-fit", "src/workspace_graph.rs", include_str!("../workspace_graph.rs")),
        ("compact-original", "src/cleanup_plan/replay/live_oracle/source_custody/presence.json", include_str!("../../docs/evidence/source-custody-452-inputs-20261017/2.current.txt"))] {
        let prior = later_452_input(domain, path, current);
        assert_ne!(prior, current);
        for bad in [String::new(), current[..current.len()-1].to_owned(), format!("{current} "), prior] {
            assert!(std::panic::catch_unwind(|| later_452_input(domain, path, &bad)).is_err());
        }
    }
    assert_eq!(later_452_input("foreign", "foreign", "untouched"), "untouched");
}
