# codegen/native_emit/scratch_tests.rs

- status · function · L9-L20 — fn status(success: bool) -> ExitStatus
- Outcome · enum · L23-L28 — enum Outcome
- exercise · function · L30-L140 — fn exercise(outcome: Outcome)
- SOURCE · constant · L31-L31 — const SOURCE: &str = "int main(void) { return 0; }\n";
- synthetic_compiler_success_cleans_only_its_source_scratch · function · L143-L145 — fn synthetic_compiler_success_cleans_only_its_source_scratch()
- synthetic_compiler_failure_retains_source_and_primary_diagnostic · function · L148-L150 — fn synthetic_compiler_failure_retains_source_and_primary_diagnostic()
- synthetic_compiler_io_error_retains_source_and_primary_diagnostic · function · L153-L155 — fn synthetic_compiler_io_error_retains_source_and_primary_diagnostic()
- synthetic_compiler_success_with_foreign_inventory_preserves_success_and_files · function · L158-L160 — fn synthetic_compiler_success_with_foreign_inventory_preserves_success_and_files()
