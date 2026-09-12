# public_generic_type/tests.rs

- SOURCE · constant · L12-L54 — const SOURCE: &str = r#"
- program · function · L56-L59 — fn program() -> ResolvedProgram
- nominal · function · L61-L66 — fn nominal(declaration: &str, arguments: Vec<ResolvedType>) -> ResolvedType
- pair · function · L68-L70 — fn pair(arguments: Vec<ResolvedType>) -> ResolvedType
- a_flat_instance_renders_one_exact_target_neutral_term · function · L75-L99 — fn a_flat_instance_renders_one_exact_target_neutral_term()
- a_zero_arity_record_renders_empty_ordered_arguments · function · L104-L115 — fn a_zero_arity_record_renders_empty_ordered_arguments()
- nested_instances_substitute_before_descending_and_order_owned_leaves · function · L120-L152 — fn nested_instances_substitute_before_descending_and_order_owned_leaves()
- display_renames_do_not_change_any_identity · function · L158-L197 — fn display_renames_do_not_change_any_identity()
- argument_order_and_content_are_part_of_the_instance_identity · function · L203-L251 — fn argument_order_and_content_are_part_of_the_instance_identity()
- every_closed_rejection_reason_is_reachable · function · L257-L330 — fn every_closed_rejection_reason_is_reachable()
- identities_containing_grammar_punctuation_round_trip · function · L335-L363 — fn identities_containing_grammar_punctuation_round_trip()
- distinct_identities_never_render_alike · function · L368-L382 — fn distinct_identities_never_render_alike()
- malformed_terms_fail_closed · function · L387-L421 — fn malformed_terms_fail_closed()
- grammar_bounds_refuse_rather_than_truncate · function · L425-L450 — fn grammar_bounds_refuse_rather_than_truncate()
- replay_requires_byte_equality_with_the_recomputed_term · function · L455-L499 — fn replay_requires_byte_equality_with_the_recomputed_term()
- description_is_deterministic · function · L504-L514 — fn description_is_deterministic()
- digest_domains_are_separated · function · L519-L537 — fn digest_domains_are_separated()
- template_identities_bind_owner_and_position · function · L542-L576 — fn template_identities_bind_owner_and_position()
