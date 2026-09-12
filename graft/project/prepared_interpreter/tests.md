# project/prepared_interpreter/tests.rs

- replacement · module · L13-L13 — mod replacement;
- TRACE_PAYLOAD_DOMAIN · constant · L15-L15 — const TRACE_PAYLOAD_DOMAIN: &[u8] = b"semaprax.project-source-trace.payload.v1\0";
- REAL_PREPARE_SERIAL · constant · L16-L16 — static REAL_PREPARE_SERIAL: Mutex<()> = Mutex::new(());
- real_prepare_serial · function · L18-L22 — pub(super) fn real_prepare_serial() -> MutexGuard<'static, ()>
- revision · function · L24-L30 — pub(super) fn revision() -> Arc<ProjectRevision>
- remint_payload · function · L32-L55 — fn remint_payload(envelope: &str, mutate: impl FnOnce(&str) -> String) -> String
- first_event_range · function · L57-L89 — fn first_event_range(payload: &str) -> Range<usize>
- one_prepared_worker_repeats_exact_entry_and_test_with_replayable_origins · function · L92-L116 — fn one_prepared_worker_repeats_exact_entry_and_test_with_replayable_origins()
- cancellation_is_a_replayable_zero_step_outcome_and_trace_saturation_is_explicit · function · L119-L149 — fn cancellation_is_a_replayable_zero_step_outcome_and_trace_saturation_is_explicit()
- options_and_resigned_origin_mutation_fail_closed · function · L152-L178 — fn options_and_resigned_origin_mutation_fail_closed()
- worker_permit_is_bounded_and_released_exactly_once · function · L181-L198 — fn worker_permit_is_bounded_and_released_exactly_once()
- ACTIVE · constant · L182-L182 — static ACTIVE: AtomicUsize = AtomicUsize::new(0);
- production_worker_bound_is_exact_and_real_workers_release_their_permits · function · L201-L247 — fn production_worker_bound_is_exact_and_real_workers_release_their_permits()
- execution_admission_rejects_concurrent_work_and_reopens_after_release · function · L250-L263 — fn execution_admission_rejects_concurrent_work_and_reopens_after_release()
- duplicate_function_origin_must_be_exact_and_is_counted_once · function · L266-L292 — fn duplicate_function_origin_must_be_exact_and_is_counted_once()
- canonical_remints_cannot_change_structural_phase_or_escape_the_exact_closure · function · L295-L367 — fn canonical_remints_cannot_change_structural_phase_or_escape_the_exact_closure()
- canonical_remints_reject_impossible_cancellation_and_drop_accounting · function · L370-L407 — fn canonical_remints_reject_impossible_cancellation_and_drop_accounting()
- byte_range_language_status_is_in_the_exact_trace_vocabulary · function · L410-L420 — fn byte_range_language_status_is_in_the_exact_trace_vocabulary()
