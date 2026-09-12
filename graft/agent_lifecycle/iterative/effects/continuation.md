# agent_lifecycle/iterative/effects/continuation.rs

- SeededTypedRun · struct · L7-L12 — pub(crate) struct SeededTypedRun
- SeededTypedFailure · struct · L13-L19 — pub(crate) struct SeededTypedFailure
- SeedDriver · struct · L20-L26 — struct SeedDriver<'a>
- before_stage · function · L28-L44 — fn before_stage(
- read · function · L45-L50 — fn read(
- after_transition · function · L51-L59 — fn after_transition(
- run_from_seed · function · L63-L193 — pub(crate) fn run_from_seed(
- tests · module · L197-L363 — mod tests
- Host · struct · L199-L202 — struct Host
- execute · function · L204-L207 — fn execute(&mut self, _: &TypedEffectRequest<'_>) -> Option<Vec<(String, RetainedValue)>>
- budget · function · L209-L216 — fn budget() -> EffectBudget
- task · function · L217-L222 — fn task() -> LifecycleTask
- proposals · function · L223-L225 — fn proposals(compiled: &CompiledTypedEffects) -> Vec<String>
- malformed_and_oversized_migrated_host_work_keep_prior_byte_and_fuel_charges · function · L228-L281 — fn malformed_and_oversized_migrated_host_work_keep_prior_byte_and_fuel_charges()
- fuel_failure_after_host_and_inside_stage_keeps_every_successful_reservation · function · L284-L362 — fn fuel_failure_after_host_and_inside_stage_keeps_every_successful_reservation()
