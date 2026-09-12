# wasm/environment_io.rs

- text · module · L4-L4 — pub(super) mod text;
- IMPORT_COUNT · constant · L11-L11 — pub(super) const IMPORT_COUNT: u32 = 3;
- IMPORT_BASE · constant · L12-L13 — pub(super) const IMPORT_BASE: u32 =
- LEN_IMPORT · constant · L14-L14 — pub(super) const LEN_IMPORT: u32 = IMPORT_BASE;
- NAME_UTF8_IMPORT · constant · L15-L15 — pub(super) const NAME_UTF8_IMPORT: u32 = IMPORT_BASE + 1;
- VALUE_UTF8_IMPORT · constant · L16-L16 — pub(super) const VALUE_UTF8_IMPORT: u32 = IMPORT_BASE + 2;
- STATUS_GLOBAL · constant · L17-L17 — pub(super) const STATUS_GLOBAL: u32 = 16;
- STATUS_EXPORT · constant · L18-L18 — pub(super) const STATUS_EXPORT: &str = "__spx_environment_status_v1";
- NAMES · constant · L19-L23 — const NAMES: [&str; 3] = [
- emit_resolved_environment_io_v1 · function · L25-L35 — pub(crate) fn emit_resolved_environment_io_v1(
- check_permits · function · L37-L59 — pub(super) fn check_permits(permits: &[String]) -> Result<(), Diagnostic>
- ADMITTED · constant · L38-L44 — const ADMITTED: [&str; 5] = [
- intern_import_types · function · L61-L91 — pub(super) fn intern_import_types(
- emit_imports · function · L93-L97 — pub(super) fn emit_imports(imports: &mut Vec<u8>, types: &[u32; 3])
- append_global · function · L99-L101 — pub(super) fn append_global(globals: &mut Vec<u8>)
- append_export · function · L102-L106 — pub(super) fn append_export(exports: &mut Vec<u8>)
- emit_reset · function · L107-L109 — pub(super) fn emit_reset(body: &mut Vec<u8>)
