# freestanding_object/tests.rs

- COUNTER · constant · L5-L5 — static COUNTER: AtomicUsize = AtomicUsize::new(0);
- write_temp · function · L7-L15 — fn write_temp(source: &str) -> PathBuf
- cleanup · function · L17-L19 — fn cleanup(path: &Path)
- VALID_SOURCE · constant · L21-L38 — const VALID_SOURCE: &str = r#"
- options_reject_out_of_bounds_values · function · L41-L45 — fn options_reject_out_of_bounds_values()
- symbols_match_the_native_hex_encoding · function · L48-L50 — fn symbols_match_the_native_hex_encoding()
- assertion_checks_catch_planted_violations · function · L53-L60 — fn assertion_checks_catch_planted_violations()
- golden_unit_has_documented_shape_and_is_deterministic · function · L63-L76 — fn golden_unit_has_documented_shape_and_is_deterministic()
- envelope_round_trips_through_verify_envelope · function · L79-L88 — fn envelope_round_trips_through_verify_envelope()
- verify_envelope_detects_tampering · function · L91-L100 — fn verify_envelope_detects_tampering()
- module_outside_the_scalar_profile_fails_closed · function · L103-L181 — fn module_outside_the_scalar_profile_fails_closed()
- private_functions_are_excluded_by_identity_origin · function · L184-L197 — fn private_functions_are_excluded_by_identity_origin()
- byte_budget_exhaustion_fails_closed_without_truncation · function · L200-L209 — fn byte_budget_exhaustion_fails_closed_without_truncation()
