# loan_plan/boundary_tests.rs

- fixture · function · L18-L40 — fn fixture(loan_count: usize) -> (ResolvedProgram, usize)
- expression_id · function · L42-L44 — fn expression_id(function: &ResolvedFunction, path: &str) -> ExpressionId
- bool_leaf · function · L46-L54 — fn bool_leaf(function: &ResolvedFunction, path: &str) -> ResolvedExpr
- branch_root · function · L56-L68 — fn branch_root(function: &ResolvedFunction, path: &str) -> ResolvedExpr
- three_arm_match_root · function · L70-L98 — fn three_arm_match_root(function: &ResolvedFunction, path: &str) -> ResolvedExpr
- add_padding · function · L100-L119 — fn add_padding(
- cfg_counts · function · L121-L125 — fn cfg_counts(function: &ResolvedFunction) -> (usize, usize)
- install_plan · function · L127-L137 — fn install_plan(
- exact_4096_program_points_rebuild_and_first_representable_overflow_fail_closed · function · L140-L169 — fn exact_4096_program_points_rebuild_and_first_representable_overflow_fail_closed()
- exact_4096_cfg_edges_rebuild_and_edge_4097_fails_before_point_capacity · function · L172-L213 — fn exact_4096_cfg_edges_rebuild_and_edge_4097_fails_before_point_capacity()
- exact_million_work_build_replays_and_the_first_extra_unit_is_fail_closed · function · L216-L284 — fn exact_million_work_build_replays_and_the_first_extra_unit_is_fail_closed()
