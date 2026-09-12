# process_provider/tests.rs

- request · function · L3-L11 — fn request(stdout_max: usize, stderr_max: usize) -> ProcessRequest
- request_decodes_exact_non_nul_wire · function · L14-L21 — fn request_decodes_exact_non_nul_wire()
- request_refuses_invalid_wire_and_capacities · function · L24-L41 — fn request_refuses_invalid_wire_and_capacities()
- output_wire_round_trips_and_authenticates_limits · function · L44-L63 — fn output_wire_round_trips_and_authenticates_limits()
- fixture_and_budget_are_deterministic_and_non_refunding · function · L66-L86 — fn fixture_and_budget_are_deterministic_and_non_refunding()
- budget_rejects_the_first_over_capacity_without_debiting · function · L89-L101 — fn budget_rejects_the_first_over_capacity_without_debiting()
