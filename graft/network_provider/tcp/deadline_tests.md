# network_provider/tcp/deadline_tests.rs

- FAST · constant · L22-L22 — const FAST: Duration = Duration::from_secs(5);
- loopback · function · L24-L26 — fn loopback(port: u16) -> SocketAddr
- WitnessListener · struct · L29-L33 — struct WitnessListener
- bind · function · L36-L52 — fn bind() -> Self
- saw_a_connection · function · L54-L56 — fn saw_a_connection(&self) -> bool
- a_caller_selected_budget_is_clamped_and_reported · function · L60-L74 — fn a_caller_selected_budget_is_clamped_and_reported()
- slow_name_resolution_consumes_the_whole_connect_budget · function · L77-L109 — fn slow_name_resolution_consumes_the_whole_connect_budget()
- several_failing_addresses_cannot_multiply_the_connection_budget · function · L112-L146 — fn several_failing_addresses_cannot_multiply_the_connection_budget()
- a_literal_address_still_connects_under_a_short_budget · function · L149-L171 — fn a_literal_address_still_connects_under_a_short_budget()
- a_silent_peer_bounds_a_read_by_the_operation_deadline · function · L174-L201 — fn a_silent_peer_bounds_a_read_by_the_operation_deadline()
- a_wait_never_outlasts_the_operation_deadline · function · L204-L229 — fn a_wait_never_outlasts_the_operation_deadline()
- a_peer_that_never_reads_bounds_a_partial_write_in_aggregate · function · L232-L287 — fn a_peer_that_never_reads_bounds_a_partial_write_in_aggregate()
- a_bounded_accept_stops_waiting_at_the_deadline · function · L290-L322 — fn a_bounded_accept_stops_waiting_at_the_deadline()
