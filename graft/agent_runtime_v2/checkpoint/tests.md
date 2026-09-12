# agent_runtime_v2/checkpoint/tests.rs

- identity · function · L4-L12 — fn identity() -> CheckpointIdentity
- journal · function · L13-L25 — fn journal() -> OperationCheckpoint
- state · function · L26-L40 — fn state() -> RetainedValue
- context · function · L41-L51 — fn context() -> EffectContext
- Store · struct · L53-L57 — struct Store
- commit · function · L59-L67 — fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError>
- observed · function · L69-L116 — fn observed() -> (OperationCheckpoint, Store)
- canonical_roundtrip_retains_context_and_recovery_phase · function · L118-L128 — fn canonical_roundtrip_retains_context_and_recovery_phase()
- uncertain_intent_and_lost_ack_never_admit_another_intent · function · L130-L163 — fn uncertain_intent_and_lost_ack_never_admit_another_intent()
- duplicate_unknown_fields_changed_binding_and_predecessor_are_rejected · function · L165-L179 — fn duplicate_unknown_fields_changed_binding_and_predecessor_are_rejected()
- replay_and_new_tail_reservations_never_refund_fuel · function · L181-L240 — fn replay_and_new_tail_reservations_never_refund_fuel()
- wrong_state_result_phase_or_terminal_carrier_is_rejected · function · L242-L293 — fn wrong_state_result_phase_or_terminal_carrier_is_rejected()
- remint · function · L295-L313 — fn remint(document: &mut Value) -> String
- reminted_undercharges_and_live_limit_substitution_fail_closed · function · L316-L342 — fn reminted_undercharges_and_live_limit_substitution_fail_closed()
- failed_observation_is_metered_terminal_and_recovery_fuel_cannot_refund · function · L345-L447 — fn failed_observation_is_metered_terminal_and_recovery_fuel_cannot_refund()
- exact_scalar_codec_rejects_nested_duplicate_noncanonical_and_overflow_values · function · L450-L479 — fn exact_scalar_codec_rejects_nested_duplicate_noncanonical_and_overflow_values()
