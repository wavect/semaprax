# codegen/native_trace/tests.rs

- SOURCE · constant · L9-L46 — const SOURCE: &str = r#"module test.native_trace_capacity;
- program · function · L48-L51 — fn program() -> ResolvedProgram
- function_mut · function · L53-L59 — fn function_mut<'a>(program: &'a mut ResolvedProgram, id: &str) -> &'a mut ResolvedFunction
- reverse_trivial_finalizers_and_imported_finalizers_have_exact_weights · function · L62-L79 — fn reverse_trivial_finalizers_and_imported_finalizers_have_exact_weights()
- branch_capacity_uses_the_longest_path_instead_of_summing_paths · function · L82-L103 — fn branch_capacity_uses_the_longest_path_instead_of_summing_paths()
- deep_linear_plan_uses_an_explicit_traversal_stack · function · L106-L156 — fn deep_linear_plan_uses_an_explicit_traversal_stack()
- DEPTH · constant · L107-L107 — const DEPTH: u32 = 20_000;
- hostile_missing_edge_and_reachable_cycle_are_rejected · function · L159-L201 — fn hostile_missing_edge_and_reachable_cycle_are_rejected()
- hostile_lifecycle_reference_and_capacity_overflow_are_rejected · function · L204-L219 — fn hostile_lifecycle_reference_and_capacity_overflow_are_rejected()
