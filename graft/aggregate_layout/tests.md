# aggregate_layout/tests.rs

- SOURCE · constant · L12-L72 — const SOURCE: &str = r#"
- program · function · L74-L76 — fn program() -> hir::ResolvedProgram
- replace_array_holder_fields · function · L78-L105 — fn replace_array_holder_fields(
- replace_mono_wrapper_field · function · L107-L117 — fn replace_mono_wrapper_field(program: &mut hir::ResolvedProgram, ty: ResolvedType)
- native_and_wasm_layouts_have_frozen_offsets_and_digests · function · L120-L184 — fn native_and_wasm_layouts_have_frozen_offsets_and_digests()
- fixed_byte_arrays_have_target_independent_size_n_alignment_one · function · L187-L206 — fn fixed_byte_arrays_have_target_independent_size_n_alignment_one()
- direct_owned_bytes_layout_matches_existing_carriers_without_becoming_copy · function · L209-L292 — fn direct_owned_bytes_layout_matches_existing_carriers_without_becoming_copy()
- concrete_generic_owned_bytes_layout_substitutes_before_target_layout · function · L295-L347 — fn concrete_generic_owned_bytes_layout_substitutes_before_target_layout()
- nested_concrete_generic_layouts_substitute_recursively_and_remain_distinct · function · L350-L409 — fn nested_concrete_generic_layouts_substitute_recursively_and_remain_distinct()
- nested_concrete_generic_layout_admission_is_bounded_and_record_only · function · L412-L482 — fn nested_concrete_generic_layout_admission_is_bounded_and_record_only()
- monomorphic_wrapper_authenticates_one_complete_nested_generic_budget · function · L485-L533 — fn monomorphic_wrapper_authenticates_one_complete_nested_generic_budget()
- exact_reconstruction_rejects_reorder_overlap_undersize_and_alignment_mutations · function · L536-L560 — fn exact_reconstruction_rejects_reorder_overlap_undersize_and_alignment_mutations()
- unknown_duplicate_recursive_and_imported_resource_inputs_fail_closed · function · L563-L622 — fn unknown_duplicate_recursive_and_imported_resource_inputs_fail_closed()
- field_lookup_uses_stable_identity · function · L625-L640 — fn field_lookup_uses_stable_identity()
- generic_instances_bind_cache_digest_and_field_substitution_even_when_layouts_match · function · L643-L718 — fn generic_instances_bind_cache_digest_and_field_substitution_even_when_layouts_match()
