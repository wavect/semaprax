# codegen/native_emit/http_io.rs

- ADMITTED_PERMITS · constant · L21-L27 — const ADMITTED_PERMITS: [&str; 5] = [
- emit_c_with_https_io · function · L29-L32 — pub fn emit_c_with_https_io(program: &Program, command_id: &str) -> Result<String, Diagnostic>
- emit_hir_c_with_https_io · function · L34-L82 — pub fn emit_hir_c_with_https_io(
- emit_runtime · function · L84-L93 — pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram)
- emit_constants · function · L95-L123 — fn emit_constants(output: &mut impl COutput)
- emit_mozilla_roots · function · L125-L131 — fn emit_mozilla_roots(output: &mut impl COutput)
- HTTPS_RUNTIME_C · constant · L133-L543 — const HTTPS_RUNTIME_C: &str = r#"
- emit_runner · function · L545-L621 — pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str)
- tests · module · L624-L836 — mod tests
- SERIAL · constant · L632-L632 — static SERIAL: AtomicU64 = AtomicU64::new(0);
- Fixture · struct · L634-L634 — struct Fixture(PathBuf);
- drop · function · L637-L639 — fn drop(&mut self)
- source_for · function · L642-L672 — fn source_for(url: &str) -> String
- c_string · function · L674-L678 — fn c_string(path: &Path) -> String
- embedded_mozilla_root_bundle_is_pinned · function · L681-L697 — fn embedded_mozilla_root_bundle_is_pinned()
- generated_c11_https_executes_verified_tls_over_loopback · function · L700-L791 — fn generated_c11_https_executes_verified_tls_over_loopback()
- generated_c11_https_accepts_a_public_mozilla_root · function · L797-L835 — fn generated_c11_https_accepts_a_public_mozilla_root()
