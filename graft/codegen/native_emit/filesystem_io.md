# codegen/native_emit/filesystem_io.rs

- ADMITTED_PERMITS · constant · L20-L20 — const ADMITTED_PERMITS: [&str; 2] = [ops::READ_EFFECT, ops::WRITE_EFFECT];
- emit_c_with_filesystem_io · function · L23-L29 — pub fn emit_c_with_filesystem_io(
- emit_hir_c_with_filesystem_io · function · L32-L84 — pub fn emit_hir_c_with_filesystem_io(
- emit_runtime · function · L90-L96 — pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram)
- emit_constants · function · L98-L127 — fn emit_constants(output: &mut impl COutput)
- emit_runner · function · L131-L137 — pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str)
- FILESYSTEM_RUNTIME_C · constant · L139-L334 — const FILESYSTEM_RUNTIME_C: &str = r#"
- FILESYSTEM_RUNNER_C · constant · L336-L378 — const FILESYSTEM_RUNNER_C: &str = r#"
