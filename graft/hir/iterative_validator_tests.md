# hir/iterative_validator_tests.rs

- hostile_declaration_index_rejects_reserved_host_id_for_every_authored_kind · function · L8-L56 — fn hostile_declaration_index_rejects_reserved_host_id_for_every_authored_kind()
- iterative_resolver_matches_recursive_reference_outside_builder_accounting · function · L59-L215 — fn iterative_resolver_matches_recursive_reference_outside_builder_accounting()
- SOURCE · constant · L217-L289 — const SOURCE: &str = r#"
- program · function · L291-L293 — fn program() -> ResolvedProgram
- function_index · function · L295-L301 — fn function_index(program: &ResolvedProgram, id: &str) -> usize
- tail_mut · function · L303-L308 — fn tail_mut(function: &mut ResolvedFunction) -> &mut ResolvedExpr
- validation_scope · function · L310-L328 — fn validation_scope(function: &ResolvedFunction) -> BTreeMap<ValueId, ValidationBinding>
- validate_expression_hostile · function · L330-L380 — fn validate_expression_hostile(
- validator_oracle_preserves_direct_child_scope_on_late_errors · function · L383-L408 — fn validator_oracle_preserves_direct_child_scope_on_late_errors()
- validator_oracle_suppresses_failed_block_branch_lazy_and_match_child_scopes · function · L411-L465 — fn validator_oracle_suppresses_failed_block_branch_lazy_and_match_child_scopes()
- validator_oracle_handles_an_exact_depth_512_late_error_with_a_nonempty_scope · function · L468-L526 — fn validator_oracle_handles_an_exact_depth_512_late_error_with_a_nonempty_scope()
- run · function · L469-L517 — fn run()
- UNARY_NODES · constant · L470-L470 — const UNARY_NODES: usize = 510;
