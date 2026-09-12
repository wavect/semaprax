# hir/function_value/tests.rs

- SOURCE · constant · L2-L8 — const SOURCE: &str = r#"
- resolved · function · L9-L11 — fn resolved() -> ResolvedProgram
- function_values_hir_replays_target_universe_and_exact_invocation · function · L13-L41 — fn function_values_hir_replays_target_universe_and_exact_invocation()
- function_values_hir_rejects_foreign_target_and_forged_invocation_signature · function · L43-L109 — fn function_values_hir_rejects_foreign_target_and_forged_invocation_signature()
- function_values_graph_v36_binds_candidates_and_rejects_legacy_projection · function · L111-L129 — fn function_values_graph_v36_binds_candidates_and_rejects_legacy_projection()
- function_values_target_universe_bound_is_exact · function · L132-L159 — fn function_values_target_universe_bound_is_exact()
- source · function · L133-L146 — fn source(count: usize) -> String
- function_values_unreferenced_eligible_wrapper_is_not_an_invocation_candidate · function · L162-L193 — fn function_values_unreferenced_eligible_wrapper_is_not_an_invocation_candidate()
- function_values_coherent_alternative_graph_cannot_rebind_retained_source · function · L196-L213 — fn function_values_coherent_alternative_graph_cannot_rebind_retained_source()
- function_values_synthetic_invocation_cannot_be_forged_as_an_ordinary_call · function · L216-L236 — fn function_values_synthetic_invocation_cannot_be_forged_as_an_ordinary_call()
- function_values_references_cannot_target_templates_methods_or_imports · function · L239-L293 — fn function_values_references_cannot_target_templates_methods_or_imports()
