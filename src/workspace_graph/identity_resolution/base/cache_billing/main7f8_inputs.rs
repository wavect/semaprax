//! Finite test-only inputs for the independent Main7f8 source receivers.
use sha2::{Digest, Sha256};
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
// This closed fixture inventory is read before any observation, never from a
// production or budget-probe route. One regular handle, exact length and EOF.
#[path = "main7f8_reader.rs"]
mod reader;
enum Fixture {
    OursProvider,
    IncomingProvider,
    RootFit,
    Checkpoints,
}
fn immutable(fixture: Fixture) -> String {
    let (path, length, sha256) = match fixture {
        Fixture::OursProvider => (
            "docs/evidence/provider-proof-backing-714177/whole-source-inverses.json",
            8619569usize,
            "76023783b493b4a5a887270001a4944bdd94bb54f712cca71eb6db954e760ae3",
        ),
        Fixture::IncomingProvider => (
            "docs/evidence/main7f8-composition-20261017/provider-main7f8.json",
            2151415usize,
            "b6e5daa21c4389650c11b52ed53ca46d361329e7e3e7d91f773a02abf9293ebc",
        ),
        Fixture::RootFit => (
            "docs/evidence/main7f8-composition-20261017/root-fit-input.json",
            476272usize,
            "778213c317e8652ea8ab64083e2d0221b0f752a795bce54e3bf6ae619036a0af",
        ),
        Fixture::Checkpoints => (
            "docs/evidence/main7f8-composition-20261017/provider-incoming-checkpoints.json",
            443642usize,
            "abad5c4b7d98ffbca8468a18a8bd84346e9982a89be87b418e0e8e8616cafe07",
        ),
    };
    reader::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path),
        length,
        sha256,
        8_619_569,
    )
    .expect("exact authenticated Main7f8 immutable fixture")
}

pub(super) fn provider_manifest() -> String {
    // This is called by the existing complete incoming-provider owner, outside probes.
    reader::controls();
    immutable(Fixture::IncomingProvider)
}
fn manifest(incoming: bool) -> serde_json::Value {
    let text = immutable(if incoming {
        Fixture::IncomingProvider
    } else {
        Fixture::OursProvider
    });
    serde_json::from_str(&text).unwrap()
}
fn row<'a>(manifest: &'a serde_json::Value, path: &str) -> &'a serde_json::Value {
    let rows = manifest["whole_sources"].as_array().unwrap();
    assert_eq!(rows.len(), 17);
    assert_eq!(rows.iter().filter(|row| row["file"] == path).count(), 1);
    rows.iter().find(|row| row["file"] == path).unwrap()
}
pub(super) fn cache_fit_input(current: &str) -> String {
    let text = immutable(Fixture::RootFit);
    let record: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(digest(current), record["checkpoint_sha256"]);
    assert_eq!(current, record["checkpoint"].as_str().unwrap());
    let incoming = record["incoming"].as_str().unwrap();
    assert_eq!(digest(incoming), record["incoming_sha256"]);
    incoming.to_owned()
}
pub(super) fn provider_checkpoint(path: &str, current: &str) -> String {
    let current = crate::source_reflows_merge::constructor_complete_input(path, current);
    let ours = manifest(false);
    let incoming = manifest(true);
    let before = row(&ours, path);
    let after = row(&incoming, path);
    assert_eq!(digest(&current), before["current_sha256"]);
    assert_eq!(before["original_sha256"], after["original_sha256"]);
    assert_eq!(before["original_bytes"], after["original_bytes"]);
    let mut original = current.clone();
    for edit in before["inverse"].as_array().unwrap() {
        let present = edit["current"].as_str().unwrap();
        assert_eq!(original.matches(present).count(), 1);
        original = original.replacen(present, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        original.len(),
        before["original_bytes"].as_u64().unwrap() as usize
    );
    assert_eq!(digest(&original), before["original_sha256"]);
    let result = if before["current_sha256"] == after["current_sha256"] {
        current
    } else {
        let text = immutable(Fixture::Checkpoints);
        let records: serde_json::Value = serde_json::from_str(&text).unwrap();
        let records = records.as_array().unwrap();
        assert_eq!(records.len(), 8);
        assert_eq!(records.iter().filter(|row| row["path"] == path).count(), 1);
        let record = records.iter().find(|row| row["path"] == path).unwrap();
        let result = record["text"].as_str().unwrap().to_owned();
        assert_eq!(digest(&result), record["sha256"]);
        result
    };
    assert_eq!(digest(&result), after["current_sha256"]);
    let mut independently_restored = result.clone();
    for edit in after["inverse"].as_array().unwrap() {
        let present = edit["current"].as_str().unwrap();
        assert_eq!(independently_restored.matches(present).count(), 1);
        independently_restored =
            independently_restored.replacen(present, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        independently_restored, original,
        "complete independent Main7f8 original source"
    );
    result
}
pub(super) fn provider_inventory_input(path: &str, current: &str) -> String {
    let record: serde_json::Value = serde_json::from_str(match path {
        "tests/workspace/semantic_graph/source_inventory.rs" => include_str!("../../../../../docs/evidence/source-reader-followups-20261016/inventory.json"),
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => include_str!("../../../../../docs/evidence/source-reader-followups-20261016/native-inventory.json"),
        _ => panic!("unregistered Main7f8 provider inventory"),
    }).unwrap();
    let mut result = provider_checkpoint(path, current);
    assert_eq!(digest(&result), record["before_sha256"]);
    for edit in record["inverse"].as_array().unwrap().iter().rev() {
        let present = edit["before"].as_str().unwrap();
        assert_eq!(result.matches(present).count(), 1);
        result = result.replacen(present, edit["after"].as_str().unwrap(), 1);
    }
    assert_eq!(digest(&result), record["after_sha256"]);
    result
}
