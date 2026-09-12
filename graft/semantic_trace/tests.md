# semantic_trace/tests.rs

- SOURCE · constant · L12-L45 — const SOURCE: &str = r#"module test.semantic_dictionary;
- program · function · L47-L50 — fn program() -> ResolvedProgram
- function · function · L52-L58 — fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction
- reference_trace_round_trips_only_through_emitted_ordinals · function · L61-L149 — fn reference_trace_round_trips_only_through_emitted_ordinals()
- failure_and_checked_statuses_are_dictionary_bound · function · L152-L190 — fn failure_and_checked_statuses_are_dictionary_bound()
- owned_transfer_selection_and_failed_postcondition_round_trip_exactly · function · L193-L259 — fn owned_transfer_selection_and_failed_postcondition_round_trip_exactly()
- unknown_function_and_imported_lifecycle_fail_closed · function · L262-L290 — fn unknown_function_and_imported_lifecycle_fail_closed()
- typed_result_staging_is_rejected_before_callable_trace_admission · function · L293-L312 — fn typed_result_staging_is_rejected_before_callable_trace_admission()
