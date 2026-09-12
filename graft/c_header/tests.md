# c_header/tests.rs

- COUNTER · constant · L5-L5 — static COUNTER: AtomicUsize = AtomicUsize::new(0);
- write_temp · function · L7-L15 — fn write_temp(source: &str) -> PathBuf
- cleanup · function · L17-L19 — fn cleanup(path: &Path)
- VALID_SOURCE · constant · L21-L41 — const VALID_SOURCE: &str = r#"
- double_options · function · L43-L45 — fn double_options() -> CHeaderOptions
- options_reject_out_of_bounds_values · function · L48-L58 — fn options_reject_out_of_bounds_values()
- include_guard_is_deterministic_and_identity_sensitive · function · L61-L71 — fn include_guard_is_deterministic_and_identity_sensitive()
- symbols_match_the_native_hex_encoding · function · L74-L76 — fn symbols_match_the_native_hex_encoding()
- hygiene_rejects_comment_hostile_text · function · L79-L85 — fn hygiene_rejects_comment_hostile_text()
- golden_header_has_expected_shape_and_is_deterministic · function · L88-L106 — fn golden_header_has_expected_shape_and_is_deterministic()
- envelope_round_trips_through_verify_envelope · function · L109-L115 — fn envelope_round_trips_through_verify_envelope()
- verify_envelope_detects_tampering · function · L118-L127 — fn verify_envelope_detects_tampering()
- signature_matches_the_native_projection_line · function · L130-L146 — fn signature_matches_the_native_projection_line()
- selection_errors_fail_closed · function · L149-L160 — fn selection_errors_fail_closed()
- every_exclusion_reason_is_reachable · function · L163-L214 — fn every_exclusion_reason_is_reachable()
- private_functions_are_excluded_by_identity_origin · function · L217-L235 — fn private_functions_are_excluded_by_identity_origin()
- byte_budget_exhaustion_fails_closed_without_truncation · function · L238-L252 — fn byte_budget_exhaustion_fails_closed_without_truncation()
- indexed_byte_loop_scalar_header_is_pinned_and_matches_native · function · L255-L320 — fn indexed_byte_loop_scalar_header_is_pinned_and_matches_native()
