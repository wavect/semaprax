# project/execution/tests.rs

- PROJECT_REVISION · constant · L3-L4 — const PROJECT_REVISION: &str =
- WORKSPACE_REVISION · constant · L5-L6 — const WORKSPACE_REVISION: &str =
- canonical_return · function · L8-L23 — fn canonical_return(max_bytes: usize) -> String
- rendered_return · function · L25-L49 — fn rendered_return(value: i64) -> serde_json::Value
- returned_i64_extremes_are_lossless_decimal_strings · function · L52-L61 — fn returned_i64_extremes_are_lossless_decimal_strings()
- rendering_is_fail_closed_when_the_bound_cannot_hold_the_envelope · function · L64-L84 — fn rendering_is_fail_closed_when_the_bound_cannot_hold_the_envelope()
- complete_envelope_is_a_frozen_kat_and_independently_verifies · function · L87-L92 — fn complete_envelope_is_a_frozen_kat_and_independently_verifies()
- verifier_rejects_noncanonical_confused_and_mutated_envelopes · function · L95-L134 — fn verifier_rejects_noncanonical_confused_and_mutated_envelopes()
- verifier_reconstructs_the_closed_status_table · function · L137-L184 — fn verifier_reconstructs_the_closed_status_table()
- verifier_rejects_self_consistent_but_impossible_semantic_facts · function · L187-L222 — fn verifier_rejects_self_consistent_but_impossible_semantic_facts()
- rendering_and_verification_honor_the_exact_max_bytes_boundary · function · L225-L284 — fn rendering_and_verification_honor_the_exact_max_bytes_boundary()
- contract_failure_fixture · function · L286-L304 — fn contract_failure_fixture() -> ProjectContractFailure
- test_envelope_with_cases · function · L306-L332 — fn test_envelope_with_cases(
- test_envelopes_carry_cases_and_contract_detail_that_replay_and_verify · function · L335-L408 — fn test_envelopes_carry_cases_and_contract_detail_that_replay_and_verify()
