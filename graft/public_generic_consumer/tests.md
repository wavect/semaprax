# public_generic_consumer/tests.rs

- SOURCE · constant · L12-L49 — const SOURCE: &str = r#"
- surface · function · L51-L59 — fn surface() -> CandidateSurface
- metadata · function · L61-L63 — fn metadata() -> ConsumerMetadata
- metadata_carries_every_record_kind_in_canonical_order · function · L68-L94 — fn metadata_carries_every_record_kind_in_canonical_order()
- canonical_metadata_round_trips_and_is_accepted · function · L99-L109 — fn canonical_metadata_round_trips_and_is_accepted()
- hostile_metadata_fails_closed_with_a_closed_reason · function · L113-L181 — fn hostile_metadata_fails_closed_with_a_closed_reason()
- reordered_records_are_refused · function · L186-L197 — fn reordered_records_are_refused()
- the_refusal_vocabulary_is_closed_and_reachable · function · L201-L224 — fn the_refusal_vocabulary_is_closed_and_reachable()
- generation_is_deterministic_and_identity_derived · function · L229-L277 — fn generation_is_deterministic_and_identity_derived()
- nested_declarations_precede_the_instances_that_hold_them · function · L282-L299 — fn nested_declarations_precede_the_instances_that_hold_them()
- generated_consumers_embed_accepted_metadata · function · L305-L321 — fn generated_consumers_embed_accepted_metadata()
- template_normalization_makes_generation_host_independent · function · L328-L343 — fn template_normalization_makes_generation_host_independent()
- metadata_bounds_refuse · function · L347-L358 — fn metadata_bounds_refuse()
