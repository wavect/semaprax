# workspace_patch_evidence/tests.rs

- NEXT_FIXTURE · constant · L5-L5 — static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
- Fixture · struct · L7-L12 — struct Fixture
- new · function · L15-L69 — fn new() -> Self
- active · function · L71-L73 — fn active(&self) -> std::path::PathBuf
- revision · function · L75-L80 — fn revision(&self) -> String
- generation_names · function · L82-L84 — fn generation_names(&self) -> Vec<String>
- staging_names · function · L86-L88 — fn staging_names(&self) -> Vec<String>
- directory_names · function · L91-L98 — fn directory_names(path: &Path) -> Vec<String>
- drop · function · L101-L103 — fn drop(&mut self)
- child_and_aggregate_node_limit_diagnostics_are_distinct_and_exact · function · L107-L127 — fn child_and_aggregate_node_limit_diagnostics_are_distinct_and_exact()
- aggregate_usage_limit_diagnostics_name_each_exact_wire_field · function · L130-L201 — fn aggregate_usage_limit_diagnostics_name_each_exact_wire_field()
- assert_limit · function · L152-L169 — macro_rules! assert_limit
- structural_depth_limit_is_distinct_from_malformed_json · function · L204-L214 — fn structural_depth_limit_is_distinct_from_malformed_json()
- owned_inputs_and_final_source_recheck_are_route_specific · function · L217-L256 — fn owned_inputs_and_final_source_recheck_are_route_specific()
- apply_owns_evidence_and_rechecks_the_owned_patch_before_pivot · function · L259-L306 — fn apply_owns_evidence_and_rechecks_the_owned_patch_before_pivot()
- replay_boundary_is_no_write_and_shared_pivot_boundaries_are_exact · function · L309-L372 — fn replay_boundary_is_no_write_and_shared_pivot_boundaries_are_exact()
- snapshot_lock_handoff_precedes_immediate_stale_evidence_reapply · function · L375-L404 — fn snapshot_lock_handoff_precedes_immediate_stale_evidence_reapply()
- capsule_and_receipt_self_caps_accept_exact_and_reject_one_less · function · L407-L443 — fn capsule_and_receipt_self_caps_accept_exact_and_reject_one_less()
