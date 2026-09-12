# agent_lifecycle/iterative/effects/durable/tests.rs

- Store · struct · L4-L8 — struct Store
- commit · function · L10-L19 — fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError>
- Handler · struct · L22-L25 — struct Handler
- execute · function · L27-L37 — fn execute(&mut self, _: &TypedEffectRequest<'_>) -> Option<Vec<(String, RetainedValue)>>
- task · function · L39-L44 — fn task() -> LifecycleTask
- proposals · function · L45-L47 — fn proposals(compiled: &CompiledTypedEffects) -> Vec<String>
- budget · function · L48-L55 — fn budget() -> EffectBudget
- root · function · L56-L58 — fn root() -> String
- run · function · L59-L78 — fn run(
- durable_three_turn_run_and_completed_replay_do_not_repeat_host_work · function · L80-L102 — fn durable_three_turn_run_and_completed_replay_do_not_repeat_host_work()
- lost_ack_intent_is_uncertain_but_observed_and_transitions_replay_once · function · L104-L138 — fn lost_ack_intent_is_uncertain_but_observed_and_transitions_replay_once()
- retained_failure_replays_without_handler_and_wrong_root_rejects_before_store · function · L140-L173 — fn retained_failure_replays_without_handler_and_wrong_root_rejects_before_store()
- reducer_and_recovery_fuel_reservations_cannot_be_refunded · function · L175-L211 — fn reducer_and_recovery_fuel_reservations_cannot_be_refunded()
