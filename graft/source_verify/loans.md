# source_verify/loans.rs

- local_borrow_origin · function · L13-L70 — pub(super) fn local_borrow_origin(
- activate_local_loan · function · L72-L79 — pub(super) fn activate_local_loan(variables: &mut HashMap<String, Binding>, origin: &BorrowOrigin)
- activate_match_loan · function · L81-L96 — pub(super) fn activate_match_loan(
- has_active_overlapping_loan · function · L98-L102 — pub(super) fn has_active_overlapping_loan(binding: &Binding, projections: &[String]) -> bool
- expression_uses_name · function · L104-L189 — pub(super) fn expression_uses_name(expression: &Expr, name: &str) -> bool
- release_dead_local_loans · function · L191-L225 — pub(super) fn release_dead_local_loans(
- mark_value_sources_moved · function · L227-L364 — pub(super) fn mark_value_sources_moved(
- Frame · enum · L234-L248 — enum Frame<'a>
- merge_moved · function · L366-L380 — pub(super) fn merge_moved(
- join_conditional · function · L382-L400 — pub(super) fn join_conditional(
