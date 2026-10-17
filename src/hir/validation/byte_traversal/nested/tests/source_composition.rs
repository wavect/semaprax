//! Filesystem custody belongs to the test-only source-reader layer.
pub(super) fn incoming_current(raw: &str) -> Option<String> {
    crate::source_reflows::nested_source_incoming_current(raw)
}
#[test]
fn joined_nested_source_keeps_full_live_union_and_both_complete_parent_custodies() {
    crate::source_reflows::nested_source_keeps_full_live_union_and_both_complete_parent_custodies();
}
#[test]
fn joined_nested_source_refuses_foreign_truncated_drifted_predicates_and_partial_recipes() {
    crate::source_reflows::nested_source_refuses_foreign_truncated_drifted_predicates_and_partial_recipes();
}
