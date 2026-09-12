# hir/inspection/tests.rs

- CALLS · constant · L15-L43 — const CALLS: &str = r#"
- resolved · function · L45-L48 — fn resolved(source: &str, path: &str) -> ResolvedProgram
- site_pairs · function · L50-L55 — fn site_pairs(program: &ResolvedProgram) -> Vec<(String, String)>
- edge_pairs · function · L57-L62 — fn edge_pairs(program: &ResolvedProgram) -> Vec<(String, String)>
- call_edges_cover_requires_body_and_ensures_and_deduplicate · function · L65-L80 — fn call_edges_cover_requires_body_and_ensures_and_deduplicate()
- call_sites_keep_authored_order_and_repeat_one_callee_per_site · function · L83-L99 — fn call_sites_keep_authored_order_and_repeat_one_callee_per_site()
- every_call_site_carries_a_distinct_expression_identity · function · L102-L113 — fn every_call_site_carries_a_distinct_expression_identity()
- call_projections_are_identical_across_repeated_resolutions · function · L116-L123 — fn call_projections_are_identical_across_repeated_resolutions()
- call_sites_include_generic_templates_while_call_edges_do_not · function · L126-L162 — fn call_sites_include_generic_templates_while_call_edges_do_not()
- visiting_calls_reports_the_instance_and_type_arguments_of_a_generic_call · function · L165-L204 — fn visiting_calls_reports_the_instance_and_type_arguments_of_a_generic_call()
- GUARDS · constant · L209-L235 — const GUARDS: &str = r#"
- call_projections_reach_a_callee_called_only_from_a_match_guard · function · L238-L253 — fn call_projections_reach_a_callee_called_only_from_a_match_guard()
- guard_calls_precede_arm_value_calls_in_authored_order · function · L256-L284 — fn guard_calls_precede_arm_value_calls_in_authored_order()
- LIFECYCLE · constant · L286-L311 — const LIFECYCLE: &str = r#"
- lifecycle_effects_come_back_in_canonical_sorted_order · function · L314-L330 — fn lifecycle_effects_come_back_in_canonical_sorted_order()
- lifecycle_effects_are_empty_for_scalars_and_fail_closed_for_unknown_types · function · L333-L348 — fn lifecycle_effects_are_empty_for_scalars_and_fail_closed_for_unknown_types()
- path_prefix_matching_is_directional · function · L351-L360 — fn path_prefix_matching_is_directional()
