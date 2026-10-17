//! Exact current-to-immediate source join; every raw audit remains physical.

/// Exact whole-sequence front join for the immutable prior Clippy/source owner.
/// Raw source inventories never call this test-only input composition.
pub(super) fn sequence(current: &str) -> String {
    use sha2::{Digest, Sha256};
    let data = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/opt858-sequence-probe-header-20261016/sequence-input.json"));
    assert_eq!(data.len(), 294);
    assert_eq!(<[u8; 32]>::from(Sha256::digest(data.as_bytes())), [255, 241, 6, 214, 205, 46, 75, 179, 249, 220, 250, 133, 12, 97, 159, 72, 10, 175, 178, 22, 45, 180, 62, 82, 164, 241, 203, 80, 121, 196, 203, 168]);
    let row: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))), row["current_sha256"].as_str().unwrap());
    let after = "path_array::sequence_minimum::optimized::push!(";
    assert_eq!(row["after"], after);
    assert_eq!(row["before"], "path_array::sequence_minimum::push!(");
    assert_eq!(current.matches(after).count(), 1);
    let restored = current.replacen(after, row["before"].as_str().unwrap(), 1);
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))), row["original_sha256"].as_str().unwrap());
    restored
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_sequence_join_refuses_empty_foreign_duplicate_and_old_input() {
        let root = include_str!("../cleanup_plan/replay.rs");
        let current = format!("fn sequence_skeleton_paths({}", root.split_once("fn sequence_skeleton_paths(").unwrap().1.split_once("\nuse path_summary::has_active_paths;").unwrap().0);
        let private = sequence(&current);
        assert_ne!(private, current);
        for wrong in [String::new(), format!("{current} "), format!("{current}path_array::sequence_minimum::optimized::push!("), private] {
            assert!(std::panic::catch_unwind(|| sequence(&wrong)).is_err());
        }
    }
}
