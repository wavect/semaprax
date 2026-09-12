# codegen/native_host_contract/tests.rs

- SOURCE · constant · L12-L41 — const SOURCE: &str = r#"module test.native_host_contract;
- program · function · L43-L46 — fn program() -> ResolvedProgram
- function · function · L48-L54 — fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction
- admit · function · L56-L71 — fn admit(
- template_is_deterministic_and_preserves_complete_signature_order · function · L74-L110 — fn template_is_deterministic_and_preserves_complete_signature_order()
- template_ignores_display_and_whitespace_but_tracks_scalar_abi_shape · function · L113-L170 — fn template_ignores_display_and_whitespace_but_tracks_scalar_abi_shape()
- owned_result_maps_to_resource_ordinal_not_signature_index · function · L173-L186 — fn owned_result_maps_to_resource_ordinal_not_signature_index()
- owned_result_selects_the_exact_second_same_type_owner · function · L189-L202 — fn owned_result_selects_the_exact_second_same_type_owner()
- contract_labels_flow_through_requires_and_checked_admission · function · L205-L215 — fn contract_labels_flow_through_requires_and_checked_admission()
- adapter_binding_is_stage_b_authority_and_instances_are_distinct · function · L218-L281 — fn adapter_binding_is_stage_b_authority_and_instances_are_distinct()
- detached_and_mutated_admission_evidence_cannot_be_mixed · function · L284-L327 — fn detached_and_mutated_admission_evidence_cannot_be_mixed()
- detached_function_mutations_fail_closed · function · L330-L358 — fn detached_function_mutations_fail_closed()
- lifecycle_and_abi_mutations_fail_closed · function · L361-L411 — fn lifecycle_and_abi_mutations_fail_closed()
- imported_lifecycle_remains_rejected · function · L414-L437 — fn imported_lifecycle_remains_rejected()
- function_mut · function · L439-L445 — fn function_mut<'a>(program: &'a mut ResolvedProgram, id: &str) -> &'a mut ResolvedFunction
