use super::*;

#[test]
fn live_builder_evidence_preserves_message_and_names_only_the_known_phase() {
    let (_, overflowed, used, refusal) = crate::bounded_output::with_limit_usage_refusal(5, || {
        assert!(crate::bounded_output::reserve_active_required(2));
        assert!(!crate::bounded_output::reserve_active_required(4));
    });
    assert!(overflowed);
    assert_eq!(used, 2);
    let diagnostic = live_builder_refusal(5, refusal);
    assert_eq!(diagnostic.code, "SPX-G171");
    assert_eq!(
        diagnostic.message,
        "Workspace Semantic Graph `builder_bytes` exceeds 5"
    );
    assert!(diagnostic.path.is_none());
    assert!(diagnostic.span.is_none());
    let help = diagnostic.help.unwrap();
    assert!(help.contains("resolved-core phase first sticky ledger refusal: requested 4 bytes with 3 remaining and 0 reserved floor."));
    assert!(help
        .contains("cumulative reservation bytes, not a retained-memory forecast or process RSS"));
    assert!(help.contains("exact inner operation is unknown"));
    assert!(help.len() < 4096);
    let unknown = live_builder_refusal(5, None).help.unwrap();
    assert!(unknown.contains("first reservation and exact inner operation are unknown"));
}
