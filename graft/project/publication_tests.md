# project/publication_tests.rs

- SERIAL · constant · L5-L5 — static SERIAL: AtomicU64 = AtomicU64::new(0);
- fixture · function · L7-L41 — fn fixture(version: u8) -> PathBuf
- expected · function · L43-L61 — fn expected(build: &ProjectNpmBuild) -> Vec<(String, Vec<u8>)>
- owned_profiles_handoff_exact_six_artifacts_and_preserve_callback_errors · function · L64-L107 — fn owned_profiles_handoff_exact_six_artifacts_and_preserve_callback_errors()
- drift_before_handoff_skips_host_and_drift_after_success_is_uncertain · function · L110-L142 — fn drift_before_handoff_skips_host_and_drift_after_success_is_uncertain()
- scalar_project_is_rejected_before_host_handoff · function · L145-L168 — fn scalar_project_is_rejected_before_host_handoff()
- publisher_error_stays_primary_when_callback_also_changes_source · function · L171-L193 — fn publisher_error_stays_primary_when_callback_also_changes_source()
- direct_owned_carriers_reject_before_parent_effects_or_foreign_output_access · function · L197-L220 — fn direct_owned_carriers_reject_before_parent_effects_or_foreign_output_access()
