# agent_lifecycle/iterative/tests.rs

- source · function · L4-L39 — pub(super) fn source(terminal: &str) -> String
- compile · function · L41-L49 — fn compile(terminal: &str) -> CompiledIterativeLifecycle
- proposals · function · L50-L52 — fn proposals(compiled: &CompiledIterativeLifecycle) -> Vec<String>
- task · function · L53-L58 — fn task() -> LifecycleTask
- three_turns_execute_checked_steps_and_distinct_fresh_authorizations · function · L61-L106 — fn three_turns_execute_checked_steps_and_distinct_fresh_authorizations()
- suspend_and_failure_are_reducer_selected_terminal_values · function · L109-L121 — fn suspend_and_failure_are_reducer_selected_terminal_values()
- iteration_and_stage_limits_stop_before_unbudgeted_effects · function · L124-L144 — fn iteration_and_stage_limits_stop_before_unbudgeted_effects()
- wrong_step_shape_is_rejected_before_any_runner_exists · function · L147-L167 — fn wrong_step_shape_is_rejected_before_any_runner_exists()
- cancellation_during_effect_prevents_reduction_and_later_effects · function · L170-L214 — fn cancellation_during_effect_prevents_reduction_and_later_effects()
- CancelRead · struct · L171-L174 — struct CancelRead
- read · function · L176-L180 — fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>>
- evidence_binds_all_invocation_inputs_even_before_first_stage · function · L217-L286 — fn evidence_binds_all_invocation_inputs_even_before_first_stage()
