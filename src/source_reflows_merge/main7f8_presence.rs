//! Independent Main7f8 presence input after the unchanged older EB2 front.
use sha2::{Digest, Sha256};
const LENGTH: usize = 90030;
const SHA256: &str = "9f87c5e8143e2ce8cf028bf186d087c97c392bcf54c71d6c16e48a2e0a8e930b";
const DATA: &str =
    include_str!("../../docs/evidence/main7f8-presence-composition-20261017/checkpoints.json");
fn digest(source: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}
fn restore(record: &serde_json::Value, current: &str) -> String {
    assert_eq!(digest(current), record["after_sha256"]);
    let mut result = current.to_owned();
    for edit in record["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(result.matches(after).count(), 1);
        result = result.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(digest(&result), record["before_sha256"]);
    result
}
pub(crate) fn after_eb2(path: &str, current: &str) -> String {
    assert_eq!(DATA.len(), LENGTH);
    assert_eq!(digest(DATA), SHA256);
    let rows: serde_json::Value = serde_json::from_str(DATA).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows.iter().filter(|r| r["path"] == path).count(), 1);
    let row = rows.iter().find(|r| r["path"] == path).unwrap();
    assert_eq!(digest(current), row["checkpoint_sha256"]);
    assert_eq!(current, row["checkpoint"].as_str().unwrap());
    let incoming = row["incoming"].as_str().unwrap();
    assert_eq!(digest(incoming), row["incoming_sha256"]);
    assert_eq!(
        restore(&row["old_presence"], current),
        restore(&row["incoming_presence"], incoming),
        "complete independent old and incoming presence source originals"
    );
    incoming.to_owned()
}
pub(crate) fn controls() {
    compact_controls();
    let rows: serde_json::Value = serde_json::from_str(DATA).unwrap();
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let current = row["checkpoint"].as_str().unwrap();
        assert_eq!(after_eb2(path, current), row["incoming"].as_str().unwrap());
        let before = before_eb2(path, row["incoming"].as_str().unwrap());
        let old = super::main_eb2_inputs::presence(path, &before);
        assert_eq!(after_eb2(path, &old), row["incoming"].as_str().unwrap());
        for wrong in [
            String::new(),
            format!("{current} "),
            format!("{current}{current}"),
        ] {
            assert!(std::panic::catch_unwind(|| after_eb2(path, &wrong)).is_err());
        }
        assert!(std::panic::catch_unwind(|| after_eb2("foreign", current)).is_err());
    }
}

// Distinct compact-owner input domain: its metadata itself changed in524.
fn compact_restore(record: &serde_json::Value, current: &str) -> String {
    let mut result = current.to_owned();
    for edit in record["edits"].as_array().unwrap().iter().rev() {
        let present = edit["new"].as_str().unwrap();
        assert_eq!(result.matches(present).count(), 1);
        result = result.replacen(present, edit["old"].as_str().unwrap(), 1);
    }
    assert_eq!(digest(&result), record["sha256"]);
    result
}
fn compact_data() -> serde_json::Value {
    const DATA: &str = include_str!(
        "../../docs/evidence/main7f8-presence-composition-20261017/compact-checkpoints.json"
    );
    assert_eq!(DATA.len(), 277898);
    assert_eq!(
        digest(DATA),
        "b8347a8c5c0fd7491223c860005451c357abbbe5af972bdfcdb698f8eb7d5c1b"
    );
    serde_json::from_str(DATA).unwrap()
}
pub(crate) fn compact_after_eb2(path: &str, current: &str) -> String {
    let rows = compact_data();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows.iter().filter(|r| r["path"] == path).count(), 1);
    let row = rows.iter().find(|r| r["path"] == path).unwrap();
    assert_eq!(digest(current), row["checkpoint_sha256"]);
    assert_eq!(current, row["checkpoint"].as_str().unwrap());
    let incoming = row["incoming"].as_str().unwrap();
    assert_eq!(digest(incoming), row["incoming_sha256"]);
    assert_eq!(
        compact_restore(&row["old_compact"], current),
        compact_restore(&row["incoming_compact"], incoming),
        "both complete independent compact cursor originals"
    );
    incoming.to_owned()
}

fn compact_controls() {
    let rows = compact_data();
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let current = row["checkpoint"].as_str().unwrap();
        assert_eq!(
            compact_after_eb2(path, current),
            row["incoming"].as_str().unwrap()
        );
        for wrong in [
            String::new(),
            format!("{current} "),
            format!("{current}{current}"),
        ] {
            assert!(std::panic::catch_unwind(|| compact_after_eb2(path, &wrong)).is_err());
        }
        assert!(std::panic::catch_unwind(|| compact_after_eb2("foreign", current)).is_err());
    }
}

// The new raw inline-never checkpoint may reach this historical route directly.
// Other inputs retain the unchanged EB2 guard, which refuses unknown bytes.
pub(crate) fn before_eb2(path: &str, current: &str) -> String {
    assert_eq!(DATA.len(), LENGTH);
    assert_eq!(digest(DATA), SHA256);
    let rows: serde_json::Value = serde_json::from_str(DATA).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let row = rows
        .iter()
        .find(|r| r["path"] == path)
        .expect("closed presence source path");
    if digest(current) == row["incoming_sha256"] {
        assert_eq!(current, row["incoming"].as_str().unwrap());
        row["checkpoint"].as_str().unwrap().to_owned()
    } else {
        current.to_owned()
    }
}
