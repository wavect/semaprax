# project/prepared_interpreter/tests/replacement.rs

- Candidate · struct · L9-L12 — struct Candidate
- new · function · L15-L64 — fn new(automatic_test_helper: bool) -> Self
- SERIAL · constant · L16-L16 — static SERIAL: AtomicUsize = AtomicUsize::new(0);
- reopen · function · L66-L70 — fn reopen(&self) -> Arc<ProjectRevision>
- cleanup · function · L74-L96 — fn cleanup(self)
- assert_plain · function · L99-L109 — fn assert_plain(path: &Path, directory: bool)
- assert_inventory · function · L111-L118 — fn assert_inventory(directory: &Path, expected: &[&str])
- pair · function · L120-L129 — fn pair(
- assert_exact_pair · function · L131-L139 — fn assert_exact_pair(left: &[PreparedProjectExecution; 2], right: &[PreparedProjectExecution; 2])
- assert_legacy_parity · function · L141-L170 — fn assert_legacy_parity(revision: &ProjectRevision, executions: &[PreparedProjectExecution; 2])
- replacement_switches_both_closures_preserves_old_revision_and_binds_traces · function · L173-L260 — fn replacement_switches_both_closures_preserves_old_revision_and_binds_traces()
- replacement_repeats_same_candidate_and_uses_content_not_epoch_tokens · function · L263-L313 — fn replacement_repeats_same_candidate_and_uses_content_not_epoch_tokens()
- malformed_stale_and_inadmissible_replacements_preserve_both_exact_old_traces · function · L316-L394 — fn malformed_stale_and_inadmissible_replacements_preserve_both_exact_old_traces()
- replacement_preserves_cancellation_trace_saturation_fuel_and_original_ceilings · function · L397-L480 — fn replacement_preserves_cancellation_trace_saturation_fuel_and_original_ceilings()
