# workspace_graph/owned_function_import/tests.rs

- PROVIDER · constant · L2-L13 — const PROVIDER: &str = r#"
- APP · constant · L14-L30 — const APP: &str = r#"
- sources · function · L31-L42 — fn sources(app: &str, provider: &str) -> Vec<WorkspaceSource>
- internal_owned_record_imports_preserve_checked_calls_and_scalar_boundary · function · L44-L93 — fn internal_owned_record_imports_preserve_checked_calls_and_scalar_boundary()
- internal_owned_record_import_requires_direct_types_and_no_generic_widening · function · L95-L112 — fn internal_owned_record_import_requires_direct_types_and_no_generic_widening()
- internal_owned_record_import_rejects_same_shape_wrong_identity · function · L115-L125 — fn internal_owned_record_import_rejects_same_shape_wrong_identity()
- internal_owned_record_import_preserves_contract_failure_and_reentry · function · L128-L150 — fn internal_owned_record_import_preserves_contract_failure_and_reentry()
- borrowed_str_with_owned_byte_record_import_preserves_the_checked_transfer · function · L153-L187 — fn borrowed_str_with_owned_byte_record_import_preserves_the_checked_transfer()
- borrowed_str_does_not_admit_a_non_byte_record_import · function · L190-L214 — fn borrowed_str_does_not_admit_a_non_byte_record_import()
- owned_input_copy_record_result_authenticates_direct_identity_and_cleanup · function · L217-L307 — fn owned_input_copy_record_result_authenticates_direct_identity_and_cleanup()
