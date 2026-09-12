# package_build/tests.rs

- NEXT_FIXTURE · constant · L9-L9 — static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
- Fixture · struct · L11-L16 — struct Fixture
- fixture · function · L18-L60 — fn fixture() -> Fixture
- temporary_source_path · function · L62-L68 — fn temporary_source_path() -> PathBuf
- generation_is_exact_and_independently_replayable · function · L71-L106 — fn generation_is_exact_and_independently_replayable()
- artifact_and_evidence_mutations_fail_exact_replay · function · L109-L151 — fn artifact_and_evidence_mutations_fail_exact_replay()
- duplicate_wire_keys_are_rejected_before_replay · function · L154-L181 — fn duplicate_wire_keys_are_rejected_before_replay()
- structurally_valid_outer_digest_drift_is_replay_not_wire_failure · function · L184-L209 — fn structurally_valid_outer_digest_drift_is_replay_not_wire_failure()
- public_selection_and_authority_options_fail_closed · function · L212-L238 — fn public_selection_and_authority_options_fail_closed()
- nested_diagnostics_map_to_the_closed_package_build_family · function · L241-L263 — fn nested_diagnostics_map_to_the_closed_package_build_family()
