# codegen/native_emit/environment_io.rs

- emit · function · L6-L37 — pub fn emit(program: &ResolvedProgram, command_id: &str) -> Result<String, Diagnostic>
- check_permits · function · L39-L53 — pub(crate) fn check_permits(permits: &[String]) -> Result<(), Diagnostic>
- ADMITTED · constant · L40-L46 — const ADMITTED: &[&str] = &[
- emit_runtime · function · L55-L62 — pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram)
- emit_runner · function · L64-L148 — pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str)
