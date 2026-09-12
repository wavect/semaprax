# hir/byte_capacity/tests.rs

- NESTED · constant · L16-L40 — const NESTED: &str = r#"
- resolved · function · L42-L45 — fn resolved(source: &str, path: &str) -> ResolvedProgram
- nominal · function · L47-L52 — fn nominal(id: &str) -> ResolvedType
- shape · function · L54-L56 — fn shape(slots: &[ArrayStorageSlot]) -> Vec<(ArrayStorageKind, u32)>
- inline_array_payload_sums_every_nested_fixed_array_field · function · L59-L83 — fn inline_array_payload_sums_every_nested_fixed_array_field()
- inline_array_payload_fails_closed_on_unknown_and_unsubstituted_types · function · L86-L106 — fn inline_array_payload_fails_closed_on_unknown_and_unsubstituted_types()
- only_byte_bearing_slots_are_recorded_and_the_empty_array_still_is · function · L109-L149 — fn only_byte_bearing_slots_are_recorded_and_the_empty_array_still_is()
- capacity_inputs_report_parameters_then_result_then_body_slots · function · L152-L199 — fn capacity_inputs_report_parameters_then_result_then_body_slots()
- an_array_literal_argument_charges_both_staging_and_a_temporary · function · L202-L241 — fn an_array_literal_argument_charges_both_staging_and_a_temporary()
- frame_source · function · L243-L250 — fn frame_source(extra: &str) -> String
- a_frame_of_exactly_the_inline_array_limit_is_admitted · function · L253-L268 — fn a_frame_of_exactly_the_inline_array_limit_is_admitted()
- one_byte_past_the_inline_array_limit_is_rejected · function · L271-L281 — fn one_byte_past_the_inline_array_limit_is_rejected()
- stdout_argument · function · L283-L294 — fn stdout_argument(function: &ResolvedFunction) -> &ResolvedExpr
- transcript_source_is_fixed_for_a_known_array_root_and_unknown_otherwise · function · L297-L345 — fn transcript_source_is_fixed_for_a_known_array_root_and_unknown_otherwise()
- call_targets_resolve_only_through_their_own_template · function · L348-L401 — fn call_targets_resolve_only_through_their_own_template()
