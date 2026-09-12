# wasm/process_io.rs

- IMPORT_COUNT · constant · L4-L4 — pub(super) const IMPORT_COUNT: u32 = 2;
- RUN_IMPORT · constant · L5-L6 — pub(super) const RUN_IMPORT: u32 =
- SETTLE_IMPORT · constant · L7-L7 — pub(super) const SETTLE_IMPORT: u32 = RUN_IMPORT + 1;
- STATUS_GLOBAL · constant · L8-L8 — pub(super) const STATUS_GLOBAL: u32 = 17;
- RUN_COUNT_GLOBAL · constant · L9-L9 — pub(super) const RUN_COUNT_GLOBAL: u32 = 18;
- BYTE_COUNT_GLOBAL · constant · L10-L10 — pub(super) const BYTE_COUNT_GLOBAL: u32 = 19;
- emit_resolved_process_io_v1 · function · L11-L21 — pub(crate) fn emit_resolved_process_io_v1(
- check_permits · function · L22-L40 — pub(super) fn check_permits(permits: &[String]) -> Result<(), Diagnostic>
- ALLOWED · constant · L23-L30 — const ALLOWED: [&str; 6] = [
- intern_import_types · function · L41-L65 — pub(super) fn intern_import_types(
- emit_imports · function · L66-L73 — pub(super) fn emit_imports(imports: &mut Vec<u8>, types: &[u32; 2])
- append_globals · function · L74-L79 — pub(super) fn append_globals(globals: &mut Vec<u8>)
- append_export · function · L80-L84 — pub(super) fn append_export(exports: &mut Vec<u8>)
- emit_reset · function · L85-L92 — pub(super) fn emit_reset(body: &mut Vec<u8>)
- emit_settle · function · L94-L116 — pub(super) fn emit_settle(body: &mut Vec<u8>, status: u32, scratch: u32)
