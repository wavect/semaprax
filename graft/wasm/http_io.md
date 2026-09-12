# wasm/http_io.rs

- IMPORT_COUNT · constant · L17-L17 — pub(super) const IMPORT_COUNT: u32 = 1;
- IMPORT_BASE · constant · L18-L19 — pub(super) const IMPORT_BASE: u32 =
- GET_IMPORT · constant · L20-L20 — pub(super) const GET_IMPORT: u32 = IMPORT_BASE;
- STATUS_GLOBAL · constant · L23-L23 — pub(super) const STATUS_GLOBAL: u32 = 15;
- STATUS_EXPORT · constant · L24-L24 — pub(super) const STATUS_EXPORT: &str = "__spx_http_status_v1";
- IMPORT_NAME · constant · L25-L25 — pub(super) const IMPORT_NAME: &str = "spx_https_get_v1";
- emit_resolved_https_command_io_v1 · function · L27-L33 — pub(crate) fn emit_resolved_https_command_io_v1(
- prepare · function · L35-L44 — pub(super) fn prepare(
- check_permits · function · L46-L73 — pub(super) fn check_permits(permits: &[String]) -> Result<(), Diagnostic>
- ADMITTED · constant · L47-L53 — const ADMITTED: [&str; 5] = [
- intern_import_type · function · L75-L87 — pub(super) fn intern_import_type(
- emit_import · function · L89-L91 — pub(super) fn emit_import(imports: &mut Vec<u8>, ty: u32)
- append_export · function · L93-L97 — pub(super) fn append_export(exports: &mut Vec<u8>)
- emit_reset · function · L99-L102 — pub(super) fn emit_reset(body: &mut Vec<u8>)
- tests · module · L106-L106 — mod tests;
