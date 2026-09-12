# project/candidate/signature/tests.rs

- program · function · L6-L8 — fn program(source: &str) -> Program
- outcome · function · L10-L17 — fn outcome(program: &Program) -> ResolvedEvaluationOutcome
- evolve · function · L19-L26 — fn evolve(programs: &mut [Program], parameters: Value) -> Result<super::super::IntentSummary>
- implicit_string_retention_guard_agrees_with_real_checked_parameter_ownership · function · L29-L68 — fn implicit_string_retention_guard_agrees_with_real_checked_parameter_ownership()
- reordered_copy_parameters_preserve_value_and_stage_all_original_arguments · function · L71-L87 — fn reordered_copy_parameters_preserve_value_and_stage_all_original_arguments()
- borrowed_views_reorder_and_rename_without_copying_or_escaping_the_loan · function · L90-L112 — fn borrowed_views_reorder_and_rename_without_copying_or_escaping_the_loan()
- dropped_argument_and_reordered_arguments_preserve_first_checked_failure · function · L115-L139 — fn dropped_argument_and_reordered_arguments_preserve_first_checked_failure()
- generated_staging_names_do_not_capture_existing_parameters_or_local_bindings · function · L142-L160 — fn generated_staging_names_do_not_capture_existing_parameters_or_local_bindings()
- import_alias_and_declared_effect_calls_keep_original_staging_order · function · L163-L186 — fn import_alias_and_declared_effect_calls_keep_original_staging_order()
- removal_of_used_parameter_fails_real_verifier_and_type_guesses_reject · function · L189-L211 — fn removal_of_used_parameter_fails_real_verifier_and_type_guesses_reject()
- simultaneous_parameter_renames_preserve_contracts_and_avoid_local_capture · function · L214-L241 — fn simultaneous_parameter_renames_preserve_contracts_and_avoid_local_capture()
- local_initializer_and_assignment_follow_their_original_binding · function · L244-L265 — fn local_initializer_and_assignment_follow_their_original_binding()
- renaming_to_a_removed_parameter_name_cannot_capture_a_live_reference · function · L268-L279 — fn renaming_to_a_removed_parameter_name_cannot_capture_a_live_reference()
- owning_borrowed_and_shared_signature_omissions_reject_before_mutation · function · L282-L311 — fn owning_borrowed_and_shared_signature_omissions_reject_before_mutation()
