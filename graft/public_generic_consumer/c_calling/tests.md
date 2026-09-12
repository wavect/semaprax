# public_generic_consumer/c_calling/tests.rs

- DESCRIPTOR_BYTES · constant · L4-L4 — const DESCRIPTOR_BYTES: &[u8] = b"fixture-public-generic-descriptor-bytes-issue-158";
- binding · function · L6-L17 — fn binding() -> NativeProviderBindingV1
- shapes · function · L19-L29 — fn shapes() -> (RecordShape, RecordShape)
- generate · function · L31-L35 — fn generate() -> CallingConsumer
- regeneration_is_byte_identical · function · L38-L42 — fn regeneration_is_byte_identical()
- emits_the_expected_file_set_in_a_stable_order · function · L45-L61 — fn emits_the_expected_file_set_in_a_stable_order()
- every_file_is_lf_only_and_ends_with_a_trailing_newline · function · L64-L72 — fn every_file_is_lf_only_and_ends_with_a_trailing_newline()
- shipped_header_is_byte_identical_to_the_frozen_native_abi_header · function · L75-L83 — fn shipped_header_is_byte_identical_to_the_frozen_native_abi_header()
- consumer_header_names_no_native_abi_type_and_only_two_includes · function · L90-L116 — fn consumer_header_names_no_native_abi_type_and_only_two_includes()
- only_the_source_file_includes_the_native_abi_header · function · L122-L138 — fn only_the_source_file_includes_the_native_abi_header()
- field_names_are_derived_from_identity_bytes_not_display_text · function · L141-L149 — fn field_names_are_derived_from_identity_bytes_not_display_text()
- duplicate_field_identity_in_one_record_is_rejected · function · L152-L167 — fn duplicate_field_identity_in_one_record_is_rejected()
- mismatched_leaf_counts_are_rejected · function · L170-L182 — fn mismatched_leaf_counts_are_rejected()
- embeds_the_exact_trusted_descriptor_and_binding_bytes · function · L185-L207 — fn embeds_the_exact_trusted_descriptor_and_binding_bytes()
- no_host_path_or_checkout_specific_text_survives_generation · function · L210-L221 — fn no_host_path_or_checkout_specific_text_survives_generation()
- field_count_and_field_lists_scale_with_the_shape · function · L224-L247 — fn field_count_and_field_lists_scale_with_the_shape()
- empty_trusted_bytes_render_a_valid_c_array_literal · function · L250-L260 — fn empty_trusted_bytes_render_a_valid_c_array_literal()
