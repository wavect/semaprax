# public_generic_abi/interpreter/tests.rs

- DESCRIPTOR_FIXTURE · constant · L11-L11 — const DESCRIPTOR_FIXTURE: &[u8] = b"fixture-descriptor-bytes";
- trusted_binding · function · L13-L19 — fn trusted_binding() -> CarrierBindingV1
- open_provider · function · L21-L30 — fn open_provider() -> InterpreterProvider
- open_rejects_a_binding_naming_another_target_profile · function · L33-L47 — fn open_rejects_a_binding_naming_another_target_profile()
- open_rejects_a_descriptor_replay_mismatch · function · L50-L60 — fn open_rejects_a_descriptor_replay_mismatch()
- open_rejects_a_binding_replay_mismatch · function · L63-L78 — fn open_rejects_a_binding_replay_mismatch()
- a_success_round_trip_settles_and_zeroes_every_resource · function · L81-L107 — fn a_success_round_trip_settles_and_zeroes_every_resource()
- zero_length_leaf_round_trips_with_no_null_ambiguity · function · L110-L124 — fn zero_length_leaf_round_trips_with_no_null_ambiguity()
- embedded_zero_bytes_are_preserved_exactly · function · L127-L144 — fn embedded_zero_bytes_are_preserved_exactly()
- leaf_over_max_bytes_is_rejected_before_endpoint_execution · function · L147-L156 — fn leaf_over_max_bytes_is_rejected_before_endpoint_execution()
- leaf_count_over_max_is_rejected_before_endpoint_execution · function · L159-L167 — fn leaf_count_over_max_is_rejected_before_endpoint_execution()
- value_release_before_call_is_a_legal_abandon · function · L170-L185 — fn value_release_before_call_is_a_legal_abandon()
- double_release_of_a_result_handle_is_rejected · function · L188-L204 — fn double_release_of_a_result_handle_is_rejected()
- a_stale_handle_from_a_prior_provider_generation_is_rejected · function · L207-L220 — fn a_stale_handle_from_a_prior_provider_generation_is_rejected()
- every_non_terminal_trace_label_injection_zeroes_every_resource · function · L223-L267 — fn every_non_terminal_trace_label_injection_zeroes_every_resource()
- cleanup_cannot_overwrite_an_earlier_sticky_failure · function · L270-L294 — fn cleanup_cannot_overwrite_an_earlier_sticky_failure()
- repeated_invocation_leaks_no_state_between_independent_calls · function · L297-L323 — fn repeated_invocation_leaks_no_state_between_independent_calls()
- provider_recreation_rejects_stale_child_handles · function · L326-L340 — fn provider_recreation_rejects_stale_child_handles()
