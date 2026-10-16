//! Compose the exact current-root inverse with the unchanged older inverse.
use sha2::{Digest, Sha256};
const ORIGINAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/single-state-groups-v1/path_join.before.rs.txt"
));
fn raw_hash(source: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}
pub(in crate::cleanup_plan::replay::path_join) fn restore_root() -> String {
    assert_eq!(
        raw_hash(ORIGINAL),
        "e9c7b460d06344a5253e1b3948cc5252f1ad9649b63f2a4b2c1c52851ed3b69a"
    );
    let changes: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/single-state-groups-v1/inverse.json"
    )))
    .unwrap();
    let mut restored = super::super::main_root_input();
    assert_eq!(changes.as_array().unwrap().len(), 3);
    for change in changes.as_array().unwrap().iter().rev() {
        let at = change[0].as_u64().unwrap() as usize;
        let new = change[1].as_str().unwrap();
        let old = change[2].as_str().unwrap();
        assert_eq!(restored.get(at..at + new.len()), Some(new));
        restored.replace_range(at..at + new.len(), old);
    }
    assert_eq!(restored, ORIGINAL);
    restored
}
#[test]
fn whole_previous_producer_and_both_complete_key_constructors_stay_bound() {
    restore_root();
    let source = include_str!("../grouping.rs");
    for (name, original, hash, copies) in [
        (
            "group",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/single-state-groups-v1/group_producer.before.rs.txt"
            )),
            "51c55ece7a60d7f77c3e41a1cdb76b2ae996413fc2eaff8f047f081b319f4c1d",
            1,
        ),
        (
            "key",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/single-state-groups-v1/key_producer.before.rs.txt"
            )),
            "10e0dbe3b664d78898a4cbd7d9de6ef485a81ffe6543a3a8024a80c3f72f883b",
            1,
        ),
    ] {
        assert_eq!(raw_hash(original), hash);
        assert_eq!(source.matches(original).count(), copies, "{name}");
        if name == "key" {
            assert_eq!(original.matches("&state").count(), 3);
            let borrowed = original.replace("&state", "state");
            assert_eq!(source.matches(&borrowed).count(), 1);
        }
    }
}
#[test]
fn the_entire_older_receiver_test_only_adds_the_certified_source_inverse() {
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/single-state-groups-v1/receiver_tests.before.rs.txt"
    ));
    assert_eq!(
        raw_hash(original),
        "ffc6d94bbf586ccfc9ef094e87e8b4934ba59dbc89d2fc11841f56a35c7ffa39"
    );
    let current = include_str!("../receiver_tests.rs");
    let new = "    let mut restored = grouping::source_custody::restore_root();";
    let old = "    let mut restored = include_str!(\"../path_join.rs\").to_owned();";
    assert_eq!(current.matches(new).count(), 1);
    assert_eq!(current.replacen(new, old, 1), original);
}
