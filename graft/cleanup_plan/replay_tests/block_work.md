# cleanup_plan/replay_tests/block_work.rs

- source · function · L4-L12 — fn source(statements: usize, literal: &str) -> String
- checked · function · L14-L19 — fn checked(statements: usize, literal: &str) -> crate::ast::Program
- metered_body · function · L21-L39 — fn metered_body(statements: usize, literal: &str) -> (ResolvedProgram, ResolvedExpr)
- measure · function · L41-L62 — fn measure(
- successful_path · function · L64-L69 — fn successful_path(paths: Vec<ExprSkeletonPath>)
- flat_literal_block_meter_charges_before_each_materialization · function · L72-L99 — fn flat_literal_block_meter_charges_before_each_materialization()
- flat_literal_block_census_covers_actual_metered_work · function · L102-L121 — fn flat_literal_block_census_covers_actual_metered_work()
- shallow_wide_scalar_and_string_bindings_resolve_without_budget_underestimation · function · L124-L141 — fn shallow_wide_scalar_and_string_bindings_resolve_without_budget_underestimation()
- wide_constructor_source · function · L143-L169 — fn wide_constructor_source(variant: bool, fields: usize) -> String
- wide_record_and_variant_constructor_census_covers_real_replay_work · function · L172-L198 — fn wide_record_and_variant_constructor_census_covers_real_replay_work()
- cleanup_inert_lazy_boolean_decisions_do_not_enumerate_outcome_products · function · L201-L218 — fn cleanup_inert_lazy_boolean_decisions_do_not_enumerate_outcome_products()
- long_status_only_statement_sequences_use_bounded_summary_replay · function · L221-L237 — fn long_status_only_statement_sequences_use_bounded_summary_replay()
- wide_cleanup_inert_match_uses_bounded_decision_summary · function · L240-L260 — fn wide_cleanup_inert_match_uses_bounded_decision_summary()
- valid_resource_bindings_and_block_results_include_transfer_work · function · L263-L319 — fn valid_resource_bindings_and_block_results_include_transfer_work()
