# project/rename/tests.rs

- SERIAL · constant · L7-L7 — static SERIAL: AtomicU64 = AtomicU64::new(0);
- Fixture · struct · L9-L9 — struct Fixture(PathBuf);
- drop · function · L12-L14 — fn drop(&mut self)
- fixture · function · L17-L34 — fn fixture() -> Fixture
- inventory · function · L36-L53 — fn inventory(root: &Path) -> BTreeMap<String, Vec<u8>>
- visit · function · L37-L49 — fn visit(root: &Path, directory: &Path, result: &mut BTreeMap<String, Vec<u8>>)
- assert_rename_error · function · L55-L60 — fn assert_rename_error(result: Result<PreparedProjectRename, Vec<Diagnostic>>, text: &str)
- stable_export_plan_is_deterministic_digest_bound_and_read_only · function · L63-L131 — fn stable_export_plan_is_deterministic_digest_bound_and_read_only()
- wrong_from_non_export_collision_and_invalid_complete_candidate_fail_closed · function · L134-L154 — fn wrong_from_non_export_collision_and_invalid_complete_candidate_fail_closed()
- newer_project_profiles_fail_before_v1_rename_evidence_is_constructed · function · L157-L165 — fn newer_project_profiles_fail_before_v1_rename_evidence_is_constructed()
- automatic_function_identity_is_rejected_before_planning · function · L168-L181 — fn automatic_function_identity_is_rejected_before_planning()
- sealed_plan_acquires_a0_for_validated_imported_module_without_main · function · L184-L216 — fn sealed_plan_acquires_a0_for_validated_imported_module_without_main()
