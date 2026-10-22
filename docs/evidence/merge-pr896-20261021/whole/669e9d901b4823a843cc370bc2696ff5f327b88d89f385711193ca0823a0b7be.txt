use super::*;
use crate::source_reflows_merge::main_e177_inputs::read as main_e177_read;
static RECORDS: OnceLock<serde_json::Value> = OnceLock::new();
pub(super) fn records() -> &'static [serde_json::Value] {
    RECORDS
        .get_or_init(|| {
            serde_json::from_str(
                &main_e177_read(
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/source_reflows.json"),
                )
                .unwrap(),
            )
            .unwrap()
        })
        .as_array()
        .unwrap()
}

pub(super) fn restore(record: &serde_json::Value, current: &str) -> String {
    let path = record["path"].as_str().unwrap();
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        record["after_sha256"],
        "reviewed CI input drifted: {path}"
    );
    let mut restored = current.to_owned();
    for edit in record["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(
            restored.matches(after).count(),
            1,
            "nonunique CI inverse: {path}"
        );
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        record["before_sha256"],
        "CI inverse did not preserve the complete predecessor: {path}"
    );
    restored
}
