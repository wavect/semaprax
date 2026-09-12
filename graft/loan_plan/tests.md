# loan_plan/tests.rs

- FIXTURE · constant · L5-L18 — const FIXTURE: &str = r#"
- fixture · function · L20-L24 — fn fixture() -> ResolvedProgram
- run_mutation · function · L26-L41 — fn run_mutation(name: &str, mut mutate: impl FnMut(&mut LoanPlan, &ResolvedFunction))
- nested_and_lazy_paths_have_edge_qualified_terminations · function · L44-L65 — fn nested_and_lazy_paths_have_edge_qualified_terminations()
- every_attached_plan_surface_is_replayed_exactly · function · L68-L107 — fn every_attached_plan_surface_is_replayed_exactly()
- own_match_payload_loans_are_canonical_and_cannot_be_omitted · function · L110-L164 — fn own_match_payload_loans_are_canonical_and_cannot_be_omitted()
- option_try_residual_edge_terminates_a_normal_path_loan · function · L167-L233 — fn option_try_residual_edge_terminates_a_normal_path_loan()
- loan_limit_accepts_256_and_rejects_257 · function · L236-L258 — fn loan_limit_accepts_256_and_rejects_257()
- source · function · L237-L246 — fn source(count: usize) -> String
- loan_free_function_above_cfg_point_bound_preserves_legacy_admission · function · L261-L288 — fn loan_free_function_above_cfg_point_bound_preserves_legacy_admission()
- terminal_borrowed_contract_call_has_authenticated_completion_edge · function · L291-L348 — fn terminal_borrowed_contract_call_has_authenticated_completion_edge()
