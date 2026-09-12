# impact/tests.rs

- NEXT_FIXTURE · constant · L6-L6 — static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);
- SOURCE · constant · L8-L11 — const SOURCE: &str = r#"module impact.final_check;
- fixture · function · L13-L30 — fn fixture(label: &str) -> (std::path::PathBuf, std::path::PathBuf, String)
- canonical_equivalent_source_byte_drift_is_rejected_at_final_check · function · L33-L48 — fn canonical_equivalent_source_byte_drift_is_rejected_at_final_check()
- same_bytes_with_replaced_identity_are_rejected_at_final_check · function · L51-L69 — fn same_bytes_with_replaced_identity_are_rejected_at_final_check()
- patch_path_mutation_after_one_read_does_not_change_processed_digest · function · L72-L93 — fn patch_path_mutation_after_one_read_does_not_change_processed_digest()
- exhausted_complete_node_budget_stops_before_wide_frontier_materialization · function · L96-L140 — fn exhausted_complete_node_budget_stops_before_wide_frontier_materialization()
- tiny_complete_byte_budget_stops_large_operation_and_change_serialization · function · L143-L166 — fn tiny_complete_byte_budget_stops_large_operation_and_change_serialization()
