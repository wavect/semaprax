//! Exact one-literal arm normalization, then the complete common predecessor.
//! These source views grant no compiler, filesystem or budget authority.
use super::digest;
const COMMON: [u8; 32] = [
    170, 140, 67, 163, 43, 160, 216, 111, 88, 41, 190, 226, 38, 75, 151, 83, 237, 235, 109, 31,
    180, 16, 189, 90, 160, 64, 217, 189, 212, 95, 149, 254,
];
const EMBEDDED: [u8; 32] = [
    151, 55, 164, 123, 234, 27, 149, 4, 35, 119, 2, 145, 53, 161, 115, 17, 198, 107, 60, 203, 195,
    146, 106, 8, 170, 15, 142, 1, 44, 47, 114, 21,
];
pub(super) fn normalize(source: &str, current: &str) -> String {
    if source != "src/source_reflows_merge/runtime_json.rs" {
        return current.to_owned();
    }
    let actual = digest(current.as_bytes());
    if actual == COMMON {
        return current.to_owned();
    }
    assert_eq!(actual, EMBEDDED, "exact selected transport source arm");
    assert_eq!(
        current
            .matches("runtime_json/transport_embedded.rs")
            .count(),
        1
    );
    assert!(!current.contains("runtime_json/transport_runtime.rs"));
    let common = current.replacen(
        "runtime_json/transport_embedded.rs",
        "runtime_json/transport_runtime.rs",
        1,
    );
    assert_eq!(
        digest(common.as_bytes()),
        COMMON,
        "complete common transport source"
    );
    common
}
pub(super) fn previous(source: &str, current: &str) -> String {
    let common = normalize(source, current);
    super::super::project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/bulk-transport-pair-848-next-20261016/current-to-0c.json"
        )),
        source,
        &common,
    )
}
