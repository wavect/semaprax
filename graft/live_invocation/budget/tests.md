# live_invocation/budget/tests.rs

- request · function · L5-L15 — fn request(effective_budget: i64) -> ModelInvocationRequest
- usage · function · L17-L24 — fn usage(response_bytes: usize, failed: bool) -> InvocationUsage
- a_zero_amount_request_is_admitted_and_commits_nothing · function · L29-L36 — fn a_zero_amount_request_is_admitted_and_commits_nothing()
- a_request_at_exactly_the_remaining_ceiling_is_admitted_and_exhausts_it · function · L39-L48 — fn a_request_at_exactly_the_remaining_ceiling_is_admitted_and_exhausts_it()
- a_request_one_unit_past_the_remaining_ceiling_is_refused_and_commits_nothing · function · L51-L62 — fn a_request_one_unit_past_the_remaining_ceiling_is_refused_and_commits_nothing()
- two_reservations_that_individually_fit_but_together_overrun_the_ceiling · function · L65-L78 — fn two_reservations_that_individually_fit_but_together_overrun_the_ceiling()
- a_negative_request_is_refused_before_any_commit · function · L81-L87 — fn a_negative_request_is_refused_before_any_commit()
- an_overflowing_request_refuses_rather_than_panics · function · L90-L100 — fn an_overflowing_request_refuses_rather_than_panics()
- record_never_reduces_committed_even_when_actual_usage_is_far_smaller · function · L105-L116 — fn record_never_reduces_committed_even_when_actual_usage_is_far_smaller()
- record_never_reduces_committed_on_a_failed_attempt_either · function · L119-L131 — fn record_never_reduces_committed_on_a_failed_attempt_either()
- a_request_before_the_deadline_is_admitted · function · L136-L141 — fn a_request_before_the_deadline_is_admitted()
- a_request_exactly_at_the_deadline_is_refused_as_deadline_exceeded_not_budget_exhausted · function · L144-L158 — fn a_request_exactly_at_the_deadline_is_refused_as_deadline_exceeded_not_budget_exhausted()
- a_deadline_refusal_and_a_budget_refusal_are_distinguishable_by_reason_text · function · L161-L175 — fn a_deadline_refusal_and_a_budget_refusal_are_distinguishable_by_reason_text()
- resuming_after_a_simulated_crash_never_refunds_the_already_committed_reservation · function · L180-L225 — fn resuming_after_a_simulated_crash_never_refunds_the_already_committed_reservation()
- resume_sums_every_prior_reservation_across_multiple_completed_turns · function · L228-L253 — fn resume_sums_every_prior_reservation_across_multiple_completed_turns()
- resume_preserves_an_absolute_deadline_across_the_same_simulated_crash · function · L256-L274 — fn resume_preserves_an_absolute_deadline_across_the_same_simulated_crash()
