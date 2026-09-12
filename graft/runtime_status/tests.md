# runtime_status/tests.rs

- arena · function · L7-L9 — fn arena(nonce: u64, capacity: u32) -> StatusArena
- zero_is_success_and_never_resolves_to_a_record · function · L12-L20 — fn zero_is_success_and_never_resolves_to_a_record()
- records_receive_immutable_one_based_tokens_in_insertion_order · function · L23-L39 — fn records_receive_immutable_one_based_tokens_in_insertion_order()
- equal_records_still_receive_distinct_stable_tokens · function · L42-L48 — fn equal_records_still_receive_distinct_stable_tokens()
- exhaustion_is_a_non_mutating_harness_error · function · L51-L62 — fn exhaustion_is_a_non_mutating_harness_error()
- zero_capacity_fails_without_creating_a_language_status · function · L65-L72 — fn zero_capacity_fails_without_creating_a_language_status()
- scoped_tokens_cannot_cross_contexts_even_when_raw_indices_match · function · L75-L97 — fn scoped_tokens_cannot_cross_contexts_even_when_raw_indices_match()
- same_context_nonce_does_not_alias_distinct_arenas · function · L100-L118 — fn same_context_nonce_does_not_alias_distinct_arenas()
- unknown_nonzero_tokens_are_rejected · function · L121-L128 — fn unknown_nonzero_tokens_are_rejected()
- arithmetic_normalization_uses_the_exact_v1_table · function · L131-L150 — fn arithmetic_normalization_uses_the_exact_v1_table()
- contract_normalization_uses_phase_codes_not_ordinals · function · L153-L164 — fn contract_normalization_uses_phase_codes_not_ordinals()
- arbitrary_normalized_records_round_trip_without_semantic_rewriting · function · L167-L178 — fn arbitrary_normalized_records_round_trip_without_semantic_rewriting()
