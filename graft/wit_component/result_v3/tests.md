# wit_component/result_v3/tests.rs

- SOURCE · constant · L6-L18 — const SOURCE: &str = r#"module test.component_result_v3;
- artifact · function · L20-L23 — fn artifact() -> PrivateResultComponentArtifactV3
- deterministic_result_component_is_exactly_parsed_and_upstream_valid · function · L26-L85 — fn deterministic_result_component_is_exactly_parsed_and_upstream_valid()
- every_component_byte_prefix_trailing_and_noncanonical_length_reject · function · L88-L129 — fn every_component_byte_prefix_trailing_and_noncanonical_length_reject()
- v1_v2_v3_profiles_are_not_confused · function · L132-L155 — fn v1_v2_v3_profiles_are_not_confused()
- rehashed_signature_type_order_index_and_lift_hostiles_reject · function · L158-L199 — fn rehashed_signature_type_order_index_and_lift_hostiles_reject()
- extract_first_core · function · L201-L205 — fn extract_first_core(candidate: &[u8]) -> &[u8]
- node_executes_status_out_with_poison_preservation_and_exact_statuses · function · L208-L268 — fn node_executes_status_out_with_poison_preservation_and_exact_statuses()
- node_specialized_arithmetic_paths_return_typed_status_without_trapping · function · L271-L354 — fn node_specialized_arithmetic_paths_return_typed_status_without_trapping()
- excluded_profiles_fail_closed_without_changing_the_public_backend_gate · function · L357-L389 — fn excluded_profiles_fail_closed_without_changing_the_public_backend_gate()
