# codegen/native_emit/network_io.rs

- ADMITTED_PERMITS · constant · L26-L34 — const ADMITTED_PERMITS: [&str; 7] = [
- emit_c_with_network_io · function · L38-L41 — pub fn emit_c_with_network_io(program: &Program, command_id: &str) -> Result<String, Diagnostic>
- emit_hir_c_with_network_io · function · L49-L104 — pub fn emit_hir_c_with_network_io(
- emit_feature_macros · function · L109-L115 — pub(super) fn emit_feature_macros(output: &mut impl COutput)
- emit_runtime · function · L123-L131 — pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram)
- emit_constants · function · L135-L182 — fn emit_constants(output: &mut impl COutput)
- NETWORK_RUNTIME_C · constant · L189-L1008 — const NETWORK_RUNTIME_C: &str = r#"
- emit_runner · function · L1013-L1111 — pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str)
