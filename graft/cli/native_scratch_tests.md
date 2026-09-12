# cli/native_scratch_tests.rs

- UNSUPPORTED_NATIVE · constant · L11-L17 — const UNSUPPORTED_NATIVE: &str = r#"module scratch.native_import;
- plain · function · L19-L31 — fn plain(path: &Path, directory: bool)
- write_new · function · L33-L41 — fn write_new(path: &Path, bytes: &[u8])
- rejected_source_preserves_the_former_predictable_run_path · function · L44-L117 — fn rejected_source_preserves_the_former_predictable_run_path()
- SENTINEL · constant · L47-L47 — const SENTINEL: &[u8] = b"foreign legacy run-path sentinel\n";
