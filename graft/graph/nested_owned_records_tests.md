# graph/nested_owned_records_tests.rs

- DESTRUCTURE_SOURCE · constant · L7-L15 — const DESTRUCTURE_SOURCE: &str = r#"
- destructure_program · function · L17-L23 — fn destructure_program(source: &str) -> crate::hir::ResolvedProgram
- nested_cleanup_versions_are_closed_and_legacy_selection_is_unchanged · function · L26-L85 — fn nested_cleanup_versions_are_closed_and_legacy_selection_is_unchanged()
- v28_and_v29_are_selected_only_for_exact_nested_destructure_compositions · function · L88-L123 — fn v28_and_v29_are_selected_only_for_exact_nested_destructure_compositions()
- one_valid_nested_loan_cannot_mask_an_invalid_sibling_loan · function · L126-L245 — fn one_valid_nested_loan_cannot_mask_an_invalid_sibling_loan()
- native_v25_cannot_mask_v28_or_v29 · function · L248-L264 — fn native_v25_cannot_mask_v28_or_v29()
- nested_update_selects_v30_or_universally_authenticated_v31 · function · L267-L308 — fn nested_update_selects_v30_or_universally_authenticated_v31()
- iterator_v10_does_not_bypass_nested_update_loan_authentication · function · L311-L369 — fn iterator_v10_does_not_bypass_nested_update_loan_authentication()
- iterator_prelude_presence_keeps_legacy_parts_graph_schema · function · L372-L388 — fn iterator_prelude_presence_keeps_legacy_parts_graph_schema()
- local_done_constructor_selects_v7_and_v38_without_iterator_operations · function · L391-L428 — fn local_done_constructor_selects_v7_and_v38_without_iterator_operations()
- generic_iterator_helpers_retain_instances_and_v10_cleanup · function · L431-L493 — fn generic_iterator_helpers_retain_instances_and_v10_cleanup()
