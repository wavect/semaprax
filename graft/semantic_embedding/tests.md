# semantic_embedding/tests.rs

- request · function · L11-L18 — fn request() -> EmbeddingRequest
- NOT_CANCELLED · constant · L20-L20 — const NOT_CANCELLED: &dyn Fn() -> bool = &|| false;
- CANCELLED · constant · L21-L21 — const CANCELLED: &dyn Fn() -> bool = &|| true;
- cancelled_before_dispatch_never_reaches_the_provider · function · L24-L36 — fn cancelled_before_dispatch_never_reaches_the_provider()
- oversized_input_never_reaches_the_provider · function · L39-L53 — fn oversized_input_never_reaches_the_provider()
- a_settled_vector_of_the_wrong_length_is_reported_malformed_not_passed_through · function · L56-L70 — fn a_settled_vector_of_the_wrong_length_is_reported_malformed_not_passed_through()
- a_non_finite_component_is_reported_malformed_not_passed_through · function · L73-L88 — fn a_non_finite_component_is_reported_malformed_not_passed_through()
- an_infinite_component_is_reported_malformed_not_passed_through · function · L91-L106 — fn an_infinite_component_is_reported_malformed_not_passed_through()
- a_correctly_shaped_finite_vector_passes_through_unchanged · function · L109-L117 — fn a_correctly_shaped_finite_vector_passes_through_unchanged()
- a_provider_failure_passes_through_unchanged · function · L120-L134 — fn a_provider_failure_passes_through_unchanged()
- the_fixture_provider_is_byte_identical_across_independent_calls · function · L137-L159 — fn the_fixture_provider_is_byte_identical_across_independent_calls()
- distinct_input_bytes_produce_distinct_vectors · function · L162-L179 — fn distinct_input_bytes_produce_distinct_vectors()
- the_capability_carries_the_exact_reason_it_was_granted_with · function · L182-L185 — fn the_capability_carries_the_exact_reason_it_was_granted_with()
