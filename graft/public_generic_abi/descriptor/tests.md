# public_generic_abi/descriptor/tests.rs

- sample_input · function · L9-L14 — fn sample_input() -> InstanceBinding
- sample_result · function · L16-L21 — fn sample_result() -> InstanceBinding
- sample · function · L23-L33 — fn sample() -> DescriptorV1
- encode_is_deterministic · function · L36-L39 — fn encode_is_deterministic()
- encode_decode_round_trips · function · L42-L47 — fn encode_decode_round_trips()
- a_display_rename_changes_wire_bytes_but_not_identity · function · L50-L69 — fn a_display_rename_changes_wire_bytes_but_not_identity()
- replay_accepts_an_identical_candidate · function · L72-L76 — fn replay_accepts_an_identical_candidate()
- decode_rejects_truncated_bytes · function · L79-L84 — fn decode_rejects_truncated_bytes()
- decode_rejects_trailing_bytes · function · L87-L93 — fn decode_rejects_trailing_bytes()
- decode_rejects_an_unknown_schema_literal · function · L96-L107 — fn decode_rejects_an_unknown_schema_literal()
- decode_rejects_an_oversized_length_claim · function · L110-L117 — fn decode_rejects_an_oversized_length_claim()
- decode_rejects_bytes_reordered_from_the_canonical_field_order · function · L120-L134 — fn decode_rejects_bytes_reordered_from_the_canonical_field_order()
- decode_rejects_a_total_size_over_the_wire_bound · function · L137-L141 — fn decode_rejects_a_total_size_over_the_wire_bound()
- replay_rejects_a_cross_paired_descriptor_on_every_bound_field · function · L144-L188 — fn replay_rejects_a_cross_paired_descriptor_on_every_bound_field()
- Mutator · type · L145-L145 — type Mutator = fn(DescriptorV1) -> DescriptorV1;
- replay_rejects_a_stale_boundary_profile_or_type_grammar_version · function · L191-L204 — fn replay_rejects_a_stale_boundary_profile_or_type_grammar_version()
- instance_binding_reuses_the_grammar_facts_rather_than_reinventing_them · function · L207-L226 — fn instance_binding_reuses_the_grammar_facts_rather_than_reinventing_them()
