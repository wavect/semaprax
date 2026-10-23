//! Whole-source inverse and immutable shared dependency binding for the oracle.
use sha2::{Digest, Sha256};
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
#[test]
fn complete_immediate_old_driver_and_each_helper_restore_their_whole_preimages() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        digest(include_bytes!("../before_frontier/custody/inverse.json")),
        "522a451f88422fff1f931b183d753bf121ca9e14fbbef12fd3d087c5352fb595"
    );
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("../before_frontier/custody/inverse.json")).unwrap();
    for row in rows.as_array().unwrap() {
        let mut actual =
            std::fs::read_to_string(root.join(row["compiled"].as_str().unwrap())).unwrap();
        let original = std::fs::read_to_string(root.join(row["before"].as_str().unwrap())).unwrap();
        assert_eq!(digest(original.as_bytes()), row["sha256"].as_str().unwrap());
        for change in row["inverse"].as_array().unwrap().iter().rev() {
            let at = change[0].as_u64().unwrap() as usize;
            let current = change[1].as_str().unwrap();
            let before = change[2].as_str().unwrap();
            assert_eq!(actual.get(at..at + current.len()), Some(current));
            actual.replace_range(at..at + current.len(), before);
        }
        assert_eq!(actual, original, "whole inverse for {}", row["compiled"]);
    }
    assert_eq!(
        include_str!("../before_frontier/whole.rs.txt"),
        include_str!("../before_frontier/custody/mod.rs.txt")
    );
}
#[test]
fn shared_replay_dependencies_and_unchanged_driver_helpers_remain_exactly_source_bound() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        digest(include_bytes!(
            "../before_frontier/custody/shared_readset.json"
        )),
        "a6004b3b67c5af61e98e225ae1716e3aa5dd9c08d01f14afa02d1ba73f62c4f7"
    );
    let rows: Vec<(String, String)> = serde_json::from_str(include_str!(
        "../before_frontier/custody/shared_readset.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 946);
    assert!(rows
        .iter()
        .all(|(path, _)| path != "src/cleanup_plan/replay/path_join.rs"));
    // The changed driver is held in full by whole.rs.txt and its separately
    // compiled inverse above. Its shared helper bodies and selected module
    // declaration remain exactly the immediate-old suffix, while all selected
    // implementation files retain their own unchanged readset bindings.
    let current = include_str!("../../path_join.rs");
    let before = include_str!("../before_frontier/whole.rs.txt");
    let marker = "fn validate_control_group_with_budget";
    assert_eq!(
        current.split_once(marker).unwrap().1,
        before.split_once(marker).unwrap().1
    );
    for (path, expected) in rows {
        let actual = std::fs::read(root.join(&path)).unwrap();
        assert_eq!(
            digest(&actual),
            expected,
            "immediate-old shared dependency {path}"
        );
    }
    assert!(std::mem::size_of::<super::Incoming<super::PathState>>() >= std::mem::size_of::<std::collections::BTreeSet<Box<super::PathState>>>>());
}
