# codegen/native_cleanup/tests.rs

- SUPPORTED · constant · L9-L31 — const SUPPORTED: &str = r#"module test.native_cleanup_index;
- resolve · function · L33-L36 — fn resolve(source: &str) -> ResolvedProgram
- function · function · L38-L44 — fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction
- projected_borrow_shape_gate_rejects_wrong_operation_and_depth · function · L47-L69 — fn projected_borrow_shape_gate_rejects_wrong_operation_and_depth()
- supported_direct_resource_indexes_preserve_exact_order · function · L72-L170 — fn supported_direct_resource_indexes_preserve_exact_order()
- conditional_and_lazy_control_flow_are_rejected_without_reconstruction · function · L173-L199 — fn conditional_and_lazy_control_flow_are_rejected_without_reconstruction()
- resource_valued_binary_operands_are_rejected_even_in_hostile_hir · function · L202-L229 — fn resource_valued_binary_operands_are_rejected_even_in_hostile_hir()
- initialize_and_cleanup_bearing_continue_are_rejected_by_the_classifier · function · L232-L299 — fn initialize_and_cleanup_bearing_continue_are_rejected_by_the_classifier()
- records_are_rejected_precisely · function · L302-L317 — fn records_are_rejected_precisely()
- imported_lifecycles_are_rejected_precisely · function · L320-L339 — fn imported_lifecycles_are_rejected_precisely()
- resource_bearing_calls_are_rejected_precisely · function · L342-L356 — fn resource_bearing_calls_are_rejected_precisely()
- scalar_calls_from_resource_owning_functions_are_rejected_precisely · function · L359-L373 — fn scalar_calls_from_resource_owning_functions_are_rejected_precisely()
- empty_call_commit_transitions_are_rejected_without_repair · function · L376-L393 — fn empty_call_commit_transitions_are_rejected_without_repair()
- projected_cleanup_places_are_rejected_without_repair · function · L396-L408 — fn projected_cleanup_places_are_rejected_without_repair()
- generic_cleanup_slots_are_rejected_without_repair · function · L411-L422 — fn generic_cleanup_slots_are_rejected_without_repair()
- forged_slot_lifecycle_and_transfer_type_mismatches_are_rejected · function · L425-L468 — fn forged_slot_lifecycle_and_transfer_type_mismatches_are_rejected()
