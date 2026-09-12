# codegen/native_command/tests.rs

- NEXT_ID · constant · L8-L8 — static NEXT_ID: AtomicU64 = AtomicU64::new(0);
- SOURCE · constant · L10-L40 — const SOURCE: &str = r#"
- resolved · function · L42-L47 — fn resolved(source: &str) -> hir::ResolvedProgram
- generated · function · L49-L51 — fn generated(source: &str) -> String
- compile · function · L53-L89 — fn compile(source: &str, optimization: &str) -> Option<PathBuf>
- run · function · L91-L101 — fn run(executable: &Path, needle: &std::ffi::OsStr, input: &[u8]) -> Output
- shared_plan_rejects_wrong_signature_selection_contract_and_authority · function · L104-L138 — fn shared_plan_rejects_wrong_signature_selection_contract_and_authority()
- projection_has_one_fixed_process_entry_and_no_legacy_failure_path · function · L141-L167 — fn projection_has_one_fixed_process_entry_and_no_legacy_failure_path()
- unix_o0_o2_process_adapter_seals_output_and_enforces_exact_boundaries · function · L170-L218 — fn unix_o0_o2_process_adapter_seals_output_and_enforces_exact_boundaries()
- unix_rejects_non_utf8_needle_before_semantic_execution · function · L222-L240 — fn unix_rejects_non_utf8_needle_before_semantic_execution()
- semantic_failure_after_write_discards_staged_transcript · function · L243-L256 — fn semantic_failure_after_write_discards_staged_transcript()
- semantic_false_after_write_publishes_no_transcript_and_exits_one · function · L259-L271 — fn semantic_false_after_write_publishes_no_transcript_and_exits_one()
