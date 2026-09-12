# wasm/aggregate/internal_strings.rs

- LITERAL_IMPORT · constant · L11-L11 — pub(super) const LITERAL_IMPORT: u32 = 0;
- CLONE_IMPORT · constant · L12-L12 — pub(super) const CLONE_IMPORT: u32 = 1;
- EQ_IMPORT · constant · L13-L13 — pub(super) const EQ_IMPORT: u32 = 6;
- IMPORT_COUNT · constant · L14-L14 — const IMPORT_COUNT: u32 = 10;
- RESULT_OFFSET · constant · L15-L15 — const RESULT_OFFSET: u32 = 65_536;
- MAX_MODULE_BYTES · constant · L16-L16 — const MAX_MODULE_BYTES: usize = 16 * 1024 * 1024;
- _ · constant · L17-L17 — const _: () = assert!(BYTE_DROP_IMPORT == 9);
- emit · function · L19-L246 — pub(in crate::wasm) fn emit(
- function_types_count · function · L248-L251 — fn function_types_count(functions: usize, exports: usize) -> Result<u32, Diagnostic>
- append_body · function · L253-L265 — fn append_body(code: &mut Vec<u8>, body: Vec<u8>) -> Result<(), Diagnostic>
- wrapper · function · L267-L288 — fn wrapper(export: &Export, target: u32) -> Vec<u8>
- string_capacity_guard · function · L291-L301 — pub(super) fn string_capacity_guard(&mut self, local: u32) -> Result<(), Diagnostic>
- drop_internal_string · function · L303-L314 — fn drop_internal_string(&mut self, value: &Value) -> Result<(), Diagnostic>
- emit_internal_string_operation · function · L316-L377 — pub(super) fn emit_internal_string_operation(
