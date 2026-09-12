# public_generic_consumer/typescript_calling/tests.rs

- DESCRIPTOR_BYTES · constant · L5-L5 — const DESCRIPTOR_BYTES: &[u8] = b"fixture-public-generic-descriptor-bytes-issue-157";
- binding · function · L7-L18 — fn binding() -> WasmProviderBindingV1
- shapes · function · L20-L30 — fn shapes() -> (RecordShape, RecordShape)
- generate · function · L32-L36 — fn generate() -> CallingConsumer
- regeneration_is_byte_identical · function · L39-L43 — fn regeneration_is_byte_identical()
- emits_the_expected_file_set_in_a_stable_order · function · L46-L68 — fn emits_the_expected_file_set_in_a_stable_order()
- every_file_is_lf_only_and_ends_with_a_trailing_newline · function · L71-L79 — fn every_file_is_lf_only_and_ends_with_a_trailing_newline()
- no_any_appears_in_the_generated_typescript_public_surface · function · L82-L99 — fn no_any_appears_in_the_generated_typescript_public_surface()
- duplicate_field_identity_in_one_record_is_rejected · function · L102-L117 — fn duplicate_field_identity_in_one_record_is_rejected()
- mismatched_leaf_counts_are_rejected · function · L120-L132 — fn mismatched_leaf_counts_are_rejected()
- embeds_the_exact_trusted_descriptor_and_binding_bytes · function · L135-L158 — fn embeds_the_exact_trusted_descriptor_and_binding_bytes()
- no_host_path_or_checkout_specific_text_survives_generation · function · L161-L172 — fn no_host_path_or_checkout_specific_text_survives_generation()
- field_count_and_field_lists_scale_with_the_shape · function · L175-L199 — fn field_count_and_field_lists_scale_with_the_shape()
- field_names_are_derived_from_identity_bytes_not_display_text · function · L202-L210 — fn field_names_are_derived_from_identity_bytes_not_display_text()
- package_json_and_lockfile_pin_the_same_typescript_version · function · L213-L227 — fn package_json_and_lockfile_pin_the_same_typescript_version()
- generated_package_declares_no_runtime_dependency · function · L230-L241 — fn generated_package_declares_no_runtime_dependency()
