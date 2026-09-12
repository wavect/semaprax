# source_verify/declaration/functions.rs

- check_function_declarations · function · L28-L442 — pub(super) fn check_function_declarations<'p>(
- check_generic_function_cycles · function · L444-L541 — pub(super) fn check_generic_function_cycles<'p>(
- MAX_STATIC_MAPPING_DEPTH · constant · L543-L543 — const MAX_STATIC_MAPPING_DEPTH: usize = 128;
- static_binding · function · L545-L557 — fn static_binding(ty: Type) -> Binding
- check_omitted_generic_mappings · function · L562-L928 — fn check_omitted_generic_mappings(
- clear_pattern_bindings · function · L930-L947 — fn clear_pattern_bindings(pattern: &MatchPattern, bindings: &mut HashMap<String, Binding>)
- clear_record_pattern_bindings · function · L949-L964 — fn clear_record_pattern_bindings(
- check_function_bodies · function · L966-L1364 — pub(super) fn check_function_bodies<'p>(
- generic_inference_tests · module · L1367-L1495 — mod generic_inference_tests
- parsed · function · L1371-L1373 — fn parsed(source: &str) -> Program
- mapping_diagnostics · function · L1375-L1408 — fn mapping_diagnostics(program: &Program) -> Vec<Diagnostic>
- nested_calls · function · L1410-L1447 — fn nested_calls(count: usize, explicit: bool) -> Program
- omitted_mapping_depth_fails_closed_without_rejecting_explicit_mapping · function · L1450-L1462 — fn omitted_mapping_depth_fails_closed_without_rejecting_explicit_mapping()
- unknown_let_shadow_cannot_reuse_the_outer_parameter_fact · function · L1465-L1478 — fn unknown_let_shadow_cannot_reuse_the_outer_parameter_fact()
- match_binding_shadow_cannot_reuse_the_outer_parameter_fact · function · L1481-L1494 — fn match_binding_shadow_cannot_reuse_the_outer_parameter_fact()
