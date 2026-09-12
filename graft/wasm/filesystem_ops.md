# wasm/filesystem_ops.rs

- IMPORT_COUNT · constant · L13-L13 — pub(super) const IMPORT_COUNT: u32 = 2;
- IMPORT_BASE · constant · L14-L15 — pub(super) const IMPORT_BASE: u32 =
- READ_IMPORT · constant · L16-L16 — pub(super) const READ_IMPORT: u32 = IMPORT_BASE;
- WRITE_NEW_IMPORT · constant · L17-L17 — pub(super) const WRITE_NEW_IMPORT: u32 = IMPORT_BASE + 1;
- STATUS_GLOBAL · constant · L18-L18 — pub(super) const STATUS_GLOBAL: u32 = 15;
- OPERATION_COUNT_GLOBAL · constant · L19-L19 — pub(super) const OPERATION_COUNT_GLOBAL: u32 = 16;
- BYTE_COUNT_GLOBAL · constant · L20-L20 — pub(super) const BYTE_COUNT_GLOBAL: u32 = 17;
- STATUS_EXPORT · constant · L21-L21 — pub(super) const STATUS_EXPORT: &str = "__spx_filesystem_status_v1";
- IMPORT_NAMES · constant · L22-L23 — pub(super) const IMPORT_NAMES: [&str; IMPORT_COUNT as usize] =
- emit_resolved_filesystem_ops_v1 · function · L25-L31 — pub fn emit_resolved_filesystem_ops_v1(
- prepare · function · L33-L42 — pub(super) fn prepare(
- check_permits · function · L44-L62 — pub(super) fn check_permits(permits: &[String]) -> Result<(), Diagnostic>
- ADMITTED · constant · L45-L48 — const ADMITTED: [&str; 2] = [
- intern_import_types · function · L64-L86 — pub(super) fn intern_import_types(
- emit_imports · function · L88-L92 — pub(super) fn emit_imports(imports: &mut Vec<u8>, types: &[u32; IMPORT_COUNT as usize])
- append_export · function · L94-L98 — pub(super) fn append_export(exports: &mut Vec<u8>)
- emit_reset · function · L100-L107 — pub(super) fn emit_reset(body: &mut Vec<u8>)
- append_globals · function · L109-L113 — pub(super) fn append_globals(globals: &mut Vec<u8>)
