//! Exact durable prefixes at failed continued Observe cleanup boundaries.

pub(super) fn assert_cleanup_prewrite_snapshot(before: &[u8], after: &[u8], fault: u8) {
    assert!(
        after.starts_with(before),
        "prewrite fault changed durable history"
    );
    let suffix = &after[before.len()..];
    assert!(suffix.is_empty() || suffix.ends_with(b"\n"));
    let expected: &[&str] = match fault {
        21 => &[],
        22 => &["owned_cleanup_started"],
        23 => &["owned_cleanup_started", "owned_cleanup_settled"],
        _ => panic!("unknown failed Observe cleanup fault"),
    };
    let rows = suffix
        .split(|byte| *byte == b'\n')
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        expected.len(),
        "wrong durable cleanup row count"
    );
    for (row, kind) in rows.iter().zip(expected) {
        let value: serde_json::Value = serde_json::from_slice(row).expect("durable cleanup row");
        assert_eq!(value["kind"], *kind);
        assert_eq!(value["owner"], "state");
        assert_eq!(value["turn"], 2);
        assert!(value["attempt"].is_null());
    }
}
