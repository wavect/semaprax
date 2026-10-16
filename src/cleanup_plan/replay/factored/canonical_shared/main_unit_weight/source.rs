
pub(super) fn historical_main_source() -> String {
    use sha2::{Digest, Sha256};
    let current = include_str!("../main_unit_weight.rs");
    let table: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main-beb0-independent-controls-20261016/runtime-input.json"))).unwrap();
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))), "71b6f03b83115e6c048e46630179c0a17562c9b1f974fac5af3ff132ea86bcb6");
    assert_eq!(current, table["current"].as_str().unwrap());
    let original = table["main"].as_str().unwrap();
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))), "c6c624d0d0dc1dd74238e5419fb3d683b879619d492cd94eded556ea71071960");
    original.to_owned()
}
