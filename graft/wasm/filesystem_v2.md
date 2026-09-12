# wasm/filesystem_v2.rs

- IMPORT_COUNT · constant · L4-L4 — pub(super) const IMPORT_COUNT: u32 = 5;
- BASE · constant · L5-L6 — pub(super) const BASE: u32 =
- STAT · constant · L7-L7 — pub(super) const STAT: u32 = BASE;
- LIST · constant · L8-L8 — pub(super) const LIST: u32 = BASE + 1;
- CREATE_DIR · constant · L9-L9 — pub(super) const CREATE_DIR: u32 = BASE + 2;
- REMOVE · constant · L10-L10 — pub(super) const REMOVE: u32 = BASE + 3;
- WRITE_ATOMIC · constant · L11-L11 — pub(super) const WRITE_ATOMIC: u32 = BASE + 4;
- NAMES · constant · L12-L18 — const NAMES: [&str; 5] = [
- emit_resolved_filesystem_ops_v2 · function · L19-L29 — pub fn emit_resolved_filesystem_ops_v2(
- intern_import_types · function · L30-L44 — pub(super) fn intern_import_types(
- emit_imports · function · L45-L49 — pub(super) fn emit_imports(imports: &mut Vec<u8>, types: &[u32; 5])
- append_export · function · L50-L54 — pub(super) fn append_export(exports: &mut Vec<u8>)
- needs_list_scan · function · L55-L65 — pub(super) fn needs_list_scan(program: &ResolvedProgram) -> bool
