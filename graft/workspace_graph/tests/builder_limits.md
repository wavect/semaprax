# workspace_graph/tests/builder_limits.rs

- minimum_successful_builder_limit · function · L3-L16 — fn minimum_successful_builder_limit(sources: &[WorkspaceSource]) -> usize
- private_builder_limit_cannot_widen_the_production_cap · function · L22-L24 — fn private_builder_limit_cannot_widen_the_production_cap()
- assert_exact_builder_limit_error · function · L26-L32 — fn assert_exact_builder_limit_error(error: &[Diagnostic], limit: usize)
- all_four_generic_materializations_have_an_exact_minimum_builder_limit · function · L35-L66 — fn all_four_generic_materializations_have_an_exact_minimum_builder_limit()
- stub_charge_provider · function · L68-L78 — fn stub_charge_provider(statements: usize) -> WorkspaceSource
- stub_charge_prebound · function · L80-L90 — fn stub_charge_prebound(sources: &[WorkspaceSource], module: &str) -> usize
- imported_function_bodies_are_not_charged_as_resolved_structure · function · L99-L121 — fn imported_function_bodies_are_not_charged_as_resolved_structure()
- late_module_work_has_an_exact_combined_minimum_builder_limit · function · L124-L168 — fn late_module_work_has_an_exact_combined_minimum_builder_limit()
- identity_scale_workspace · function · L170-L193 — fn identity_scale_workspace(
- identity_length_does_not_dominate_the_builder_pre_bound · function · L203-L212 — fn identity_length_does_not_dominate_the_builder_pre_bound()
- a_twenty_one_kilobyte_workspace_fits_the_production_builder_budget · function · L222-L230 — fn a_twenty_one_kilobyte_workspace_fits_the_production_builder_budget()
- the_builder_pre_bound_still_refuses_an_oversized_workspace · function · L236-L243 — fn the_builder_pre_bound_still_refuses_an_oversized_workspace()
- traversal_source · function · L245-L262 — fn traversal_source(binder: &str, item: &str) -> WorkspaceSource
- a_for_item_binding_is_charged_exactly_like_a_let_binding · function · L272-L284 — fn a_for_item_binding_is_charged_exactly_like_a_let_binding()
- core_retry_fixture · function · L289-L313 — fn core_retry_fixture() -> Vec<WorkspaceSource>
- identity_prebound_production_core_retry_preserves_phase_debit_and_nested_refusal · function · L316-L357 — fn identity_prebound_production_core_retry_preserves_phase_debit_and_nested_refusal()
