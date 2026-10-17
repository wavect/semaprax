//! Whole frozen/current source binding; no runtime authority.
use sha2::{Digest, Sha256};
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
const PACKET: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/ordinary-hir-prefix-877-v1/source-inverse.json"
));
fn restore(current: &str, row: &serde_json::Value) -> String {
    let acquired = census_checkpoint(row["path"].as_str().unwrap(), current);
    let current = acquired.as_str();
    assert_eq!(digest(current), row["after_sha256"]);
    let mut text = current.to_owned();
    for edit in row["inverse"].as_array().unwrap().iter().rev() {
        let start = edit["start"].as_u64().unwrap() as usize;
        let end = edit["end"].as_u64().unwrap() as usize;
        assert_eq!(&text[start..end], edit["after"].as_str().unwrap());
        text.replace_range(start..end, edit["before"].as_str().unwrap());
    }
    assert_eq!(digest(&text), row["before_sha256"]);
    text
}
#[test]
fn six_complete_predecessors_and_four_created_sources_keep_exact_bytes_and_refuse_drift() {
    assert_eq!(PACKET.len(), 81152);
    assert_eq!(
        digest(PACKET),
        "aae139ddecbb438f145def3e154cbd2b5a4c133f54a4b6565c2de6c81f382a97"
    );
    let packet: serde_json::Value = serde_json::from_str(PACKET).unwrap();
    let inputs = [
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs")),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay.rs.before.txt"))),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work.rs")),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay__skeleton_work.rs.before.txt"))),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history.rs")),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay__ordinary_plan_history.rs.before.txt"))),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/ordinary_dag.rs")),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay__skeleton_work__ordinary_dag.rs.before.txt"))),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/ordinary_dag/frontier.rs")),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay__skeleton_work__ordinary_dag__frontier.rs.before.txt"))),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/prefix_fusion.rs")),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay__skeleton_work__prefix_fusion.rs.before.txt"))),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs")),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay__skeleton_work__enumerated_prefix.rs.before.txt"))),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix/before.rs")),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay__skeleton_work__enumerated_prefix__before.rs.before.txt"))),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs")),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay__skeleton_work__enumerated_prefix__tests.rs.before.txt"))),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/before_hir_compare.rs")),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay__ordinary_plan_history__before_hir_compare.rs.before.txt"))),
    ];
    assert_eq!(packet["files"].as_array().unwrap().len(), inputs.len());
    for ((current, before), row) in inputs.iter().zip(packet["files"].as_array().unwrap()) {
        assert_eq!(restore(current, row), *before);
        for bad in [
            current.to_string() + "\n",
            current.chars().skip(1).collect(),
        ] {
            assert!(std::panic::catch_unwind(|| restore(&bad, row)).is_err());
        }
    }
}

// Physical current -> whole4ce29 precedes the unchanged original ten-row packet.
fn census_checkpoint(source: &str, current: &str) -> String {
    if !matches!(
        source,
        "src/cleanup_plan/replay.rs" | "src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs"
    ) {
        return current.to_owned();
    }
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/ordinary-hir-prefix-877-census-20261017/core-inverse.json"
    ));
    assert_eq!(raw.len(), 717969);
    assert_eq!(
        digest(raw),
        "6273833f330a9b67129e2425c9bb23970af8b49057fb521db8ddcf042a302375"
    );
    let packet: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_eq!(
        packet["checkpoint"],
        "4ce29a5c509e1fcfb7cfbbdb75fb8b0bd0a223e6"
    );
    let rows = packet["files"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows.iter().filter(|row| row["source"] == source).count(), 1);
    let row = rows.iter().find(|row| row["source"] == source).unwrap();
    assert_eq!(current.len() as u64, row["after_length"]);
    assert_eq!(digest(current), row["after_sha256"]);
    assert_eq!(row["inverse"].as_array().unwrap().len(), 1);
    assert_eq!(current, row["inverse"][0]["after"].as_str().unwrap());
    let before = row["inverse"][0]["before"].as_str().unwrap();
    assert_eq!(before.len() as u64, row["before_length"]);
    assert_eq!(digest(before), row["before_sha256"]);
    let archived = match source {
        "src/cleanup_plan/replay.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-census-20261017/before/src/cleanup_plan/replay.rs")),
        "src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-hir-prefix-877-census-20261017/before/src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs")),
        _ => unreachable!(),
    };
    assert_eq!(before, archived);
    before.to_owned()
}
