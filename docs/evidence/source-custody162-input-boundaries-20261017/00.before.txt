//! Exact current-to-immediate source join; every raw audit remains physical.

/// Exact whole-sequence front join for the immutable prior Clippy/source owner.
/// Raw source inventories never call this test-only input composition.
pub(super) fn sequence(current: &str) -> String {
    let acquired = before_later_sequence(current);
    let current = acquired.as_str();
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

// Prefix fusion and output publication changed the whole sequence after the
// frozen858 generation. Acquire that exact physical fragment before its guard.
fn before_later_sequence(current: &str) -> String {
    use sha2::{Digest, Sha256};
    let sha = |text: &str| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes())));
    const RAW: &str = include_str!("../../docs/evidence/source-custody-452-sequence-20261017/sequence.current.rs.txt");
    const PRIOR: &str = include_str!("../../docs/evidence/source-custody-452-sequence-20261017/sequence.prior.rs.txt");
    assert_eq!(RAW.len(), 8944);
    assert_eq!(sha(RAW), "562708b270e472605af04f9d2ace2d06101b7bd5a2b3295589e64c6cf48a86e7");
    assert_eq!(PRIOR.len(), 9058);
    assert_eq!(sha(PRIOR), "4f22f98575c40645de39c95f918240e440c2e639605aadd5b36dd553fe5b5804");
    let physical = include_str!("../cleanup_plan/replay.rs");
    let fragment = format!("fn sequence_skeleton_paths({}", physical.split_once("fn sequence_skeleton_paths(").unwrap().1.split_once("\nuse path_summary::has_active_paths;").unwrap().0);
    assert_eq!(fragment, RAW, "whole compiled sequence fragment");
    assert_eq!(current, RAW, "whole supplied sequence fragment");
    let mut restored = current.to_owned();
    let after = r###"        // Lexical remainder of the original first clone: no second charge and
"###;
    let before = r###"        let Some((last, earlier)) = pending.as_slice().split_last() else {
            continue;
        };
        // Lexical remainder of the original first clone: no second charge and
"###;
    assert_eq!(restored.matches(after).count(), 1);
    restored = restored.replacen(after, before, 1);
    let after = r###"        let observations = skeleton_work::prefix_fusion::after_work!(
            work, prefix.observations, first_suffix.observations
        );
"###;
    let before = r###"        work.budget
            .reserve_clone(work.function, &prefix.observations, "skeleton prefix clone")?;
        note_skeleton_materialization();
        let observations = prefix.observations.clone();
        let observations = work.extend_owned_observations(
            observations,
            &first_suffix.observations,
            "skeleton suffix clone",
        )?;
"###;
    assert_eq!(restored.matches(after).count(), 1);
    restored = restored.replacen(after, before, 1);
    let after = r###"        // Preserve the original first-output remaining-count receiver: only
        // after that publication may the last suffix leave this iterator.
        let Some(last) = pending.next_back() else {
            continue;
        };
        for suffix in pending {
"###;
    let before = r###"        for suffix in earlier.iter() {
"###;
    assert_eq!(restored.matches(after).count(), 1);
    restored = restored.replacen(after, before, 1);
    assert_eq!(restored, PRIOR, "complete frozen858 sequence input");
    restored
}
#[test]
fn later_sequence_fusion_keeps_complete858_input_and_rejects_other_stages() {
    let raw = include_str!("../../docs/evidence/source-custody-452-sequence-20261017/sequence.current.rs.txt");
    let prior = before_later_sequence(raw);
    assert_ne!(prior, raw);
    assert_eq!(sequence(raw), include_str!("../../docs/evidence/opt858-sequence-probe-header-20261016/immediate-sequence.rs.before.txt"));
    for bad in [String::new(), raw[..raw.len()-1].to_owned(), format!("{raw} "), prior] {
        assert!(std::panic::catch_unwind(|| before_later_sequence(&bad)).is_err());
    }
}
