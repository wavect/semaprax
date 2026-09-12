# package_compatibility/tests.rs

- report_from_source · function · L8-L22 — fn report_from_source(tag: &str, source: &str) -> String
- NEXT · constant · L9-L9 — static NEXT: AtomicU64 = AtomicU64::new(0);
- input_for · function · L24-L50 — fn input_for(report: &str, version: &str, capabilities: &[&str]) -> CompatibilityInput
- input_with_dependency · function · L52-L91 — fn input_with_dependency(
- outcome_precedence_and_option_boundaries_are_closed · function · L94-L111 — fn outcome_precedence_and_option_boundaries_are_closed()
- nested_primitive_name_change_is_breaking_not_scrubbed · function · L114-L132 — fn nested_primitive_name_change_is_breaking_not_scrubbed()
- exact_selected_subjects_replay_to_compatible_evidence · function · L135-L173 — fn exact_selected_subjects_replay_to_compatible_evidence()
- contract_only_nominal_type_is_in_shared_reachable_closure · function · L176-L193 — fn contract_only_nominal_type_is_in_shared_reachable_closure()
- type_display_scrub_keeps_primitive_semantics · function · L196-L206 — fn type_display_scrub_keeps_primitive_semantics()
- selected_report_mismatch_and_context_drift_are_fail_closed · function · L209-L235 — fn selected_report_mismatch_and_context_drift_are_fail_closed()
- evidence_outer_remint_cannot_forge_outcome · function · L238-L261 — fn evidence_outer_remint_cannot_forge_outcome()
- aggregate_dependency_unproven_is_publicly_indeterminate_and_subject_order_is_canonical · function · L264-L296 — fn aggregate_dependency_unproven_is_publicly_indeterminate_and_subject_order_is_canonical()
- contract_calls_and_imported_resources_are_publicly_indeterminate · function · L299-L366 — fn contract_calls_and_imported_resources_are_publicly_indeterminate()
- finding_and_public_output_bounds_fail_with_stable_limit_diagnostic · function · L369-L422 — fn finding_and_public_output_bounds_fail_with_stable_limit_diagnostic()
