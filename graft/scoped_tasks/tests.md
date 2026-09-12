# scoped_tasks/tests.rs

- succeed · function · L3-L11 — fn succeed(id: &str, scope: &str) -> TaskSpec
- drain · function · L13-L19 — fn drain(run: &mut ScopedTaskRun<'_>) -> Vec<TaskEvent>
- constructor_rejects_every_structural_ambiguity · function · L22-L170 — fn constructor_rejects_every_structural_ambiguity()
- joins_must_cover_each_direct_child_exactly_once · function · L173-L208 — fn joins_must_cover_each_direct_child_exactly_once()
- dependencies_must_stay_inside_the_scope_lineage · function · L211-L239 — fn dependencies_must_stay_inside_the_scope_lineage()
- physical_failure_code_zero_is_never_admitted · function · L242-L258 — fn physical_failure_code_zero_is_never_admitted()
- bounds_and_work_budget_fail_closed · function · L261-L302 — fn bounds_and_work_budget_fail_closed()
- canonical_json_is_valid_and_order_insensitive · function · L305-L335 — fn canonical_json_is_valid_and_order_insensitive()
- run_hostile_operations_reject_or_drain_idempotently · function · L338-L365 — fn run_hostile_operations_reject_or_drain_idempotently()
