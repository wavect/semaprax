# codegen/native_resource/tests.rs

- resolve · function · L7-L10 — fn resolve(source: &str) -> ResolvedProgram
- source · function · L12-L42 — fn source(resource_name: &str, interface_name: &str, import_name: &str) -> String
- display_renames_do_not_change_the_resource_abi · function · L45-L54 — fn display_renames_do_not_change_the_resource_abi()
- distinct_resource_ids_produce_distinct_wrapper_types · function · L57-L61 — fn distinct_resource_ids_produce_distinct_wrapper_types()
- generated_identifiers_fit_the_portable_internal_identifier_budget · function · L64-L85 — fn generated_identifiers_fit_the_portable_internal_identifier_budget()
- zero_payload_is_never_emitted_as_a_liveness_test · function · L88-L94 — fn zero_payload_is_never_emitted_as_a_liveness_test()
- emission_is_deterministic · function · L97-L129 — fn emission_is_deterministic()
- type_selection_rejects_unknown_record_and_generic_shapes · function · L132-L170 — fn type_selection_rejects_unknown_record_and_generic_shapes()
- imported_and_trivial_lifecycles_have_distinct_descriptors · function · L173-L195 — fn imported_and_trivial_lifecycles_have_distinct_descriptors()
- identifier_registration_rejects_collisions · function · L198-L214 — fn identifier_registration_rejects_collisions()
